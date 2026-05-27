import styles from "./DropOverlay.module.css";

export function DropOverlay() {
  return (
    <div className={styles.overlay}>
      <span className={styles.label}>Drop image here</span>
    </div>
  );
}
