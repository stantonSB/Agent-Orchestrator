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
import { decodeBase64, encodeBase64 } from "../../lib/base64";
import styles from "./TerminalArea.module.css";
import { DropOverlay } from "./DropOverlay";
import { useImageDrop } from "./useImageDrop";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface TerminalSession {
  id: string;
  name: string;
  cwd: string;
  worktreeCwd?: string | null;
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
  const activeSession = sessions.find((s) => s.id === activeSessionId);
  const { isDragging, dropHandlers } = useImageDrop({
    activeSessionId,
    isActiveSessionReadOnly: activeSession?.persisted ?? false,
    mockMode,
  });

  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const refsMap = useRef(new Map<string, XTermInstanceHandle>());
  const outputListeners = useRef(new Map<string, Promise<() => void>>());
  const exitListeners = useRef(new Map<string, Promise<() => void>>());
  const outputBuffers = useRef(new Map<string, Uint8Array[]>());
  const onSessionExitRef = useRef(onSessionExitProp);
  onSessionExitRef.current = onSessionExitProp;

  // Per-session callback caches. Each `getX(id)` returns the SAME function
  // instance across renders, so the props handed to a (memoized) XTermInstance
  // stay referentially stable and React doesn't detach/reattach every terminal
  // (and re-run the ref flush) on each unrelated re-render.
  const refCallbacks = useRef(
    new Map<string, (handle: XTermInstanceHandle | null) => void>(),
  );
  const dataHandlers = useRef(new Map<string, (data: string) => void>());
  const resizeHandlers = useRef(
    new Map<string, (cols: number, rows: number) => void>(),
  );

  const getRefCallback = useCallback((id: string) => {
    const existing = refCallbacks.current.get(id);
    if (existing) return existing;
    const cb = (handle: XTermInstanceHandle | null) => {
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
    };
    refCallbacks.current.set(id, cb);
    return cb;
  }, []);

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
        if (!payload) return;
        const bytes = decodeBase64(payload);
        const handle = refsMap.current.get(sid);
        if (handle) {
          handle.write(bytes);
        } else {
          let buf = outputBuffers.current.get(sid);
          if (!buf) {
            buf = [];
            outputBuffers.current.set(sid, buf);
          }
          buf.push(bytes);
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

    // Drop cached per-session callbacks for sessions that no longer exist so
    // the caches don't grow unbounded over a long-lived window.
    for (const sid of [...refCallbacks.current.keys()]) {
      if (!currentIds.has(sid)) {
        refCallbacks.current.delete(sid);
        dataHandlers.current.delete(sid);
        resizeHandlers.current.delete(sid);
      }
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

  // Forward user keystrokes to PTY via IPC. Keystrokes are base64-encoded —
  // the same byte transport the PTY-output path uses.
  const handleSessionData = useCallback(
    (sessionId: string, data: string) => {
      if (mockMode) return;
      const bytes = new TextEncoder().encode(data);
      writeToSession({ id: sessionId, data: encodeBase64(bytes) }).catch((err) => {
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

  // Route the cached per-session handlers through refs so the cached function
  // identities (created once per id) never go stale even if the underlying
  // callbacks change.
  const handleSessionDataRef = useRef(handleSessionData);
  handleSessionDataRef.current = handleSessionData;
  const handleSessionResizeRef = useRef(handleSessionResize);
  handleSessionResizeRef.current = handleSessionResize;

  const getDataHandler = useCallback((id: string) => {
    const existing = dataHandlers.current.get(id);
    if (existing) return existing;
    const handler = (data: string) => handleSessionDataRef.current(id, data);
    dataHandlers.current.set(id, handler);
    return handler;
  }, []);

  const getResizeHandler = useCallback((id: string) => {
    const existing = resizeHandlers.current.get(id);
    if (existing) return existing;
    const handler = (cols: number, rows: number) =>
      handleSessionResizeRef.current(id, cols, rows);
    resizeHandlers.current.set(id, handler);
    return handler;
  }, []);

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
    <div className={styles.terminalArea} {...dropHandlers}>
      <div className={styles.terminalContainer}>
        {isDragging && <DropOverlay />}
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
            ref={getRefCallback(session.id)}
            sessionId={session.id}
            cwd={session.cwd}
            worktreeCwd={session.worktreeCwd}
            isActive={session.id === activeSessionId}
            mockMode={mockMode}
            readOnly={session.persisted}
            onData={getDataHandler(session.id)}
            onResize={getResizeHandler(session.id)}
          />
        ))}
      </div>
    </div>
  );
}
