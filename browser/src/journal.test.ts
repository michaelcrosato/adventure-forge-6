import { describe, expect, it } from "vitest";

import { parseJournal } from "./journal";

const INSTANCE_ID = "a".repeat(64);
const SESSION_ID = "b".repeat(64);

function journal(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    version: 1,
    instance_id: INSTANCE_ID,
    lifecycle: "none",
    session_id: null,
    pending: null,
    ...overrides,
  });
}

function pendingJournal(
  pending: Record<string, unknown>,
  overrides: Record<string, unknown> = {},
): string {
  return journal({
    lifecycle: "open",
    session_id: SESSION_ID,
    pending,
    ...overrides,
  });
}

const startBody =
  '{"creation_id":"create-one","start":{"kind":"preset","character_preset_id":"rook","seed":"71"}}';

const actionBody = JSON.stringify({
  command_id: "action-one",
  expected_revision: "4",
  expected_state_id: "c".repeat(64),
  action_id: "d".repeat(64),
});

describe("journal admission", () => {
  it("accepts the exact durable shapes and preserves opaque bodies", () => {
    const resumeBody =
      '{"creation_id":"resume-one","save_json":"{\\"seed\\":\\"71\\"}\\n"}';
    const parsed = parseJournal(
      journal({
        lifecycle: "none",
        pending: {
          kind: "resume",
          path: "/api/resume",
          body: resumeBody,
          label: "Resuming",
          session_id: null,
        },
      }),
    );
    expect(parsed.instance_id).toBe(INSTANCE_ID);
    expect(parsed.pending?.body).toBe(resumeBody);
    expect(parsed.pending?.path).toBe("/api/resume");
  });

  it("accepts a custom start recipe with decimal seed text", () => {
    const body = JSON.stringify({
      creation_id: "create-custom",
      start: {
        kind: "custom",
        seed: "18446744073709551615",
        selection: {
          name: "Rook:Ash",
          choices: [{ slot_id: "origin:lineage", choice_id: "kiln.born" }],
        },
      },
    });
    const parsed = parseJournal(
      journal({
        pending: {
          kind: "start",
          path: "/api/sessions",
          body,
          label: "Starting",
          session_id: null,
        },
      }),
    );
    expect(parsed.pending?.kind).toBe("start");
    expect(parsed.pending?.body).toBe(body);
  });

  it("retains server-invalid decimal and opaque save input for definitive replay", () => {
    const seed = "184467440737095516160000000000000000000";
    const start = JSON.stringify({
      creation_id: "create-invalid-seed",
      start: {
        kind: "preset",
        character_preset_id: "preset:rook.v1",
        seed,
      },
    });
    const parsedStart = parseJournal(
      journal({
        pending: {
          kind: "start",
          path: "/api/sessions",
          body: start,
          label: "Starting",
          session_id: null,
        },
      }),
    );
    expect(parsedStart.pending?.body).toBe(start);

    const opaqueSave = `not-json:${"x".repeat(8_192)}`;
    const resume = JSON.stringify({
      creation_id: "resume-invalid-save",
      save_json: opaqueSave,
    });
    const parsedResume = parseJournal(
      journal({
        pending: {
          kind: "resume",
          path: "/api/resume",
          body: resume,
          label: "Resuming",
          session_id: null,
        },
      }),
    );
    expect(parsedResume.pending?.body).toBe(resume);
  });

  it("retains a non-decimal seed until the server gives its definitive error", () => {
    const body = JSON.stringify({
      creation_id: "create-bad-seed",
      start: {
        kind: "preset",
        character_preset_id: "preset:rook.v1",
        seed: "not-a-decimal",
      },
    });
    const parsed = parseJournal(
      journal({
        pending: {
          kind: "start",
          path: "/api/sessions",
          body,
          label: "Starting",
          session_id: null,
        },
      }),
    );
    expect(parsed.pending?.body).toBe(body);
  });

  it("retains a name beyond the kernel limit until the server gives its definitive error", () => {
    const body = JSON.stringify({
      creation_id: "create-long-name",
      start: {
        kind: "custom",
        selection: {
          name: "N".repeat(257),
          choices: [],
        },
        seed: "71",
      },
    });
    const parsed = parseJournal(
      journal({
        pending: {
          kind: "start",
          path: "/api/sessions",
          body,
          label: "Starting",
          session_id: null,
        },
      }),
    );
    expect(parsed.pending?.body).toBe(body);
  });

  it.each([
    ["unsupported version", journal({ version: 2 })],
    ["unknown root field", journal({ view: {} })],
    ["persisted bearer token", journal({ token: "secret" })],
    [
      "unknown pending field",
      journal({
        pending: {
          kind: "start",
          path: "/api/sessions",
          body: startBody,
          label: "Starting",
          session_id: null,
          view: {},
        },
      }),
    ],
    [
      "kind and path mismatch",
      pendingJournal({
        kind: "action",
        path: "/api/resume",
        body: actionBody,
        label: "Act",
        session_id: SESSION_ID,
      }),
    ],
    [
      "modified session binding",
      pendingJournal({
        kind: "action",
        path: `/api/sessions/${"e".repeat(64)}/actions`,
        body: actionBody,
        label: "Act",
        session_id: SESSION_ID,
      }),
    ],
    [
      "encoded path",
      pendingJournal({
        kind: "action",
        path: `/api/sessions/${SESSION_ID}/%61ctions`,
        body: actionBody,
        label: "Act",
        session_id: SESSION_ID,
      }),
    ],
    [
      "query path",
      pendingJournal({
        kind: "action",
        path: `/api/sessions/${SESSION_ID}/actions?next=close`,
        body: actionBody,
        label: "Act",
        session_id: SESSION_ID,
      }),
    ],
    [
      "backslash path",
      pendingJournal({
        kind: "action",
        path: `/api/sessions/${SESSION_ID}\\actions`,
        body: actionBody,
        label: "Act",
        session_id: SESSION_ID,
      }),
    ],
    [
      "absolute path",
      pendingJournal({
        kind: "action",
        path: `https://example.invalid/api/sessions/${SESSION_ID}/actions`,
        body: actionBody,
        label: "Act",
        session_id: SESSION_ID,
      }),
    ],
    [
      "unknown body field",
      journal({
        pending: {
          kind: "start",
          path: "/api/sessions",
          body: '{"creation_id":"create-one","start":{"kind":"preset","character_preset_id":"rook","seed":"71"},"view":{}}',
          label: "Starting",
          session_id: null,
        },
      }),
    ],
    [
      "duplicate body key",
      journal({
        pending: {
          kind: "start",
          path: "/api/sessions",
          body: '{"creation_id":"create-one","creation_id":"create-two","start":{"kind":"preset","character_preset_id":"rook","seed":"71"}}',
          label: "Starting",
          session_id: null,
        },
      }),
    ],
    [
      "numeric seed instead of decimal text",
      journal({
        pending: {
          kind: "start",
          path: "/api/sessions",
          body: '{"creation_id":"create-one","start":{"kind":"preset","character_preset_id":"rook","seed":71}}',
          label: "Starting",
          session_id: null,
        },
      }),
    ],
    [
      "closed journal with pending request",
      journal({
        lifecycle: "closed",
        session_id: SESSION_ID,
        pending: {
          kind: "start",
          path: "/api/sessions",
          body: startBody,
          label: "Starting",
          session_id: null,
        },
      }),
    ],
  ])("rejects %s", (_label, raw) => {
    expect(() => parseJournal(raw)).toThrow("storage_invalid");
  });
});
