export type PendingKind = "start" | "resume" | "action" | "close";

export interface PendingRequest {
  kind: PendingKind;
  path: string;
  body: string;
  label: string;
  session_id: string | null;
}

export interface Journal {
  version: 1;
  instance_id: string;
  lifecycle: "none" | "open" | "closed";
  session_id: string | null;
  pending: PendingRequest | null;
  last_saved_save_json?: string;
}

export class JournalError extends Error {
  readonly code = "storage_invalid";

  constructor() {
    super("storage_invalid");
    this.name = "JournalError";
  }
}

const LOWER_HEX_ID = /^[0-9a-f]{64}$/;
const SAFE_ID = /^[A-Za-z0-9_-]{1,128}$/;
const DECIMAL = /^(0|[1-9][0-9]*)$/;
const MAX_JOURNAL_LENGTH = 2 * 1024 * 1024;
const MAX_PENDING_BODY_LENGTH = 6 * 256 * 1024 + 4096;
const MAX_GAME_STRING_LENGTH = 4096;
const MAX_SAVE_JSON_LENGTH = 256 * 1024;
const UTF8_ENCODER = new TextEncoder();

function invalid(): never {
  throw new JournalError();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function hasOwn(object: Record<string, unknown>, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(object, key);
}

function exactKeys(
  object: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[] = [],
): void {
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(object)) {
    if (!allowed.has(key)) invalid();
  }
  for (const key of required) {
    if (!hasOwn(object, key)) invalid();
  }
}

function requiredString(
  object: Record<string, unknown>,
  key: string,
  predicate: (value: string) => boolean = (value) => value.length > 0,
): string {
  const value = object[key];
  if (typeof value !== "string" || !predicate(value)) invalid();
  return value;
}

function lowerHexId(value: unknown): string {
  if (typeof value !== "string" || !LOWER_HEX_ID.test(value)) invalid();
  return value;
}

function safeId(value: unknown): string {
  if (typeof value !== "string" || !SAFE_ID.test(value)) invalid();
  return value;
}

function decimal(value: unknown): string {
  if (typeof value !== "string" || !DECIMAL.test(value)) invalid();
  return value;
}

function gameString(object: Record<string, unknown>, key: string): string {
  return boundedString(object[key], MAX_GAME_STRING_LENGTH);
}

function opaqueString(object: Record<string, unknown>, key: string): string {
  return boundedString(object[key], MAX_SAVE_JSON_LENGTH);
}

function boundedString(value: unknown, maximumLength: number): string {
  if (
    typeof value !== "string" ||
    UTF8_ENCODER.encode(value).byteLength > maximumLength
  )
    invalid();
  return value;
}

function parseUniqueJson(text: string): unknown {
  if (UTF8_ENCODER.encode(text).byteLength > MAX_JOURNAL_LENGTH) invalid();
  try {
    scanJsonForDuplicateKeys(text);
    return JSON.parse(text) as unknown;
  } catch (error) {
    if (error instanceof JournalError) throw error;
    invalid();
  }
}

function scanJsonForDuplicateKeys(text: string): void {
  let index = 0;

  const skipWhitespace = () => {
    while (index < text.length && /\s/.test(text[index] ?? "")) index += 1;
  };

  const scanString = (): void => {
    if (text[index] !== '"') invalid();
    index += 1;
    while (index < text.length) {
      const character = text[index];
      if (character === '"') {
        index += 1;
        return;
      }
      if (character === undefined || character < " ") invalid();
      if (character === "\\") {
        index += 1;
        const escaped = text[index];
        if (escaped === "u") {
          if (!/^[0-9a-fA-F]{4}$/.test(text.slice(index + 1, index + 5)))
            invalid();
          index += 5;
        } else if (escaped !== undefined && '"\\/bfnrt'.includes(escaped)) {
          index += 1;
        } else {
          invalid();
        }
      } else {
        index += 1;
      }
    }
    invalid();
  };

  const scanValue = (): void => {
    skipWhitespace();
    const character = text[index];
    if (character === '"') {
      scanString();
      return;
    }
    if (character === "{") {
      scanObject();
      return;
    }
    if (character === "[") {
      scanArray();
      return;
    }
    for (const literal of ["true", "false", "null"]) {
      if (text.startsWith(literal, index)) {
        index += literal.length;
        return;
      }
    }
    const number = text
      .slice(index)
      .match(/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/);
    if (number?.[0]) {
      index += number[0].length;
      return;
    }
    invalid();
  };

  const scanObject = (): void => {
    index += 1;
    skipWhitespace();
    const keys = new Set<string>();
    if (text[index] === "}") {
      index += 1;
      return;
    }
    while (true) {
      skipWhitespace();
      const keyStart = index;
      scanString();
      let key: unknown;
      try {
        key = JSON.parse(text.slice(keyStart, index)) as unknown;
      } catch {
        invalid();
      }
      if (typeof key !== "string" || keys.has(key)) invalid();
      keys.add(key);
      skipWhitespace();
      if (text[index] !== ":") invalid();
      index += 1;
      scanValue();
      skipWhitespace();
      if (text[index] === "}") {
        index += 1;
        return;
      }
      if (text[index] !== ",") invalid();
      index += 1;
    }
  };

  const scanArray = (): void => {
    index += 1;
    skipWhitespace();
    if (text[index] === "]") {
      index += 1;
      return;
    }
    while (true) {
      scanValue();
      skipWhitespace();
      if (text[index] === "]") {
        index += 1;
        return;
      }
      if (text[index] !== ",") invalid();
      index += 1;
    }
  };

  scanValue();
  skipWhitespace();
  if (index !== text.length) invalid();
}

function parseBody(body: string): Record<string, unknown> {
  const value = parseUniqueJson(body);
  if (!isRecord(value)) invalid();
  return value;
}

function validateStart(value: unknown): void {
  if (!isRecord(value)) invalid();
  const kind = requiredString(value, "kind");
  if (kind === "preset") {
    exactKeys(value, ["kind", "character_preset_id", "seed"]);
    gameString(value, "character_preset_id");
    gameString(value, "seed");
    return;
  }
  if (kind !== "custom") invalid();
  exactKeys(value, ["kind", "selection", "seed"]);
  gameString(value, "seed");
  if (!isRecord(value.selection)) invalid();
  exactKeys(value.selection, ["name", "choices"]);
  gameString(value.selection, "name");
  if (!Array.isArray(value.selection.choices)) invalid();
  for (const choice of value.selection.choices) {
    if (!isRecord(choice)) invalid();
    exactKeys(choice, ["slot_id", "choice_id"]);
    gameString(choice, "slot_id");
    gameString(choice, "choice_id");
  }
}

function validatePendingBody(pending: PendingRequest): void {
  if (UTF8_ENCODER.encode(pending.body).byteLength > MAX_PENDING_BODY_LENGTH)
    invalid();
  const body = parseBody(pending.body);
  switch (pending.kind) {
    case "start":
      exactKeys(body, ["creation_id", "start"]);
      safeId(body.creation_id);
      validateStart(body.start);
      return;
    case "resume":
      exactKeys(body, ["creation_id", "save_json"]);
      safeId(body.creation_id);
      opaqueString(body, "save_json");
      return;
    case "action":
      exactKeys(body, [
        "command_id",
        "expected_revision",
        "expected_state_id",
        "action_id",
      ]);
      safeId(body.command_id);
      decimal(body.expected_revision);
      lowerHexId(body.expected_state_id);
      lowerHexId(body.action_id);
      return;
    case "close":
      exactKeys(body, []);
      return;
  }
}

function validatePending(
  value: unknown,
  journalSessionId: string | null,
): PendingRequest | null {
  if (value === null) return null;
  if (!isRecord(value)) invalid();
  exactKeys(value, ["kind", "path", "body", "label", "session_id"]);
  const kind = requiredString(value, "kind") as PendingKind;
  if (!["start", "resume", "action", "close"].includes(kind)) invalid();
  const path = requiredString(value, "path");
  if (
    path.includes("%") ||
    path.includes("\\") ||
    path.includes("?") ||
    path.includes("#")
  ) {
    invalid();
  }
  const body = requiredString(value, "body");
  const label = requiredString(
    value,
    "label",
    (text) => UTF8_ENCODER.encode(text).byteLength <= MAX_GAME_STRING_LENGTH,
  );
  const sessionValue = value.session_id;
  const sessionId = sessionValue === null ? null : lowerHexId(sessionValue);
  const pending = { kind, path, body, label, session_id: sessionId };
  validatePendingBody(pending);

  if (kind === "start" || kind === "resume") {
    if (
      sessionId !== null ||
      path !== (kind === "start" ? "/api/sessions" : "/api/resume")
    ) {
      invalid();
    }
  } else {
    if (sessionId === null || journalSessionId === null) invalid();
    const suffix = kind === "action" ? "actions" : "close";
    if (
      sessionId !== journalSessionId ||
      path !== `/api/sessions/${sessionId}/${suffix}`
    )
      invalid();
  }
  return pending;
}

export function parseJournal(raw: string): Journal {
  if (typeof raw !== "string") invalid();
  const value = parseUniqueJson(raw);
  if (!isRecord(value)) invalid();
  exactKeys(
    value,
    ["version", "instance_id", "lifecycle", "session_id", "pending"],
    ["last_saved_save_json"],
  );
  if (value.version !== 1) invalid();
  const instanceId = lowerHexId(value.instance_id);
  const lifecycle = value.lifecycle;
  if (lifecycle !== "none" && lifecycle !== "open" && lifecycle !== "closed")
    invalid();
  const sessionValue = value.session_id;
  const sessionId = sessionValue === null ? null : lowerHexId(sessionValue);
  const pending = validatePending(value.pending, sessionId);
  if (value.last_saved_save_json !== undefined) {
    boundedString(value.last_saved_save_json, MAX_SAVE_JSON_LENGTH);
  }
  if (lifecycle === "none" && sessionId !== null) invalid();
  if (lifecycle === "open" && sessionId === null) invalid();
  if (lifecycle === "closed" && (sessionId === null || pending !== null))
    invalid();
  if (
    pending !== null &&
    (pending.kind === "action" || pending.kind === "close") &&
    lifecycle !== "open"
  ) {
    invalid();
  }

  return {
    version: 1,
    instance_id: instanceId,
    lifecycle,
    session_id: sessionId,
    pending,
    ...(value.last_saved_save_json === undefined
      ? {}
      : { last_saved_save_json: value.last_saved_save_json as string }),
  };
}
