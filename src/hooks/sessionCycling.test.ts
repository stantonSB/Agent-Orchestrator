import { describe, it, expect } from "vitest";
import { getCycledIndex } from "./useGlobalKeybindings";

describe("getCycledIndex", () => {
  const ids = ["a", "b", "c", "d"];

  it("returns null with 0 sessions", () => {
    expect(getCycledIndex("next", "a", [])).toBeNull();
  });

  it("returns null with 1 session", () => {
    expect(getCycledIndex("next", "a", ["a"])).toBeNull();
  });

  it("returns null with no active session", () => {
    expect(getCycledIndex("next", null, ids)).toBeNull();
  });

  it("cycles next from middle", () => {
    expect(getCycledIndex("next", "b", ids)).toBe(2);
  });

  it("wraps next from last to first", () => {
    expect(getCycledIndex("next", "d", ids)).toBe(0);
  });

  it("cycles prev from middle", () => {
    expect(getCycledIndex("prev", "c", ids)).toBe(1);
  });

  it("wraps prev from first to last", () => {
    expect(getCycledIndex("prev", "a", ids)).toBe(3);
  });

  it("returns null for unknown activeId", () => {
    expect(getCycledIndex("next", "unknown", ids)).toBeNull();
  });
});
