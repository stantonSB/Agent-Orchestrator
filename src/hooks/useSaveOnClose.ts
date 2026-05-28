import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../stores/sessionStore";

let isQuitting = false;

export function useSaveOnClose() {
  useEffect(() => {
    const appWindow = getCurrentWindow();

    const unlisten = appWindow.onCloseRequested(async (event) => {
      if (isQuitting) return;
      event.preventDefault();
      useSessionStore.getState().setShowQuitConfirm(true);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);
}

export async function saveSessionsAndQuit() {
  const appWindow = getCurrentWindow();

  try {
    const state = useSessionStore.getState();
    const sessions = Array.from(state.sessions.values());
    const persistSessions: Array<{
      id: string;
      name: string;
      cwd: string;
      session_type: string;
      is_git_repo: boolean;
      created_at_epoch_ms: number;
      status_at_close: string;
    }> = [];
    const scrollbacks: Record<string, string> = {};

    const allScrollbacks = (window as any).__aoGetAllScrollbacks?.() ?? {};

    for (const session of sessions) {
      if (session.persisted) continue;
      persistSessions.push({
        id: session.id,
        name: session.name,
        cwd: session.cwd,
        session_type: session.sessionType,
        is_git_repo: session.isGitRepo,
        created_at_epoch_ms: session.createdAt,
        status_at_close: session.status,
      });
      scrollbacks[session.id] = allScrollbacks[session.id] ?? "";
    }

    if (persistSessions.length > 0) {
      await invoke("save_sessions", {
        sessions: persistSessions,
        scrollbacks,
      });
    }
  } catch (err) {
    console.error("Failed to save sessions on close:", err);
  }

  isQuitting = true;
  await appWindow.close();
}
