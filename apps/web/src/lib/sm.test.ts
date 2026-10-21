import { describe, expect, it } from "vitest";
import { actionSummary, autoLayout, guardSummary, toGuardArgs } from "./sm";

describe("state machine helpers", () => {
  it("auto-lays out unplaced states and keeps saved positions", () => {
    const pos = autoLayout(["A", "B", "C", "Global"], { B: [500, 40] }, "Global");
    expect(pos.B).toEqual([500, 40]);
    expect(pos.A).not.toEqual(pos.C);
    expect(pos.Global[1]).toBeGreaterThan(pos.A[1]);
  });

  it("flows left to right from the initial state", () => {
    const pos = autoLayout(["D", "A", "B", "C"], {}, undefined, "A", [["A", "B"], ["A", "C"], ["B", "D"]]);
    expect(pos.A[0]).toBeLessThan(pos.B[0]);
    expect(pos.B[0]).toBe(pos.C[0]);
    expect(pos.B[1]).not.toBe(pos.C[1]);
    expect(pos.D[0]).toBeGreaterThan(pos.B[0]);
  });

  it("summarises guards", () => {
    expect(guardSummary([])).toBe("always");
    expect(guardSummary([{ type: "Event", inputName: "tap" }])).toBe("on tap");
    expect(guardSummary([{ type: "Numeric", inputName: "n", conditionType: "GreaterThan", compareTo: 3 }, { type: "Boolean", inputName: "on", conditionType: "Equal", compareTo: true }])).toBe("n > 3 & on = true");
  });

  it("summarises actions", () => {
    expect(actionSummary({ type: "Toggle", inputName: "on" })).toBe("toggle on");
    expect(actionSummary({ type: "SetNumeric", inputName: "n", value: 4 })).toBe("n = 4");
    expect(actionSummary({ type: "OpenUrl", url: "https://x.y" })).toBe("open https://x.y");
    expect(actionSummary({ type: "Fire", inputName: "go" })).toBe("fire go");
  });

  it("converts spec guards back to tool args", () => {
    expect(toGuardArgs([{ type: "Numeric", inputName: "n", conditionType: "LessThanOrEqual", compareTo: 2 }])).toEqual([{ input: "n", condition: "less_than_or_equal", value: 2 }]);
    expect(toGuardArgs([{ type: "Event", inputName: "go", compareTo: "false", conditionType: "Equal" }])).toEqual([{ input: "go" }]);
  });
});
