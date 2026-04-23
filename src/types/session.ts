export type SessionStatus =
  | "starting"
  | "working"
  | "idle"
  | "needs_attention"
  | "finished"
  | "error";

export interface SessionInfo {
  id: string;
  name: string;
  status: SessionStatus;
  createdAt: number; // unix timestamp ms
  cwd: string; // working directory path
}

export interface SubagentStatus {
  id: string;
  index: number;
  status: SessionStatus;
  name: string | null;
}
