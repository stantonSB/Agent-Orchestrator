import { useState, useMemo } from "react";
import { useSessionStore } from "./stores/sessionStore";
import { TitleBar } from "./components/TitleBar/TitleBar";
import { SessionPanel } from "./components/SessionPanel/SessionPanel";
import { NewSessionModal } from "./components/NewSessionModal/NewSessionModal";
import { XTermInstance } from "./components/XTermInstance/XTermInstance";
import { useInitializeSessions } from "./hooks/useInitializeSessions";
import styles from "./App.module.css";

export function App() {
  useInitializeSessions();
  const [isModalOpen, setIsModalOpen] = useState(false);

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

  return (
    <div className={styles.app}>
      <TitleBar />
      <div className={styles.content}>
        <div className={styles.terminalArea}>
          {sessionList.length === 0 && (
            <div className={styles.emptyTerminal}>
              <p className={styles.emptyText}>
                Create a session to get started
              </p>
              <button
                className={styles.emptyButton}
                onClick={handleNewSession}
                type="button"
              >
                + New Session
              </button>
            </div>
          )}
          {sessionList.map((session) => (
            <XTermInstance
              key={session.id}
              sessionId={session.id}
              isActive={session.id === activeSessionId}
            />
          ))}
        </div>
        <SessionPanel
          sessions={sessionList}
          activeSessionId={activeSessionId}
          onSessionClick={setActiveSession}
          onNewSession={handleNewSession}
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
