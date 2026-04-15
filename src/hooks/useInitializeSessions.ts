import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../stores/sessionStore";
import type { SessionInfo } from "../types/session";

export function useInitializeSessions() {
  const addSession = useSessionStore((s) => s.addSession);
  const setupEventListeners = useSessionStore((s) => s.setupEventListeners);

  useEffect(() => {
    async function init() {
      try {
        const existing = await invoke<SessionInfo[]>("list_sessions");
        for (const session of existing) {
          addSession(session);
          setupEventListeners(session.id);
        }
      } catch (err) {
        console.error("Failed to initialize sessions:", err);
      }
    }
    init();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
}
