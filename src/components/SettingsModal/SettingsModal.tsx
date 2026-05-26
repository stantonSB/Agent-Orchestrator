import { useState, useEffect, useRef } from "react";
import styles from "./SettingsModal.module.css";

const STORAGE_KEY = "ao-default-session-name";
const DEFAULT_PATTERN = "Session {n}";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
  const [namePattern, setNamePattern] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setNamePattern(localStorage.getItem(STORAGE_KEY) ?? "");
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleSave = () => {
    const trimmed = namePattern.trim();
    if (trimmed) {
      localStorage.setItem(STORAGE_KEY, trimmed);
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
    if (e.key === "Enter") {
      handleSave();
    }
  };

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div
        className={styles.modal}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        tabIndex={-1}
      >
        <h2 className={styles.title}>Settings</h2>

        <div className={styles.field}>
          <label className={styles.label} htmlFor="name-pattern">
            Default Session Name
          </label>
          <input
            ref={inputRef}
            id="name-pattern"
            className={styles.input}
            type="text"
            placeholder={DEFAULT_PATTERN}
            value={namePattern}
            onChange={(e) => setNamePattern(e.target.value)}
            autoComplete="off"
          />
          <span className={styles.hint}>
            Use {"{n}"} for auto-incrementing number
          </span>
        </div>

        <div className={styles.actions}>
          <button
            className={styles.cancelButton}
            onClick={onClose}
            type="button"
          >
            Cancel
          </button>
          <button
            className={styles.saveButton}
            onClick={handleSave}
            type="button"
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
