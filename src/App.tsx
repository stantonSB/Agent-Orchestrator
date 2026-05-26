import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { createPortal } from "react-dom";
import { listen } from "@tauri-apps/api/event";
import { useSessionStore } from "./stores/sessionStore";
import type { SessionMode } from "./types/session";
import { TitleBar } from "./components/TitleBar/TitleBar";
import { SessionPanel, groupSessionsByProject } from "./components/SessionPanel/SessionPanel";
import { NewSessionModal } from "./components/NewSessionModal/NewSessionModal";
import { TerminalArea } from "./components/TerminalArea/TerminalArea";
import { ToastContainer } from "./components/ToastContainer/ToastContainer";
import { CloseConfirmDialog } from "./components/CloseConfirmDialog/CloseConfirmDialog";
import { useInitializeSessions } from "./hooks/useInitializeSessions";
import { useSaveOnClose } from "./hooks/useSaveOnClose";
import { useGlobalKeybindings, getCycledIndex } from "./hooks/useGlobalKeybindings";
import styles from "./App.module.css";

export function App() {
  useInitializeSessions();
  useSaveOnClose();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const addToast = useSessionStore((s) => s.addToast);

  useEffect(() => {
    const unlisten = listen<{ error: string }>("spawn-error", (event) => {
      addToast(event.payload.error, "error");
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [addToast]);
  const [panelWidth, setPanelWidth] = useState(300);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);
  const isDragging = useRef(false);

  const sessions = useSessionStore((s) => s.sessions);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const lastUsedDirectory = useSessionStore((s) => s.lastUsedDirectory);
  const setActiveSession = useSessionStore((s) => s.setActiveSession);
  const createSession = useSessionStore((s) => s.createSession);
  const closeSession = useSessionStore((s) => s.closeSession);
  const dismissSession = useSessionStore((s) => s.dismissSession);

  const activeSession = activeSessionId ? sessions.get(activeSessionId) ?? null : null;
  const activeIsRunning = activeSession
    ? activeSession.status !== "finished" && activeSession.status !== "error" && activeSession.status !== "exited"
    : false;

  const handleNewSession = useCallback(() => {
    setIsModalOpen(true);
  }, []);

  const handleCloseActiveSession = useCallback(() => {
    if (!activeSession) return;
    setShowCloseConfirm(true);
  }, [activeSession]);

  const handleConfirmClose = useCallback(() => {
    if (!activeSession) return;
    if (activeIsRunning) {
      closeSession(activeSession.id);
    } else {
      dismissSession(activeSession.id);
    }
    setShowCloseConfirm(false);
  }, [activeSession, activeIsRunning, closeSession, dismissSession]);

  const sessionList = useMemo(() => {
    return Array.from(sessions.values()).sort(
      (a, b) => b.createdAt - a.createdAt
    );
  }, [sessions]);

  const orderedSessionIds = useMemo(() => {
    const groups = groupSessionsByProject(sessionList);
    return groups.flatMap((group) => group.sessions.map((s) => s.id));
  }, [sessionList]);

  const handleSwitchToSession = useCallback(
    (index: number) => {
      if (index < orderedSessionIds.length) {
        setActiveSession(orderedSessionIds[index]);
      }
    },
    [orderedSessionIds, setActiveSession],
  );

  const handleCyclePrev = useCallback(() => {
    const idx = getCycledIndex("prev", activeSessionId, orderedSessionIds);
    if (idx !== null) setActiveSession(orderedSessionIds[idx]);
  }, [orderedSessionIds, activeSessionId, setActiveSession]);

  const handleCycleNext = useCallback(() => {
    const idx = getCycledIndex("next", activeSessionId, orderedSessionIds);
    if (idx !== null) setActiveSession(orderedSessionIds[idx]);
  }, [orderedSessionIds, activeSessionId, setActiveSession]);

  useGlobalKeybindings({
    onNewSession: handleNewSession,
    onCloseActiveSession: handleCloseActiveSession,
    onSwitchToSession: handleSwitchToSession,
    onCyclePrev: handleCyclePrev,
    onCycleNext: handleCycleNext,
    onOpenSettings: () => {}, // wired in Task 7
  });

  const handleCreateSession = async (name: string, cwd: string, sessionMode: SessionMode, pullLatest: boolean, isGitRepo: boolean) => {
    try {
      setIsModalOpen(false);
      await createSession(name, cwd, sessionMode, pullLatest, isGitRepo);
    } catch (err) {
      addToast(
        `Failed to create session: ${err instanceof Error ? err.message : String(err)}`,
        "error"
      );
    }
  };

  const handleResizeMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      isDragging.current = true;

      const onMouseMove = (moveEvent: MouseEvent) => {
        if (!isDragging.current) return;
        const newWidth = Math.min(600, Math.max(200, window.innerWidth - moveEvent.clientX));
        setPanelWidth(newWidth);
      };

      const onMouseUp = () => {
        isDragging.current = false;
        document.removeEventListener("mousemove", onMouseMove);
        document.removeEventListener("mouseup", onMouseUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("mouseup", onMouseUp);
    },
    [],
  );

  return (
    <div className={styles.app}>
      <TitleBar />
      <div className={styles.content}>
        <TerminalArea
          sessions={sessionList}
          activeSessionId={activeSessionId}
        />
        <div
          className={styles.resizeHandle}
          onMouseDown={handleResizeMouseDown}
        />
        <SessionPanel
          sessions={sessionList}
          activeSessionId={activeSessionId}
          onSessionClick={setActiveSession}
          onNewSession={handleNewSession}
          style={{ width: panelWidth }}
        />
      </div>
      <NewSessionModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        onCreate={handleCreateSession}
        lastUsedDirectory={lastUsedDirectory}
      />
      <ToastContainer />
      {showCloseConfirm && activeSession &&
        createPortal(
          <CloseConfirmDialog
            sessionName={activeSession.name}
            isRunning={activeIsRunning}
            onConfirm={handleConfirmClose}
            onCancel={() => setShowCloseConfirm(false)}
          />,
          document.body
        )}
    </div>
  );
}
