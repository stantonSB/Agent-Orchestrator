import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SessionInfo, SessionMode, SessionStatus, SubagentStatus } from "../types/session";
import type { ToastData } from "../components/Toast/Toast";

interface SessionState {
  sessions: Map<string, SessionInfo>;
  activeSessionId: string | null;
  lastUsedDirectory: string | null;
  subagents: Map<string, SubagentStatus[]>;

  // Mutations
  addSession: (session: SessionInfo) => void;
  setLastUsedDirectory: (dir: string) => void;
  removeSession: (id: string) => void;
  updateSessionStatus: (id: string, status: SessionStatus) => void;
  setActiveSession: (id: string) => void;
  updateSubagents: (sessionId: string, subagents: SubagentStatus[]) => void;

  // Session management
  dismissSession: (id: string) => void;

  // Tauri IPC actions
  createSession: (name: string, cwd: string, sessionMode?: SessionMode, pullLatest?: boolean, isGitRepo?: boolean) => Promise<void>;
  closeSession: (id: string) => Promise<void>;
  renameSession: (id: string, name: string) => Promise<void>;

  // Toast notifications
  toasts: ToastData[];
  addToast: (message: string, type: ToastData["type"]) => void;
  dismissToast: (id: string) => void;

  // Event listener management
  setupEventListeners: (sessionId: string) => void;
}

const eventCleanups = new Map<string, UnlistenFn[]>();
const subagentCleanupTimers = new Map<string, ReturnType<typeof setTimeout>>();

function scheduleSubagentCleanup(sessionId: string) {
  const existing = subagentCleanupTimers.get(sessionId);
  if (existing) clearTimeout(existing);

  const timer = setTimeout(() => {
    subagentCleanupTimers.delete(sessionId);
    const state = useSessionStore.getState();
    const list = state.subagents.get(sessionId);
    if (!list) return;

    if (state.activeSessionId !== sessionId) return;

    const remaining = list.filter((a) => a.status !== "finished");
    state.updateSubagents(sessionId, remaining);
  }, 30_000);

  subagentCleanupTimers.set(sessionId, timer);
}

function cancelSubagentCleanup(sessionId: string) {
  const timer = subagentCleanupTimers.get(sessionId);
  if (timer) {
    clearTimeout(timer);
    subagentCleanupTimers.delete(sessionId);
  }
}

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,
  lastUsedDirectory: null,
  subagents: new Map(),
  toasts: [],

  addToast: (message, type) => {
    const id = crypto.randomUUID();
    set((state) => ({
      toasts: [...state.toasts, { id, message, type }],
    }));
  },

  dismissToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },

  setLastUsedDirectory: (dir) => set({ lastUsedDirectory: dir }),

  addSession: (session) =>
    set((state) => {
      const next = new Map(state.sessions);
      next.set(session.id, session);
      return { sessions: next };
    }),

  removeSession: (id) => {
    cancelSubagentCleanup(id);
    set((state) => {
      const next = new Map(state.sessions);
      next.delete(id);
      const nextSubagents = new Map(state.subagents);
      nextSubagents.delete(id);

      const cleanups = eventCleanups.get(id);
      if (cleanups) {
        cleanups.forEach((unlisten) => unlisten());
        eventCleanups.delete(id);
      }

      let activeSessionId = state.activeSessionId;
      if (activeSessionId === id) {
        const remaining = Array.from(next.keys());
        activeSessionId = remaining.length > 0 ? remaining[0] : null;
      }

      return {
        sessions: next,
        subagents: nextSubagents,
        activeSessionId,
      };
    });
  },

  updateSubagents: (sessionId, subagentList) =>
    set((state) => {
      const next = new Map(state.subagents);
      if (subagentList.length === 0) {
        next.delete(sessionId);
        cancelSubagentCleanup(sessionId);
      } else {
        next.set(sessionId, subagentList);
        if (state.activeSessionId === sessionId && subagentList.some((a) => a.status === "finished")) {
          scheduleSubagentCleanup(sessionId);
        }
      }
      return { subagents: next };
    }),

  updateSessionStatus: (id, status) =>
    set((state) => {
      const session = state.sessions.get(id);
      if (!session) return state;

      const next = new Map(state.sessions);
      next.set(id, { ...session, status });
      return { sessions: next };
    }),

  setActiveSession: (id) => {
    const prevActive = useSessionStore.getState().activeSessionId;
    if (prevActive) cancelSubagentCleanup(prevActive);

    set({ activeSessionId: id });

    const subagents = useSessionStore.getState().subagents.get(id);
    if (subagents?.some((a) => a.status === "finished")) {
      scheduleSubagentCleanup(id);
    }
  },

  dismissSession: (id) => {
    cancelSubagentCleanup(id);
    set((state) => {
      const next = new Map(state.sessions);
      next.delete(id);
      const nextSubagents = new Map(state.subagents);
      nextSubagents.delete(id);
      const cleanups = eventCleanups.get(id);
      if (cleanups) {
        cleanups.forEach((unlisten) => unlisten());
        eventCleanups.delete(id);
      }
      let activeSessionId = state.activeSessionId;
      if (activeSessionId === id) {
        const remaining = Array.from(next.keys());
        activeSessionId = remaining.length > 0 ? remaining[0] : null;
      }
      return { sessions: next, subagents: nextSubagents, activeSessionId };
    });
  },

  createSession: async (name, cwd, sessionMode = "claude", pullLatest = false, isGitRepo = true) => {
    if (pullLatest) {
      await invoke("git_pull_main", { cwd });
    }

    let id: string;
    let session: SessionInfo;

    if (sessionMode === "terminal") {
      id = await invoke<string>("create_session", {
        name,
        cwd,
        sessionType: "terminal",
      });
      session = {
        id,
        name,
        status: "terminal",
        createdAt: Date.now(),
        cwd,
        sessionType: "terminal",
        isGitRepo: false,
      };
    } else {
      const args: string[] = [];
      if (sessionMode === "claude-auto") {
        args.push("--auto");
      } else if (sessionMode === "claude-skip") {
        args.push("--dangerously-skip-permissions");
      } else if (sessionMode === "claude-plan") {
        args.push("--plan");
      }
      if (isGitRepo) {
        args.push("--worktree");
      }
      id = await invoke<string>("create_session", {
        name,
        cwd,
        command: "claude",
        args,
        sessionType: "claude",
      });
      session = {
        id,
        name,
        status: "starting",
        createdAt: Date.now(),
        cwd,
        sessionType: "claude",
        isGitRepo,
      };
    }

    get().addSession(session);
    get().setActiveSession(id);
    get().setupEventListeners(id);
    set({ lastUsedDirectory: cwd });

    if (sessionMode !== "terminal") {
      try {
        const currentStatus = await invoke<string | null>("get_session_status", { id });
        if (currentStatus && currentStatus !== "starting") {
          get().updateSessionStatus(id, currentStatus as SessionStatus);
        }
      } catch {
        // Session may have already been removed
      }
    }
  },

  closeSession: async (id) => {
    await invoke("close_session", { id });
    get().removeSession(id);
  },

  renameSession: async (id, name) => {
    await invoke("rename_session", { id, name });
    set((state) => {
      const session = state.sessions.get(id);
      if (!session) return state;
      const next = new Map(state.sessions);
      next.set(id, { ...session, name });
      return { sessions: next };
    });
  },

  setupEventListeners: (sessionId) => {
    let cancelled = false;
    const cleanups: Promise<UnlistenFn>[] = [];
    const session = get().sessions.get(sessionId);

    // Only listen for status events on Claude sessions
    if (session?.sessionType !== "terminal") {
      cleanups.push(
        listen<{ status: SessionStatus }>(`session-status-${sessionId}`, (event) => {
          get().updateSessionStatus(sessionId, event.payload.status);
        })
      );
    }

    cleanups.push(
      listen<{ code: number | null }>(`session-exit-${sessionId}`, (event) => {
        const session = get().sessions.get(sessionId);
        if (session?.sessionType === "terminal") {
          // Terminal sessions only show error on non-zero exit
          if (event.payload.code !== null && event.payload.code !== 0) {
            get().updateSessionStatus(sessionId, "error");
          }
        } else {
          const status: SessionStatus = event.payload.code === 0 ? "finished" : "error";
          get().updateSessionStatus(sessionId, status);
        }
      })
    );

    // Only listen for subagent events on Claude sessions
    if (session?.sessionType !== "terminal") {
      cleanups.push(
        listen<SubagentStatus[]>(`session-subagents-${sessionId}`, (event) => {
          get().updateSubagents(sessionId, event.payload);
        })
      );
    }

    eventCleanups.set(sessionId, [() => { cancelled = true; }]);

    Promise.all(cleanups).then((unlistenFns) => {
      if (cancelled) {
        unlistenFns.forEach((unlisten) => unlisten());
        return;
      }
      eventCleanups.set(sessionId, unlistenFns);
    });
  },
}));
