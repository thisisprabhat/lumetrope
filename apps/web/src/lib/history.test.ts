import { describe, expect, it } from "vitest";
import { changeSummary, relativeTime } from "./history";

describe("history helpers", () => {
  it("formats relative times", () => {
    const now = 1_000_000_000;
    expect(relativeTime(now - 5_000, now)).toBe("just now");
    expect(relativeTime(now - 90_000, now)).toBe("1 min ago");
    expect(relativeTime(now - 3 * 3_600_000, now)).toBe("3 h ago");
    expect(relativeTime(now - 2 * 86_400_000, now)).toBe("2 days ago");
  });

  it("summarises a diff", () => {
    expect(changeSummary({ layers_added: ["A", "B"], layers_removed: [], layers_changed: ["C"], settings_changed: ["w"], state_machines_changed: [], themes_changed: [], segments_changed: false })).toBe("+2 layers · 1 edited · scene settings");
    expect(changeSummary({ layers_added: [], layers_removed: [], layers_changed: [], settings_changed: [], state_machines_changed: [], themes_changed: [], segments_changed: false })).toBe("no changes");
    expect(changeSummary({ layers_added: [], layers_removed: ["X"], layers_changed: [], settings_changed: [], state_machines_changed: ["m"], themes_changed: ["t"], segments_changed: true })).toBe("−1 layer · state machines · themes · segments");
  });
});
