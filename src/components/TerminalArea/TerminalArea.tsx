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
  const outputListeners = useRef(new Map<string, Promise<() => void>>());
  const exitListeners = useRef(new Map<string, Promise<() => void>>());
  const outputBuffers = useRef(new Map<string, Uint8Array[]>());
  const onSessionExitRef = useRef(onSessionExitProp);
  onSessionExitRef.current = onSessionExitProp;

  const setRef = useCallback(
    (id: string) => (handle: XTermInstanceHandle | null) => {
      if (handle) {
        refsMap.current.set(id, handle);
        // Flush any output that arrived before the terminal mounted
        const buffer = outputBuffers.current.get(id);
        if (buffer) {
          for (const chunk of buffer) handle.write(chunk);
          outputBuffers.current.delete(id);
        }
      } else {
        refsMap.current.delete(id);
      }
    },
    [],
  );

  // Incrementally manage per-session output and exit listeners.
  // Listeners persist in refs so adding session B never tears down session A's listener.
  useEffect(() => {
    if (mockMode) return;

    const currentIds = new Set(sessions.map((s) => s.id));

    // Register output listeners for new sessions
    for (const session of sessions) {
      const sid = session.id;
      if (outputListeners.current.has(sid)) continue;

      const promise = onSessionOutput(sid, (payload) => {
        if (!payload.data) return;
        const handle = refsMap.current.get(sid);
        if (handle) {
          handle.write(new Uint8Array(payload.data));
        } else {
          let buf = outputBuffers.current.get(sid);
          if (!buf) {
            buf = [];
            outputBuffers.current.set(sid, buf);
          }
          buf.push(new Uint8Array(payload.data));
        }
      }).catch((err) => {
        console.error(`Failed to register output listener for ${sid}:`, err);
        return () => {};
      });
      outputListeners.current.set(sid, promise);
    }

    // Register exit listeners for new sessions
    for (const session of sessions) {
      const sid = session.id;
      if (exitListeners.current.has(sid)) continue;

      const promise = onSessionExit(sid, (payload) => {
        onSessionExitRef.current?.(sid, payload);
      }).catch((err) => {
        console.error(`Failed to register exit listener for ${sid}:`, err);
        return () => {};
      });
      exitListeners.current.set(sid, promise);
    }

    // Clean up listeners for removed sessions
    const staleOutput = [...outputListeners.current.keys()].filter(
      (sid) => !currentIds.has(sid),
    );
    for (const sid of staleOutput) {
      outputListeners.current.get(sid)!.then((unlisten) => unlisten());
      outputListeners.current.delete(sid);
      outputBuffers.current.delete(sid);
    }

    const staleExit = [...exitListeners.current.keys()].filter(
      (sid) => !currentIds.has(sid),
    );
    for (const sid of staleExit) {
      exitListeners.current.get(sid)!.then((unlisten) => unlisten());
      exitListeners.current.delete(sid);
    }
  }, [sessions, mockMode]);

  // Clean up all listeners on unmount.
  useEffect(() => {
    return () => {
      for (const [, promise] of outputListeners.current) {
        promise.then((unlisten) => unlisten());
      }
      for (const [, promise] of exitListeners.current) {
        promise.then((unlisten) => unlisten());
      }
      outputListeners.current.clear();
      exitListeners.current.clear();
      outputBuffers.current.clear();
    };
  }, []);

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
