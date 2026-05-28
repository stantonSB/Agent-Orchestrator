import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { SessionMode } from "../../types/session";
import { useSessionStore } from "../../stores/sessionStore";
import styles from "./NewSessionModal.module.css";

const DEFAULT_NAME_STORAGE_KEY = "ao-default-session-name";
const DEFAULT_PATTERN = "Session {n}";

let sessionCounter = 0;

export function getNextSessionNumber(): number {
  return ++sessionCounter;
}

export function peekNextSessionNumber(): number {
  return sessionCounter + 1;
}

export function getDefaultSessionName(n: number): string {
  const pattern = localStorage.getItem(DEFAULT_NAME_STORAGE_KEY) || DEFAULT_PATTERN;
  return pattern.replace(/\{n\}/g, String(n));
}

// Only for tests
export function _resetCounterForTesting(): void {
  sessionCounter = 0;
}

const STORAGE_KEY = "ao-last-session-mode";
const DIR_STORAGE_KEY = "ao-last-directory";
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
  onCreate: (name: string, cwd: string, sessionMode: SessionMode, pullLatest: boolean, isGitRepo: boolean, parentSessionId?: string) => void;
  lastUsedDirectory: string | null;
}

export function NewSessionModal({
  isOpen,
  onClose,
  onCreate,
  lastUsedDirectory,
}: NewSessionModalProps) {
  const [name, setName] = useState("");
  const [defaultName, setDefaultName] = useState("");
  const [directory, setDirectory] = useState<string | null>(null);
  const [sessionMode, setSessionMode] = useState<SessionMode>(getStoredMode);
  const [pullLatest, setPullLatest] = useState(false);
  const [isGitRepo, setIsGitRepo] = useState<boolean | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [selectedWorktreeSessionId, setSelectedWorktreeSessionId] = useState<string | null>(null);

  const sessions = useSessionStore((s) => s.sessions);
  const worktreeSessions = useMemo(() => {
    if (sessionMode !== "terminal") return [];
    return Array.from(sessions.values()).filter(
      (s) =>
        s.sessionType === "claude" &&
        s.worktreeCwd &&
        s.status !== "exited"
    );
  }, [sessions, sessionMode]);

  useEffect(() => {
    if (isOpen) {
      setName("");
      setDefaultName(getDefaultSessionName(peekNextSessionNumber()));
      const initialDir = lastUsedDirectory ?? localStorage.getItem(DIR_STORAGE_KEY);
      setDirectory(initialDir);
      setSessionMode(getStoredMode());
      setPullLatest(false);
      setIsGitRepo(null);
      setSelectedWorktreeSessionId(null);
      if (initialDir) {
        invoke<boolean>("check_is_git_repo", { cwd: initialDir })
          .then(setIsGitRepo)
          .catch(() => setIsGitRepo(false));
      }
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen, lastUsedDirectory]);

  useEffect(() => {
    setSelectedWorktreeSessionId(null);
  }, [sessionMode]);

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

  const selectedWorktreeSession = selectedWorktreeSessionId
    ? sessions.get(selectedWorktreeSessionId)
    : null;
  const effectiveDirectory = selectedWorktreeSession?.worktreeCwd ?? directory;

  const handleCreate = () => {
    const trimmedName = name.trim();
    const finalName = trimmedName || getDefaultSessionName(getNextSessionNumber());
    if (!effectiveDirectory) return;
    localStorage.setItem(STORAGE_KEY, sessionMode);
    if (directory && !directory.includes("/.claude/worktrees/")) {
      localStorage.setItem(DIR_STORAGE_KEY, directory);
    }
    onCreate(
      finalName,
      effectiveDirectory,
      sessionMode,
      effectivePullLatest,
      isGitRepo ?? false,
      selectedWorktreeSessionId ?? undefined
    );
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
    if (e.key === "Enter" && effectiveDirectory) {
      handleCreate();
    }
  };

  const isValid = effectiveDirectory !== null;

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
            placeholder={defaultName}
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoComplete="off"
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label}>Project Directory</label>
          <div className={styles.folderRow}>
            <div
              className={`${styles.folderPath} ${effectiveDirectory ? styles.hasValue : ""} ${selectedWorktreeSessionId ? styles.disabled : ""}`}
              title={effectiveDirectory ?? undefined}
            >
              {effectiveDirectory ?? "No directory selected"}
            </div>
            <button
              className={styles.browseButton}
              onClick={handleBrowse}
              type="button"
              disabled={!!selectedWorktreeSessionId}
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

        {sessionMode === "terminal" && worktreeSessions.length > 0 && (
          <div className={styles.field}>
            <label className={styles.label} htmlFor="worktree-select">
              Worktree
            </label>
            <select
              id="worktree-select"
              className={styles.select}
              value={selectedWorktreeSessionId ?? ""}
              onChange={(e) => setSelectedWorktreeSessionId(e.target.value || null)}
            >
              <option value="">None</option>
              {worktreeSessions.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </div>
        )}

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
