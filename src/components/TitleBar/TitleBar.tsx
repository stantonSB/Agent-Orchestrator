import { getCurrentWindow } from "@tauri-apps/api/window";
import styles from "./TitleBar.module.css";

interface TitleBarProps {
  onSettingsClick: () => void;
}

export function TitleBar({ onSettingsClick }: TitleBarProps) {
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
      <div className={styles.rightControls}>
        <button
          className={styles.settingsButton}
          aria-label="Settings"
          onClick={onSettingsClick}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
        </button>
      </div>
    </div>
  );
}
