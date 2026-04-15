import styles from "./NewSessionButton.module.css";

interface NewSessionButtonProps {
  onClick: () => void;
}

export function NewSessionButton({ onClick }: NewSessionButtonProps) {
  return (
    <button className={styles.button} onClick={onClick} type="button">
      <span className={styles.plus}>+</span>
      New Session
    </button>
  );
}
