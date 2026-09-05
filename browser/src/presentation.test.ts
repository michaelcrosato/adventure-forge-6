import { describe, expect, it } from "vitest";
import { actionLabel, filterActions, publicMessage } from "./presentation";
import type { ActionView } from "./types";

describe("player presentation", () => {
  it("distinguishes canonical actions using only public parameter names", () => {
    const action: ActionView = { action_id: "opaque", definition_id: "private-definition", label: "Travel", category: "Travel", time_cost: { minimum_ticks: "1", maximum_ticks: "1" }, consequence_preview: null, parameters: { destination: "hidden-id" }, parameter_display_values: { destination: "Lowsail Docks" } };
    expect(actionLabel(action)).toBe("Travel: Lowsail Docks");
    expect(actionLabel({ ...action, parameter_display_values: { destination: "Levee Road" } })).toBe("Travel: Levee Road");
    expect(actionLabel({ ...action, parameter_display_values: {} })).toBe("Travel");
  });
  it("never reflects unknown server errors into player prose", () => {
    expect(publicMessage("/home/private-secret")).not.toContain("private-secret");
    expect(publicMessage("invalid_save")).toContain("unmodified save");
  });
  it("searches beyond 256 actions without reordering or searching private IDs", () => {
    const actions = Array.from({ length: 513 }, (_, index): ActionView => ({ action_id: `opaque-${index}`, definition_id: "not-public-prose", label: `Choice ${index}`, category: index % 2 ? "Travel" : "Inspect", time_cost: { minimum_ticks: "1", maximum_ticks: "1" }, consequence_preview: index === 512 ? "Keep the inland trade." : null, parameters: { destination: "internal-route-id" }, parameter_display_values: { destination: `Place ${index}` } }));
    expect(filterActions(actions, "", "")).toEqual(actions);
    expect(filterActions(actions, "inland", "Inspect")).toEqual([actions[512]]);
    expect(filterActions(actions, "place 512", "")).toEqual([actions[512]]);
    expect(filterActions(actions, "opaque", "")).toEqual([]);
    expect(filterActions(actions, "internal-route-id", "")).toEqual([]);
    expect(filterActions(actions, "", "Travel")).toEqual(actions.filter((_, index) => index % 2));
  });
});
