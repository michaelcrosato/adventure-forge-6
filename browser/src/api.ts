import type {
  ActionPage,
  ActionRequest,
  BootstrapResponse,
  CurrentResponse,
  DecimalString,
  SessionHandle,
  SessionView,
  StartOptions,
  StartRecipe,
} from "./types";

export type FetchLike = typeof globalThis.fetch;

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly retryable: boolean;

  constructor(status: number, code: string, retryable = false) {
    super(code);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.retryable = retryable;
  }
}

export function isDecimalString(value: unknown): value is DecimalString {
  return typeof value === "string" && /^(0|[1-9][0-9]*)$/.test(value);
}

const LOWER_HEX_ID = /^[0-9a-f]{64}$/;

function invalidApiPath(): never {
  throw new ApiError(0, "invalid_api_path");
}

function requireOpaqueId(
  object: Record<string, unknown>,
  key: string,
  label: string,
): string {
  const value = object[key];
  if (typeof value !== "string" || !LOWER_HEX_ID.test(value)) {
    throw new ApiError(0, `invalid_${label}_response`);
  }
  return value;
}

function validateApiPath(path: string, method: string): void {
  if (
    (method !== "GET" && method !== "POST") ||
    !path.startsWith("/") ||
    path.startsWith("//") ||
    path.includes("%") ||
    path.includes("\\") ||
    path.includes("?") ||
    path.includes("#")
  ) {
    invalidApiPath();
  }
  if (
    (path === "/api/bootstrap" ||
      path === "/api/options" ||
      path === "/api/current") &&
    method === "GET"
  ) {
    return;
  }
  if (path === "/api/sessions" || path === "/api/resume") {
    if (method === "POST") return;
    invalidApiPath();
  }
  const match = path.match(
    /^\/api\/sessions\/([0-9a-f]{64})(?:\/(catalog|actions|save|close))?$/,
  );
  if (!match) invalidApiPath();
  const suffix = match[2];
  if (suffix === undefined) {
    if (method === "GET") return;
    invalidApiPath();
  }
  if (suffix === "save") {
    if (method === "GET") return;
  } else if (method === "POST") {
    return;
  }
  invalidApiPath();
}

async function responseText(response: Response): Promise<string> {
  try {
    return await response.text();
  } catch {
    throw new ApiError(0, "network", true);
  }
}

function requireObject(value: unknown, label: string): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new ApiError(0, `invalid_${label}_response`);
  }
  return value as Record<string, unknown>;
}

function decodeJson<T>(text: string, label: string): T {
  try {
    const value: unknown = JSON.parse(text);
    requireObject(value, label);
    return value as T;
  } catch (error) {
    if (error instanceof ApiError) {
      throw error;
    }
    throw new ApiError(0, `invalid_${label}_response`);
  }
}

function retryableStatus(status: number, code: string): boolean {
  return (
    status === 408 ||
    status === 425 ||
    status === 429 ||
    status >= 500 ||
    code === "busy"
  );
}

function errorCodeFromBody(text: string, status: number): string {
  try {
    const value: unknown = JSON.parse(text);
    if (
      value !== null &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      typeof (value as Record<string, unknown>).error === "string"
    ) {
      return (value as Record<string, unknown>).error as string;
    }
  } catch {
    // The public error contract is stable; malformed error bodies are hidden.
  }
  return status >= 500 ? "unavailable" : `http_${status}`;
}

export class GameApi {
  private readonly fetcher: FetchLike;
  private token: string | null = null;

  constructor(fetcher?: FetchLike) {
    const selected = fetcher ?? globalThis.fetch?.bind(globalThis);
    if (!selected) {
      throw new Error("fetch is unavailable");
    }
    this.fetcher = selected;
  }

  async bootstrap(): Promise<BootstrapResponse> {
    this.token = null;
    const response = await this.send("/api/bootstrap", "GET", undefined, true);
    const value = requireObject(
      decodeJson<unknown>(await responseText(response), "bootstrap"),
      "bootstrap",
    );
    const token = requireOpaqueId(value, "token", "bootstrap");
    const instanceId = requireOpaqueId(value, "instance_id", "bootstrap");
    this.token = token;
    return { token, instance_id: instanceId };
  }

  async options(): Promise<StartOptions> {
    return this.json<StartOptions>("/api/options", "GET");
  }

  async current(): Promise<CurrentResponse> {
    return this.json<CurrentResponse>("/api/current", "GET");
  }

  async start(creationId: string, start: StartRecipe): Promise<SessionHandle> {
    return this.json<SessionHandle>("/api/sessions", "POST", {
      creation_id: creationId,
      start,
    });
  }

  async resume(creationId: string, saveJson: string): Promise<SessionHandle> {
    return this.json<SessionHandle>("/api/resume", "POST", {
      creation_id: creationId,
      save_json: saveJson,
    });
  }

  async observe(sessionId: string): Promise<SessionView> {
    return this.json<SessionView>(`/api/sessions/${sessionId}`, "GET");
  }

  async catalog(
    sessionId: string,
    expectedStateId: string,
    offset: DecimalString,
    pageSize: DecimalString = "128",
  ): Promise<ActionPage> {
    return this.json<ActionPage>(`/api/sessions/${sessionId}/catalog`, "POST", {
      expected_state_id: expectedStateId,
      offset,
      page_size: pageSize,
    });
  }

  async act(sessionId: string, request: ActionRequest): Promise<SessionView> {
    return this.json<SessionView>(
      `/api/sessions/${sessionId}/actions`,
      "POST",
      request,
    );
  }

  async save(sessionId: string): Promise<string> {
    const response = await this.send(`/api/sessions/${sessionId}/save`, "GET");
    return responseText(response);
  }

  async close(sessionId: string): Promise<{ closed: true }> {
    const value = await this.json<{ closed: boolean }>(
      `/api/sessions/${sessionId}/close`,
      "POST",
      {},
    );
    if (value.closed !== true) {
      throw new ApiError(0, "invalid_close_response");
    }
    return { closed: true };
  }

  /** Replay a journaled POST without parsing or reserializing its body. */
  async replayJson<T>(path: string, body: string): Promise<T> {
    const response = await this.sendRaw(path, "POST", body);
    return decodeJson<T>(await responseText(response), "api");
  }

  private async json<T>(
    path: string,
    method: "GET" | "POST",
    body?: unknown,
  ): Promise<T> {
    const response = await this.send(path, method, body);
    return decodeJson<T>(await responseText(response), "api");
  }

  private async send(
    path: string,
    method: "GET" | "POST",
    body?: unknown,
    allowUnauthenticated = false,
  ): Promise<Response> {
    return this.sendRaw(
      path,
      method,
      body === undefined ? undefined : JSON.stringify(body),
      allowUnauthenticated,
    );
  }

  private async sendRaw(
    path: string,
    method: "GET" | "POST",
    body: string | undefined,
    allowUnauthenticated = false,
  ): Promise<Response> {
    validateApiPath(path, method);
    if (!allowUnauthenticated && this.token === null) {
      throw new ApiError(401, "unauthorized");
    }

    const headers = new Headers({ Accept: "application/json" });
    if (body !== undefined) {
      headers.set("Content-Type", "application/json");
    }
    if (!allowUnauthenticated) {
      headers.set("Authorization", `Bearer ${this.token}`);
    }

    let response: Response;
    try {
      response = await this.fetcher(path, {
        method,
        headers,
        body,
        credentials: "same-origin",
        cache: "no-store",
        redirect: "error",
      });
    } catch {
      throw new ApiError(0, "network", true);
    }
    if (
      response.redirected ||
      (response.status >= 300 && response.status < 400)
    ) {
      throw new ApiError(0, "redirect");
    }
    if (!response.ok) {
      const code = errorCodeFromBody(
        await responseText(response),
        response.status,
      );
      throw new ApiError(
        response.status,
        code,
        retryableStatus(response.status, code),
      );
    }
    return response;
  }
}
