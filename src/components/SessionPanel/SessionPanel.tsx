import styles from "./SessionPanel.module.css";

interface SessionPanelProps {
  sessionCount: number;
}

export function SessionPanel({ sessionCount }: SessionPanelProps) {
  return (
    <div className={styles.sessionPanel}>
      <div className={styles.header}>
        <h2 className={styles.headerTitle}>Sessions</h2>
        <span className={styles.sessionCount}>{sessionCount}</span>
      </div>

      <button className={styles.newSessionButton}>
        <span className={styles.plusIcon}>+</span>
        New Session
      </button>

      <div className={styles.sessionList}>
        {sessionCount === 0 ? (
          <div className={styles.emptyList}>
            <p className={styles.emptyText}>
              No sessions yet. Click "New Session" to start.
            </p>
          </div>
        ) : (
          <div className={styles.placeholderCards}>
            <p className={styles.emptyText}>
              Session cards will appear here.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
