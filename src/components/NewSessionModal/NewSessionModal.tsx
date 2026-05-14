import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { SessionMode } from "../../types/session";
import styles from "./NewSessionModal.module.css";

const STORAGE_KEY = "ao-last-session-mode";
const VALID_MODES: SessionMode[] = ["claude-auto", "claude", "claude-skip", "claude-plan", "terminal"];

function getStoredMode(): SessionMode {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored && VALID_MODES.includes(stored as SessionMode)) {
    return stored as SessionMode;
  }
  return "claude";
}

interface NewSessionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreate: (name: string, cwd: string, sessionMode: SessionMode, pullLatest: boolean, isGitRepo: boolean) => void;
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
  const [sessionMode, setSessionMode] = useState<SessionMode>(getStoredMode);
  const [pullLatest, setPullLatest] = useState(false);
  const [isGitRepo, setIsGitRepo] = useState<boolean | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setName("");
      setDirectory(lastUsedDirectory);
      setSessionMode(getStoredMode());
      setPullLatest(false);
      setIsGitRepo(null);
      if (lastUsedDirectory) {
        invoke<boolean>("check_is_git_repo", { cwd: lastUsedDirectory })
          .then(setIsGitRepo)
          .catch(() => setIsGitRepo(false));
      }
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen, lastUsedDirectory]);

  useEffect(() => {
    if (!directory) {
      setIsGitRepo(null);
      return;
    }
    invoke<boolean>("check_is_git_repo", { cwd: directory })
      .then(setIsGitRepo)
      .catch(() => setIsGitRepo(false));
  }, [directory]);

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

  const effectivePullLatest = isGitRepo === false ? false : pullLatest;

  const handleCreate = () => {
    const trimmedName = name.trim();
    if (!trimmedName || !directory) return;
    localStorage.setItem(STORAGE_KEY, sessionMode);
    onCreate(trimmedName, directory, sessionMode, effectivePullLatest, isGitRepo ?? false);
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
              title={directory ?? undefined}
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

        <div className={styles.field}>
          <label className={styles.label} htmlFor="session-mode">
            Session Mode
          </label>
          <select
            id="session-mode"
            className={styles.select}
            value={sessionMode}
            onChange={(e) => setSessionMode(e.target.value as SessionMode)}
          >
            <option value="claude-auto">Claude (auto)</option>
            <option value="claude">Claude</option>
            <option value="claude-skip">Claude (skip permissions)</option>
            <option value="claude-plan">Claude (plan mode)</option>
            <option value="terminal">Terminal</option>
          </select>
        </div>

        <label className={`${styles.checkboxRow} ${isGitRepo === false ? styles.checkboxDisabled : ""}`}>
          <input
            type="checkbox"
            checked={effectivePullLatest}
            onChange={(e) => setPullLatest(e.target.checked)}
            disabled={isGitRepo === false}
            className={styles.checkbox}
          />
          <span className={styles.checkboxLabel}>Pull latest from main</span>
        </label>

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
