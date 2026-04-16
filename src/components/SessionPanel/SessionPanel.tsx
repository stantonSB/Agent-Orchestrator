import type { CSSProperties } from "react";
import type { SessionInfo } from "../../types/session";
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
            />
          ))}
        </div>
      )}
    </div>
  );
}
