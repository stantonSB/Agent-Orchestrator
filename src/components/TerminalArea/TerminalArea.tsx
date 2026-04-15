import styles from "./TerminalArea.module.css";

interface TerminalAreaProps {
  activeSessionId: string | null;
}

export function TerminalArea({ activeSessionId }: TerminalAreaProps) {
  return (
    <div className={styles.terminalArea}>
      {activeSessionId ? (
        <div className={styles.terminalContainer}>
          <div className={styles.placeholder}>
            <span className={styles.placeholderText}>
              Terminal for session: {activeSessionId}
            </span>
          </div>
        </div>
      ) : (
        <div className={styles.emptyState}>
          <div className={styles.emptyIcon}>&#9654;</div>
          <h2 className={styles.emptyTitle}>No Active Session</h2>
          <p className={styles.emptyDescription}>
            Create a new session from the sidebar to start working with Claude
            Code.
          </p>
        </div>
      )}
    </div>
  );
}
