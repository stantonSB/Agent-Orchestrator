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
  created_at: number;
}

/**
 * Payload emitted by the Rust PTY manager when a session produces output.
 *
 * A base64-encoded string of the raw PTY bytes — one compact string rather
 * than a JSON array of integers. Decode with `decodeBase64` before writing to
 * the terminal.
 */
export type SessionOutputPayload = string;

/** Payload emitted when a session's child process exits. */
export interface SessionExitPayload {
  code: number | null;
}

// ---------------------------------------------------------------------------
// Event-name helpers  (match the Rust side's naming convention)
// ---------------------------------------------------------------------------

export function sessionOutputEvent(id: string): string {
  return `session-output-${id}`;
}

export function sessionStatusEvent(id: string): string {
  return `session-status-${id}`;
}

export function sessionExitEvent(id: string): string {
  return `session-exit-${id}`;
}
