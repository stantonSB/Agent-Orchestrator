import { useSyncExternalStore } from "react";
import { subscribeNow, getNow } from "../../lib/sharedTicker";
import styles from "./DurationTimer.module.css";

interface DurationTimerProps {
  createdAt: number;
  active: boolean;
}

// Inactive timers never tick — subscribe to nothing.
const noopSubscribe = () => () => {};

function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
}

export function DurationTimer({ createdAt, active }: DurationTimerProps) {
  // All active timers share one 1 Hz ticker instead of each owning an interval.
  const now = useSyncExternalStore(
    active ? subscribeNow : noopSubscribe,
    getNow,
    getNow,
  );

  const elapsed = (active ? now : Date.now()) - createdAt;

  return (
    <span className={`${styles.duration} ${active ? styles.active : styles.inactive}`}>
      {formatDuration(elapsed)}
    </span>
  );
}
