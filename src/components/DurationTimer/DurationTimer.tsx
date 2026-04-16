import { useEffect, useState } from "react";
import styles from "./DurationTimer.module.css";

interface DurationTimerProps {
  createdAt: number;
  active: boolean;
}

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
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (!active) return;
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [active]);

  const elapsed = (active ? now : Date.now()) - createdAt;

  return (
    <span className={`${styles.duration} ${active ? styles.active : styles.inactive}`}>
      {formatDuration(elapsed)}
    </span>
  );
}
