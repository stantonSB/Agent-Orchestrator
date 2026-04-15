import { describe, it, expect, beforeEach, vi } from "vitest";
import { useSessionStore } from "./sessionStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}));

describe("sessionStore", () => {
  beforeEach(() => {
    useSessionStore.setState({
      sessions: new Map(),
      activeSessionId: null,
      lastUsedDirectory: null,
    });
  });

  describe("addSession", () => {
    it("adds a session to the map", () => {
      const { addSession } = useSessionStore.getState();
      addSession({
        id: "abc-123",
        name: "Test Session",
        status: "starting",
        createdAt: Date.now(),
      });

      const { sessions } = useSessionStore.getState();
      expect(sessions.size).toBe(1);
      expect(sessions.get("abc-123")?.name).toBe("Test Session");
    });
  });

  describe("removeSession", () => {
    it("removes a session from the map", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
      });
      store.removeSession("abc-123");

      const { sessions } = useSessionStore.getState();
      expect(sessions.size).toBe(0);
    });

    it("clears activeSessionId if the removed session was active", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
      });
      store.setActiveSession("abc-123");
      store.removeSession("abc-123");

      const { activeSessionId } = useSessionStore.getState();
      expect(activeSessionId).toBeNull();
    });
  });

  describe("updateSessionStatus", () => {
    it("updates the status of an existing session", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "starting",
        createdAt: Date.now(),
      });
      store.updateSessionStatus("abc-123", "working");

      const session = useSessionStore.getState().sessions.get("abc-123");
      expect(session?.status).toBe("working");
    });

    it("no-ops for a non-existent session", () => {
      const store = useSessionStore.getState();
      store.updateSessionStatus("nonexistent", "working");
      expect(useSessionStore.getState().sessions.size).toBe(0);
    });
  });

  describe("setActiveSession", () => {
    it("sets the active session id", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
      });
      store.setActiveSession("abc-123");

      expect(useSessionStore.getState().activeSessionId).toBe("abc-123");
    });
  });

  describe("createSession", () => {
    it("calls Tauri invoke and adds the session", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("new-id-456");

      const store = useSessionStore.getState();
      await store.createSession("My Session", "/path/to/project");

      expect(invoke).toHaveBeenCalledWith("create_session", {
        name: "My Session",
        cwd: "/path/to/project",
      });

      const { sessions, activeSessionId } = useSessionStore.getState();
      expect(sessions.has("new-id-456")).toBe(true);
      expect(sessions.get("new-id-456")?.name).toBe("My Session");
      expect(sessions.get("new-id-456")?.status).toBe("starting");
      expect(activeSessionId).toBe("new-id-456");
    });
  });

  describe("closeSession", () => {
    it("calls Tauri invoke and removes the session", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
      });

      await store.closeSession("abc-123");

      expect(invoke).toHaveBeenCalledWith("close_session", { id: "abc-123" });
      expect(useSessionStore.getState().sessions.has("abc-123")).toBe(false);
    });
  });

  describe("createSession — lastUsedDirectory", () => {
    it("sets lastUsedDirectory after creating a session", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("dir-test-id");

      const store = useSessionStore.getState();
      await store.createSession("Dir Test", "/projects/my-app");

      const { lastUsedDirectory } = useSessionStore.getState();
      expect(lastUsedDirectory).toBe("/projects/my-app");
    });
  });

  describe("setupEventListeners", () => {
    it("registers listeners for status and exit events", async () => {
      const { listen } = await import("@tauri-apps/api/event");

      const store = useSessionStore.getState();
      store.setupEventListeners("test-session");

      expect(listen).toHaveBeenCalledWith(
        "session-status-test-session",
        expect.any(Function)
      );
      expect(listen).toHaveBeenCalledWith(
        "session-exit-test-session",
        expect.any(Function)
      );
    });
  });
});
