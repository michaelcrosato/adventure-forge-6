import { ApiError, GameApi, isDecimalString, type FetchLike } from "./api";
import { parseJournal, type Journal, type PendingRequest } from "./journal";
import type {
  ActionRequest,
  ActionView,
  ClientState,
  SessionHandle,
  SessionView,
  StartRecipe,
} from "./types";

export const JOURNAL_KEY = "adventure-forge.browser.v1";

type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export interface GameClientOptions {
  fetch?: FetchLike;
  storage?: StorageLike;
  commandId?: () => string;
}

export class ClientError extends Error {
  readonly code: string;

  constructor(code: string) {
    super(code);
    this.name = "ClientError";
    this.code = code;
  }
}

interface LoadedJournal {
  journal: Journal | null;
  warning: string | null;
}

function freezeDeep<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value as Record<string, unknown>)) {
      freezeDeep(child);
    }
    Object.freeze(value);
  }
  return value;
}

function defaultStorage(): StorageLike | undefined {
  try {
    return typeof globalThis.sessionStorage === "undefined"
      ? undefined
      : globalThis.sessionStorage;
  } catch {
    return undefined;
  }
}

function defaultId(): string {
  const randomUuid = globalThis.crypto?.randomUUID;
  if (randomUuid) {
    return randomUuid.call(globalThis.crypto);
  }
  throw new ClientError("id_generation_unavailable");
}

function validId(value: string): boolean {
  return /^[A-Za-z0-9_-]{1,128}$/.test(value);
}

function jsonBody(value: unknown): string {
  const body = JSON.stringify(value);
  if (typeof body !== "string") {
    throw new ClientError("request_serialization_failed");
  }
  return body;
}

function messageFor(error: unknown): string {
  if (error instanceof ApiError || error instanceof ClientError) {
    return error.code;
  }
  return "client_error";
}

function isRetryable(error: unknown): boolean {
  return error instanceof ApiError && (error.retryable || error.status === 0);
}

function isStale(error: unknown): boolean {
  return error instanceof ApiError && error.code === "stale_state";
}

function isClosed(error: unknown): boolean {
  return error instanceof ApiError && (error.status === 410 || error.code === "session_closed");
}

function isUnknownSession(error: unknown): boolean {
  return error instanceof ApiError && (error.status === 404 || error.code === "unknown_session");
}

function isIdempotencyConflict(error: unknown): boolean {
  return error instanceof ApiError && error.code === "idempotency_conflict";
}

function isDefinitiveRequestFailure(error: unknown): boolean {
  return (
    error instanceof ApiError &&
    (error.code === "invalid_input" ||
      error.code === "invalid_save" ||
      error.code === "invalid_action" ||
      error.code === "stale_state" ||
      error.code === "resource_limit")
  );
}

function isClosedAcknowledgement(value: unknown): value is { closed: true } {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const record = value as Record<string, unknown>;
  return Object.keys(record).length === 1 && record.closed === true;
}

function loadJournal(storage: StorageLike | undefined): LoadedJournal {
  if (!storage) {
    return { journal: null, warning: "storage_unavailable" };
  }
  try {
    const raw = storage.getItem(JOURNAL_KEY);
    return raw === null
      ? { journal: null, warning: null }
      : { journal: parseJournal(raw), warning: null };
  } catch {
    return { journal: null, warning: "storage_invalid" };
  }
}

function emptyState(storageWarning: string | null): ClientState {
  return freezeDeep({
    phase: "booting",
    options: null,
    session: null,
    actions: [],
    catalogComplete: false,
    message: null,
    storageWarning,
    pendingLabel: null,
  });
}

export class GameClient {
  private readonly api: GameApi;
  private readonly storage: StorageLike | undefined;
  private readonly commandIdFactory: () => string;
  private readonly listeners = new Set<() => void>();
  private state: ClientState;
  private journal: Journal | null;
  private instanceId: string | null = null;
  private bootPromise: Promise<void> | null = null;
  private generation = 0;

  constructor(options: GameClientOptions = {}) {
    this.api = new GameApi(options.fetch);
    this.storage = options.storage ?? defaultStorage();
    this.commandIdFactory = options.commandId ?? defaultId;
    const loaded = loadJournal(this.storage);
    this.journal = loaded.journal;
    this.state = emptyState(loaded.warning);
  }

  getSnapshot(): ClientState {
    return this.state;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  boot(): Promise<void> {
    if (this.bootPromise) {
      return this.bootPromise;
    }
    if (this.state.phase === "working") {
      return Promise.resolve();
    }
    const generation = ++this.generation;
    this.bootPromise = this.bootInternal(generation).catch((error: unknown) => {
      if (this.isCurrent(generation)) {
        this.setState({ phase: "error", message: messageFor(error), pendingLabel: null });
      }
    });
    return this.bootPromise;
  }

  async start(start: StartRecipe): Promise<void> {
    try {
      if (this.isBusy()) {
        return;
      }
      if (!this.canBeginCreation()) {
        this.setState({ phase: "error", message: "session_not_available", pendingLabel: null });
        return;
      }
      const creationId = this.newId("create");
      const body = jsonBody({ creation_id: creationId, start });
      const pending: PendingRequest = {
        kind: "start",
        path: "/api/sessions",
        body,
        label: "Starting",
        session_id: null,
      };
      if (!this.persistPending(pending)) {
        return;
      }
      const generation = ++this.generation;
      this.setState({ phase: "working", message: null, pendingLabel: pending.label });
      try {
        const handle = await this.api.replayJson<SessionHandle>(pending.path, pending.body);
        await this.finishCreation(handle, pending, generation);
      } catch (error) {
        await this.handlePendingFailure(error, pending, generation);
      }
    } catch (error) {
      this.setState({ phase: "error", message: messageFor(error), pendingLabel: null });
    }
  }

  async resume(saveText: string): Promise<void> {
    try {
      if (this.isBusy()) {
        return;
      }
      if (!this.canBeginCreation()) {
        this.setState({ phase: "error", message: "session_not_available", pendingLabel: null });
        return;
      }
      if (typeof saveText !== "string" || saveText.length === 0) {
        this.setState({ phase: "error", message: "invalid_save", pendingLabel: null });
        return;
      }
      const creationId = this.newId("resume");
      const body = jsonBody({ creation_id: creationId, save_json: saveText });
      const pending: PendingRequest = {
        kind: "resume",
        path: "/api/resume",
        body,
        label: "Resuming",
        session_id: null,
      };
      if (!this.persistPending(pending)) {
        return;
      }
      const generation = ++this.generation;
      this.setState({ phase: "working", message: null, pendingLabel: pending.label });
      try {
        const handle = await this.api.replayJson<SessionHandle>(pending.path, pending.body);
        await this.finishCreation(handle, pending, generation);
      } catch (error) {
        await this.handlePendingFailure(error, pending, generation);
      }
    } catch (error) {
      this.setState({ phase: "error", message: messageFor(error), pendingLabel: null });
    }
  }

  async act(action: ActionView): Promise<void> {
    try {
      if (this.isBusy()) {
        return;
      }
      const session = this.state.session;
      if (!session || this.state.phase !== "ready") {
        this.setState({ phase: "error", message: "session_not_ready", pendingLabel: null });
        return;
      }
      const commandId = this.newId("action");
      const request: ActionRequest = {
        command_id: commandId,
        expected_revision: session.view.revision,
        expected_state_id: session.view.observation.state_id,
        action_id: action.action_id,
      };
      const pending: PendingRequest = {
        kind: "action",
        path: `/api/sessions/${session.session_id}/actions`,
        body: jsonBody(request),
        label: action.label,
        session_id: session.session_id,
      };
      if (!this.persistPending(pending)) {
        return;
      }
      const generation = ++this.generation;
      this.setState({ phase: "working", message: null, pendingLabel: action.label });
      try {
        await this.api.replayJson<SessionView>(pending.path, pending.body);
        await this.finishAction(session.session_id, pending, generation);
      } catch (error) {
        await this.handlePendingFailure(error, pending, generation);
      }
    } catch (error) {
      this.setState({ phase: "error", message: messageFor(error), pendingLabel: null });
    }
  }

  async retry(): Promise<void> {
    if (this.isBusy()) {
      return;
    }
    if (this.needsBootstrap()) {
      await this.rebootstrap();
      return;
    }
    const pending = this.journal?.pending;
    if (pending) {
      const generation = ++this.generation;
      this.setState({ phase: "working", message: null, pendingLabel: pending.label });
      await this.replayPending(pending, generation);
      return;
    }
    if (this.state.session) {
      await this.refresh();
      return;
    }
    this.bootPromise = null;
    await this.boot();
  }

  async refresh(): Promise<void> {
    if (this.isBusy()) {
      return;
    }
    if (this.needsBootstrap()) {
      await this.rebootstrap();
      return;
    }
    if (this.journal?.pending) {
      this.setState({ phase: "uncertain", message: "retry_pending_request", pendingLabel: this.journal.pending.label });
      return;
    }
    await this.refreshCurrent();
  }

  private async refreshCurrent(): Promise<void> {
    const sessionId = this.sessionId();
    if (!sessionId) {
      const generation = ++this.generation;
      this.setState({ phase: "working", message: null, pendingLabel: "Recovering" });
      try {
        const current = await this.api.current();
        if (!this.isCurrent(generation)) return;
        if (current.session) {
          await this.finishCreation(current.session, null, generation);
        } else {
          this.setState({ phase: "start", session: null, actions: [], catalogComplete: false, message: null, pendingLabel: null });
        }
      } catch (error) {
        await this.handleReadFailure(error, generation);
      }
      return;
    }
    const generation = ++this.generation;
    this.setState({ phase: "working", message: null, pendingLabel: "Refreshing" });
    try {
      const view = await this.api.observe(sessionId);
      await this.finishRead(sessionId, view, generation);
    } catch (error) {
      await this.handleReadFailure(error, generation);
    }
  }

  private needsBootstrap(): boolean {
    return this.instanceId === null || this.state.options === null || this.state.message === "unauthorized";
  }

  private async rebootstrap(): Promise<void> {
    this.bootPromise = null;
    await this.boot();
  }

  async close(): Promise<void> {
    if (this.state.phase === "closed") {
      return;
    }
    if (this.isBusy()) {
      return;
    }
    if (this.state.phase !== "ready") {
      this.setState({ phase: "error", message: "session_not_ready", pendingLabel: null });
      return;
    }
    const sessionId = this.sessionId();
    if (!sessionId) {
      this.setState({ phase: "closed", message: null, pendingLabel: null });
      return;
    }
    const pending: PendingRequest = {
      kind: "close",
      path: `/api/sessions/${sessionId}/close`,
      body: "{}",
      label: "Closing",
      session_id: sessionId,
    };
    if (!this.persistPending(pending)) {
      return;
    }
    const generation = ++this.generation;
    this.setState({ phase: "working", message: null, pendingLabel: pending.label });
    try {
      const acknowledgement = await this.api.replayJson<unknown>(pending.path, pending.body);
      if (!isClosedAcknowledgement(acknowledgement)) {
        throw new ClientError("invalid_close_response");
      }
      await this.finishClose(sessionId, pending, generation);
    } catch (error) {
      await this.handlePendingFailure(error, pending, generation);
    }
  }

  async save(): Promise<string> {
    if (this.state.phase !== "ready" && this.state.phase !== "closed") {
      throw new ClientError("session_not_ready");
    }
    const sessionId = this.sessionId();
    if (!sessionId) {
      throw new ClientError("session_not_available");
    }
    try {
      const saveText = await this.api.save(sessionId);
      const journal = this.journalForInstance();
      if (journal) {
        this.persistJournal({ ...journal, last_saved_save_json: saveText });
      }
      return saveText;
    } catch (error) {
      this.setState({ phase: "error", message: messageFor(error), pendingLabel: null });
      throw error;
    }
  }

  async newGame(): Promise<void> {
    if (this.isBusy()) {
      return;
    }
    if (this.state.phase !== "closed") {
      this.setState({ phase: "error", message: "session_not_closed", pendingLabel: null });
      return;
    }
    const instanceId = this.instanceId;
    if (!instanceId) {
      this.setState({ phase: "error", message: "not_bootstrapped", pendingLabel: null });
      return;
    }
    const journal: Journal = {
      version: 1,
      instance_id: instanceId,
      lifecycle: "none",
      session_id: null,
      pending: null,
    };
    if (!this.persistJournal(journal)) {
      return;
    }
    ++this.generation;
    this.setState({
      phase: "start",
      session: null,
      actions: [],
      catalogComplete: false,
      message: null,
      pendingLabel: null,
    });
  }

  async acknowledgeRestart(): Promise<void> {
    if (this.state.phase !== "restarted") {
      return;
    }
    const instanceId = this.instanceId;
    if (!instanceId) {
      this.setState({ phase: "error", message: "not_bootstrapped", pendingLabel: null });
      return;
    }
    const journal: Journal = {
      version: 1,
      instance_id: instanceId,
      lifecycle: "none",
      session_id: null,
      pending: null,
    };
    if (!this.persistJournal(journal)) {
      return;
    }
    const generation = ++this.generation;
    this.setState({ phase: "working", message: "restart_acknowledged", pendingLabel: "Recovering" });
    try {
      const current = await this.api.current();
      if (current.session) {
        await this.finishCreation(current.session, null, generation);
      } else {
        this.setState({ phase: "start", session: null, actions: [], catalogComplete: false, message: null, pendingLabel: null });
      }
    } catch (error) {
      if (this.isCurrent(generation)) {
        this.setState({ phase: "error", message: messageFor(error), pendingLabel: null });
      }
    }
  }

  private async bootInternal(generation: number): Promise<void> {
    this.setState({ phase: "booting", message: null, pendingLabel: null });
    const bootstrap = await this.api.bootstrap();
    if (!this.isCurrent(generation)) return;
    this.instanceId = bootstrap.instance_id;
    const options = await this.api.options();
    if (!this.isCurrent(generation)) return;
    this.setState({ options });

    const journal = this.journal;
    if (journal && journal.instance_id !== bootstrap.instance_id) {
      this.setState({
        phase: "restarted",
        session: null,
        actions: [],
        catalogComplete: false,
        message: "server_restarted",
        pendingLabel: null,
      });
      return;
    }
    if (journal?.pending) {
      this.setState({ phase: "working", message: null, pendingLabel: journal.pending.label });
      await this.replayPending(journal.pending, generation);
      return;
    }
    if (journal?.lifecycle === "closed" && journal.session_id) {
      this.setState({ phase: "closed", session: null, actions: [], catalogComplete: false, message: null, pendingLabel: null });
      return;
    }

    const current = await this.api.current();
    if (!this.isCurrent(generation)) return;
    if (current.session) {
      if (journal?.session_id && journal.session_id !== current.session.session_id) {
        this.setState({ phase: "restarted", session: null, actions: [], catalogComplete: false, message: "different_active_session", pendingLabel: null });
        return;
      }
      await this.finishCreation(current.session, null, generation);
      return;
    }
    if (journal?.lifecycle === "open" && journal.session_id) {
      try {
        const view = await this.api.observe(journal.session_id);
        await this.finishRead(journal.session_id, view, generation);
      } catch (error) {
        if (isClosed(error)) {
          this.setState({ phase: "closed", session: null, actions: [], catalogComplete: false, message: null, pendingLabel: null });
        } else {
          this.setState({ phase: "restarted", session: null, actions: [], catalogComplete: false, message: isUnknownSession(error) ? "session_unknown" : messageFor(error), pendingLabel: null });
        }
      }
      return;
    }
    this.setState({ phase: "start", session: null, actions: [], catalogComplete: false, message: null, pendingLabel: null });
  }

  private async replayPending(pending: PendingRequest, generation: number): Promise<void> {
    try {
      if (pending.kind === "start" || pending.kind === "resume") {
        const handle = await this.api.replayJson<SessionHandle>(pending.path, pending.body);
        await this.finishCreation(handle, pending, generation);
      } else if (pending.kind === "action") {
        await this.api.replayJson<SessionView>(pending.path, pending.body);
        await this.finishAction(pending.session_id as string, pending, generation);
      } else {
        const acknowledgement = await this.api.replayJson<unknown>(pending.path, pending.body);
        if (!isClosedAcknowledgement(acknowledgement)) {
          throw new ClientError("invalid_close_response");
        }
        await this.finishClose(pending.session_id as string, pending, generation);
      }
    } catch (error) {
      await this.handlePendingFailure(error, pending, generation);
    }
  }

  private async finishCreation(
    handle: SessionHandle,
    pending: PendingRequest | null,
    generation: number,
  ): Promise<void> {
    if (!this.isCurrent(generation)) return;
    const currentJournal = this.journalForInstance();
    const acknowledged: Journal = {
      ...(currentJournal ?? this.newJournal()),
      lifecycle: "open",
      session_id: handle.session_id,
      pending,
    };
    if (!this.persistJournal(acknowledged)) {
      this.setState({ phase: "uncertain", session: handle, pendingLabel: pending?.label ?? null });
      return;
    }
    this.setState({
      phase: "working",
      session: handle,
      actions: handle.view.catalog.actions,
      catalogComplete: false,
      message: null,
      pendingLabel: pending?.label ?? "Loading actions",
    });
    try {
      const currentView = await this.api.observe(handle.session_id);
      const currentHandle: SessionHandle = { session_id: handle.session_id, view: currentView };
      this.setState({
        phase: "working",
        session: currentHandle,
        actions: currentView.catalog.actions,
        catalogComplete: false,
        message: null,
        pendingLabel: pending?.label ?? "Loading actions",
      });
      const actions = await this.completeCatalog(handle.session_id, currentView, generation);
      if (!this.isCurrent(generation)) return;
      const finalJournal: Journal = {
        ...acknowledged,
        pending: null,
      };
      if (!this.persistJournal(finalJournal)) {
        this.setState({ phase: "uncertain", pendingLabel: pending?.label ?? null });
        return;
      }
      this.setState({
        phase: "ready",
        session: currentHandle,
        actions,
        catalogComplete: true,
        message: null,
        pendingLabel: null,
      });
    } catch (error) {
      if (pending) {
        await this.handlePendingFailure(error, pending, generation);
      } else {
        await this.handleReadFailure(error, generation);
      }
    }
  }

  private async finishAction(
    sessionId: string,
    pending: PendingRequest,
    generation: number,
  ): Promise<void> {
    if (!this.isCurrent(generation)) return;
    const acknowledged: Journal = {
      ...(this.journalForInstance() ?? this.newJournal()),
      lifecycle: "open",
      session_id: sessionId,
      pending,
    };
    if (!this.persistJournal(acknowledged)) {
      this.setState({ phase: "uncertain", pendingLabel: pending.label });
      return;
    }
    try {
      // An action retry can return an old acknowledgment. Observe is the
      // authority for the view that should become current in the browser.
      const currentView = await this.api.observe(sessionId);
      const actions = await this.completeCatalog(sessionId, currentView, generation);
      if (!this.isCurrent(generation)) return;
      const finalJournal: Journal = { ...acknowledged, pending: null };
      if (!this.persistJournal(finalJournal)) {
        this.setState({
          phase: "uncertain",
          session: { session_id: sessionId, view: currentView },
          actions,
          catalogComplete: true,
          pendingLabel: pending.label,
        });
        return;
      }
      this.setState({
        phase: "ready",
        session: { session_id: sessionId, view: currentView },
        actions,
        catalogComplete: true,
        message: null,
        pendingLabel: null,
      });
    } catch (error) {
      await this.handlePendingFailure(error, pending, generation);
    }
  }

  private async finishClose(
    sessionId: string,
    pending: PendingRequest,
    generation: number,
  ): Promise<void> {
    if (!this.isCurrent(generation)) return;
    const currentJournal = this.journalForInstance() ?? this.newJournal();
    const closed: Journal = {
      ...currentJournal,
      lifecycle: "closed",
      session_id: sessionId,
      pending: null,
    };
    if (!this.persistJournal(closed)) {
      this.setState({ phase: "uncertain", pendingLabel: pending.label });
      return;
    }
    this.setState({
      phase: "closed",
      session: null,
      actions: [],
      catalogComplete: false,
      message: null,
      pendingLabel: null,
    });
  }

  private async finishRead(
    sessionId: string,
    view: SessionView,
    generation: number,
  ): Promise<void> {
    if (!this.isCurrent(generation)) return;
    this.setState({
      phase: "working",
      session: { session_id: sessionId, view },
      actions: view.catalog.actions,
      catalogComplete: false,
      message: null,
      pendingLabel: "Loading actions",
    });
    try {
      const actions = await this.completeCatalog(sessionId, view, generation);
      if (!this.isCurrent(generation)) return;
      const journal = this.journalForInstance();
      if (journal && !this.persistJournal({ ...journal, lifecycle: "open", session_id: sessionId, pending: null })) {
        this.setState({ phase: "uncertain", pendingLabel: "Saving recovery record" });
        return;
      }
      this.setState({
        phase: "ready",
        session: { session_id: sessionId, view },
        actions,
        catalogComplete: true,
        message: null,
        pendingLabel: null,
      });
    } catch (error) {
      await this.handleReadFailure(error, generation);
    }
  }

  private async completeCatalog(
    sessionId: string,
    view: SessionView,
    generation: number,
  ): Promise<ActionView[]> {
    const expectedBuild = view.observation.build_id;
    const expectedState = view.observation.state_id;
    const expectedDigest = view.observation.action_set_digest;
    if (!isDecimalString(view.observation.action_count)) {
      throw new ClientError("invalid_catalog");
    }
    const expectedTotal = BigInt(view.observation.action_count);
    let page = view.catalog;
    const seenOffsets = new Set<string>();
    const seenActions = new Set<string>();
    const actions: ActionView[] = [];
    let expectedOffset = "0";

    while (true) {
      if (!this.isCurrent(generation)) {
        throw new ClientError("superseded");
      }
      if (
        page.build_id !== expectedBuild ||
        page.state_id !== expectedState ||
        page.digest !== expectedDigest ||
        !isDecimalString(page.total) ||
        !isDecimalString(page.offset) ||
        (page.next_offset !== null && !isDecimalString(page.next_offset))
      ) {
        throw new ClientError("invalid_catalog");
      }
      if (page.offset !== expectedOffset) {
        throw new ClientError("catalog_cursor");
      }
      if (seenOffsets.has(page.offset)) {
        throw new ClientError("catalog_cycle");
      }
      seenOffsets.add(page.offset);
      const pageTotal = BigInt(page.total);
      if (expectedTotal !== pageTotal) {
        throw new ClientError("catalog_total_changed");
      }
      for (const action of page.actions) {
        if (
          typeof action.action_id !== "string" ||
          action.action_id.length === 0 ||
          typeof action.definition_id !== "string" ||
          action.definition_id.length === 0 ||
          seenActions.has(action.action_id)
        ) {
          throw new ClientError("catalog_duplicate_action");
        }
        seenActions.add(action.action_id);
        actions.push(action);
      }
      const nextExpected = BigInt(page.offset) + BigInt(page.actions.length);
      if (page.next_offset === null) {
        if (nextExpected !== expectedTotal || expectedTotal !== BigInt(actions.length)) {
          throw new ClientError("catalog_incomplete");
        }
        return actions;
      }
      if (BigInt(page.next_offset) !== nextExpected || BigInt(page.next_offset) >= expectedTotal) {
        throw new ClientError("catalog_cursor");
      }
      expectedOffset = page.next_offset;
      page = await this.api.catalog(sessionId, expectedState, page.next_offset, "128");
    }
  }

  private async handlePendingFailure(
    error: unknown,
    pending: PendingRequest,
    generation: number,
  ): Promise<void> {
    if (!this.isCurrent(generation)) return;
    if (isClosed(error)) {
      if (!this.persistClosed(pending.session_id)) {
        this.setPendingStorageFailure(pending);
        return;
      }
      this.setState({ phase: "closed", session: null, actions: [], catalogComplete: false, message: null, pendingLabel: null });
      return;
    }
    if (isStale(error) || (error instanceof ApiError && error.code === "invalid_action")) {
      if (!this.clearPending()) {
        this.setPendingStorageFailure(pending);
        return;
      }
      await this.refreshCurrent();
      return;
    }
    if (isIdempotencyConflict(error)) {
      if (!this.clearPending()) {
        this.setPendingStorageFailure(pending);
        return;
      }
      this.setState({
        phase: "error",
        message: "idempotency_conflict",
        pendingLabel: null,
      });
      return;
    }
    if (isUnknownSession(error)) {
      this.setState({
        phase: "restarted",
        session: null,
        actions: [],
        catalogComplete: false,
        message: "session_unknown",
        pendingLabel: null,
      });
      return;
    }
    if (isDefinitiveRequestFailure(error)) {
      if (!this.clearPending()) {
        this.setPendingStorageFailure(pending);
        return;
      }
      if (error instanceof ApiError && error.code === "resource_limit") {
        await this.refreshCurrent();
        if (this.state.phase === "ready" || this.state.phase === "start") {
          this.setState({ message: "resource_limit", pendingLabel: null });
        }
        return;
      }
      if (pending.kind === "start" || pending.kind === "resume") {
        this.setState({
          phase: "start",
          session: null,
          actions: [],
          catalogComplete: false,
          message: messageFor(error),
          pendingLabel: null,
        });
      } else {
        await this.refreshCurrent();
      }
      return;
    }
    this.setState({
      phase: isRetryable(error) ? "uncertain" : "error",
      message: messageFor(error),
      pendingLabel: pending.label,
    });
  }

  private async handleReadFailure(error: unknown, generation: number): Promise<void> {
    if (!this.isCurrent(generation)) return;
    if (isClosed(error)) {
      const sessionId = this.sessionId();
      if (sessionId && !this.persistClosed(sessionId)) {
        this.setState({
          phase: "uncertain",
          message: "storage_write_failed",
          pendingLabel: "Saving recovery record",
        });
        return;
      }
      this.setState({ phase: "closed", session: null, actions: [], catalogComplete: false, message: null, pendingLabel: null });
    } else if (isUnknownSession(error)) {
      this.setState({ phase: "restarted", session: null, actions: [], catalogComplete: false, message: "session_unknown", pendingLabel: null });
    } else {
      this.setState({ phase: isRetryable(error) ? "uncertain" : "error", message: messageFor(error), pendingLabel: null });
    }
  }

  private canBeginCreation(): boolean {
    return (
      this.instanceId !== null &&
      this.state.phase === "start" &&
      this.state.session === null &&
      (this.journal?.pending ?? null) === null
    );
  }

  private isBusy(): boolean {
    return this.state.phase === "booting" || this.state.phase === "working";
  }

  private sessionId(): string | null {
    return this.state.session?.session_id ?? this.journal?.session_id ?? null;
  }

  private newId(prefix: string): string {
    const candidate = `${prefix}-${this.commandIdFactory()}`;
    if (!validId(candidate)) {
      throw new ClientError("invalid_generated_id");
    }
    return candidate;
  }

  private newJournal(): Journal {
    if (!this.instanceId) {
      throw new ClientError("not_bootstrapped");
    }
    return {
      version: 1,
      instance_id: this.instanceId,
      lifecycle: "none",
      session_id: null,
      pending: null,
    };
  }

  private journalForInstance(): Journal | null {
    return this.journal && this.journal.instance_id === this.instanceId ? this.journal : null;
  }

  private persistPending(pending: PendingRequest): boolean {
    const journal = this.journalForInstance() ?? this.newJournal();
    const persisted = this.persistJournal({
      ...journal,
      lifecycle: pending.kind === "close" ? "open" : journal.lifecycle,
      session_id: pending.session_id ?? journal.session_id,
      pending,
    });
    if (!persisted) {
      this.setState({ phase: "error", message: "storage_write_failed", pendingLabel: null });
    }
    return persisted;
  }

  private clearPending(): boolean {
    const journal = this.journalForInstance();
    if (!journal) return true;
    const cleared: Journal = { ...journal, pending: null };
    return this.persistJournal(cleared);
  }

  private persistClosed(sessionId: string | null): boolean {
    const journal = this.journalForInstance();
    if (!journal) return false;
    return this.persistJournal({
      ...journal,
      lifecycle: "closed",
      session_id: sessionId ?? journal.session_id,
      pending: null,
    });
  }

  private setPendingStorageFailure(pending: PendingRequest): void {
    this.setState({
      phase: "uncertain",
      message: "storage_write_failed",
      pendingLabel: pending.label,
    });
  }

  private persistJournal(journal: Journal): boolean {
    if (!this.storage) {
      this.setState({ storageWarning: "storage_unavailable" });
      return false;
    }
    try {
      const serialized = JSON.stringify(journal);
      // Validate writes with the same strict parser used during reload. This
      // keeps a malformed caller-supplied action from poisoning the retry log.
      parseJournal(serialized);
      this.storage.setItem(JOURNAL_KEY, serialized);
      this.journal = journal;
      if (this.state.storageWarning !== null) {
        this.setState({ storageWarning: null });
      }
      return true;
    } catch {
      this.setState({ storageWarning: "storage_write_failed" });
      return false;
    }
  }

  private isCurrent(generation: number): boolean {
    return generation === this.generation;
  }

  private setState(patch: Partial<ClientState>): void {
    this.state = freezeDeep({ ...this.state, ...patch });
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // A subscriber cannot corrupt the transport state machine.
      }
    }
  }
}
