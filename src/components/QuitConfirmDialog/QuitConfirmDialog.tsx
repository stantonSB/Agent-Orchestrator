import styles from "./QuitConfirmDialog.module.css";

interface QuitConfirmDialogProps {
  onConfirm: () => void;
  onCancel: () => void;
}

export function QuitConfirmDialog({ onConfirm, onCancel }: QuitConfirmDialogProps) {
  return (
    <div className={styles.overlay} onClick={onCancel}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <h3 className={styles.title}>Quit Agent Orchestrator?</h3>
        <p className={styles.message}>
          All running sessions will be terminated. Session state will be saved.
        </p>
        <div className={styles.actions}>
          <button className={styles.cancelBtn} onClick={onCancel}>Cancel</button>
          <button className={styles.confirmBtn} onClick={onConfirm}>Quit</button>
        </div>
      </div>
    </div>
  );
}
