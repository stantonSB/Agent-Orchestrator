import { getCurrentWindow } from "@tauri-apps/api/window";
import styles from "./TitleBar.module.css";

export function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <div className={styles.titleBar} data-tauri-drag-region>
      <div className={styles.title} data-tauri-drag-region>
        Agent Orchestrator
      </div>
      <div className={styles.windowControls}>
        <button
          className={`${styles.controlButton} ${styles.minimize}`}
          aria-label="Minimize"
          onClick={() => appWindow.minimize()}
        >
          <svg width="10" height="1" viewBox="0 0 10 1">
            <rect width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        <button
          className={`${styles.controlButton} ${styles.maximize}`}
          aria-label="Maximize"
          onClick={() => appWindow.toggleMaximize()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1" />
          </svg>
        </button>
        <button
          className={`${styles.controlButton} ${styles.close}`}
          aria-label="Close"
          onClick={() => appWindow.close()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" strokeWidth="1.2" />
            <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
      </div>
    </div>
  );
}
