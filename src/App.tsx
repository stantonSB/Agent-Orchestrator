import { useState, useMemo, useCallback, useRef } from "react";
import { useSessionStore } from "./stores/sessionStore";
import { TitleBar } from "./components/TitleBar/TitleBar";
import { SessionPanel } from "./components/SessionPanel/SessionPanel";
import { NewSessionModal } from "./components/NewSessionModal/NewSessionModal";
import { TerminalArea } from "./components/TerminalArea/TerminalArea";
import { useInitializeSessions } from "./hooks/useInitializeSessions";
import styles from "./App.module.css";

export function App() {
  useInitializeSessions();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [panelWidth, setPanelWidth] = useState(300);
  const isDragging = useRef(false);

  const sessions = useSessionStore((s) => s.sessions);
  const activeSessionId = useSessionStore((s) => s.activeSessionId);
  const lastUsedDirectory = useSessionStore((s) => s.lastUsedDirectory);
  const setActiveSession = useSessionStore((s) => s.setActiveSession);
  const createSession = useSessionStore((s) => s.createSession);

  const sessionList = useMemo(() => {
    return Array.from(sessions.values()).sort(
      (a, b) => b.createdAt - a.createdAt
    );
  }, [sessions]);

  const handleNewSession = () => {
    setIsModalOpen(true);
  };

  const handleCreateSession = async (name: string, cwd: string) => {
    setIsModalOpen(false);
    await createSession(name, cwd);
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
    </div>
  );
}
