import { useEffect, useRef, useState } from "react";
import styles from "./CloseConfirmDialog.module.css";

interface CloseConfirmDialogProps {
  sessionName: string;
  isRunning?: boolean;
  hasWorktree?: boolean;
  onConfirm: (deleteWorktree: boolean) => void;
  onCancel: () => void;
}

export function CloseConfirmDialog({ sessionName, isRunning = true, hasWorktree = false, onConfirm, onCancel }: CloseConfirmDialogProps) {
  const confirmRef = useRef<HTMLButtonElement>(null);
  const [deleteWorktree, setDeleteWorktree] = useState(true);
  const title = isRunning ? "Close Session" : "Dismiss Session";
  const message = isRunning
    ? <>Are you sure you want to close <strong>{sessionName}</strong>? This will terminate the Claude process.</>
    : <>Are you sure you want to dismiss <strong>{sessionName}</strong>? This will remove it from the session list.</>;
  const confirmLabel = isRunning ? "Close Session" : "Dismiss";

  useEffect(() => {
    confirmRef.current?.focus();
  }, []);

  return (
    <div className={styles.overlay} onClick={onCancel}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <h3 className={styles.title}>{title}</h3>
        <p className={styles.message}>{message}</p>
        {hasWorktree && (
          <label className={styles.checkboxRow}>
            <input
              type="checkbox"
              checked={deleteWorktree}
              onChange={(e) => setDeleteWorktree(e.target.checked)}
              className={styles.checkbox}
            />
            <span className={styles.checkboxLabel}>Delete worktree</span>
          </label>
        )}
        <div className={styles.actions}>
          <button className={styles.cancelBtn} onClick={onCancel}>Cancel</button>
          <button ref={confirmRef} className={styles.confirmBtn} onClick={() => onConfirm(hasWorktree && deleteWorktree)}>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}
