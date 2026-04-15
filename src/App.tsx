import { useState, useCallback } from "react";
import styles from "./App.module.css";
import { TitleBar } from "./components";
import {
  TerminalArea,
  type TerminalSession,
} from "./components/TerminalArea/TerminalArea";
import {
  createSession,
  closeSession,
} from "./lib/tauri-ipc";
import type { SessionExitPayload } from "./types/tauri-events";

// Toggle for development: set to true to use mock terminals without a backend.
const MOCK_MODE = false;

const MOCK_SESSIONS: TerminalSession[] = MOCK_MODE
  ? [{ id: "mock-1", name: "Mock Session 1" }]
  : [];

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

function App() {
  const [sessions, setSessions] = useState<TerminalSession[]>(MOCK_SESSIONS);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(
    MOCK_MODE ? "mock-1" : null,
  );

  const handleCreateSession = useCallback(async () => {
    if (MOCK_MODE) {
      const id = `mock-${Date.now()}`;
      setSessions((prev) => [...prev, { id, name: `Session ${prev.length + 1}` }]);
      setActiveSessionId(id);
      return;
    }

    try {
      const name = `Session ${sessions.length + 1}`;
      const cwd = "/tmp";
      const id = await createSession({ name, cwd });
      setSessions((prev) => [...prev, { id, name }]);
      setActiveSessionId(id);
    } catch (err) {
      console.error("Failed to create session:", err);
    }
  }, [sessions.length]);

  const handleCloseSession = useCallback(
    async (sessionId: string) => {
      if (!MOCK_MODE) {
        try {
          await closeSession({ id: sessionId });
        } catch (err) {
          console.error("Failed to close session:", err);
        }
      }
      setSessions((prev) => {
        const remaining = prev.filter((s) => s.id !== sessionId);
        if (activeSessionId === sessionId) {
          setActiveSessionId(remaining.length > 0 ? remaining[0].id : null);
        }
        return remaining;
      });
    },
    [activeSessionId],
  );

  const handleSessionExit = useCallback(
    (sessionId: string, payload: SessionExitPayload) => {
      console.log(`[Session ${sessionId}] Exited with code:`, payload.code);
    },
    [],
  );

  return (
    <div className={styles.app}>
      <TitleBar />
      <div className={styles.mainContent}>
        <TerminalArea
          sessions={sessions}
          activeSessionId={activeSessionId}
          mockMode={MOCK_MODE}
          onSessionExit={handleSessionExit}
        />
        <div className={styles.sessionPanel}>
          <div style={{ padding: "16px" }}>
            <button
              onClick={handleCreateSession}
              style={{
                width: "100%",
                padding: "8px 12px",
                backgroundColor: "#1a1b26",
                color: "#7aa2f7",
                border: "1px solid #3b3f5c",
                borderRadius: "6px",
                cursor: "pointer",
                fontFamily: "inherit",
                fontSize: "13px",
              }}
            >
              + New Session
            </button>
            <div style={{ marginTop: "12px" }}>
              {sessions.map((s) => (
                <div
                  key={s.id}
                  onClick={() => setActiveSessionId(s.id)}
                  style={{
                    padding: "8px",
                    marginBottom: "4px",
                    borderRadius: "4px",
                    cursor: "pointer",
                    backgroundColor:
                      s.id === activeSessionId ? "#1a1b26" : "transparent",
                    color: s.id === activeSessionId ? "#c0caf5" : "#565f89",
                    fontSize: "13px",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                  }}
                >
                  <span>{s.name}</span>
                  <span
                    onClick={(e) => {
                      e.stopPropagation();
                      handleCloseSession(s.id);
                    }}
                    style={{ color: "#f7768e", cursor: "pointer", fontSize: "11px" }}
                  >
                    close
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
