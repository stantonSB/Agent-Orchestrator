import { useState } from "react";
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
}

const STATUS_DOT_CLASS: Record<SessionStatus, string> = {
  starting: styles.statusStarting,
  working: styles.statusWorking,
  idle: styles.statusIdle,
  needs_attention: styles.statusNeedsAttention,
  finished: styles.statusFinished,
  error: styles.statusError,
};

const STATUS_LABEL: Record<SessionStatus, string> = {
  starting: "Starting...",
  working: "Working",
  idle: "Idle",
  needs_attention: "Needs Attention",
  finished: "Finished",
  error: "Error",
};

function isRunning(status: SessionStatus): boolean {
  return status !== "finished" && status !== "error";
}

export function SessionCard({ session, isActive, onClick, onClose, onDismiss }: SessionCardProps) {
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);

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
    if (!isRunning(session.status)) {
      return [
        {
          label: "Dismiss",
          onClick: () => onDismiss?.(session.id),
        },
      ];
    }
    return [
      {
        label: "Close Session",
        danger: true,
        onClick: () => setShowCloseConfirm(true),
      },
    ];
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
            <span className={styles.name}>{session.name}</span>
            <DurationTimer createdAt={session.createdAt} active={isRunning(session.status)} />
          </div>
          <span className={styles.status}>{STATUS_LABEL[session.status]}</span>
        </div>
        <ActivityPulse active={session.status === "working"} />
        <button
          className={styles.closeBtn}
          title={isRunning(session.status) ? "Close session" : "Dismiss session"}
          onClick={(e) => {
            e.stopPropagation();
            if (isRunning(session.status)) {
              setShowCloseConfirm(true);
            } else {
              onDismiss?.(session.id);
            }
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
            onConfirm={() => {
              onClose?.(session.id);
              setShowCloseConfirm(false);
            }}
            onCancel={() => setShowCloseConfirm(false)}
          />,
          document.body
        )}
    </>
  );
}
