import type { SubagentStatus, SessionStatus } from "../../types/session";
import styles from "./SubagentList.module.css";

interface SubagentListProps {
  subagents: SubagentStatus[];
}

const DOT_CLASS: Record<SessionStatus, string> = {
  starting: styles.statusStarting,
  working: styles.statusWorking,
  idle: styles.statusIdle,
  needs_attention: styles.statusNeedsAttention,
  finished: styles.statusFinished,
  error: styles.statusError,
};

export function SubagentList({ subagents }: SubagentListProps) {
  if (subagents.length === 0) return null;

  return (
    <div className={styles.list}>
      {subagents.map((agent) => (
        <div
          key={agent.id}
          className={`${styles.entry} ${agent.status === "finished" ? styles.finished : ""}`}
        >
          <span className={`${styles.dot} ${DOT_CLASS[agent.status]}`} />
          <span className={styles.name}>
            {agent.name ?? `Agent ${agent.index}`}
          </span>
        </div>
      ))}
    </div>
  );
}
