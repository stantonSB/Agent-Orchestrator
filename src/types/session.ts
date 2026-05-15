export type SessionStatus =
  | "starting"
  | "working"
  | "idle"
  | "needs_attention"
  | "finished"
  | "error"
  | "terminal"
  | "exited";

export type SessionMode = "claude-auto" | "claude" | "claude-skip" | "claude-plan" | "terminal";

export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number; // unix timestamp ms
  cwd: string; // working directory path
  sessionType: "claude" | "terminal";
  isGitRepo: boolean;
  persisted?: boolean;
  scrollbackText?: string;
}

export interface SubagentStatus {
  id: string;
  index: number;
  status: SessionStatus;
  name: string | null;
  created_at: number;
}
