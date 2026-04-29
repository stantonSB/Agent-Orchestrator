import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { useSessionStore } from "./sessionStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}));

describe("sessionStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSessionStore.setState({
      sessions: new Map(),
      activeSessionId: null,
      lastUsedDirectory: null,
      subagents: new Map(),
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
        cwd: "/test/path",
        sessionType: "claude",
        isGitRepo: true,
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
        cwd: "/test/path",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.removeSession("abc-123");

      const { sessions } = useSessionStore.getState();
      expect(sessions.size).toBe(0);
    });

    it("clears activeSessionId if the removed session was the only one", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "abc-123",
        name: "Test",
        status: "idle",
        createdAt: Date.now(),
        cwd: "/test/path",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.setActiveSession("abc-123");
      store.removeSession("abc-123");

      const { activeSessionId } = useSessionStore.getState();
      expect(activeSessionId).toBeNull();
    });

    it("auto-selects next session when active session is removed", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "First",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test/path",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.addSession({
        id: "session-2",
        name: "Second",
        status: "idle",
        createdAt: Date.now(),
        cwd: "/test/path",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.setActiveSession("session-1");
      store.removeSession("session-1");

      const { activeSessionId } = useSessionStore.getState();
      expect(activeSessionId).toBe("session-2");
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
        cwd: "/test/path",
        sessionType: "claude",
        isGitRepo: true,
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
        cwd: "/test/path",
        sessionType: "claude",
        isGitRepo: true,
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
      await store.createSession("My Session", "/path/to/project", "claude-skip");

      expect(invoke).toHaveBeenCalledWith("create_session", {
        name: "My Session",
        cwd: "/path/to/project",
        command: "claude",
        args: ["--dangerously-skip-permissions", "--worktree"],
        sessionType: "claude",
      });

      const { sessions, activeSessionId } = useSessionStore.getState();
      expect(sessions.has("new-id-456")).toBe(true);
      expect(sessions.get("new-id-456")?.name).toBe("My Session");
      expect(sessions.get("new-id-456")?.status).toBe("starting");
      expect(sessions.get("new-id-456")?.cwd).toBe("/path/to/project");
      expect(activeSessionId).toBe("new-id-456");
    });

    it("calls git_pull_main before create_session when pullLatest is true", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const callOrder: string[] = [];
      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        callOrder.push(cmd);
        if (cmd === "create_session") return "pull-id-789";
        return undefined;
      });

      const store = useSessionStore.getState();
      await store.createSession("Pull Session", "/path/to/project", "claude-skip", true);

      expect(callOrder).toEqual(["git_pull_main", "create_session", "get_session_status"]);
      expect(invoke).toHaveBeenCalledWith("git_pull_main", {
        cwd: "/path/to/project",
      });
    });

    it("does NOT call git_pull_main when pullLatest is false", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("no-pull-id");

      const store = useSessionStore.getState();
      await store.createSession("No Pull", "/path/to/project", "claude-skip", false);

      expect(invoke).not.toHaveBeenCalledWith("git_pull_main", expect.anything());
      expect(invoke).toHaveBeenCalledWith("create_session", expect.anything());
    });

    it("creates a terminal session when mode is 'terminal'", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("terminal-id-1");

      const store = useSessionStore.getState();
      await store.createSession("My Terminal", "/path/to/project", "terminal");

      expect(invoke).toHaveBeenCalledWith("create_session", {
        name: "My Terminal",
        cwd: "/path/to/project",
        sessionType: "terminal",
      });

      const session = useSessionStore.getState().sessions.get("terminal-id-1");
      expect(session?.sessionType).toBe("terminal");
      expect(session?.status).toBe("terminal");
    });

    it("does NOT create session when git_pull_main fails", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockRejectedValueOnce(new Error("git pull failed"));

      const store = useSessionStore.getState();
      await expect(
        store.createSession("Fail Pull", "/path/to/project", "claude-skip", true)
      ).rejects.toThrow("git pull failed");

      expect(invoke).not.toHaveBeenCalledWith("create_session", expect.anything());
      expect(useSessionStore.getState().sessions.size).toBe(0);
    });

    it("omits --worktree when isGitRepo is false", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("non-git-id");

      const store = useSessionStore.getState();
      await store.createSession("Non-Git Session", "/path/to/non-git", "claude-skip", false, false);

      expect(invoke).toHaveBeenCalledWith("create_session", {
        name: "Non-Git Session",
        cwd: "/path/to/non-git",
        command: "claude",
        args: ["--dangerously-skip-permissions"],
        sessionType: "claude",
      });

      const session = useSessionStore.getState().sessions.get("non-git-id");
      expect(session?.isGitRepo).toBe(false);
    });

    it("includes --worktree when isGitRepo is true", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("git-id");

      const store = useSessionStore.getState();
      await store.createSession("Git Session", "/path/to/git-repo", "claude-skip", false, true);

      expect(invoke).toHaveBeenCalledWith("create_session", {
        name: "Git Session",
        cwd: "/path/to/git-repo",
        command: "claude",
        args: ["--dangerously-skip-permissions", "--worktree"],
        sessionType: "claude",
      });

      const session = useSessionStore.getState().sessions.get("git-id");
      expect(session?.isGitRepo).toBe(true);
    });

    it("creates a claude session with no extra args when mode is 'claude'", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("claude-default-id");

      const store = useSessionStore.getState();
      await store.createSession("Default Claude", "/path/to/project", "claude");

      expect(invoke).toHaveBeenCalledWith("create_session", {
        name: "Default Claude",
        cwd: "/path/to/project",
        command: "claude",
        args: ["--worktree"],
        sessionType: "claude",
      });
    });

    it("creates a claude session with --plan when mode is 'claude-plan'", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce("plan-id");

      const store = useSessionStore.getState();
      await store.createSession("Plan Session", "/path/to/project", "claude-plan");

      expect(invoke).toHaveBeenCalledWith("create_session", {
        name: "Plan Session",
        cwd: "/path/to/project",
        command: "claude",
        args: ["--plan", "--worktree"],
        sessionType: "claude",
      });
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
        cwd: "/test/path",
        sessionType: "claude",
        isGitRepo: true,
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

  describe("subagents", () => {
    it("initializes with empty subagents map", () => {
      const { subagents } = useSessionStore.getState();
      expect(subagents.size).toBe(0);
    });

    it("updates subagents for a session", () => {
      const store = useSessionStore.getState();
      store.updateSubagents("session-1", [
        { id: "cc-child-1", index: 1, status: "working", name: null, created_at: 1000 },
        { id: "cc-child-2", index: 2, status: "idle", name: "Exploring", created_at: 2000 },
      ]);

      const { subagents } = useSessionStore.getState();
      expect(subagents.get("session-1")?.length).toBe(2);
      expect(subagents.get("session-1")?.[0].status).toBe("working");
      expect(subagents.get("session-1")?.[1].name).toBe("Exploring");
    });

    it("clears subagents when session is removed", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.updateSubagents("session-1", [
        { id: "cc-child-1", index: 1, status: "working", name: null, created_at: 1000 },
      ]);
      store.removeSession("session-1");

      const { subagents } = useSessionStore.getState();
      expect(subagents.has("session-1")).toBe(false);
    });

    it("clears subagents when session is dismissed", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "finished",
        createdAt: Date.now(),
        cwd: "/test",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.updateSubagents("session-1", [
        { id: "cc-child-1", index: 1, status: "finished", name: null, created_at: 1000 },
      ]);
      store.dismissSession("session-1");

      const { subagents } = useSessionStore.getState();
      expect(subagents.has("session-1")).toBe(false);
    });

    it("registers listener for session-subagents event", async () => {
      const { listen } = await import("@tauri-apps/api/event");
      const store = useSessionStore.getState();
      store.setupEventListeners("test-session");

      expect(listen).toHaveBeenCalledWith(
        "session-subagents-test-session",
        expect.any(Function)
      );
    });
  });

  describe("subagent cleanup", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("removes finished subagents 30s after parent becomes active", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.setActiveSession("session-1");
      store.updateSubagents("session-1", [
        { id: "child-1", index: 1, status: "finished", name: null, created_at: 1000 },
        { id: "child-2", index: 2, status: "working", name: null, created_at: 2000 },
      ]);

      vi.advanceTimersByTime(30_000);

      const { subagents } = useSessionStore.getState();
      const list = subagents.get("session-1");
      expect(list?.length).toBe(1);
      expect(list?.[0].id).toBe("child-2");
    });

    it("does not remove finished subagents if parent is not active", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.addSession({
        id: "session-2",
        name: "Other",
        status: "idle",
        createdAt: Date.now(),
        cwd: "/other",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.setActiveSession("session-2");
      store.updateSubagents("session-1", [
        { id: "child-1", index: 1, status: "finished", name: null, created_at: 1000 },
      ]);

      vi.advanceTimersByTime(60_000);

      const { subagents } = useSessionStore.getState();
      expect(subagents.get("session-1")?.length).toBe(1);
    });

    it("cancels timer when switching away from parent session", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.addSession({
        id: "session-2",
        name: "Other",
        status: "idle",
        createdAt: Date.now(),
        cwd: "/other",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.setActiveSession("session-1");
      store.updateSubagents("session-1", [
        { id: "child-1", index: 1, status: "finished", name: null, created_at: 1000 },
      ]);

      store.setActiveSession("session-2");
      vi.advanceTimersByTime(60_000);

      const { subagents } = useSessionStore.getState();
      expect(subagents.get("session-1")?.length).toBe(1);
    });

    it("starts timer when switching to parent session with finished subagents", () => {
      const store = useSessionStore.getState();
      store.addSession({
        id: "session-1",
        name: "Test",
        status: "working",
        createdAt: Date.now(),
        cwd: "/test",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.addSession({
        id: "session-2",
        name: "Other",
        status: "idle",
        createdAt: Date.now(),
        cwd: "/other",
        sessionType: "claude",
        isGitRepo: true,
      });
      store.setActiveSession("session-2");
      store.updateSubagents("session-1", [
        { id: "child-1", index: 1, status: "finished", name: null, created_at: 1000 },
      ]);

      store.setActiveSession("session-1");
      vi.advanceTimersByTime(30_000);

      const { subagents } = useSessionStore.getState();
      expect(subagents.has("session-1")).toBe(false);
    });
  });
});
