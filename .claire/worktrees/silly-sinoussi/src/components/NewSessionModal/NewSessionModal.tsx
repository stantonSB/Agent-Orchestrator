import { useState, useEffect, useRef } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import styles from "./NewSessionModal.module.css";

interface NewSessionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreate: (name: string, cwd: string) => void;
  lastUsedDirectory: string | null;
}

export function NewSessionModal({
  isOpen,
  onClose,
  onCreate,
  lastUsedDirectory,
}: NewSessionModalProps) {
  const [name, setName] = useState("");
  const [directory, setDirectory] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setName("");
      setDirectory(lastUsedDirectory);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen, lastUsedDirectory]);

  if (!isOpen) return null;

  const handleBrowse = async () => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Select project directory",
      defaultPath: directory ?? undefined,
    });
    if (typeof selected === "string") {
      setDirectory(selected);
    }
  };

  const handleCreate = () => {
    const trimmedName = name.trim();
    if (!trimmedName || !directory) return;
    onCreate(trimmedName, directory);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
    if (e.key === "Enter" && name.trim() && directory) {
      handleCreate();
    }
  };

  const isValid = name.trim().length > 0 && directory !== null;

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div
        className={styles.modal}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
        tabIndex={-1}
        ref={(el) => el?.focus()}
      >
        <h2 className={styles.title}>New Session</h2>

        <div className={styles.field}>
          <label className={styles.label} htmlFor="session-name">
            Session Name
          </label>
          <input
            ref={inputRef}
            id="session-name"
            className={styles.input}
            type="text"
            placeholder="e.g. fix-auth-bug"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoComplete="off"
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label}>Project Directory</label>
          <div className={styles.folderRow}>
            <div
              className={`${styles.folderPath} ${directory ? styles.hasValue : ""}`}
            >
              {directory ?? "No directory selected"}
            </div>
            <button
              className={styles.browseButton}
              onClick={handleBrowse}
              type="button"
            >
              Browse
            </button>
          </div>
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
            className={styles.createButton}
            onClick={handleCreate}
            disabled={!isValid}
            type="button"
          >
            Create
          </button>
        </div>
      </div>
    </div>
  );
}
