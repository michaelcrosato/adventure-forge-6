import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import type { ActionView, ClientState, SessionView } from "./types";

function fixture(): ClientState {
  const action: ActionView = { action_id: "opaque-action", definition_id: "opaque-definition", label: "Travel", category: "Travel", time_cost: { minimum_ticks: "1", maximum_ticks: "1" }, parameters: { destination: "internal-place-id" }, parameter_display_values: { destination: "The Docks" }, consequence_preview: "Trade moves inland." };
  const view: SessionView = { revision: "18446744073709551615", observation: { build_id: "opaque-build", state_id: "opaque-state", location_id: "internal-location-id", title: "Lowsail", text: "The bell rings. The docks remain open.", result: "The bell rings.", world_time: "18446744073709551615", upcoming_events: [{ label: "Surge", remaining_ticks: "9007199254740993" }], supplies: { resources: [{ id: "coin", name: "Coin", amount: "-9223372036854775808" }], items: [{ id: "rope", name: "Rope", count: "18446744073709551615" }] }, action_count: "1", action_set_digest: "opaque-digest" }, catalog: { build_id: "opaque-build", state_id: "opaque-state", digest: "opaque-digest", total: "1", offset: "0", next_offset: null, actions: [action] } };
  return { phase: "ready", options: null, session: { session_id: "opaque-session", view }, actions: [action], catalogComplete: true, message: null, storageWarning: null, pendingLabel: null };
}

function render(state: ClientState): string {
  const client = { getSnapshot: () => state, subscribe: () => () => {}, save: vi.fn(async () => "opaque save"), start: vi.fn(async () => {}), resume: vi.fn(async () => {}), close: vi.fn(async () => {}), act: vi.fn(async () => {}), retry: vi.fn(async () => {}), refresh: vi.fn(async () => {}), acknowledgeRestart: vi.fn(async () => {}), newGame: vi.fn() };
  return renderToStaticMarkup(<App client={client} />);
}

describe("public browser rendering", () => {
  it("renders result-first text once, exact quantities, and public destinations", () => {
    const html = render(fixture());
    expect(html.match(/The bell rings\./g)).toHaveLength(1);
    expect(html).toContain("18446744073709551615");
    expect(html).toContain("9007199254740993");
    expect(html).toContain("-9223372036854775808");
    expect(html).toContain("Travel: The Docks");
    expect(html).toContain("Trade moves inland.");
    expect(html).not.toContain("internal-place-id");
    expect(html).not.toContain("opaque-action");
  });
  it("escapes malicious public prose rather than interpreting HTML", () => {
    const state = fixture();
    state.session!.view.observation.text = '<img src=x onerror="alert(1)"><script>stolen()</script>';
    const action = { ...state.actions[0]!, label: "<svg onload=stolen()>" };
    const html = render({ ...state, actions: [action] });
    expect(html).toContain("&lt;script&gt;stolen()&lt;/script&gt;");
    expect(html).toContain("&lt;svg onload=stolen()&gt;");
    expect(html).not.toContain("<script>");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<svg");
  });
  it("shows a retry control but disables mutation and save controls while uncertain", () => {
    const html = render({ ...fixture(), phase: "uncertain", pendingLabel: "Travel" });
    expect(html).toContain("Retry request");
    expect(html).toMatch(/class="action-card"[^>]*disabled=""/);
    expect(html).toMatch(/disabled=""[^>]*>Save and close/);
    expect(html).toMatch(/disabled=""[^>]*>Download save/);
  });
  it("keeps save export and a new start available after closed-session reload", () => {
    const html = render({ ...fixture(), phase: "closed", session: null, actions: [], catalogComplete: false });
    expect(html).toContain("Session closed");
    expect(html).toContain(">Download save</button>");
    expect(html).toContain(">New character</button>");
    expect(html).not.toContain("action-card");
  });
});
