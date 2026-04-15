import { useRef, useCallback, useEffect } from "react";
import {
  XTermInstance,
  type XTermInstanceHandle,
} from "../XTermInstance/XTermInstance";
import {
  onSessionOutput,
  onSessionExit,
  writeToSession,
  resizeSession,
} from "../../lib/tauri-ipc";
import type { SessionExitPayload } from "../../types/tauri-events";
import styles from "./TerminalArea.module.css";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface TerminalSession {
  id: string;
  name: string;
}

interface TerminalAreaProps {
  sessions: TerminalSession[];
  activeSessionId: string | null;
  onSessionExit?: (sessionId: string, payload: SessionExitPayload) => void;
  mockMode?: boolean;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function TerminalArea({
  sessions,
  activeSessionId,
  onSessionExit: onSessionExitProp,
  mockMode = false,
}: TerminalAreaProps) {
  const refsMap = useRef(new Map<string, XTermInstanceHandle>());

  const setRef = useCallback(
    (id: string) => (handle: XTermInstanceHandle | null) => {
      if (handle) {
        refsMap.current.set(id, handle);
      } else {
        refsMap.current.delete(id);
      }
    },
    [],
  );

  // Wire Tauri event listeners for each session (skip in mock mode).
  useEffect(() => {
    if (mockMode) return;

    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      for (const session of sessions) {
        const sid = session.id;

        const [unlistenOutput, unlistenExit] = await Promise.all([
          onSessionOutput(sid, (payload) => {
            if (cancelled) return;
            const handle = refsMap.current.get(sid);
            if (handle && payload.data) {
              handle.write(new Uint8Array(payload.data));
            }
          }),
          onSessionExit(sid, (payload) => {
            if (cancelled) return;
            onSessionExitProp?.(sid, payload);
          }),
        ]);

        unlisteners.push(unlistenOutput, unlistenExit);
      }

      if (cancelled) {
        for (const fn of unlisteners) fn();
      }
    })();

    return () => {
      cancelled = true;
      for (const fn of unlisteners) fn();
    };
  }, [sessions, mockMode, onSessionExitProp]);

  // Forward user keystrokes to PTY via IPC.
  const handleSessionData = useCallback(
    (sessionId: string, data: string) => {
      if (mockMode) return;
      const encoder = new TextEncoder();
      const bytes = Array.from(encoder.encode(data));
      writeToSession({ id: sessionId, data: bytes }).catch((err) => {
        console.error(`Failed to write to session ${sessionId}:`, err);
      });
    },
    [mockMode],
  );

  // Forward terminal resize to PTY via IPC.
  const handleSessionResize = useCallback(
    (sessionId: string, cols: number, rows: number) => {
      if (mockMode) return;
      resizeSession({ id: sessionId, cols, rows }).catch((err) => {
        console.error(`Failed to resize session ${sessionId}:`, err);
      });
    },
    [mockMode],
  );

  if (sessions.length === 0) {
    return (
      <div className={styles.terminalArea}>
        <div className={styles.emptyState}>
          <div className={styles.emptyIcon}>&#9654;</div>
          <h2 className={styles.emptyTitle}>No Active Session</h2>
          <p className={styles.emptyDescription}>
            Create a new session from the sidebar to start working with Claude
            Code.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.terminalArea}>
      <div className={styles.terminalContainer}>
        {sessions.map((session) => (
          <XTermInstance
            key={session.id}
            ref={setRef(session.id)}
            sessionId={session.id}
            isActive={session.id === activeSessionId}
            mockMode={mockMode}
            onData={(data) => handleSessionData(session.id, data)}
            onResize={(cols, rows) =>
              handleSessionResize(session.id, cols, rows)
            }
          />
        ))}
      </div>
    </div>
  );
}
