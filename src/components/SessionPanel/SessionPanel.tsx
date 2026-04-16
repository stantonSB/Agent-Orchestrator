import type { CSSProperties } from "react";
import type { SessionInfo } from "../../types/session";
import { useSessionStore } from "../../stores/sessionStore";
import { SessionCard } from "../SessionCard/SessionCard";
import { NewSessionButton } from "../NewSessionButton/NewSessionButton";
import styles from "./SessionPanel.module.css";

interface SessionPanelProps {
  sessions: SessionInfo[];
  activeSessionId: string | null;
  onSessionClick: (id: string) => void;
  onNewSession: () => void;
  style?: CSSProperties;
}

export function SessionPanel({
  sessions,
  activeSessionId,
  onSessionClick,
  onNewSession,
  style,
}: SessionPanelProps) {
  const closeSession = useSessionStore((s) => s.closeSession);
  const dismissSession = useSessionStore((s) => s.dismissSession);

  return (
    <div className={styles.panel} style={style}>
      <div className={styles.header}>Sessions</div>
      <NewSessionButton onClick={onNewSession} />
      {sessions.length === 0 ? (
        <div className={styles.empty}>No active sessions</div>
      ) : (
        <div className={styles.sessionList}>
          {sessions.map((session) => (
            <SessionCard
              key={session.id}
              session={session}
              isActive={session.id === activeSessionId}
              onClick={onSessionClick}
              onClose={closeSession}
              onDismiss={dismissSession}
            />
          ))}
        </div>
      )}
    </div>
  );
}
