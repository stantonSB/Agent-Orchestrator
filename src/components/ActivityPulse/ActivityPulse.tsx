import styles from "./ActivityPulse.module.css";

interface ActivityPulseProps {
  active: boolean;
}

export function ActivityPulse({ active }: ActivityPulseProps) {
  if (!active) return null;
  return (
    <div className={styles.pulseContainer}>
      <div className={styles.pulseBar} />
    </div>
  );
}
