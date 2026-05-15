import { useRef, useState, useCallback, useEffect } from "react";
import {
  XTermInstance,
  type XTermInstanceHandle,
} from "../XTermInstance/XTermInstance";
import { SearchBar } from "../SearchBar/SearchBar";
import {
  onSessionOutput,
  onSessionExit,
  writeToSession,
  resizeSession,
} from "../../lib/tauri-ipc";
import type { SessionExitPayload } from "../../types/tauri-events";
import { useSessionStore } from "../../stores/sessionStore";
import styles from "./TerminalArea.module.css";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface TerminalSession {
  id: string;
  name: string;
  cwd: string;
  persisted?: boolean;
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
  const [isSearchOpen, setIsSearchOpen] = useState(false);
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
      // Skip PTY listeners for persisted (read-only) sessions
      if (session.persisted) continue;

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
      // Skip PTY listeners for persisted (read-only) sessions
      if (session.persisted) continue;

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

  // Register global functions for the store to access scrollback
  useEffect(() => {
    (window as any).__aoGetScrollback = (sessionId: string): string => {
      const handle = refsMap.current.get(sessionId);
      return handle?.getScrollbackText(500) ?? "";
    };
    (window as any).__aoGetAllScrollbacks = (): Record<string, string> => {
      const result: Record<string, string> = {};
      for (const [id, handle] of refsMap.current) {
        result[id] = handle.getScrollbackText(500);
      }
      return result;
    };
    return () => {
      delete (window as any).__aoGetScrollback;
      delete (window as any).__aoGetAllScrollbacks;
    };
  }, []);

  // Load scrollback for persisted sessions when they become active
  const loadScrollback = useSessionStore((s) => s.loadScrollback);

  useEffect(() => {
    if (!activeSessionId) return;
    const session = sessions.find(s => s.id === activeSessionId);
    if (!session?.persisted) return;

    const doLoad = async () => {
      await loadScrollback(activeSessionId);
      const updated = useSessionStore.getState().sessions.get(activeSessionId);
      if (updated?.scrollbackText) {
        const handle = refsMap.current.get(activeSessionId);
        if (handle) {
          handle.write(updated.scrollbackText);
        }
      }
    };

    doLoad();
  }, [activeSessionId, sessions, loadScrollback]);

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
  // Guard against bogus dimensions — once a small size reaches the PTY,
  // the child process (Claude Code) reformats all output for that width,
  // permanently breaking the session.
  const handleSessionResize = useCallback(
    (sessionId: string, cols: number, rows: number) => {
      if (mockMode) return;
      if (cols < 20 || rows < 5) {
        console.warn(`Ignoring suspicious resize for ${sessionId}: ${cols}x${rows}`);
        return;
      }
      resizeSession({ id: sessionId, cols, rows }).catch((err) => {
        console.error(`Failed to resize session ${sessionId}:`, err);
      });
    },
    [mockMode],
  );

  const openSearch = useCallback(() => {
    if (activeSessionId && sessions.length > 0) {
      setIsSearchOpen(true);
    }
  }, [activeSessionId, sessions.length]);

  const closeSearch = useCallback(() => {
    if (activeSessionId) {
      refsMap.current.get(activeSessionId)?.clearSearch();
      refsMap.current.get(activeSessionId)?.focus();
    }
    setIsSearchOpen(false);
  }, [activeSessionId]);

  // Close search when switching sessions
  useEffect(() => {
    setIsSearchOpen(false);
  }, [activeSessionId]);

  // Cmd+F keybinding
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.metaKey && e.key === "f") {
        e.preventDefault();
        openSearch();
      }
      if (e.key === "Escape" && isSearchOpen) {
        e.preventDefault();
        closeSearch();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [openSearch, closeSearch, isSearchOpen]);

  const handleFindNext = useCallback(
    (query: string) => {
      if (!activeSessionId) return false;
      return refsMap.current.get(activeSessionId)?.findNext(query) ?? false;
    },
    [activeSessionId],
  );

  const handleFindPrevious = useCallback(
    (query: string) => {
      if (!activeSessionId) return false;
      return refsMap.current.get(activeSessionId)?.findPrevious(query) ?? false;
    },
    [activeSessionId],
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
        {isSearchOpen && (
          <SearchBar
            onFindNext={handleFindNext}
            onFindPrevious={handleFindPrevious}
            onClose={closeSearch}
          />
        )}
        {sessions.map((session) => (
          <XTermInstance
            key={session.id}
            ref={setRef(session.id)}
            sessionId={session.id}
            cwd={session.cwd}
            isActive={session.id === activeSessionId}
            mockMode={mockMode}
            readOnly={session.persisted}
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
