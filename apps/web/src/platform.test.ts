import { describe, expect, it } from "vitest";
import { dialogFilters, isTauri } from "./platform";

describe("platform", () => {
  it("detects the Tauri shell", () => {
    expect(isTauri({})).toBe(false);
    expect(isTauri({ __TAURI_INTERNALS__: {} })).toBe(true);
  });
  it("maps accept strings to native dialog filters", () => {
    expect(dialogFilters(".json,.lottie")).toEqual([{ name: "Supported files", extensions: ["json", "lottie"] }]);
    expect(dialogFilters("")).toEqual([]);
  });
});
