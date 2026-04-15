import type { SessionInfo, SessionStatus } from "../../types/session";
import styles from "./SessionCard.module.css";

interface SessionCardProps {
  session: SessionInfo;
  isActive: boolean;
  onClick: (id: string) => void;
}

const STATUS_DOT_CLASS: Record<SessionStatus, string> = {
  starting: styles.statusStarting,
  working: styles.statusWorking,
  idle: styles.statusIdle,
  needs_attention: styles.statusNeedsAttention,
  finished: styles.statusFinished,
  error: styles.statusError,
};

const STATUS_LABEL: Record<SessionStatus, string> = {
  starting: "Starting",
  working: "Working",
  idle: "Idle",
  needs_attention: "Needs Attention",
  finished: "Finished",
  error: "Error",
};

function isDismissed(status: SessionStatus): boolean {
  return status === "finished" || status === "error";
}

export function SessionCard({ session, isActive, onClick }: SessionCardProps) {
  const cardClass = [
    styles.card,
    isActive ? styles.active : "",
    isDismissed(session.status) ? styles.dismissed : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={cardClass}
      onClick={() => onClick(session.id)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          onClick(session.id);
        }
      }}
    >
      {session.status === "finished" ? (
        <span className={styles.finishedIcon}>&#10003;</span>
      ) : (
        <span
          className={`${styles.statusDot} ${STATUS_DOT_CLASS[session.status]}`}
        />
      )}
      <div className={styles.info}>
        <span className={styles.name}>{session.name}</span>
        <span className={styles.status}>{STATUS_LABEL[session.status]}</span>
      </div>
    </div>
  );
}
