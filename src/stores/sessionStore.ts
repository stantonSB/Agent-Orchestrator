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
  updateWorktreeCwd: (id: string, worktreeCwd: string) => void;
  setActiveSession: (id: string) => void;
  updateSubagents: (sessionId: string, subagents: SubagentStatus[]) => void;

  // Session management
  dismissSession: (id: string, deleteWorktree?: boolean) => void;

  // Tauri IPC actions
  createSession: (name: string, cwd: string, sessionMode?: SessionMode, pullLatest?: boolean, isGitRepo?: boolean, parentSessionId?: string) => Promise<void>;
  closeSession: (id: string, deleteWorktree?: boolean) => Promise<void>;
  renameSession: (id: string, name: string) => Promise<void>;

  // Quit confirmation
  showQuitConfirm: boolean;
  setShowQuitConfirm: (show: boolean) => void;

  // Toast notifications
  toasts: ToastData[];
  addToast: (message: string, type: ToastData["type"]) => void;
  dismissToast: (id: string) => void;

  // Persistence
  loadPersistedSessions: () => Promise<void>;
  loadScrollback: (sessionId: string) => Promise<void>;

  // Event listener management
  setupEventListeners: (sessionId: string, sessionMode?: SessionMode) => void;
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

async function tryRemoveWorktree(worktreeCwd: string | undefined | null, addToast: SessionState["addToast"]) {
  if (!worktreeCwd) return;
  try {
    const result = await invoke<{ removed: boolean; dirty: boolean; message: string }>(
      "remove_worktree",
      { worktreePath: worktreeCwd, force: false }
    );
    if (result.dirty) {
      // Worktree has uncommitted changes — force-remove with a warning toast
      addToast("Worktree had uncommitted changes — force removing", "warning");
      await invoke("remove_worktree", { worktreePath: worktreeCwd, force: true });
    }
  } catch (err) {
    addToast(
      `Failed to remove worktree: ${err instanceof Error ? err.message : String(err)}`,
      "error"
    );
  }
}

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,
  lastUsedDirectory: null,
  subagents: new Map(),
  showQuitConfirm: false,
  setShowQuitConfirm: (show) => set({ showQuitConfirm: show }),
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

  updateWorktreeCwd: (id, worktreeCwd) =>
    set((state) => {
      const session = state.sessions.get(id);
      if (!session) return state;
      const next = new Map(state.sessions);
      next.set(id, { ...session, worktreeCwd });
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

  dismissSession: (id, deleteWorktree) => {
    const session = get().sessions.get(id);
    const worktreeCwd = session?.worktreeCwd;
    if (session?.persisted) {
      // Delete from disk (fire-and-forget)
      invoke("delete_persisted_session", { sessionId: id }).catch((err) => {
        console.error("Failed to delete persisted session:", err);
      });
    }
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
    if (deleteWorktree) {
      tryRemoveWorktree(worktreeCwd, get().addToast);
    }
  },

  createSession: async (name, cwd, sessionMode = "claude", pullLatest = false, isGitRepo = true, parentSessionId?) => {
    let id: string;
    let session: SessionInfo;

    // "claude" maps to undefined because the backend treats missing/unknown as Default.
    const claudeModeMap: Record<Exclude<SessionMode, "terminal">, string | undefined> = {
      "claude": undefined,
      "claude-auto": "auto",
      "claude-skip": "skip",
      "claude-plan": "plan",
    };

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
        ...(parentSessionId ? { parentSessionId } : {}),
      };
    } else {
      id = await invoke<string>("create_session", {
        name,
        cwd,
        sessionType: "claude",
        sessionMode: claudeModeMap[sessionMode],
        isGitRepo,
        pullLatest,
      });
      session = {
        id,
        name,
        status: "starting",
        createdAt: Date.now(),
        cwd,
        sessionType: "claude",
        isGitRepo,
        ...(cwd.includes("/.claude/worktrees/") ? { worktreeCwd: cwd } : {}),
      };
    }

    get().addSession(session);
    get().setActiveSession(id);
    get().setupEventListeners(id, sessionMode);
    // Don't let worktree paths contaminate lastUsedDirectory — they're
    // derivatives, not real project directories.
    if (!cwd.includes("/.claude/worktrees/")) {
      set({ lastUsedDirectory: cwd });
    }

    if (sessionMode !== "terminal") {
      try {
        const currentStatus = await invoke<string | null>("get_session_status", { id });
        if (currentStatus && currentStatus !== "starting") {
          get().updateSessionStatus(id, currentStatus as SessionStatus);
        }
      } catch (err) {
        // Expected if session was removed before status fetch completed.
        // Log unexpected errors so they aren't silently swallowed.
        if (get().sessions.has(id)) {
          console.warn("Failed to fetch initial session status:", err);
        }
      }

      try {
        const worktreeCwd = await invoke<string | null>("get_session_worktree_cwd", { id });
        if (worktreeCwd) {
          get().updateWorktreeCwd(id, worktreeCwd);
        }
      } catch {
        // Non-critical — worktree CWD will arrive via event if available.
      }
    }
  },

  closeSession: async (id, deleteWorktree) => {
    const state = get();
    const session = state.sessions.get(id);
    const worktreeCwd = session?.worktreeCwd;

    // Find child sessions (worktree-linked terminals)
    const children = Array.from(state.sessions.values()).filter(
      (s) => s.parentSessionId === id
    );

    if (session?.persisted) {
      // Close persisted children first
      for (const child of children) {
        if (child.persisted) {
          try {
            await invoke("delete_persisted_session", { sessionId: child.id });
          } catch (err) {
            console.error("Failed to delete persisted child session:", err);
          }
        } else {
          try {
            await invoke("close_session", { id: child.id });
          } catch (err) {
            console.error("Failed to close child session:", err);
          }
        }
        get().removeSession(child.id);
      }
      try {
        await invoke("delete_persisted_session", { sessionId: id });
      } catch (err) {
        console.error("Failed to delete persisted session:", err);
      }
      get().removeSession(id);
      if (deleteWorktree) {
        tryRemoveWorktree(worktreeCwd, get().addToast);
      }
      return;
    }

    // Close children in parallel before closing parent
    await Promise.all(
      children.map(async (child) => {
        try {
          await invoke("close_session", { id: child.id });
        } catch (err) {
          console.error("Failed to close child session:", err);
        }
        get().removeSession(child.id);
      })
    );

    await invoke("close_session", { id });
    get().removeSession(id);
    if (deleteWorktree) {
      tryRemoveWorktree(worktreeCwd, get().addToast);
    }
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

  loadPersistedSessions: async () => {
    try {
      const persisted = await invoke<Array<{
        id: string;
        name: string;
        cwd: string;
        session_type: string;
        is_git_repo: boolean;
        created_at_epoch_ms: number;
        status_at_close: string;
      }>>("list_persisted_sessions");

      const { sessions, addSession } = get();

      for (const raw of persisted) {
        if (sessions.has(raw.id)) continue;

        const sessionType = raw.session_type === "terminal" ? "terminal" as const : "claude" as const;
        const session: SessionInfo = {
          id: raw.id,
          name: raw.name,
          cwd: raw.cwd,
          createdAt: raw.created_at_epoch_ms,
          status: "exited",
          sessionType,
          isGitRepo: raw.is_git_repo,
          persisted: true,
        };
        addSession(session);
      }
    } catch (err) {
      console.error("Failed to load persisted sessions:", err);
    }
  },

  loadScrollback: async (sessionId) => {
    const session = get().sessions.get(sessionId);
    if (!session || session.scrollbackText !== undefined) return;

    try {
      const text = await invoke<string | null>("get_session_scrollback", { sessionId });
      if (text !== null) {
        set((state) => {
          const sessions = new Map(state.sessions);
          const s = sessions.get(sessionId);
          if (s) {
            sessions.set(sessionId, { ...s, scrollbackText: text });
          }
          return { sessions };
        });
      }
    } catch (err) {
      console.error("Failed to load scrollback:", err);
    }
  },

  setupEventListeners: (sessionId, sessionMode) => {
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

          if (sessionMode === "claude-auto" && status === "error") {
            const session = get().sessions.get(sessionId);
            const ageSeconds = session ? (Date.now() - session.createdAt) / 1000 : Infinity;
            if (ageSeconds < 10) {
              get().addToast(
                "Auto mode failed to start. Make sure Claude Code is updated to v2.1.83 or later: run 'claude update' in your terminal.",
                "error"
              );
            }
          }
        }

        // Persist session on exit (fire-and-forget)
        const exitedSession = get().sessions.get(sessionId);
        if (exitedSession && !exitedSession.persisted) {
          setTimeout(async () => {
            try {
              const scrollback = (window as any).__aoGetScrollback?.(sessionId) ?? "";
              await invoke("save_single_session", {
                session: {
                  id: exitedSession.id,
                  name: exitedSession.name,
                  cwd: exitedSession.cwd,
                  session_type: exitedSession.sessionType,
                  is_git_repo: exitedSession.isGitRepo,
                  created_at_epoch_ms: exitedSession.createdAt,
                  status_at_close: get().sessions.get(sessionId)?.status ?? exitedSession.status,
                },
                scrollback,
              });
            } catch (err) {
              console.error(`Failed to persist session ${sessionId} on exit:`, err);
            }
          }, 500);
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

    // Listen for worktree cwd updates on non-terminal sessions
    if (session?.sessionType !== "terminal") {
      cleanups.push(
        listen<{ worktreeCwd: string }>(`session-worktree-cwd-${sessionId}`, (event) => {
          get().updateWorktreeCwd(sessionId, event.payload.worktreeCwd);
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
