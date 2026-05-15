import { describe, it, expect, vi, beforeEach } from "vitest";
import { useSessionStore } from "../stores/sessionStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { invoke } from "@tauri-apps/api/core";
const mockInvoke = vi.mocked(invoke);

describe("Session persistence", () => {
  beforeEach(() => {
    useSessionStore.setState({
      sessions: new Map(),
      activeSessionId: null,
    });
    vi.clearAllMocks();
  });

  describe("loadPersistedSessions", () => {
    it("loads persisted sessions with exited status", async () => {
      mockInvoke.mockResolvedValueOnce([
        {
          id: "abc",
          name: "test-session",
          cwd: "/tmp",
          session_type: "claude",
          is_git_repo: true,
          created_at_epoch_ms: 1715000000000,
          status_at_close: "working",
        },
      ]);

      await useSessionStore.getState().loadPersistedSessions();

      const session = useSessionStore.getState().sessions.get("abc");
      expect(session).toBeDefined();
      expect(session!.status).toBe("exited");
      expect(session!.persisted).toBe(true);
      expect(session!.name).toBe("test-session");
    });

    it("skips persisted sessions that conflict with live sessions", async () => {
      useSessionStore.getState().addSession({
        id: "abc",
        name: "live-session",
        cwd: "/tmp",
        status: "working",
        createdAt: 1715000000000,
        sessionType: "claude",
        isGitRepo: true,
      });

      mockInvoke.mockResolvedValueOnce([
        {
          id: "abc",
          name: "persisted-session",
          cwd: "/tmp",
          session_type: "claude",
          is_git_repo: true,
          created_at_epoch_ms: 1715000000000,
          status_at_close: "finished",
        },
      ]);

      await useSessionStore.getState().loadPersistedSessions();

      const session = useSessionStore.getState().sessions.get("abc");
      expect(session!.name).toBe("live-session");
      expect(session!.persisted).toBeUndefined();
    });
  });

  describe("closeSession for persisted sessions", () => {
    it("calls delete_persisted_session for persisted sessions", async () => {
      mockInvoke.mockResolvedValue(undefined);

      useSessionStore.getState().addSession({
        id: "abc",
        name: "old-session",
        cwd: "/tmp",
        status: "exited",
        createdAt: 1715000000000,
        sessionType: "claude",
        isGitRepo: true,
        persisted: true,
      });

      await useSessionStore.getState().closeSession("abc");

      expect(mockInvoke).toHaveBeenCalledWith("delete_persisted_session", {
        sessionId: "abc",
      });
      expect(useSessionStore.getState().sessions.has("abc")).toBe(false);
    });
  });

  describe("loadScrollback", () => {
    it("loads scrollback text for a session", async () => {
      useSessionStore.getState().addSession({
        id: "abc",
        name: "test",
        cwd: "/tmp",
        status: "exited",
        createdAt: 1715000000000,
        sessionType: "claude",
        isGitRepo: true,
        persisted: true,
      });

      mockInvoke.mockResolvedValueOnce("line1\nline2\nline3");

      await useSessionStore.getState().loadScrollback("abc");

      const session = useSessionStore.getState().sessions.get("abc");
      expect(session!.scrollbackText).toBe("line1\nline2\nline3");
    });

    it("does not reload if scrollback already loaded", async () => {
      useSessionStore.getState().addSession({
        id: "abc",
        name: "test",
        cwd: "/tmp",
        status: "exited",
        createdAt: 1715000000000,
        sessionType: "claude",
        isGitRepo: true,
        persisted: true,
        scrollbackText: "already loaded",
      });

      await useSessionStore.getState().loadScrollback("abc");

      expect(mockInvoke).not.toHaveBeenCalled();
    });
  });
});
