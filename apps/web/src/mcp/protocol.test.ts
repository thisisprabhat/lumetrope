import { describe, expect, it } from "vitest";
import { backoffMs, parseRequest, response } from "./protocol";

describe("mcp protocol", () => {
  it("parses tool requests and rejects junk", () => {
    expect(parseRequest(JSON.stringify({ type: "tool_request", id: "1", tool: "read_scene", args: { a: 1 } }))).toEqual({ id: "1", tool: "read_scene", args: { a: 1 } });
    expect(parseRequest(JSON.stringify({ type: "tool_request", id: "2", tool: "undo" }))).toEqual({ id: "2", tool: "undo", args: {} });
    expect(parseRequest("not json")).toBeNull();
    expect(parseRequest(JSON.stringify({ type: "hello" }))).toBeNull();
    expect(parseRequest(JSON.stringify({ type: "tool_request", id: 3, tool: "x" }))).toBeNull();
  });
  it("builds responses", () => {
    expect(JSON.parse(response("1", { ok: true }))).toEqual({ type: "tool_response", id: "1", result: { ok: true } });
    expect(JSON.parse(response("1", undefined, "boom"))).toEqual({ type: "tool_response", id: "1", error: "boom" });
  });
  it("backs off exponentially with a cap", () => {
    expect(backoffMs(0)).toBe(1000);
    expect(backoffMs(3)).toBe(8000);
    expect(backoffMs(10)).toBe(10000);
  });
});
