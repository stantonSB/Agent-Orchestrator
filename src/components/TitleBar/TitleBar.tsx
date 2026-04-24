import { getCurrentWindow } from "@tauri-apps/api/window";
import styles from "./TitleBar.module.css";

export function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <div className={styles.titleBar} data-tauri-drag-region>
      <div className={styles.windowControls}>
        <button
          className={`${styles.trafficLight} ${styles.close}`}
          aria-label="Close"
          onClick={() => appWindow.close()}
        >
          <svg width="6" height="6" viewBox="0 0 6 6">
            <line x1="0" y1="0" x2="6" y2="6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
            <line x1="6" y1="0" x2="0" y2="6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          </svg>
        </button>
        <button
          className={`${styles.trafficLight} ${styles.minimize}`}
          aria-label="Minimize"
          onClick={() => appWindow.minimize()}
        >
          <svg width="8" height="2" viewBox="0 0 8 2">
            <line x1="0" y1="1" x2="8" y2="1" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
          </svg>
        </button>
        <button
          className={`${styles.trafficLight} ${styles.maximize}`}
          aria-label="Maximize"
          onClick={() => appWindow.toggleMaximize()}
        >
          <svg width="6" height="6" viewBox="0 0 6 6">
            <path d="M0.5 3.5 L0.5 0.5 L3.5 0.5" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
            <path d="M5.5 2.5 L5.5 5.5 L2.5 5.5" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
      </div>
      <div className={styles.title} data-tauri-drag-region>
        Agent Orchestrator
      </div>
    </div>
  );
}
