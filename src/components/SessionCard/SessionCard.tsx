import { useState, useRef } from "react";
import { createPortal } from "react-dom";
import type { SessionInfo, SessionStatus } from "../../types/session";
import { ActivityPulse } from "../ActivityPulse/ActivityPulse";
import { DurationTimer } from "../DurationTimer/DurationTimer";
import { ContextMenu } from "../ContextMenu/ContextMenu";
import { CloseConfirmDialog } from "../CloseConfirmDialog/CloseConfirmDialog";
import styles from "./SessionCard.module.css";

interface SessionCardProps {
  session: SessionInfo;
  isActive: boolean;
  onClick: (id: string) => void;
  onClose?: (id: string) => void;
  onDismiss?: (id: string) => void;
  onRename?: (id: string, name: string) => void;
}

const STATUS_DOT_CLASS: Record<SessionStatus, string> = {
  starting: styles.statusStarting,
  working: styles.statusWorking,
  idle: styles.statusIdle,
  needs_attention: styles.statusNeedsAttention,
  finished: styles.statusFinished,
  error: styles.statusError,
  terminal: styles.statusTerminal,
};

const STATUS_LABEL: Record<SessionStatus, string> = {
  starting: "Starting...",
  working: "Working",
  idle: "Idle",
  needs_attention: "Needs Attention",
  finished: "Finished",
  error: "Error",
  terminal: "Terminal",
};

function isRunning(status: SessionStatus): boolean {
  return status !== "finished" && status !== "error";
}

export function SessionCard({ session, isActive, onClick, onClose, onDismiss, onRename }: SessionCardProps) {
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const savingRef = useRef(false);

  const cardClass = [
    styles.card,
    isActive ? styles.active : "",
    !isRunning(session.status) ? styles.dismissed : "",
  ]
    .filter(Boolean)
    .join(" ");

  function handleContextMenu(e: React.MouseEvent) {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
  }

  function getContextMenuItems() {
    const renameItem = { label: "Rename", onClick: () => { savingRef.current = false; setIsEditing(true); } };
    if (!isRunning(session.status)) {
      return [
        renameItem,
        { label: "Dismiss", onClick: () => setShowCloseConfirm(true) },
      ];
    }
    return [
      renameItem,
      { label: "Close Session", danger: true, onClick: () => setShowCloseConfirm(true) },
    ];
  }

  function handleRename(newName: string) {
    if (savingRef.current) return;
    savingRef.current = true;
    const trimmed = newName.trim();
    if (trimmed && trimmed.length <= 50 && trimmed !== session.name) {
      onRename?.(session.id, trimmed);
    }
    setIsEditing(false);
  }

  return (
    <>
      <div
        className={cardClass}
        onClick={() => onClick(session.id)}
        onContextMenu={handleContextMenu}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            onClick(session.id);
          }
        }}
      >
        {session.status === "finished" ? (
          <span className={styles.finishedIcon}>&#10003;</span>
        ) : (
          <span
            className={`${styles.statusDot} ${STATUS_DOT_CLASS[session.status]}`}
          />
        )}
        <div className={styles.info}>
          <div className={styles.nameRow}>
            {isEditing ? (
              <input
                className={styles.nameInput}
                defaultValue={session.name}
                maxLength={50}
                autoFocus
                onFocus={(e) => e.target.select()}
                onKeyDown={(e) => {
                  e.stopPropagation();
                  if (e.key === "Enter") {
                    handleRename(e.currentTarget.value);
                  } else if (e.key === "Escape") {
                    setIsEditing(false);
                  }
                }}
                onBlur={(e) => handleRename(e.target.value)}
                onClick={(e) => e.stopPropagation()}
              />
            ) : (
              <span
                className={styles.name}
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  savingRef.current = false;
                  setIsEditing(true);
                }}
              >
                {session.name}
              </span>
            )}
            {session.sessionType !== "terminal" && (
              <>
                <span
                  className={styles.worktreeIcon}
                  title={session.isGitRepo ? "Running in a git worktree" : "No worktree — not a git repository"}
                >
                  {session.isGitRepo ? "🌳" : "📁"}
                </span>
                <DurationTimer createdAt={session.createdAt} active={isRunning(session.status)} />
              </>
            )}
          </div>
          <span className={styles.status}>{STATUS_LABEL[session.status]}</span>
        </div>
        <ActivityPulse active={session.status === "working"} />
        <button
          className={styles.closeBtn}
          title={isRunning(session.status) ? "Close session" : "Dismiss session"}
          onClick={(e) => {
            e.stopPropagation();
            setShowCloseConfirm(true);
          }}
          aria-label="Close session"
        >
          &#x2715;
        </button>
      </div>

      {contextMenu &&
        createPortal(
          <ContextMenu
            x={contextMenu.x}
            y={contextMenu.y}
            items={getContextMenuItems()}
            onClose={() => setContextMenu(null)}
          />,
          document.body
        )}

      {showCloseConfirm &&
        createPortal(
          <CloseConfirmDialog
            sessionName={session.name}
            isRunning={isRunning(session.status)}
            onConfirm={() => {
              if (isRunning(session.status)) {
                onClose?.(session.id);
              } else {
                onDismiss?.(session.id);
              }
              setShowCloseConfirm(false);
            }}
            onCancel={() => setShowCloseConfirm(false)}
          />,
          document.body
        )}
    </>
  );
}
