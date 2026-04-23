import { useState, useMemo, useCallback } from "react";
import type { CSSProperties } from "react";
import type { SessionInfo } from "../../types/session";
import { useSessionStore } from "../../stores/sessionStore";
import { ProjectGroup } from "../ProjectGroup/ProjectGroup";
import { NewSessionButton } from "../NewSessionButton/NewSessionButton";
import styles from "./SessionPanel.module.css";

interface SessionPanelProps {
  sessions: SessionInfo[];
  activeSessionId: string | null;
  onSessionClick: (id: string) => void;
  onNewSession: () => void;
  style?: CSSProperties;
}

function getProjectName(cwd: string): string {
  const segments = cwd.replace(/\/+$/, "").split("/");
  return segments[segments.length - 1] || cwd;
}

export interface ProjectGroupData {
  cwd: string;
  displayName: string;
  sessions: SessionInfo[];
  newestCreatedAt: number;
}

export function groupSessionsByProject(sessions: SessionInfo[]): ProjectGroupData[] {
  const groups = new Map<string, ProjectGroupData>();

  for (const session of sessions) {
    const existing = groups.get(session.cwd);
    if (existing) {
      existing.sessions.push(session);
      if (session.createdAt > existing.newestCreatedAt) {
        existing.newestCreatedAt = session.createdAt;
      }
    } else {
      groups.set(session.cwd, {
        cwd: session.cwd,
        displayName: getProjectName(session.cwd),
        sessions: [session],
        newestCreatedAt: session.createdAt,
      });
    }
  }

  // Sort groups by newest session (most recent first)
  const sorted = Array.from(groups.values()).sort(
    (a, b) => b.newestCreatedAt - a.newestCreatedAt
  );

  // Sort sessions within each group by createdAt descending
  for (const group of sorted) {
    group.sessions.sort((a, b) => b.createdAt - a.createdAt);
  }

  return sorted;
}

export function SessionPanel({
  sessions,
  activeSessionId,
  onSessionClick,
  onNewSession,
  style,
}: SessionPanelProps) {
  const closeSession = useSessionStore((s) => s.closeSession);
  const dismissSession = useSessionStore((s) => s.dismissSession);
  const renameSession = useSessionStore((s) => s.renameSession);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());

  const projectGroups = useMemo(() => groupSessionsByProject(sessions), [sessions]);

  const toggleCollapse = useCallback((cwd: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(cwd)) {
        next.delete(cwd);
      } else {
        next.add(cwd);
      }
      return next;
    });
  }, []);

  return (
    <div className={styles.panel} style={style}>
      <div className={styles.header}>Sessions</div>
      <NewSessionButton onClick={onNewSession} />
      {sessions.length === 0 ? (
        <div className={styles.empty}>No active sessions</div>
      ) : (
        <div className={styles.sessionList}>
          {projectGroups.map((group) => (
            <ProjectGroup
              key={group.cwd}
              projectName={group.displayName}
              sessions={group.sessions}
              activeSessionId={activeSessionId}
              isCollapsed={collapsedGroups.has(group.cwd)}
              onToggleCollapse={() => toggleCollapse(group.cwd)}
              onSessionClick={onSessionClick}
              onClose={closeSession}
              onDismiss={dismissSession}
              onRename={renameSession}
            />
          ))}
        </div>
      )}
    </div>
  );
}
