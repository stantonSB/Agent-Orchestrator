import type { SessionInfo } from "../../types/session";
import { SessionCard } from "../SessionCard/SessionCard";
import styles from "./ProjectGroup.module.css";

interface ProjectGroupProps {
  projectName: string;
  sessions: SessionInfo[];
  activeSessionId: string | null;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
  onSessionClick: (id: string) => void;
  onClose: (id: string) => Promise<void>;
  onDismiss: (id: string) => void;
  onRename?: (id: string, name: string) => void;
}

export function ProjectGroup({
  projectName,
  sessions,
  activeSessionId,
  isCollapsed,
  onToggleCollapse,
  onSessionClick,
  onClose,
  onDismiss,
  onRename,
}: ProjectGroupProps) {
  return (
    <div className={styles.group}>
      <div
        className={styles.header}
        onClick={onToggleCollapse}
        role="button"
        tabIndex={0}
        aria-expanded={!isCollapsed}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onToggleCollapse();
          }
        }}
      >
        <span className={`${styles.chevron} ${isCollapsed ? styles.collapsed : ""}`}>
          ▼
        </span>
        <span className={styles.projectName}>{projectName}</span>
        <div className={styles.divider} />
      </div>
      {!isCollapsed && (
        <div className={styles.sessions}>
          {sessions.map((session) => (
            <SessionCard
              key={session.id}
              session={session}
              isActive={session.id === activeSessionId}
              onClick={onSessionClick}
              onClose={onClose}
              onDismiss={onDismiss}
              onRename={onRename}
            />
          ))}
        </div>
      )}
    </div>
  );
}
