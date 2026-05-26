import { describe, it, expect, beforeEach, vi } from "vitest";
import { getDefaultSessionName, getNextSessionNumber, _resetCounterForTesting } from "./NewSessionModal";

const store: Record<string, string> = {};
const mockLocalStorage = {
  getItem: vi.fn((key: string) => store[key] ?? null),
  setItem: vi.fn((key: string, value: string) => { store[key] = value; }),
  removeItem: vi.fn((key: string) => { delete store[key]; }),
  clear: vi.fn(() => { for (const key of Object.keys(store)) delete store[key]; }),
  length: 0,
  key: vi.fn(() => null),
};

vi.stubGlobal("localStorage", mockLocalStorage);

describe("getDefaultSessionName", () => {
  beforeEach(() => {
    _resetCounterForTesting();
    mockLocalStorage.clear();
  });

  it("returns 'Session 1' for first session with no custom pattern", () => {
    expect(getDefaultSessionName(1)).toBe("Session 1");
  });

  it("returns 'Session 5' for n=5", () => {
    expect(getDefaultSessionName(5)).toBe("Session 5");
  });

  it("uses custom pattern from localStorage", () => {
    mockLocalStorage.setItem("ao-default-session-name", "Agent {n}");
    expect(getDefaultSessionName(3)).toBe("Agent 3");
  });

  it("uses pattern as-is when no {n} token", () => {
    mockLocalStorage.setItem("ao-default-session-name", "My Task");
    expect(getDefaultSessionName(7)).toBe("My Task");
  });

  it("falls back to default when localStorage is empty string", () => {
    mockLocalStorage.setItem("ao-default-session-name", "");
    expect(getDefaultSessionName(2)).toBe("Session 2");
  });
});

describe("getNextSessionNumber", () => {
  beforeEach(() => {
    _resetCounterForTesting();
  });

  it("starts at 1", () => {
    expect(getNextSessionNumber()).toBe(1);
  });

  it("increments on each call", () => {
    expect(getNextSessionNumber()).toBe(1);
    expect(getNextSessionNumber()).toBe(2);
    expect(getNextSessionNumber()).toBe(3);
  });
});
