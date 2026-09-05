import { describe, expect, it } from "vitest";

import { ApiError, GameApi, type FetchLike } from "./api";

const TOKEN = "1".repeat(64);
const INSTANCE_ID = "2".repeat(64);
const SESSION_ID = "3".repeat(64);

function jsonResponse(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function fetchRecorder(
  response:
    | Response
    | ((
        input: RequestInfo | URL,
        init: RequestInit | undefined,
      ) => Response | Promise<Response>) = () => jsonResponse({}),
): {
  fetcher: FetchLike;
  requests: Array<{ input: RequestInfo | URL; init: RequestInit | undefined }>;
} {
  const requests: Array<{
    input: RequestInfo | URL;
    init: RequestInit | undefined;
  }> = [];
  const fetcher: FetchLike = async (input, init) => {
    requests.push({ input, init });
    return typeof response === "function" ? response(input, init) : response;
  };
  return { fetcher, requests };
}

async function bootApi(fetcher: FetchLike): Promise<GameApi> {
  const api = new GameApi(fetcher);
  await api.bootstrap();
  return api;
}

describe("browser API boundary", () => {
  it("validates opaque bootstrap token and instance identifiers", async () => {
    const valid = fetchRecorder(
      jsonResponse({ token: TOKEN, instance_id: INSTANCE_ID }),
    );
    const api = new GameApi(valid.fetcher);
    await expect(api.bootstrap()).resolves.toEqual({
      token: TOKEN,
      instance_id: INSTANCE_ID,
    });

    for (const value of ["short", "G".repeat(64), "a".repeat(63)]) {
      const invalid = fetchRecorder(
        jsonResponse({ token: value, instance_id: INSTANCE_ID }),
      );
      await expect(
        new GameApi(invalid.fetcher).bootstrap(),
      ).rejects.toMatchObject({
        code: "invalid_bootstrap_response",
      });
    }
    const invalidInstance = fetchRecorder(
      jsonResponse({ token: TOKEN, instance_id: "instance" }),
    );
    await expect(
      new GameApi(invalidInstance.fetcher).bootstrap(),
    ).rejects.toMatchObject({
      code: "invalid_bootstrap_response",
    });
  });

  it("drops an earlier bearer token before a failed rebootstrap", async () => {
    let bootstrapCount = 0;
    const recorded = fetchRecorder((input) => {
      if (input === "/api/bootstrap") {
        bootstrapCount += 1;
        return bootstrapCount === 1
          ? jsonResponse({ token: TOKEN, instance_id: INSTANCE_ID })
          : jsonResponse({ token: "invalid", instance_id: INSTANCE_ID });
      }
      return jsonResponse({});
    });
    const api = new GameApi(recorded.fetcher);
    await api.bootstrap();
    await expect(api.bootstrap()).rejects.toMatchObject({
      code: "invalid_bootstrap_response",
    });
    await expect(api.current()).rejects.toMatchObject({
      code: "unauthorized",
    });
    expect(recorded.requests).toHaveLength(2);
  });

  it("rejects hostile replay paths before fetching or attaching the bearer token", async () => {
    const recorded = fetchRecorder();
    const api = new GameApi(recorded.fetcher);
    for (const path of [
      "https://example.invalid/collect",
      "//example.invalid/collect",
      "/api/sessions/../close",
      `/api/sessions/${SESSION_ID}/actions?next=close`,
      `/api/sessions/${SESSION_ID}/%61ctions`,
      `/api/sessions/${SESSION_ID}\\actions`,
      "/api/not-a-route",
    ]) {
      await expect(api.replayJson(path, "{}")).rejects.toMatchObject({
        code: "invalid_api_path",
      });
    }
    expect(recorded.requests).toHaveLength(0);
  });

  it("allows only the exact local route and sets redirect failure mode", async () => {
    const recorded = fetchRecorder((input) =>
      input === "/api/bootstrap"
        ? jsonResponse({ token: TOKEN, instance_id: INSTANCE_ID })
        : jsonResponse({ closed: true }),
    );
    const api = await bootApi(recorded.fetcher);
    await expect(
      api.replayJson(`/api/sessions/${SESSION_ID}/close`, "{}"),
    ).resolves.toEqual({
      closed: true,
    });
    const request = recorded.requests.at(-1);
    expect(request?.input).toBe(`/api/sessions/${SESSION_ID}/close`);
    expect(request?.init?.redirect).toBe("error");
    expect(new Headers(request?.init?.headers).get("Authorization")).toBe(
      `Bearer ${TOKEN}`,
    );
  });

  it("turns redirects into non-success errors", async () => {
    const redirectRequests = fetchRecorder((input) =>
      input === "/api/bootstrap"
        ? jsonResponse({ token: TOKEN, instance_id: INSTANCE_ID })
        : new Response(null, { status: 302 }),
    );
    const redirectClient = await bootApi(redirectRequests.fetcher);
    await expect(redirectClient.current()).rejects.toMatchObject({
      code: "redirect",
    });
  });

  it("classifies a response-body disconnect as retryable network loss", async () => {
    const brokenResponse = {
      ok: true,
      status: 200,
      redirected: false,
      text: async () => {
        throw new Error("body disconnected");
      },
    } as unknown as Response;
    const recorded = fetchRecorder(brokenResponse);
    const broken = new GameApi(recorded.fetcher);
    await expect(broken.bootstrap()).rejects.toMatchObject({
      code: "network",
      retryable: true,
    });
  });

  it("keeps close response validation available", async () => {
    const close = fetchRecorder(jsonResponse({ closed: false }));
    const client = new GameApi(async (input, init) => {
      if (input === "/api/bootstrap")
        return jsonResponse({ token: TOKEN, instance_id: INSTANCE_ID });
      return close.fetcher(input, init);
    });
    await client.bootstrap();
    await expect(client.close(SESSION_ID)).rejects.toMatchObject({
      code: "invalid_close_response",
    });
  });

  it("does not hide a transport error behind an ordinary response error", async () => {
    const broken = fetchRecorder({
      ok: false,
      status: 503,
      redirected: false,
      text: async () => {
        throw new Error("body disconnected");
      },
    } as unknown as Response);
    const api = new GameApi(broken.fetcher);
    await expect(api.bootstrap()).rejects.toBeInstanceOf(ApiError);
    await expect(api.bootstrap()).rejects.toMatchObject({
      code: "network",
      retryable: true,
    });
  });
});
