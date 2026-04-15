# Wave 2: IPC Bridge & Terminal Integration

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect the Rust PTY backend to the React frontend so that sessions can be created, terminal output streams to xterm.js, and user keystrokes flow back to the PTY.

**Architecture:** Tauri commands (`create_session`, `close_session`, `write_to_session`, `resize_session`, `rename_session`, `list_sessions`) bridge the frontend to the PTY manager thread via mpsc channels with oneshot response channels. The PTY manager thread emits Tauri events (`session-output-{id}`, `session-status-{id}`, `session-exit-{id}`) that the frontend listens to and feeds into per-session xterm.js Terminal instances. Only the active session's terminal is attached to the DOM; inactive terminals stay in memory with a 10k line scrollback cap.

**Tech Stack:** Tauri 2 (Rust IPC commands + events), portable-pty, tokio mpsc/oneshot channels, React 18, xterm.js 5, @xterm/addon-fit, TypeScript, CSS Modules

---

## Assumed Wave 1 Outputs

This plan assumes the following files exist from Wave 1:

```
src-tauri/
  Cargo.toml              # Dependencies: tauri, portable-pty, serde, tokio, uuid
  src/
    main.rs               # Tauri entry point (calls tauri::Builder)
    lib.rs                 # Module declarations
    pty/
      mod.rs              # pub mod manager; pub mod types;
      types.rs            # PtyRequest, PtyResponse, SessionInfo, SessionStatus enums
      manager.rs          # PtyManager struct with run() loop, spawn/write/resize/close/list logic
src/
  main.tsx                # React entry point
  App.tsx                 # Two-pane layout shell
  App.module.css          # Layout styles
  components/
    TitleBar/
      TitleBar.tsx
      TitleBar.module.css
```

---

## Task 2A: Tauri Commands & Events

**Files to create:**
- `src-tauri/src/commands.rs` — All 6 Tauri command functions
- `src-tauri/src/state.rs` — AppState struct holding the channel sender
- `src-tauri/tests/commands_test.rs` — Integration tests

**Files to modify:**
- `src-tauri/src/main.rs` — Register commands, spawn PTY manager, store sender in Tauri state
- `src-tauri/src/lib.rs` — Declare new modules
- `src-tauri/src/pty/types.rs` — Add `Rename` variant to PtyRequest, ensure PtyResponse covers all cases
- `src-tauri/src/pty/manager.rs` — Handle Rename request, emit Tauri events from manager thread
- `src-tauri/Cargo.toml` — Ensure tokio features include `sync`

---

### Step 2A.1: Update PtyRequest/PtyResponse types to support all commands

- [ ] **Edit `src-tauri/src/pty/types.rs`** to ensure PtyRequest and PtyResponse cover all 6 commands:

```rust
// src-tauri/src/pty/types.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::oneshot;

pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Working,
    Idle,
    NeedsAttention,
    Finished,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub name: String,
    pub status: SessionStatus,
    pub created_at: u64, // unix timestamp ms
}

/// Messages sent from Tauri command handlers to the PTY manager thread.
pub enum PtyRequest {
    Create {
        name: String,
        cwd: PathBuf,
        reply: oneshot::Sender<PtyResponse>,
    },
    Write {
        id: SessionId,
        data: Vec<u8>,
        reply: oneshot::Sender<PtyResponse>,
    },
    Resize {
        id: SessionId,
        cols: u16,
        rows: u16,
        reply: oneshot::Sender<PtyResponse>,
    },
    Close {
        id: SessionId,
        reply: oneshot::Sender<PtyResponse>,
    },
    Rename {
        id: SessionId,
        name: String,
        reply: oneshot::Sender<PtyResponse>,
    },
    ListSessions {
        reply: oneshot::Sender<PtyResponse>,
    },
}

/// Responses sent back from the PTY manager thread to Tauri command handlers.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum PtyResponse {
    Created { id: SessionId },
    Ok,
    Sessions(Vec<SessionInfo>),
    Error { message: String },
}
```

- [ ] **Verify it compiles:**

```bash
cd src-tauri && cargo check 2>&1
# Expected: no errors related to pty::types
```

---

### Step 2A.2: Create the AppState struct

- [ ] **Create `src-tauri/src/state.rs`:**

```rust
// src-tauri/src/state.rs

use crate::pty::types::PtyRequest;
use tokio::sync::mpsc;

/// Shared application state managed by Tauri.
/// Holds the sender half of the channel to the PTY manager thread.
pub struct AppState {
    pub pty_tx: mpsc::Sender<PtyRequest>,
}
```

- [ ] **Edit `src-tauri/src/lib.rs`** to declare the new modules:

```rust
// src-tauri/src/lib.rs

pub mod commands;
pub mod pty;
pub mod state;
```

- [ ] **Verify it compiles:**

```bash
cd src-tauri && cargo check 2>&1
# Expected: may warn about unused imports, no errors
```

---

### Step 2A.3: Implement all 6 Tauri command functions

- [ ] **Create `src-tauri/src/commands.rs`:**

```rust
// src-tauri/src/commands.rs

use crate::pty::types::{PtyRequest, PtyResponse, SessionInfo};
use crate::state::AppState;
use tauri::State;
use tokio::sync::oneshot;

/// Helper: send a request to the PTY manager and await the response.
async fn send_request(
    state: &State<'_, AppState>,
    make_request: impl FnOnce(oneshot::Sender<PtyResponse>) -> PtyRequest,
) -> Result<PtyResponse, String> {
    let (tx, rx) = oneshot::channel();
    let request = make_request(tx);
    state
        .pty_tx
        .send(request)
        .await
        .map_err(|_| "PTY manager thread is not running".to_string())?;
    rx.await
        .map_err(|_| "PTY manager dropped the response channel".to_string())
}

#[tauri::command]
pub async fn create_session(
    state: State<'_, AppState>,
    name: String,
    cwd: String,
) -> Result<String, String> {
    let cwd_path = std::path::PathBuf::from(&cwd);
    if !cwd_path.exists() {
        return Err(format!("Directory does not exist: {}", cwd));
    }
    if !cwd_path.join(".git").exists() {
        return Err(format!("Directory is not a git repository: {}", cwd));
    }

    let response = send_request(&state, |reply| PtyRequest::Create {
        name,
        cwd: cwd_path,
        reply,
    })
    .await?;

    match response {
        PtyResponse::Created { id } => Ok(id),
        PtyResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from PTY manager".to_string()),
    }
}

#[tauri::command]
pub async fn close_session(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let response = send_request(&state, |reply| PtyRequest::Close { id, reply }).await?;

    match response {
        PtyResponse::Ok => Ok(()),
        PtyResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from PTY manager".to_string()),
    }
}

#[tauri::command]
pub async fn write_to_session(
    state: State<'_, AppState>,
    id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let response = send_request(&state, |reply| PtyRequest::Write { id, data, reply }).await?;

    match response {
        PtyResponse::Ok => Ok(()),
        PtyResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from PTY manager".to_string()),
    }
}

#[tauri::command]
pub async fn resize_session(
    state: State<'_, AppState>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let response =
        send_request(&state, |reply| PtyRequest::Resize { id, cols, rows, reply }).await?;

    match response {
        PtyResponse::Ok => Ok(()),
        PtyResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from PTY manager".to_string()),
    }
}

#[tauri::command]
pub async fn rename_session(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    let response =
        send_request(&state, |reply| PtyRequest::Rename { id, name, reply }).await?;

    match response {
        PtyResponse::Ok => Ok(()),
        PtyResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from PTY manager".to_string()),
    }
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<SessionInfo>, String> {
    let response =
        send_request(&state, |reply| PtyRequest::ListSessions { reply }).await?;

    match response {
        PtyResponse::Sessions(sessions) => Ok(sessions),
        PtyResponse::Error { message } => Err(message),
        _ => Err("Unexpected response from PTY manager".to_string()),
    }
}
```

- [ ] **Verify it compiles:**

```bash
cd src-tauri && cargo check 2>&1
# Expected: compiles without errors (warnings about unused are OK)
```

---

### Step 2A.4: Update PtyManager to handle Rename and emit Tauri events

- [ ] **Edit `src-tauri/src/pty/manager.rs`** to add the Rename handler inside the existing `run()` match block:

Add this arm to the `match request` block in the manager's run loop:

```rust
PtyRequest::Rename { id, name, reply } => {
    if let Some(session) = self.sessions.get_mut(&id) {
        session.name = name;
        let _ = reply.send(PtyResponse::Ok);
    } else {
        let _ = reply.send(PtyResponse::Error {
            message: format!("Session not found: {}", id),
        });
    }
}
```

- [ ] **Add Tauri event emission to the PTY output reader.** In the manager's stdout reading loop (where it reads bytes from the PTY), add event emission. The manager needs an `AppHandle` to emit events. Update the manager's constructor and `run` signature:

```rust
use tauri::{AppHandle, Emitter};

impl PtyManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            sessions: HashMap::new(),
            app_handle,
        }
    }
}
```

In the stdout reader loop (spawned per session), emit output events:

```rust
// Inside the per-session stdout reading thread/task:
// After reading bytes from the PTY stdout into `buf`:
let event_name = format!("session-output-{}", session_id);
let _ = app_handle.emit(&event_name, &buf[..n]);
```

When a session's status changes:

```rust
let event_name = format!("session-status-{}", session_id);
let _ = app_handle.emit(&event_name, &new_status);
```

When a session's child process exits:

```rust
let event_name = format!("session-exit-{}", session_id);
#[derive(Serialize, Clone)]
struct ExitPayload {
    code: Option<i32>,
}
let _ = app_handle.emit(&event_name, ExitPayload { code: exit_code });
```

- [ ] **Verify it compiles:**

```bash
cd src-tauri && cargo check 2>&1
# Expected: compiles cleanly
```

---

### Step 2A.5: Wire commands into Tauri builder in main.rs

- [ ] **Edit `src-tauri/src/main.rs`** to spawn the PTY manager, store the channel sender in state, and register all commands:

```rust
// src-tauri/src/main.rs

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod pty;
mod state;

use state::AppState;
use tokio::sync::mpsc;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let (pty_tx, pty_rx) = mpsc::channel(256);

            // Spawn the PTY manager on a dedicated thread
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime for PTY manager");

                rt.block_on(async move {
                    let mut manager = pty::manager::PtyManager::new(app_handle);
                    manager.run(pty_rx).await;
                });
            });

            app.manage(AppState { pty_tx });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_session,
            commands::close_session,
            commands::write_to_session,
            commands::resize_session,
            commands::rename_session,
            commands::list_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Agent Orchestrator");
}
```

- [ ] **Verify full build compiles:**

```bash
cd src-tauri && cargo check 2>&1
# Expected: compiles without errors
```

---

### Step 2A.6: Write integration test for command channel round-trip

- [ ] **Create `src-tauri/tests/commands_test.rs`:**

```rust
// src-tauri/tests/commands_test.rs

//! Integration test: verify PtyRequest/PtyResponse round-trip through channels.
//! This does NOT spawn a real PTY — it tests the channel plumbing only.
//!
//! Types are imported from the crate to ensure tests stay in sync with real code.
//! This requires lib.rs to re-export `pub mod pty;` (which it does — see Step 2A.2).

use agent_orchestrator::pty::types::*;
use tokio::sync::{mpsc, oneshot};

#[tokio::test]
async fn test_list_sessions_round_trip() {
    let (tx, mut rx) = mpsc::channel::<PtyRequest>(16);

    // Simulate the manager thread
    let manager_handle = tokio::spawn(async move {
        if let Some(request) = rx.recv().await {
            match request {
                PtyRequest::ListSessions { reply } => {
                    let _ = reply.send(PtyResponse::Sessions(vec![]));
                }
                _ => panic!("Unexpected request"),
            }
        }
    });

    // Simulate a Tauri command sending a request
    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(PtyRequest::ListSessions { reply: reply_tx })
        .await
        .expect("Failed to send request");

    let response = reply_rx.await.expect("Failed to receive response");
    match response {
        PtyResponse::Sessions(sessions) => assert!(sessions.is_empty()),
        other => panic!("Expected PtyResponse::Sessions, got {:?}", other),
    }

    manager_handle.await.unwrap();
}

#[tokio::test]
async fn test_rename_session_not_found() {
    let (tx, mut rx) = mpsc::channel::<PtyRequest>(16);

    let manager_handle = tokio::spawn(async move {
        if let Some(request) = rx.recv().await {
            match request {
                PtyRequest::Rename { id, reply, .. } => {
                    // Simulate "session not found"
                    let _ = reply.send(PtyResponse::Error {
                        message: format!("Session not found: {}", id),
                    });
                }
                _ => panic!("Unexpected request"),
            }
        }
    });

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(PtyRequest::Rename {
        id: "nonexistent".to_string(),
        name: "new-name".to_string(),
        reply: reply_tx,
    })
    .await
    .expect("Failed to send request");

    let response = reply_rx.await.expect("Failed to receive response");
    match response {
        PtyResponse::Error { message } => {
            assert_eq!(message, "Session not found: nonexistent");
        }
        other => panic!("Expected PtyResponse::Error, got {:?}", other),
    }

    manager_handle.await.unwrap();
}
```

- [ ] **Run the tests:**

```bash
cd src-tauri && cargo test --test commands_test 2>&1
# Expected:
# running 2 tests
# test test_list_sessions_round_trip ... ok
# test test_rename_session_not_found ... ok
# test result: ok. 2 passed; 0 failed
```

---

### Step 2A.7: Verify all 6 commands are registered

- [ ] **Run a full cargo build to ensure everything links:**

```bash
cd src-tauri && cargo build 2>&1
# Expected: Compiles successfully. Warnings about unused code are acceptable.
```

- [ ] **Commit Task 2A:**

```bash
git add src-tauri/src/commands.rs src-tauri/src/state.rs src-tauri/src/pty/types.rs \
       src-tauri/src/pty/manager.rs src-tauri/src/main.rs src-tauri/src/lib.rs \
       src-tauri/tests/commands_test.rs
git commit -m "feat(2A): Tauri IPC commands and event emission for PTY bridge"
```

---

## Task 2B: xterm.js Component Shell

**Files to create:**
- `src/types/tauri-events.ts` — TypeScript interfaces for all Tauri events and IPC
- `src/components/XTermInstance/XTermInstance.tsx` — xterm.js terminal component
- `src/components/XTermInstance/XTermInstance.module.css` — Terminal container styles
- `src/components/XTermInstance/useTerminal.ts` — Hook encapsulating xterm.js lifecycle
- `src/components/TerminalArea/TerminalArea.tsx` — Container that manages XTermInstance mount/unmount
- `src/components/TerminalArea/TerminalArea.module.css` — Terminal area styles

**Files to modify:**
- `src/App.tsx` — Mount TerminalArea in the layout
- `package.json` — Add xterm.js dependencies (if not already present from Wave 1)

---

### Step 2B.1: Install xterm.js dependencies

- [ ] **Install xterm.js and addons:**

```bash
npm install @xterm/xterm @xterm/addon-fit @xterm/addon-web-links 2>&1
# Expected: added 3 packages
```

- [ ] **Verify installation:**

```bash
npx tsc --noEmit 2>&1
# Expected: no errors (confirms @xterm/xterm types resolve correctly)
```

---

### Step 2B.2: Define TypeScript interfaces for Tauri events

- [ ] **Create `src/types/tauri-events.ts`:**

```typescript
// src/types/tauri-events.ts

/**
 * TypeScript interfaces for Tauri IPC commands and events.
 * These mirror the Rust types defined in src-tauri/src/pty/types.rs.
 */

// --- Session types ---

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
  created_at: number; // unix timestamp ms
}

// --- Tauri command argument types ---

export interface CreateSessionArgs {
  name: string;
  cwd: string;
}

export interface WriteToSessionArgs {
  id: string;
  data: number[]; // Vec<u8> becomes number[] in TS
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

export interface CloseSessionArgs {
  id: string;
}

// --- Tauri event payload types ---

/** Payload for session-output-{id} events. Raw bytes from PTY stdout. */
export interface SessionOutputPayload {
  /** Raw bytes encoded as a number array. */
  data: number[];
}

/** Payload for session-status-{id} events. */
export interface SessionStatusPayload {
  status: SessionStatus;
}

/** Payload for session-exit-{id} events. */
export interface SessionExitPayload {
  code: number | null;
}

// --- Tauri event name helpers ---

export function sessionOutputEvent(id: string): string {
  return `session-output-${id}`;
}

export function sessionStatusEvent(id: string): string {
  return `session-status-${id}`;
}

export function sessionExitEvent(id: string): string {
  return `session-exit-${id}`;
}
```

- [ ] **Verify TypeScript compiles:**

```bash
npx tsc --noEmit 2>&1
# Expected: no errors
```

---

### Step 2B.3: Create the useTerminal hook

- [ ] **Create `src/components/XTermInstance/useTerminal.ts`:**

```typescript
// src/components/XTermInstance/useTerminal.ts

import { useEffect, useRef, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";

export interface UseTerminalOptions {
  /** Session ID this terminal belongs to. */
  sessionId: string;
  /** Called when the user types into the terminal. */
  onData?: (data: string) => void;
  /** Called when the terminal resizes (e.g., window resize). */
  onResize?: (cols: number, rows: number) => void;
  /** If true, feed mock data for testing without a backend. */
  mockMode?: boolean;
}

export interface UseTerminalReturn {
  /** Ref to attach to the container div. */
  containerRef: React.RefObject<HTMLDivElement | null>;
  /** Write raw data (string or Uint8Array) into the terminal. */
  write: (data: string | Uint8Array) => void;
  /** Get the Terminal instance (for advanced use). */
  getTerminal: () => Terminal | null;
  /** Trigger a fit/resize. */
  fit: () => void;
}

const TERMINAL_OPTIONS = {
  cursorBlink: true,
  cursorStyle: "block" as const,
  fontSize: 13,
  fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
  lineHeight: 1.2,
  scrollback: 10_000,
  theme: {
    background: "#1a1b26",
    foreground: "#a9b1d6",
    cursor: "#c0caf5",
    selectionBackground: "#33467c",
    black: "#32344a",
    red: "#f7768e",
    green: "#9ece6a",
    yellow: "#e0af68",
    blue: "#7aa2f7",
    magenta: "#ad8ee6",
    cyan: "#449dab",
    white: "#787c99",
    brightBlack: "#444b6a",
    brightRed: "#ff7a93",
    brightGreen: "#b9f27c",
    brightYellow: "#ff9e64",
    brightBlue: "#7da6ff",
    brightMagenta: "#bb9af7",
    brightCyan: "#0db9d7",
    brightWhite: "#acb0d0",
  },
};

export function useTerminal(options: UseTerminalOptions): UseTerminalReturn {
  const { sessionId, onData, onResize, mockMode = false } = options;
  const containerRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const mockIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Initialize terminal
  useEffect(() => {
    const terminal = new Terminal(TERMINAL_OPTIONS);
    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();

    terminal.loadAddon(fitAddon);
    terminal.loadAddon(webLinksAddon);

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    // Listen for user input
    terminal.onData((data) => {
      onData?.(data);
    });

    // Listen for terminal resize
    terminal.onResize(({ cols, rows }) => {
      onResize?.(cols, rows);
    });

    // Attach to DOM if container is ready
    if (containerRef.current) {
      terminal.open(containerRef.current);
      // Small delay to ensure DOM is laid out before fitting
      requestAnimationFrame(() => {
        fitAddon.fit();
      });
    }

    // Set up ResizeObserver for auto-fit
    const observer = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        if (fitAddonRef.current) {
          try {
            fitAddonRef.current.fit();
          } catch {
            // Container may not be visible; ignore
          }
        }
      });
    });

    if (containerRef.current) {
      observer.observe(containerRef.current);
    }
    resizeObserverRef.current = observer;

    // Mock mode: simulate output for testing
    if (mockMode) {
      terminal.writeln(
        `\x1b[1;34m[Mock Mode]\x1b[0m Session "${sessionId}" initialized.`
      );
      terminal.writeln(
        "\x1b[1;34m[Mock Mode]\x1b[0m Type anything — input will echo locally.\r\n"
      );

      // Echo user input in mock mode
      terminal.onData((data) => {
        // Echo typed characters (handle Enter specially)
        if (data === "\r") {
          terminal.writeln("");
        } else if (data === "\x7f") {
          // Backspace
          terminal.write("\b \b");
        } else {
          terminal.write(data);
        }
      });

      // Simulate periodic output
      const mockMessages = [
        "\x1b[33m⠋ Thinking...\x1b[0m",
        "\x1b[32m✓ File updated: src/App.tsx\x1b[0m",
        "\x1b[36mI'll help you with that. Let me check the codebase.\x1b[0m",
        "\x1b[33m⠙ Reading files...\x1b[0m",
      ];
      let msgIndex = 0;
      mockIntervalRef.current = setInterval(() => {
        if (terminalRef.current) {
          terminalRef.current.writeln(mockMessages[msgIndex % mockMessages.length]);
          msgIndex++;
        }
      }, 3000);
    }

    // Cleanup
    return () => {
      if (mockIntervalRef.current) {
        clearInterval(mockIntervalRef.current);
        mockIntervalRef.current = null;
      }
      observer.disconnect();
      resizeObserverRef.current = null;
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
    // sessionId and mockMode are stable for the lifetime of a session
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, mockMode]);

  const write = useCallback((data: string | Uint8Array) => {
    terminalRef.current?.write(data);
  }, []);

  const getTerminal = useCallback(() => {
    return terminalRef.current;
  }, []);

  const fit = useCallback(() => {
    try {
      fitAddonRef.current?.fit();
    } catch {
      // ignore if container is hidden
    }
  }, []);

  return { containerRef, write, getTerminal, fit };
}
```

- [ ] **Verify TypeScript compiles:**

```bash
npx tsc --noEmit 2>&1
# Expected: no errors
```

---

### Step 2B.4: Create the XTermInstance component

- [ ] **Create `src/components/XTermInstance/XTermInstance.module.css`:**

```css
/* src/components/XTermInstance/XTermInstance.module.css */

.terminalContainer {
  width: 100%;
  height: 100%;
  overflow: hidden;
  background-color: #1a1b26;
}

.terminalContainer :global(.xterm) {
  height: 100%;
  padding: 4px;
}

.terminalContainer :global(.xterm-viewport) {
  overflow-y: auto;
}
```

- [ ] **Create `src/components/XTermInstance/XTermInstance.tsx`:**

```tsx
// src/components/XTermInstance/XTermInstance.tsx

import { useEffect, forwardRef, useImperativeHandle } from "react";
import { useTerminal, UseTerminalOptions } from "./useTerminal";
import styles from "./XTermInstance.module.css";

export interface XTermInstanceHandle {
  /** Write data into the terminal (raw bytes or string). */
  write: (data: string | Uint8Array) => void;
  /** Trigger a fit/resize of the terminal to its container. */
  fit: () => void;
}

export interface XTermInstanceProps {
  /** Session ID this terminal is bound to. */
  sessionId: string;
  /** Called when the user types into the terminal. */
  onData?: (data: string) => void;
  /** Called when the terminal is resized by the user/window. */
  onResize?: (cols: number, rows: number) => void;
  /** If true, runs in mock mode with simulated output. */
  mockMode?: boolean;
  /** Whether this terminal is the active/visible one. */
  isActive: boolean;
}

export const XTermInstance = forwardRef<XTermInstanceHandle, XTermInstanceProps>(
  function XTermInstance({ sessionId, onData, onResize, mockMode, isActive }, ref) {
    const terminalOptions: UseTerminalOptions = {
      sessionId,
      onData,
      onResize,
      mockMode,
    };

    const { containerRef, write, fit } = useTerminal(terminalOptions);

    // Expose write and fit to parent via ref
    useImperativeHandle(
      ref,
      () => ({
        write,
        fit,
      }),
      [write, fit]
    );

    // Re-fit when becoming active (container may have changed size while hidden)
    useEffect(() => {
      if (isActive) {
        // Delay to allow DOM to update
        const timer = setTimeout(() => fit(), 50);
        return () => clearTimeout(timer);
      }
    }, [isActive, fit]);

    return (
      <div
        ref={containerRef}
        className={styles.terminalContainer}
        style={{ display: isActive ? "block" : "none" }}
      />
    );
  }
);
```

- [ ] **Verify TypeScript compiles:**

```bash
npx tsc --noEmit 2>&1
# Expected: no errors
```

---

### Step 2B.5: Create the TerminalArea container component

- [ ] **Create `src/components/TerminalArea/TerminalArea.module.css`:**

```css
/* src/components/TerminalArea/TerminalArea.module.css */

.terminalArea {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background-color: #1a1b26;
  position: relative;
}

.placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #565f89;
  font-family: "SF Mono", "Menlo", "Monaco", monospace;
  font-size: 14px;
  gap: 12px;
}

.placeholderIcon {
  font-size: 48px;
  opacity: 0.3;
}

.placeholderText {
  color: #565f89;
}

.placeholderHint {
  color: #3b3f5c;
  font-size: 12px;
}
```

- [ ] **Create `src/components/TerminalArea/TerminalArea.tsx`:**

```tsx
// src/components/TerminalArea/TerminalArea.tsx

import { useRef, useCallback } from "react";
import {
  XTermInstance,
  XTermInstanceHandle,
} from "../XTermInstance/XTermInstance";
import styles from "./TerminalArea.module.css";

export interface TerminalSession {
  id: string;
  name: string;
}

export interface TerminalAreaProps {
  /** All active sessions that need terminal instances. */
  sessions: TerminalSession[];
  /** The currently active (visible) session ID. */
  activeSessionId: string | null;
  /** Called when the user types in the active terminal. */
  onSessionData?: (sessionId: string, data: string) => void;
  /** Called when a terminal resizes. */
  onSessionResize?: (sessionId: string, cols: number, rows: number) => void;
  /** If true, all terminals run in mock mode. */
  mockMode?: boolean;
}

export function TerminalArea({
  sessions,
  activeSessionId,
  onSessionData,
  onSessionResize,
  mockMode = false,
}: TerminalAreaProps) {
  // Map of session ID -> XTermInstanceHandle ref
  const terminalRefs = useRef<Map<string, XTermInstanceHandle>>(new Map());

  const setTerminalRef = useCallback(
    (sessionId: string) => (handle: XTermInstanceHandle | null) => {
      if (handle) {
        terminalRefs.current.set(sessionId, handle);
      } else {
        terminalRefs.current.delete(sessionId);
      }
    },
    []
  );

  /** Write data to a specific session's terminal (used by event listeners). */
  const writeToTerminal = useCallback(
    (sessionId: string, data: string | Uint8Array) => {
      const handle = terminalRefs.current.get(sessionId);
      handle?.write(data);
    },
    []
  );

  // Expose writeToTerminal on the component for parent access
  // (In 2C this will be called from Tauri event listeners)
  // For now, store it on window for debugging in mock mode
  if (typeof window !== "undefined") {
    (window as unknown as Record<string, unknown>).__writeToTerminal =
      writeToTerminal;
  }

  if (sessions.length === 0) {
    return (
      <div className={styles.terminalArea}>
        <div className={styles.placeholder}>
          <div className={styles.placeholderIcon}>&#9654;</div>
          <div className={styles.placeholderText}>No active sessions</div>
          <div className={styles.placeholderHint}>
            Click &quot;+ New Session&quot; to start a Claude agent
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.terminalArea}>
      {sessions.map((session) => (
        <XTermInstance
          key={session.id}
          ref={setTerminalRef(session.id)}
          sessionId={session.id}
          isActive={session.id === activeSessionId}
          mockMode={mockMode}
          onData={(data) => onSessionData?.(session.id, data)}
          onResize={(cols, rows) =>
            onSessionResize?.(session.id, cols, rows)
          }
        />
      ))}
    </div>
  );
}
```

- [ ] **Verify TypeScript compiles:**

```bash
npx tsc --noEmit 2>&1
# Expected: no errors
```

---

### Step 2B.6: Mount TerminalArea in App.tsx with mock mode

- [ ] **Edit `src/App.tsx`** to add the TerminalArea with a mock session for visual testing:

```tsx
// src/App.tsx

import { useState } from "react";
import { TitleBar } from "./components/TitleBar/TitleBar";
import { TerminalArea, TerminalSession } from "./components/TerminalArea/TerminalArea";
import styles from "./App.module.css";

// Mock sessions for development — will be replaced in Wave 3 with Zustand store
const MOCK_SESSIONS: TerminalSession[] = [
  { id: "mock-1", name: "Mock Session 1" },
];

function App() {
  const [activeSessionId] = useState<string | null>("mock-1");

  return (
    <div className={styles.app}>
      <TitleBar />
      <div className={styles.mainContent}>
        <TerminalArea
          sessions={MOCK_SESSIONS}
          activeSessionId={activeSessionId}
          mockMode={true}
          onSessionData={(id, data) => {
            console.log(`[Session ${id}] Input:`, data);
          }}
          onSessionResize={(id, cols, rows) => {
            console.log(`[Session ${id}] Resize: ${cols}x${rows}`);
          }}
        />
        <div className={styles.sessionPanel}>
          {/* Session panel placeholder — built in Wave 3 */}
          <div style={{ padding: "16px", color: "#565f89", fontSize: "13px" }}>
            Session Panel (Wave 3)
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
```

- [ ] **Verify the app builds:**

```bash
npm run build 2>&1
# Expected: Build completes successfully
```

---

### Step 2B.7: Visual smoke test in dev mode

- [ ] **Start the dev server and verify the terminal renders:**

```bash
npm run tauri dev 2>&1
# Expected:
# - Window opens with title bar and two-pane layout
# - Left pane shows an xterm.js terminal with "[Mock Mode]" messages
# - Typing in the terminal echoes characters
# - Simulated output appears every 3 seconds
# - Resizing the window causes the terminal to refit
```

**Note — automated tests for xterm.js deferred:** Unit/integration tests for `useTerminal` and `XTermInstance` are intentionally deferred from Wave 2. xterm.js requires a real DOM with a canvas context, which means jsdom alone is insufficient — tests would need either a browser-based runner (Playwright component tests) or heavy mocking that provides little confidence. The visual smoke test in Step 2B.7 covers basic rendering. Proper component tests will be added in Wave 3 alongside Playwright E2E tests where a real browser context is available.

- [ ] **Commit Task 2B:**

```bash
git add src/types/tauri-events.ts \
       src/components/XTermInstance/XTermInstance.tsx \
       src/components/XTermInstance/XTermInstance.module.css \
       src/components/XTermInstance/useTerminal.ts \
       src/components/TerminalArea/TerminalArea.tsx \
       src/components/TerminalArea/TerminalArea.module.css \
       src/App.tsx \
       package.json package-lock.json
git commit -m "feat(2B): xterm.js component shell with mock mode and resize observer"
```

---

## Task 2C: Connect Frontend to Backend

**Depends on:** Tasks 2A and 2B both complete.

**Files to create:**
- `src/lib/tauri-ipc.ts` — Typed wrappers around Tauri `invoke` and `listen`

**Files to modify:**
- `src/components/TerminalArea/TerminalArea.tsx` — Add real Tauri event listeners
- `src/App.tsx` — Remove mock mode, use real IPC calls

---

### Step 2C.1: Create typed Tauri IPC wrapper

- [ ] **Create `src/lib/tauri-ipc.ts`:**

```typescript
// src/lib/tauri-ipc.ts

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  SessionInfo,
  CreateSessionArgs,
  WriteToSessionArgs,
  ResizeSessionArgs,
  RenameSessionArgs,
  CloseSessionArgs,
  SessionOutputPayload,
  SessionStatusPayload,
  SessionExitPayload,
  sessionOutputEvent,
  sessionStatusEvent,
  sessionExitEvent,
} from "../types/tauri-events";

// --- Tauri command wrappers ---

export async function createSession(args: CreateSessionArgs): Promise<string> {
  return invoke<string>("create_session", args);
}

export async function closeSession(args: CloseSessionArgs): Promise<void> {
  return invoke<void>("close_session", args);
}

export async function writeToSession(args: WriteToSessionArgs): Promise<void> {
  return invoke<void>("write_to_session", args);
}

export async function resizeSession(args: ResizeSessionArgs): Promise<void> {
  return invoke<void>("resize_session", args);
}

export async function renameSession(args: RenameSessionArgs): Promise<void> {
  return invoke<void>("rename_session", args);
}

export async function listSessions(): Promise<SessionInfo[]> {
  return invoke<SessionInfo[]>("list_sessions");
}

// --- Tauri event listeners ---

export async function onSessionOutput(
  sessionId: string,
  callback: (payload: SessionOutputPayload) => void
): Promise<UnlistenFn> {
  return listen<SessionOutputPayload>(sessionOutputEvent(sessionId), (event) => {
    callback(event.payload);
  });
}

export async function onSessionStatus(
  sessionId: string,
  callback: (payload: SessionStatusPayload) => void
): Promise<UnlistenFn> {
  return listen<SessionStatusPayload>(sessionStatusEvent(sessionId), (event) => {
    callback(event.payload);
  });
}

export async function onSessionExit(
  sessionId: string,
  callback: (payload: SessionExitPayload) => void
): Promise<UnlistenFn> {
  return listen<SessionExitPayload>(sessionExitEvent(sessionId), (event) => {
    callback(event.payload);
  });
}
```

- [ ] **Verify TypeScript compiles:**

```bash
npx tsc --noEmit 2>&1
# Expected: no errors
```

---

### Step 2C.2: Update TerminalArea to wire Tauri events to terminals

- [ ] **Edit `src/components/TerminalArea/TerminalArea.tsx`** — replace the full file to add event listener lifecycle:

```tsx
// src/components/TerminalArea/TerminalArea.tsx

import { useRef, useCallback, useEffect } from "react";
import {
  XTermInstance,
  XTermInstanceHandle,
} from "../XTermInstance/XTermInstance";
import {
  onSessionOutput,
  onSessionStatus,
  onSessionExit,
  writeToSession,
  resizeSession,
} from "../../lib/tauri-ipc";
import { SessionStatusPayload, SessionExitPayload } from "../../types/tauri-events";
import styles from "./TerminalArea.module.css";

export interface TerminalSession {
  id: string;
  name: string;
}

export interface TerminalAreaProps {
  /** All active sessions that need terminal instances. */
  sessions: TerminalSession[];
  /** The currently active (visible) session ID. */
  activeSessionId: string | null;
  /** Called when a session's status changes. */
  onSessionStatusChange?: (sessionId: string, payload: SessionStatusPayload) => void;
  /** Called when a session's process exits. */
  onSessionExit?: (sessionId: string, payload: SessionExitPayload) => void;
  /** If true, all terminals run in mock mode (no Tauri IPC). */
  mockMode?: boolean;
}

export function TerminalArea({
  sessions,
  activeSessionId,
  onSessionStatusChange,
  onSessionExit: onSessionExitProp,
  mockMode = false,
}: TerminalAreaProps) {
  const terminalRefs = useRef<Map<string, XTermInstanceHandle>>(new Map());

  const setTerminalRef = useCallback(
    (sessionId: string) => (handle: XTermInstanceHandle | null) => {
      if (handle) {
        terminalRefs.current.set(sessionId, handle);
      } else {
        terminalRefs.current.delete(sessionId);
      }
    },
    []
  );

  // Set up Tauri event listeners for each session.
  // Uses an async IIFE with a `cancelled` guard so that cleanup races
  // against the listener setup are handled correctly — if the effect
  // re-runs before all promises resolve, the stale listeners are still
  // unsubscribed once their promises settle.
  useEffect(() => {
    if (mockMode) return;

    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      for (const session of sessions) {
        const sid = session.id;

        const [unlistenOutput, unlistenStatus, unlistenExit] =
          await Promise.all([
            // Listen for PTY output and write to the terminal
            onSessionOutput(sid, (payload) => {
              if (cancelled) return;
              const handle = terminalRefs.current.get(sid);
              if (handle && payload.data) {
                const bytes = new Uint8Array(payload.data);
                handle.write(bytes);
              }
            }),
            // Listen for status changes
            onSessionStatus(sid, (payload) => {
              if (cancelled) return;
              onSessionStatusChange?.(sid, payload);
            }),
            // Listen for process exit
            onSessionExit(sid, (payload) => {
              if (cancelled) return;
              onSessionExitProp?.(sid, payload);
            }),
          ]);

        unlisteners.push(unlistenOutput, unlistenStatus, unlistenExit);
      }

      // If the effect was cleaned up while we were awaiting, tear down now.
      if (cancelled) {
        for (const unlisten of unlisteners) {
          unlisten();
        }
      }
    })();

    return () => {
      cancelled = true;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [sessions, mockMode, onSessionStatusChange, onSessionExitProp]);

  // Handle user input: forward keystrokes to PTY via IPC
  const handleSessionData = useCallback(
    (sessionId: string, data: string) => {
      if (mockMode) {
        console.log(`[Mock] Session ${sessionId} input:`, data);
        return;
      }
      const encoder = new TextEncoder();
      const bytes = Array.from(encoder.encode(data));
      writeToSession({ id: sessionId, data: bytes }).catch((err) => {
        console.error(`Failed to write to session ${sessionId}:`, err);
      });
    },
    [mockMode]
  );

  // Handle terminal resize: update PTY dimensions via IPC
  const handleSessionResize = useCallback(
    (sessionId: string, cols: number, rows: number) => {
      if (mockMode) {
        console.log(`[Mock] Session ${sessionId} resize: ${cols}x${rows}`);
        return;
      }
      resizeSession({ id: sessionId, cols, rows }).catch((err) => {
        console.error(`Failed to resize session ${sessionId}:`, err);
      });
    },
    [mockMode]
  );

  if (sessions.length === 0) {
    return (
      <div className={styles.terminalArea}>
        <div className={styles.placeholder}>
          <div className={styles.placeholderIcon}>&#9654;</div>
          <div className={styles.placeholderText}>No active sessions</div>
          <div className={styles.placeholderHint}>
            Click &quot;+ New Session&quot; to start a Claude agent
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.terminalArea}>
      {sessions.map((session) => (
        <XTermInstance
          key={session.id}
          ref={setTerminalRef(session.id)}
          sessionId={session.id}
          isActive={session.id === activeSessionId}
          mockMode={mockMode}
          onData={(data) => handleSessionData(session.id, data)}
          onResize={(cols, rows) => handleSessionResize(session.id, cols, rows)}
        />
      ))}
    </div>
  );
}
```

- [ ] **Verify TypeScript compiles:**

```bash
npx tsc --noEmit 2>&1
# Expected: no errors
```

---

### Step 2C.3: Update App.tsx for real IPC mode

- [ ] **Edit `src/App.tsx`** — add a dev toggle between mock and real mode, and wire up session creation:

```tsx
// src/App.tsx

import { useState, useCallback } from "react";
import { TitleBar } from "./components/TitleBar/TitleBar";
import {
  TerminalArea,
  TerminalSession,
} from "./components/TerminalArea/TerminalArea";
import { createSession, closeSession } from "./lib/tauri-ipc";
import { SessionStatusPayload, SessionExitPayload } from "./types/tauri-events";
import styles from "./App.module.css";

// Toggle for development: set to true to use mock terminals without a backend
const MOCK_MODE = false;

// Mock sessions only used when MOCK_MODE is true
const MOCK_SESSIONS: TerminalSession[] = MOCK_MODE
  ? [{ id: "mock-1", name: "Mock Session 1" }]
  : [];

function App() {
  const [sessions, setSessions] = useState<TerminalSession[]>(MOCK_SESSIONS);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(
    MOCK_MODE ? "mock-1" : null
  );

  const handleCreateSession = useCallback(async () => {
    if (MOCK_MODE) {
      const id = `mock-${Date.now()}`;
      setSessions((prev) => [...prev, { id, name: `Session ${prev.length + 1}` }]);
      setActiveSessionId(id);
      return;
    }

    try {
      // For now, use a default name and cwd. Wave 3 adds the NewSessionModal.
      const name = `Session ${sessions.length + 1}`;
      // NOTE: "~" is NOT expanded by Rust's PathBuf — it would fail the
      // `cwd_path.exists()` validation in create_session.  We use /tmp as a
      // safe default that always exists.  Wave 3 replaces this with a folder
      // picker that supplies a real absolute path.
      const cwd = "/tmp";
      const id = await createSession({ name, cwd });
      setSessions((prev) => [...prev, { id, name }]);
      setActiveSessionId(id);
    } catch (err) {
      console.error("Failed to create session:", err);
    }
  }, [sessions.length]);

  const handleCloseSession = useCallback(
    async (sessionId: string) => {
      if (!MOCK_MODE) {
        try {
          await closeSession({ id: sessionId });
        } catch (err) {
          console.error("Failed to close session:", err);
        }
      }
      setSessions((prev) => {
        const remaining = prev.filter((s) => s.id !== sessionId);
        // Update activeSessionId in the same updater to avoid stale closure.
        // If we just closed the active session, switch to the first remaining
        // session (or null if none left).
        if (activeSessionId === sessionId) {
          setActiveSessionId(remaining.length > 0 ? remaining[0].id : null);
        }
        return remaining;
      });
    },
    [activeSessionId]
  );

  const handleStatusChange = useCallback(
    (sessionId: string, payload: SessionStatusPayload) => {
      console.log(`[Session ${sessionId}] Status:`, payload.status);
      // Wave 3 will update the Zustand store here
    },
    []
  );

  const handleSessionExit = useCallback(
    (sessionId: string, payload: SessionExitPayload) => {
      console.log(`[Session ${sessionId}] Exited with code:`, payload.code);
      // Wave 3 will update the Zustand store here
    },
    []
  );

  return (
    <div className={styles.app}>
      <TitleBar />
      <div className={styles.mainContent}>
        <TerminalArea
          sessions={sessions}
          activeSessionId={activeSessionId}
          mockMode={MOCK_MODE}
          onSessionStatusChange={handleStatusChange}
          onSessionExit={handleSessionExit}
        />
        <div className={styles.sessionPanel}>
          {/* Minimal session panel for testing — Wave 3 builds the real one */}
          <div style={{ padding: "16px" }}>
            <button
              onClick={handleCreateSession}
              style={{
                width: "100%",
                padding: "8px 12px",
                backgroundColor: "#1a1b26",
                color: "#7aa2f7",
                border: "1px solid #3b3f5c",
                borderRadius: "6px",
                cursor: "pointer",
                fontFamily: "inherit",
                fontSize: "13px",
              }}
            >
              + New Session
            </button>
            <div style={{ marginTop: "12px" }}>
              {sessions.map((s) => (
                <div
                  key={s.id}
                  onClick={() => setActiveSessionId(s.id)}
                  style={{
                    padding: "8px",
                    marginBottom: "4px",
                    borderRadius: "4px",
                    cursor: "pointer",
                    backgroundColor:
                      s.id === activeSessionId ? "#1a1b26" : "transparent",
                    color: s.id === activeSessionId ? "#c0caf5" : "#565f89",
                    fontSize: "13px",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                  }}
                >
                  <span>{s.name}</span>
                  <span
                    onClick={(e) => {
                      e.stopPropagation();
                      handleCloseSession(s.id);
                    }}
                    style={{ color: "#f7768e", cursor: "pointer", fontSize: "11px" }}
                  >
                    close
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
```

- [ ] **Verify the app builds:**

```bash
npm run build 2>&1
# Expected: Build completes successfully
```

---

### Step 2C.4: End-to-end integration test (manual)

- [ ] **Start the app in dev mode:**

```bash
npm run tauri dev 2>&1
# Expected: App window opens
```

- [ ] **Test 1: Create a session.** Click "+ New Session". Verify:
  - A new session appears in the right panel
  - The terminal area shows an xterm.js terminal
  - If `claude` is on PATH and `cwd` is a git repo, you see Claude Code starting up
  - If `claude` is not on PATH, you see an error in the console (expected for this test)

- [ ] **Test 2: Type into the terminal.** Click in the terminal and type characters. Verify:
  - Characters appear in the terminal (they are being sent to the PTY and echoed back)
  - No errors in the browser dev console (Cmd+Opt+I in the Tauri webview)

- [ ] **Test 3: Terminal resize.** Drag the window to resize it. Verify:
  - The terminal refits to the new size
  - Console logs show the resize IPC call being made
  - No visual artifacts (no scrollbar jumping, no clipped text)

- [ ] **Test 4: Close a session.** Click "close" next to the session name. Verify:
  - The session disappears from the sidebar
  - The terminal is removed
  - The placeholder "No active sessions" appears if it was the last session

- [ ] **Test 5: Multiple sessions.** Create 2-3 sessions. Click between them in the sidebar. Verify:
  - Only the active session's terminal is visible
  - Switching back to a session shows its previous output (preserved in memory)
  - Each terminal receives its own output stream (not cross-contaminated)

---

### Step 2C.5: Fix any issues found during E2E testing

- [ ] **Address any issues** found during the manual E2E tests. Common issues:
  - Byte encoding: If terminal shows garbled output, check that `session-output-{id}` payload is correctly decoded from `number[]` to `Uint8Array`
  - Event timing: If events arrive before the terminal is mounted, add a small buffer or queue
  - Resize loop: If resize causes an infinite loop, ensure the ResizeObserver debounces correctly

---

### Step 2C.6: Commit Task 2C

- [ ] **Commit the integration:**

```bash
git add src/lib/tauri-ipc.ts \
       src/components/TerminalArea/TerminalArea.tsx \
       src/App.tsx
git commit -m "feat(2C): connect frontend to backend — Tauri IPC wired to xterm.js terminals"
```

---

## Wave 2 Completion Checklist

- [ ] All 6 Tauri commands compile and are registered in the invoke handler
- [ ] PTY manager emits `session-output-{id}`, `session-status-{id}`, `session-exit-{id}` events
- [ ] xterm.js terminals render in the app with correct theme and 10k scrollback
- [ ] Terminal attach/detach works when switching between sessions
- [ ] ResizeObserver triggers terminal fit and PTY resize IPC
- [ ] User keystrokes flow from xterm.js through IPC to PTY stdin
- [ ] PTY stdout flows from backend through Tauri events to xterm.js
- [ ] Mock mode still works for frontend-only development
- [ ] `cargo test` passes for the Rust backend
- [ ] `npm run build` succeeds for the frontend
- [ ] End-to-end: can spawn a session, see output, type input, close session
- [ ] `rename_session` command compiles and is registered in the invoke handler (not exercised in E2E tests until Wave 3 when the session panel UI exposes renaming)

**Final commit tag:**

```bash
git tag wave-2-complete
```
