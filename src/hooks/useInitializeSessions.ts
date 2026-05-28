import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../stores/sessionStore";
import type { SessionInfo } from "../types/session";

export function useInitializeSessions() {
  const addSession = useSessionStore((s) => s.addSession);
  const setupEventListeners = useSessionStore((s) => s.setupEventListeners);
  const loadPersistedSessions = useSessionStore((s) => s.loadPersistedSessions);

  useEffect(() => {
    async function init() {
      try {
        const existing = await invoke<Array<{
          id: string;
          name: string;
          cwd: string;
          created_at_epoch_ms: number;
          session_type: string;
          is_git_repo: boolean;
          worktree_cwd: string | null;
        }>>("list_sessions");
        for (const raw of existing) {
          const sessionType = raw.session_type === "terminal" ? "terminal" as const : "claude" as const;
          const session: SessionInfo = {
            id: raw.id,
            name: raw.name,
            cwd: raw.cwd,
            createdAt: raw.created_at_epoch_ms,
            status: sessionType === "terminal" ? "terminal" : "idle",
            sessionType,
            isGitRepo: raw.is_git_repo,
            worktreeCwd: raw.worktree_cwd,
          };
          addSession(session);
          setupEventListeners(session.id);
        }

        // Load persisted sessions after live ones (live wins on ID conflict)
        await loadPersistedSessions();
      } catch (err) {
        console.error("Failed to initialize sessions:", err);
      }
    }
    init();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
}
