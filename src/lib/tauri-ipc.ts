import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  type SessionOutputPayload,
  type SessionExitPayload,
  sessionOutputEvent,
  sessionExitEvent,
} from "../types/tauri-events";

// ---------------------------------------------------------------------------
// Tauri command wrappers
// ---------------------------------------------------------------------------

export interface CreateSessionArgs {
  name: string;
  cwd: string;
  cols?: number;
  rows?: number;
  sessionType?: string;
  sessionMode?: string;
  isGitRepo?: boolean;
  pullLatest?: boolean;
}

export interface CloseSessionArgs {
  id: string;
}

export interface WriteToSessionArgs {
  id: string;
  data: number[];
}

export interface ResizeSessionArgs {
  id: string;
  cols: number;
  rows: number;
}

export interface RenameSessionArgs {
  id: string;
  name: string;
}

export interface SessionInfo {
  id: string;
  name: string;
  cwd: string;
  created_at_epoch_ms: number;
}

export async function createSession(args: CreateSessionArgs): Promise<string> {
  return invoke<string>("create_session", { ...args });
}

export async function closeSession(args: CloseSessionArgs): Promise<void> {
  return invoke<void>("close_session", { ...args });
}

export async function writeToSession(args: WriteToSessionArgs): Promise<void> {
  return invoke<void>("write_to_session", { ...args });
}

export async function resizeSession(args: ResizeSessionArgs): Promise<void> {
  return invoke<void>("resize_session", { ...args });
}

export async function renameSession(args: RenameSessionArgs): Promise<void> {
  return invoke<void>("rename_session", { ...args });
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke<SessionInfo[]>("list_sessions");
}

// ---------------------------------------------------------------------------
// Tauri event listeners
// ---------------------------------------------------------------------------

export async function onSessionOutput(
  sessionId: string,
  callback: (payload: SessionOutputPayload) => void,
): Promise<UnlistenFn> {
  return listen<SessionOutputPayload>(
    sessionOutputEvent(sessionId),
    (event) => callback(event.payload),
  );
}

export async function onSessionExit(
  sessionId: string,
  callback: (payload: SessionExitPayload) => void,
): Promise<UnlistenFn> {
  return listen<SessionExitPayload>(
    sessionExitEvent(sessionId),
    (event) => callback(event.payload),
  );
}
