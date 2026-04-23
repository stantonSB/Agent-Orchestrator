import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SessionInfo, SessionStatus } from "../types/session";
import type { ToastData } from "../components/Toast/Toast";

interface SessionState {
  sessions: Map<string, SessionInfo>;
  activeSessionId: string | null;
  lastUsedDirectory: string | null;

  // Mutations
  addSession: (session: SessionInfo) => void;
  setLastUsedDirectory: (dir: string) => void;
  removeSession: (id: string) => void;
  updateSessionStatus: (id: string, status: SessionStatus) => void;
  setActiveSession: (id: string) => void;

  // Session management
  dismissSession: (id: string) => void;

  // Tauri IPC actions
  createSession: (name: string, cwd: string, skipPermissions?: boolean) => Promise<void>;
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

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,
  lastUsedDirectory: null,
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

  removeSession: (id) =>
    set((state) => {
      const next = new Map(state.sessions);
      next.delete(id);

      const cleanups = eventCleanups.get(id);
      if (cleanups) {
        cleanups.forEach((unlisten) => unlisten());
        eventCleanups.delete(id);
      }

      return {
        sessions: next,
        activeSessionId: state.activeSessionId === id ? null : state.activeSessionId,
      };
    }),

  updateSessionStatus: (id, status) =>
    set((state) => {
      const session = state.sessions.get(id);
      if (!session) return state;

      const next = new Map(state.sessions);
      next.set(id, { ...session, status });
      return { sessions: next };
    }),

  setActiveSession: (id) => set({ activeSessionId: id }),

  dismissSession: (id) =>
    set((state) => {
      const next = new Map(state.sessions);
      next.delete(id);
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
      return { sessions: next, activeSessionId };
    }),

  createSession: async (name, cwd, skipPermissions = true) => {
    const args = ["--worktree"];
    if (skipPermissions) {
      args.unshift("--dangerously-skip-permissions");
    }
    const id = await invoke<string>("create_session", {
      name,
      cwd,
      command: "claude",
      args,
    });
    const session: SessionInfo = {
      id,
      name,
      status: "starting",
      createdAt: Date.now(),
      cwd,
    };
    get().addSession(session);
    get().setActiveSession(id);
    get().setupEventListeners(id);
    set({ lastUsedDirectory: cwd });
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

    cleanups.push(
      listen<{ status: SessionStatus }>(`session-status-${sessionId}`, (event) => {
        get().updateSessionStatus(sessionId, event.payload.status);
      })
    );

    cleanups.push(
      listen<{ exitCode: number }>(`session-exit-${sessionId}`, (event) => {
        const status: SessionStatus = event.payload.exitCode === 0 ? "finished" : "error";
        get().updateSessionStatus(sessionId, status);
      })
    );

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
