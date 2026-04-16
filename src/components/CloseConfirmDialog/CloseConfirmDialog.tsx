import styles from "./CloseConfirmDialog.module.css";

interface CloseConfirmDialogProps {
  sessionName: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function CloseConfirmDialog({ sessionName, onConfirm, onCancel }: CloseConfirmDialogProps) {
  return (
    <div className={styles.overlay} onClick={onCancel}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <h3 className={styles.title}>Close Session</h3>
        <p className={styles.message}>
          Are you sure you want to close <strong>{sessionName}</strong>? This will terminate the Claude process.
        </p>
        <div className={styles.actions}>
          <button className={styles.cancelBtn} onClick={onCancel}>Cancel</button>
          <button className={styles.confirmBtn} onClick={onConfirm}>Close Session</button>
        </div>
      </div>
    </div>
  );
}
