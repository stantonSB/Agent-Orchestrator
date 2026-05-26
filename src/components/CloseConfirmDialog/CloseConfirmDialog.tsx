import { useEffect, useRef } from "react";
import styles from "./CloseConfirmDialog.module.css";

interface CloseConfirmDialogProps {
  sessionName: string;
  isRunning?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function CloseConfirmDialog({ sessionName, isRunning = true, onConfirm, onCancel }: CloseConfirmDialogProps) {
  const confirmRef = useRef<HTMLButtonElement>(null);
  const title = isRunning ? "Close Session" : "Dismiss Session";
  const message = isRunning
    ? <>Are you sure you want to close <strong>{sessionName}</strong>? This will terminate the Claude process and delete its worktree.</>
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
        <div className={styles.actions}>
          <button className={styles.cancelBtn} onClick={onCancel}>Cancel</button>
          <button ref={confirmRef} className={styles.confirmBtn} onClick={onConfirm}>{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}
