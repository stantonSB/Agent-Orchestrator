import { useEffect } from "react";
import styles from "./Toast.module.css";

export interface ToastData {
  id: string;
  message: string;
  type: "error" | "warning" | "info";
}

interface ToastProps {
  toast: ToastData;
  onDismiss: (id: string) => void;
}

export function Toast({ toast, onDismiss }: ToastProps) {
  useEffect(() => {
    const timer = setTimeout(() => onDismiss(toast.id), 8000);
    return () => clearTimeout(timer);
  }, [toast.id, onDismiss]);

  return (
    <div className={`${styles.toast} ${styles[toast.type]}`}>
      <span className={styles.message}>{toast.message}</span>
      <button className={styles.dismissBtn} onClick={() => onDismiss(toast.id)} aria-label="Dismiss">
        ×
      </button>
    </div>
  );
}
