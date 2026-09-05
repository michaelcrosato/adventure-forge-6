import type { ActionView } from "./types";

export function actionLabel(action: ActionView): string {
  const values = Object.values(action.parameter_display_values);
  return values.length ? `${action.label}: ${values.join(", ")}` : action.label;
}

/** Presentation only: preserve kernel order and search every supplied action. */
export function filterActions(actions: readonly ActionView[], query: string, category: string): ActionView[] {
  const term = query.trim().toLowerCase();
  return actions.filter((action) => (!category || action.category === category) &&
    (!term || [action.label, action.category, action.consequence_preview ?? "", ...Object.values(action.parameter_display_values)].join(" ").toLowerCase().includes(term)));
}

const messages: Record<string, string> = {
  storage_unavailable: "Tab storage is unavailable. Enable it before sending a game action.",
  storage_invalid: "The tab's recovery record could not be read. The server will check for your active game.",
  storage_write_failed: "The recovery record could not be saved. No new action can be sent until tab storage works.",
  network: "The local server could not be reached. Check that it is still running.",
  unavailable: "The local server is temporarily unavailable.",
  busy: "The local server is busy. Try again shortly.",
  unauthorized: "The connection needs to be renewed. Reconnect to check the local server.",
  invalid_input: "Check the character choices and seed, then try again.",
  invalid_save: "This save could not be accepted. Choose an unmodified save from this exact game build.",
  invalid_action: "That choice is no longer available. The current scene will be refreshed.",
  stale_state: "The game has moved on. The current scene will be refreshed.",
  resource_limit: "The local server's capacity was reached. Download your current save before restarting it.",
  conflict: "This request conflicts with an earlier request. Reconnect before continuing.",
  idempotency_conflict: "This request conflicts with an earlier request. Reconnect before continuing.",
  retry_pending_request: "Confirm the pending request before choosing another action.",
  server_restarted: "The local server restarted. Its earlier in-memory game is no longer available.",
  different_active_session: "Another tab has opened a different game. Confirm before joining it.",
  session_unknown: "This server does not recognize the previous session.",
  restart_acknowledged: "Checking the local server for an active game…",
  invalid_catalog: "The action catalog could not be confirmed. Reconnect before choosing an action.",
  catalog_incomplete: "The complete action catalog has not arrived. Reconnect before choosing an action.",
  invalid_close_response: "Session closure was not confirmed. Retry the pending request.",
};

export function publicMessage(code: string): string {
  return messages[code] ?? "The player could not confirm this operation. Reconnect to recover the session.";
}
