export type SessionStatus =
  | "starting"
  | "working"
  | "idle"
  | "needs_attention"
  | "finished"
  | "error"
  | "terminal";

export type SessionMode = "claude" | "claude-skip" | "claude-plan" | "terminal";

export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number; // unix timestamp ms
  cwd: string; // working directory path
  sessionType: "claude" | "terminal";
  isGitRepo: boolean;
}

export interface SubagentStatus {
  id: string;
  index: number;
  status: SessionStatus;
  name: string | null;
  created_at: number;
}
