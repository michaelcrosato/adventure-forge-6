import { describe, expect, it } from "vitest";

import type { FetchLike } from "./api";
import { JOURNAL_KEY, GameClient } from "./controller";
import type {
  ActionPage,
  ActionView,
  SessionView,
  StartOptions,
  StartRecipe,
} from "./types";

const START_OPTIONS: StartOptions = {
  build_id: "build-1",
  presets: [{ id: "rook", display_name: "Rook", summary: "Lock runner" }],
  creation_slots: [],
};

const PRESET_START: StartRecipe = {
  kind: "preset",
  character_preset_id: "rook",
  seed: "71",
};

function wireId(value: string): string {
  if (/^[0-9a-f]{64}$/.test(value)) {
    return value;
  }
  let encoded = "";
  for (const character of value) {
    encoded += character.charCodeAt(0).toString(16).padStart(2, "0");
  }
  return `${encoded}${"0".repeat(64)}`.slice(0, 64);
}

function action(actionId: string, label = actionId): ActionView {
  return {
    action_id: wireId(actionId),
    definition_id: `test.${actionId}`,
    label,
    category: "route",
    time_cost: { minimum_ticks: "1", maximum_ticks: "1" },
    consequence_preview: null,
    parameter_display_values: {},
    parameters: {},
  };
}

function page(
  stateId: string,
  actions: ActionView[],
  offset = "0",
  total = String(actions.length),
  nextOffset: string | null = null,
): ActionPage {
  return {
    build_id: "build-1",
    state_id: wireId(stateId),
    actions,
    total,
    digest: "digest-1",
    offset,
    next_offset: nextOffset,
  };
}

function view(
  revision: string,
  stateId: string,
  actions: ActionView[],
  firstPage = page(stateId, actions),
): SessionView {
  return {
    revision,
    observation: {
      build_id: "build-1",
      state_id: wireId(stateId),
      location_id: "lowsail_market",
      title: "Lowsail Market",
      text: "The market waits under a grey tide.",
      supplies: { resources: [], items: [] },
      result: null,
      world_time: revision,
      upcoming_events: [],
      action_set_digest: "digest-1",
      action_count: firstPage.total,
    },
    catalog: firstPage,
  };
}

function jsonResponse(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function errorResponse(status: number, code: string): Response {
  return jsonResponse({ error: code }, status);
}

function rawResponse(value: string, status = 200): Response {
  return new Response(value, {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

class MemoryStorage {
  readonly values = new Map<string, string>();
  failWrites = false;
  failOnWrite: number | null = null;
  writes = 0;

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.writes += 1;
    if (this.failWrites || this.writes === this.failOnWrite) {
      throw new Error("storage is full");
    }
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    if (this.failWrites) {
      throw new Error("storage is full");
    }
    this.values.delete(key);
  }
}

interface Call {
  path: string;
  body: string | null;
}

interface Failure {
  status: number;
  code: string;
}

class MockServer {
  readonly token = "b".repeat(64);
  readonly instanceId = "c".repeat(64);
  readonly sessionId = "a".repeat(64);
  readonly calls: Call[] = [];
  readonly startBodies: string[] = [];
  readonly resumeBodies: string[] = [];
  readonly actionBodies: string[] = [];
  readonly catalogPages = new Map<string, ActionPage>();
  bootstrapCount = 0;
  optionsCount = 0;
  observeCount = 0;
  catalogCount = 0;
  currentView = view("0", "state-0", [action("wait_tide", "Wait")]);
  saveText = '{"seed":"18446744073709551615","opaque":"keep me"}\n';
  active = false;
  closed = false;
  dropStart = false;
  dropResume = false;
  dropAction = false;
  actionView: SessionView | null = null;
  staleAction = false;
  staleView: SessionView | null = null;
  bootstrapFailure: Failure | null = null;
  optionsFailure: Failure | null = null;
  nextBootstrapInstanceId: string | null = null;
  catalogUnauthorized = false;
  startFailure: Failure | null = null;
  resumeFailure: Failure | null = null;
  actionFailure: Failure | null = null;
  observeFailure: Failure | null = null;
  closeFailure: Failure | null = null;
  closeResponse: unknown = { closed: true };
  startResponseView: SessionView | null = null;
  resumeResponseView: SessionView | null = null;
  holdStart = false;
  private startRelease: (() => void) | null = null;

  releaseStart(): void {
    this.startRelease?.();
    this.startRelease = null;
  }

  readonly fetch: FetchLike = async (input, init) => {
    const rawUrl =
      typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
    const path = new URL(rawUrl, "http://forge.local").pathname;
    const method = init?.method ?? "GET";
    const body = typeof init?.body === "string" ? init.body : null;
    this.calls.push({ path, body });

    if (path === "/api/bootstrap" && method === "GET") {
      this.bootstrapCount += 1;
      if (this.bootstrapFailure) {
        const failure = this.bootstrapFailure;
        this.bootstrapFailure = null;
        return errorResponse(failure.status, failure.code);
      }
      const instanceId = this.nextBootstrapInstanceId ?? this.instanceId;
      this.nextBootstrapInstanceId = null;
      return jsonResponse({ token: this.token, instance_id: instanceId });
    }
    if (path === "/api/options" && method === "GET") {
      this.optionsCount += 1;
      if (this.optionsFailure) {
        const failure = this.optionsFailure;
        this.optionsFailure = null;
        return errorResponse(failure.status, failure.code);
      }
      return jsonResponse(START_OPTIONS);
    }
    if (path === "/api/current" && method === "GET") {
      return jsonResponse({
        session:
          this.active && !this.closed
            ? { session_id: this.sessionId, view: this.currentView }
            : null,
      });
    }
    if (path === "/api/sessions" && method === "POST") {
      this.startBodies.push(body ?? "");
      if (this.startFailure) {
        const failure = this.startFailure;
        this.startFailure = null;
        return errorResponse(failure.status, failure.code);
      }
      this.active = true;
      this.closed = false;
      if (this.holdStart) {
        await new Promise<void>((resolve) => {
          this.startRelease = resolve;
        });
      }
      if (this.dropStart) {
        this.dropStart = false;
        throw new TypeError("connection lost after commit");
      }
      return jsonResponse({ session_id: this.sessionId, view: this.startResponseView ?? this.currentView });
    }
    if (path === "/api/resume" && method === "POST") {
      this.resumeBodies.push(body ?? "");
      if (this.resumeFailure) {
        const failure = this.resumeFailure;
        this.resumeFailure = null;
        return errorResponse(failure.status, failure.code);
      }
      this.active = true;
      this.closed = false;
      if (this.dropResume) {
        this.dropResume = false;
        throw new TypeError("connection lost after commit");
      }
      return jsonResponse({ session_id: this.sessionId, view: this.resumeResponseView ?? this.currentView });
    }

    const sessionPath = `/api/sessions/${this.sessionId}`;
    if (path === sessionPath && method === "GET") {
      this.observeCount += 1;
      if (this.observeFailure) {
        const failure = this.observeFailure;
        this.observeFailure = null;
        if (failure.status === 410) {
          this.closed = true;
          this.active = false;
        }
        return errorResponse(failure.status, failure.code);
      }
      return this.closed ? errorResponse(410, "session_closed") : jsonResponse(this.currentView);
    }
    if (path === `${sessionPath}/catalog` && method === "POST") {
      this.catalogCount += 1;
      if (this.catalogUnauthorized) {
        this.catalogUnauthorized = false;
        return errorResponse(401, "unauthorized");
      }
      const request = JSON.parse(body ?? "{}");
      return jsonResponse(this.catalogPages.get(request.offset) ?? this.currentView.catalog);
    }
    if (path === `${sessionPath}/actions` && method === "POST") {
      this.actionBodies.push(body ?? "");
      if (this.actionFailure) {
        const failure = this.actionFailure;
        this.actionFailure = null;
        return errorResponse(failure.status, failure.code);
      }
      if (this.staleAction && this.staleView) {
        this.staleAction = false;
        this.currentView = this.staleView;
        return errorResponse(409, "stale_state");
      }
      if (this.dropAction) {
        this.dropAction = false;
        if (this.actionView) {
          this.currentView = this.actionView;
        }
        throw new TypeError("connection lost after commit");
      }
      return jsonResponse(this.currentView);
    }
    if (path === `${sessionPath}/save` && method === "GET") {
      return rawResponse(this.saveText);
    }
    if (path === `${sessionPath}/close` && method === "POST") {
      if (this.closeFailure) {
        const failure = this.closeFailure;
        this.closeFailure = null;
        this.closed = true;
        this.active = false;
        return errorResponse(failure.status, failure.code);
      }
      this.active = false;
      this.closed = true;
      return jsonResponse(this.closeResponse);
    }
    return errorResponse(404, "unknown_route");
  };
}

function commandIds(...values: string[]): () => string {
  let index = 0;
  return () => values[index++] ?? `fallback-${index}`;
}

async function bootClient(
  server: MockServer,
  storage = new MemoryStorage(),
  ids = ["one", "two", "three"],
): Promise<{ client: GameClient; storage: MemoryStorage }> {
  const client = new GameClient({
    fetch: server.fetch,
    storage,
    commandId: commandIds(...ids),
  });
  await client.boot();
  return { client, storage };
}

describe("browser game controller", () => {
  it("replays a lost start byte-for-byte and never stores the bearer token", async () => {
    const server = new MockServer();
    server.dropStart = true;
    const { client, storage } = await bootClient(server);

    await client.start(PRESET_START);
    expect(client.getSnapshot().phase).toBe("uncertain");
    expect(server.startBodies).toHaveLength(1);
    const pending = JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null");
    expect(pending.pending.body).toBe(server.startBodies[0]);
    expect(storage.getItem(JOURNAL_KEY)).not.toContain(server.token);
    expect(storage.getItem(JOURNAL_KEY)).not.toContain("acknowledged_view");

    await client.retry();
    expect(client.getSnapshot().phase).toBe("ready");
    expect(server.startBodies).toHaveLength(2);
    expect(server.startBodies[1]).toBe(server.startBodies[0]);
    expect(JSON.parse(server.startBodies[0]!).start.seed).toBe("71");
  });

  it("does not permit a second mutation while the first request is working", async () => {
    const server = new MockServer();
    server.holdStart = true;
    const { client } = await bootClient(server, new MemoryStorage(), ["first", "second"]);

    const first = client.start(PRESET_START);
    await Promise.resolve();
    expect(client.getSnapshot().phase).toBe("working");
    await client.start(PRESET_START);
    expect(server.startBodies).toHaveLength(1);
    expect(client.getSnapshot().message).toBeNull();

    server.releaseStart();
    await first;
    expect(client.getSnapshot().phase).toBe("ready");
  });

  it("replays a committed action after a lost response without creating a second transition", async () => {
    const server = new MockServer();
    const next = view("1", "state-1", [action("return", "Return")]);
    server.actionView = next;
    const { client } = await bootClient(server, new MemoryStorage(), ["start", "act"]);
    await client.start(PRESET_START);

    server.dropAction = true;
    const selected = client.getSnapshot().actions[0];
    expect(selected).toBeDefined();
    await client.act(selected as ActionView);
    expect(client.getSnapshot().phase).toBe("uncertain");
    expect(server.actionBodies).toHaveLength(1);

    await client.retry();
    expect(client.getSnapshot().phase).toBe("ready");
    expect(client.getSnapshot().session?.view.revision).toBe("1");
    expect(server.actionBodies).toHaveLength(2);
    expect(server.actionBodies[1]).toBe(server.actionBodies[0]);
  });

  it("replays a lost resume with the exact opaque save payload", async () => {
    const server = new MockServer();
    server.dropResume = true;
    const { client } = await bootClient(server, new MemoryStorage(), ["resume"]);
    const saveText = '{"seed":"18446744073709551615","final":"opaque"}\n';

    await client.resume(saveText);
    expect(client.getSnapshot().phase).toBe("uncertain");
    await client.retry();
    expect(client.getSnapshot().phase).toBe("ready");
    expect(server.resumeBodies).toHaveLength(2);
    expect(server.resumeBodies[1]).toBe(server.resumeBodies[0]);
    expect(JSON.parse(server.resumeBodies[0]!).save_json).toBe(saveText);
  });

  it("reloads and replays a pending action from session storage", async () => {
    const server = new MockServer();
    const storage = new MemoryStorage();
    const first = await bootClient(server, storage, ["start", "act"]);
    await first.client.start(PRESET_START);
    server.actionView = view("1", "state-1", [action("return", "Return")]);
    server.dropAction = true;
    const selected = first.client.getSnapshot().actions[0];
    await first.client.act(selected as ActionView);
    expect(first.client.getSnapshot().phase).toBe("uncertain");

    const second = new GameClient({
      fetch: server.fetch,
      storage,
      commandId: commandIds("unused"),
    });
    await second.boot();
    expect(second.getSnapshot().phase).toBe("ready");
    expect(second.getSnapshot().session?.view.revision).toBe("1");
    expect(server.actionBodies).toHaveLength(2);
    expect(server.actionBodies[1]).toBe(server.actionBodies[0]);
  });

  it("does not replay a journal from a previous server instance until acknowledged", async () => {
    const server = new MockServer();
    const storage = new MemoryStorage();
    storage.values.set(
      JOURNAL_KEY,
      JSON.stringify({
        version: 1,
        instance_id: "e".repeat(64),
        lifecycle: "none",
        session_id: null,
        pending: {
          kind: "start",
          path: "/api/sessions",
          body: JSON.stringify({ creation_id: "create-old", start: PRESET_START }),
          label: "Starting",
          session_id: null,
        },
      }),
    );
    const { client } = await bootClient(server, storage, ["new"]);
    expect(client.getSnapshot().phase).toBe("restarted");
    expect(server.startBodies).toHaveLength(0);

    await client.acknowledgeRestart();
    expect(client.getSnapshot().phase).toBe("start");
    expect(server.startBodies).toHaveLength(0);
    await client.start(PRESET_START);
    expect(server.startBodies).toHaveLength(1);
  });

  it("refreshes the canonical view after a stale action response", async () => {
    const server = new MockServer();
    const refreshed = view("2", "state-2", [action("fresh", "Fresh")]);
    server.staleView = refreshed;
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["start", "stale"]);
    await client.start(PRESET_START);

    server.staleAction = true;
    const selected = client.getSnapshot().actions[0];
    await client.act(selected as ActionView);
    expect(client.getSnapshot().phase).toBe("ready");
    expect(client.getSnapshot().session?.view.revision).toBe("2");
    expect(client.getSnapshot().actions.map(({ action_id }) => action_id)).toEqual([wireId("fresh")]);
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).toBeNull();
    expect(server.actionBodies).toHaveLength(1);
  });

  it("loads every catalog page and preserves kernel order", async () => {
    const server = new MockServer();
    const first = [action("a-1", "One")];
    const second = [action("a-2", "Two")];
    const third = [action("a-3", "Three")];
    server.currentView = view("0", "state-0", first, page("state-0", first, "0", "3", "1"));
    server.catalogPages.set("1", page("state-0", second, "1", "3", "2"));
    server.catalogPages.set("2", page("state-0", third, "2", "3", null));
    const { client } = await bootClient(server);

    await client.start(PRESET_START);
    expect(client.getSnapshot().catalogComplete).toBe(true);
    expect(client.getSnapshot().actions.map(({ action_id }) => action_id)).toEqual([
      wireId("a-1"),
      wireId("a-2"),
      wireId("a-3"),
    ]);
    const catalogBodies = server.calls
      .filter(({ path }) => path.endsWith("/catalog"))
      .map(({ body }) => JSON.parse(body ?? "{}"));
    expect(catalogBodies.map(({ offset }) => offset)).toEqual(["1", "2"]);
    expect(catalogBodies.every(({ page_size }) => page_size === "128")).toBe(true);
  });

  it("downloads an opaque maximum-seed save after close", async () => {
    const server = new MockServer();
    server.saveText = '{"seed":"18446744073709551615","state":"opaque"}\n';
    const { client, storage } = await bootClient(server);
    await client.start(PRESET_START);
    await client.close();
    expect(client.getSnapshot().phase).toBe("closed");

    const downloaded = await client.save();
    expect(downloaded).toBe(server.saveText);
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").last_saved_save_json).toBe(
      server.saveText,
    );
  });

  it("blocks a mutation when the journal cannot be written", async () => {
    const server = new MockServer();
    const storage = new MemoryStorage();
    const { client } = await bootClient(server, storage);
    storage.failWrites = true;

    await client.start(PRESET_START);
    expect(server.startBodies).toHaveLength(0);
    expect(client.getSnapshot().phase).toBe("error");
    expect(client.getSnapshot().message).toBe("storage_write_failed");
    expect(client.getSnapshot().storageWarning).toBe("storage_write_failed");
  });

  it("reboots after an initial bootstrap failure when refresh is requested", async () => {
    const server = new MockServer();
    server.bootstrapFailure = { status: 503, code: "unavailable" };
    const { client } = await bootClient(server);

    expect(client.getSnapshot().phase).toBe("error");
    expect(client.getSnapshot().options).toBeNull();
    await client.refresh();

    expect(server.bootstrapCount).toBe(2);
    expect(client.getSnapshot().phase).toBe("start");
    expect(client.getSnapshot().options).toEqual(START_OPTIONS);
  });

  it("reboots after an options failure when refresh is requested", async () => {
    const server = new MockServer();
    server.optionsFailure = { status: 503, code: "unavailable" };
    const { client } = await bootClient(server);

    expect(client.getSnapshot().phase).toBe("error");
    await client.refresh();

    expect(server.bootstrapCount).toBe(2);
    expect(server.optionsCount).toBe(2);
    expect(client.getSnapshot().phase).toBe("start");
  });

  it("reboots before replaying a pending request after a mid-catalog 401", async () => {
    const server = new MockServer();
    const first = [action("a-1", "One")];
    const second = [action("a-2", "Two")];
    server.currentView = view("0", "state-0", first, page("state-0", first, "0", "2", "1"));
    server.catalogPages.set("1", page("state-0", second, "1", "2", null));
    server.catalogUnauthorized = true;
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["start"]);

    await client.start(PRESET_START);
    expect(client.getSnapshot().phase).toBe("error");
    expect(client.getSnapshot().message).toBe("unauthorized");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).not.toBeNull();

    await client.refresh();
    expect(server.bootstrapCount).toBe(2);
    expect(server.startBodies).toHaveLength(2);
    expect(server.startBodies[1]).toBe(server.startBodies[0]);
    expect(client.getSnapshot().phase).toBe("ready");
    expect(client.getSnapshot().actions.map(({ action_id }) => action_id)).toEqual([wireId("a-1"), wireId("a-2")]);
  });

  it("compares the new instance before replaying after a mid-catalog 401", async () => {
    const server = new MockServer();
    const first = [action("a-1", "One")];
    const second = [action("a-2", "Two")];
    server.currentView = view("0", "state-0", first, page("state-0", first, "0", "2", "1"));
    server.catalogPages.set("1", page("state-0", second, "1", "2", null));
    server.catalogUnauthorized = true;
    const { client } = await bootClient(server, new MemoryStorage(), ["start"]);
    await client.start(PRESET_START);
    server.nextBootstrapInstanceId = "d".repeat(64);

    await client.refresh();
    expect(server.bootstrapCount).toBe(2);
    expect(server.startBodies).toHaveLength(1);
    expect(client.getSnapshot().phase).toBe("restarted");
  });

  it("keeps a pending request through a 5xx and retries its exact body", async () => {
    const server = new MockServer();
    server.startFailure = { status: 503, code: "unavailable" };
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["start"]);

    await client.start(PRESET_START);
    expect(client.getSnapshot().phase).toBe("uncertain");
    const pending = JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending;
    expect(pending).not.toBeNull();
    server.startFailure = null;
    await client.retry();

    expect(server.startBodies).toHaveLength(2);
    expect(server.startBodies[1]).toBe(server.startBodies[0]);
    expect(client.getSnapshot().phase).toBe("ready");
  });

  it("clears a definitive invalid start and permits a corrected start", async () => {
    const server = new MockServer();
    server.startFailure = { status: 400, code: "invalid_input" };
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["bad", "good"]);

    await client.start(PRESET_START);
    expect(client.getSnapshot().phase).toBe("start");
    expect(client.getSnapshot().message).toBe("invalid_input");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).toBeNull();
    server.startFailure = null;
    await client.start(PRESET_START);
    expect(client.getSnapshot().phase).toBe("ready");
  });

  it("clears a definitive invalid save and permits a corrected resume", async () => {
    const server = new MockServer();
    server.resumeFailure = { status: 400, code: "invalid_save" };
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["bad", "good"]);

    await client.resume("not-a-save");
    expect(client.getSnapshot().phase).toBe("start");
    expect(client.getSnapshot().message).toBe("invalid_save");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).toBeNull();
    server.resumeFailure = null;
    await client.resume("{\"seed\":\"71\"}");
    expect(client.getSnapshot().phase).toBe("ready");
  });

  it("clears invalid actions durably and restores the current catalog", async () => {
    const server = new MockServer();
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["start", "bad", "good"]);
    await client.start(PRESET_START);
    server.actionFailure = { status: 400, code: "invalid_action" };
    const selected = client.getSnapshot().actions[0] as ActionView;

    await client.act(selected);
    expect(client.getSnapshot().phase).toBe("ready");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).toBeNull();
    expect(server.actionBodies).toHaveLength(1);
    await client.act(selected);
    expect(server.actionBodies).toHaveLength(2);
  });

  it("clears a server-already-closed request durably", async () => {
    const server = new MockServer();
    server.closeFailure = { status: 410, code: "session_closed" };
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["start"]);
    await client.start(PRESET_START);
    await client.close();

    expect(client.getSnapshot().phase).toBe("closed");
    const journal = JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null");
    expect(journal.lifecycle).toBe("closed");
    expect(journal.pending).toBeNull();
  });

  it("requires explicit refresh after idempotency conflict without retrying or mutating", async () => {
    const server = new MockServer();
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["start", "conflict"]);
    await client.start(PRESET_START);
    server.actionFailure = { status: 409, code: "idempotency_conflict" };
    const selected = client.getSnapshot().actions[0] as ActionView;

    await client.act(selected);
    expect(client.getSnapshot().phase).toBe("error");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).toBeNull();
    await client.retry();
    expect(client.getSnapshot().phase).toBe("ready");
    expect(server.actionBodies).toHaveLength(1);
  });

  it("observes after start and resume acknowledgements before becoming ready", async () => {
    const server = new MockServer();
    const observed = view("2", "state-2", [action("observed", "Observed")]);
    server.currentView = observed;
    server.startResponseView = view("0", "state-0", [action("stale", "Stale")]);
    const { client } = await bootClient(server, new MemoryStorage(), ["start", "resume"]);

    await client.start(PRESET_START);
    expect(client.getSnapshot().session?.view.revision).toBe("2");
    expect(client.getSnapshot().actions.map(({ action_id }) => action_id)).toEqual([wireId("observed")]);
    await client.close();
    await client.newGame();
    server.resumeResponseView = view("1", "state-1", [action("stale-resume", "Stale")]);
    await client.resume("{\"seed\":\"71\"}");
    expect(client.getSnapshot().session?.view.revision).toBe("2");
    expect(server.observeCount).toBeGreaterThanOrEqual(2);
  });

  it("durably closes when the acknowledged start becomes closed during readback", async () => {
    const server = new MockServer();
    server.observeFailure = { status: 410, code: "session_closed" };
    const storage = new MemoryStorage();
    const { client } = await bootClient(server, storage, ["start"]);

    await client.start(PRESET_START);

    expect(client.getSnapshot().phase).toBe("closed");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null")).toMatchObject({
      lifecycle: "closed",
      session_id: server.sessionId,
      pending: null,
    });
  });

  it("clears a resource limit and keeps an action session usable", async () => {
    const server = new MockServer();
    const storage = new MemoryStorage();
    const { client } = await bootClient(server, storage, ["start", "limited"]);
    await client.start(PRESET_START);
    server.actionFailure = { status: 413, code: "resource_limit" };

    await client.act(client.getSnapshot().actions[0] as ActionView);

    expect(client.getSnapshot().phase).toBe("ready");
    expect(client.getSnapshot().message).toBe("resource_limit");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).toBeNull();
    await expect(client.save()).resolves.toBe(server.saveText);
    await client.close();
    expect(client.getSnapshot().phase).toBe("closed");
  });

  it("recovers the active session after a tab loses its local journal", async () => {
    const server = new MockServer();
    const storage = new MemoryStorage();
    const first = await bootClient(server, storage, ["start"]);
    await first.client.start(PRESET_START);
    storage.values.delete(JOURNAL_KEY);

    const second = new GameClient({ fetch: server.fetch, storage, commandId: commandIds("second") });
    await second.boot();
    expect(second.getSnapshot().phase).toBe("ready");
    expect(second.getSnapshot().session?.session_id).toBe(server.sessionId);
  });

  it("validates the close acknowledgement before clearing the pending close", async () => {
    const server = new MockServer();
    server.closeResponse = { closed: false };
    const { client, storage } = await bootClient(server, new MemoryStorage(), ["start"]);
    await client.start(PRESET_START);
    await client.close();
    expect(client.getSnapshot().phase).toBe("error");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).not.toBeNull();

    server.closeResponse = { closed: true };
    await client.retry();
    expect(client.getSnapshot().phase).toBe("closed");
    expect(JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null").pending).toBeNull();
  });

  it("catches generated-ID and JSON serialization failures without sending", async () => {
    const server = new MockServer();
    const storage = new MemoryStorage();
    const idFailure = new GameClient({
      fetch: server.fetch,
      storage,
      commandId: () => { throw new Error("id factory failed"); },
    });
    await idFailure.boot();
    await idFailure.start(PRESET_START);
    expect(idFailure.getSnapshot().phase).toBe("error");
    expect(idFailure.getSnapshot().message).toBe("client_error");
    expect(server.startBodies).toHaveLength(0);

    const serializationFailure = await bootClient(server, new MemoryStorage(), ["serial"]);
    const invalidRecipe = { kind: "preset", character_preset_id: "rook", seed: BigInt(71) } as unknown as StartRecipe;
    await serializationFailure.client.start(invalidRecipe);
    expect(serializationFailure.client.getSnapshot().phase).toBe("error");
    expect(serializationFailure.client.getSnapshot().message).toBe("client_error");
    expect(server.startBodies).toHaveLength(0);

    const malformedAction = await bootClient(server, new MemoryStorage(), ["start", "bad-action"]);
    await malformedAction.client.start(PRESET_START);
    const invalidAction = {
      ...(malformedAction.client.getSnapshot().actions[0] as ActionView),
      action_id: "not-a-canonical-id",
    };
    await malformedAction.client.act(invalidAction);
    expect(malformedAction.client.getSnapshot().phase).toBe("error");
    expect(malformedAction.client.getSnapshot().message).toBe("storage_write_failed");
    expect(server.actionBodies).toHaveLength(0);
  });

  it("rejects nonzero initial, skipping, inconsistent, and duplicate catalog cursors", async () => {
    const cases: Array<() => MockServer> = [
      () => {
        const server = new MockServer();
        const first = [action("a-1", "One")];
        server.currentView = view("0", "state-0", first, page("state-0", first, "1", "1", null));
        return server;
      },
      () => {
        const server = new MockServer();
        const first = [action("a-1", "One")];
        server.currentView = view("0", "state-0", first, page("state-0", first, "0", "2", "2"));
        return server;
      },
      () => {
        const server = new MockServer();
        const first = [action("a-1", "One")];
        server.currentView = view("0", "state-0", first, page("state-0", first, "0", "2", "1"));
        server.catalogPages.set("1", page("state-0", [], "1", "2", null));
        return server;
      },
      () => {
        const server = new MockServer();
        const first = [action("a-1", "One")];
        server.currentView = view("0", "state-0", first, page("state-0", first, "0", "2", "1"));
        server.catalogPages.set("1", page("state-0", [action("a-1", "Duplicate")], "1", "2", null));
        return server;
      },
    ];
    for (const createServer of cases) {
      const server = createServer();
      const { client } = await bootClient(server, new MemoryStorage(), ["start"]);
      await client.start(PRESET_START);
      expect(client.getSnapshot().phase).not.toBe("ready");
      expect(client.getSnapshot().catalogComplete).toBe(false);
    }
  });

  it("does not install ready when the final journal clear fails", async () => {
    const server = new MockServer();
    const storage = new MemoryStorage();
    const { client } = await bootClient(server, storage, ["start"]);
    storage.failOnWrite = 3;

    await client.start(PRESET_START);
    expect(client.getSnapshot().phase).toBe("uncertain");
    expect(client.getSnapshot().catalogComplete).toBe(false);
    const journal = JSON.parse(storage.getItem(JOURNAL_KEY) ?? "null");
    expect(journal.pending).not.toBeNull();
  });
});
