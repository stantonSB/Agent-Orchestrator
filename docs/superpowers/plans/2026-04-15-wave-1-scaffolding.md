# Wave 1: Project Scaffolding & Core Infrastructure

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a buildable Tauri + React macOS app with a working Rust PTY module (unit-tested, no IPC yet) and a dark-themed React app shell (title bar, two-pane layout).

**Architecture:** Single Tauri frameless window. Rust backend owns all PTY state on a dedicated manager thread, communicating via mpsc channels. React frontend uses Zustand for state, CSS Modules for scoped styling, and xterm.js for terminal rendering (wired in Wave 2).

**Tech Stack:** Tauri 2.x, React 18 + TypeScript, Vite, CSS Modules, Zustand, xterm.js, portable-pty, Rust 1.94+

---

## Task 1A: Tauri + React Scaffold

**Dependency:** None (do first)
**Estimated time:** 15-20 minutes
**Outcome:** A buildable Tauri app that opens a window showing the React dev page with "Hello from Agent Orchestrator" text.

### Step 1A.1: Scaffold the Tauri project

- [ ] Run the Tauri scaffold command from the project root. This creates the entire project structure in-place.

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx create-tauri-app@latest . --template react-ts --manager npm --yes --force --tauri-version 2
```

**Expected output:** Scaffold completes, creating `src/` (React), `src-tauri/` (Rust), `package.json`, `vite.config.ts`, etc.

> **Fallback:** If the `--yes`, `--force`, or `--tauri-version 2` flags are not recognized by your version of `create-tauri-app`, run `npx create-tauri-app@latest .` interactively and select **React + TypeScript + npm** when prompted.

- [ ] Verify the scaffold created the expected structure:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
ls -la src/ src-tauri/src/ src-tauri/Cargo.toml package.json vite.config.ts tsconfig.json
```

**Expected output:** All listed files/directories exist.

### Step 1A.1b: Verify and adapt scaffold output for Tauri 2

Tauri 2 may generate a `lib.rs` + `main.rs` pair instead of just `main.rs`. Adapt accordingly.

- [ ] Check what the scaffold generated:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
ls src-tauri/src/
```

- [ ] If a `lib.rs` exists, keep it. If only `main.rs` exists, create `lib.rs`. Either way, ensure `lib.rs` contains the crate-level module export (needed by Wave 2 integration tests):

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/src/lib.rs`**

Ensure this line is present (add to existing content or create the file):

```rust
pub mod pty_manager;
```

- [ ] If the scaffold generated a `lib.rs` with a `run()` function that `main.rs` calls, keep that pattern. Remove any scaffold demo commands (e.g., `greet`) but preserve the Tauri builder structure.

### Step 1A.2: Install frontend dependencies

- [ ] Install the base dependencies plus the project-specific packages:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npm install
npm install zustand @xterm/xterm @xterm/addon-fit @xterm/addon-web-links
```

**Expected output:** `node_modules/` populated, no errors. `package.json` updated with new dependencies.

- [ ] Verify the packages are in `package.json`:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
cat package.json | grep -E '"zustand"|"@xterm/xterm"|"@xterm/addon-fit"|"@xterm/addon-web-links"'
```

**Expected output:** Four lines showing the installed packages.

### Step 1A.3: Add portable-pty Rust dependency

- [ ] Add `portable-pty` and other required crates to `src-tauri/Cargo.toml`. Open the file and add to the `[dependencies]` section:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/Cargo.toml`**

Add these lines to the existing `[dependencies]` section (keep whatever Tauri already added):

```toml
portable-pty = "0.8"  # Note: verify latest version on crates.io; 0.8 is last known stable
uuid = { version = "1", features = ["v4"] }
```

- [ ] Verify the Rust project compiles with the new dependencies:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo check
```

**Expected output:** `Finished` with no errors (warnings are OK).

### Step 1A.4: Configure Tauri for macOS frameless window

- [ ] Edit the Tauri configuration to set up the frameless window, app identifier, and minimum size.

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/tauri.conf.json`**

Replace the entire contents with:

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-config-schema/schema.json",
  "productName": "Agent Orchestrator",
  "version": "0.1.0",
  "identifier": "com.agent-orchestrator.app",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "Agent Orchestrator",
        "width": 1200,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600,
        "decorations": false,
        "transparent": false
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

### Step 1A.5: Enable Vite CSS Modules support

CSS Modules work out of the box with Vite for any file named `*.module.css`. No extra config is needed. Verify by checking the Vite config exists:

- [ ] Ensure `vite.config.ts` exists and has the React plugin. Replace with:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/vite.config.ts`**

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
```

### Step 1A.6: Set up the Rust module structure for PTY work

- [ ] Create the module file that will hold the PTY manager (empty for now, just establishing the module structure). The PTY implementation happens in Task 1B.

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/src/pty_manager.rs`**

```rust
//! PTY manager module.
//!
//! Owns all PTY state on a dedicated thread. Communicates with the rest
//! of the application via channel-based messages (PtyRequest / PtyResponse).

// Implementation in Task 1B.
```

- [ ] Register the module in `main.rs`. Replace the scaffold's `main.rs` with:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/src/main.rs`**

```rust
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pty_manager;

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Step 1A.7: Replace scaffold React code with minimal placeholder

- [ ] Clear out the scaffold's demo content. Replace `src/App.tsx`:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/App.tsx`**

```tsx
function App() {
  return (
    <div style={{ color: "#c8c8c8", background: "#1a1a2e", height: "100vh", display: "flex", alignItems: "center", justifyContent: "center" }}>
      <h1>Agent Orchestrator</h1>
    </div>
  );
}

export default App;
```

- [ ] Delete the scaffold's demo CSS and asset files that we no longer need:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
rm -f src/App.css src/styles.css src/assets/react.svg src/assets/tauri.svg
rmdir src/assets 2>/dev/null || true
```

- [ ] Create the global styles file (must exist before `main.tsx` imports it):

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/index.css`**

```css
:root {
  --bg-primary: #1a1a2e;
  --bg-secondary: #16213e;
  --bg-tertiary: #0f3460;
  --text-primary: #e0e0e0;
  --text-secondary: #a0a0b0;
  --text-muted: #6c6c80;
  --border-color: #2a2a4a;
  --accent: #4fc3f7;
  --status-starting: #64b5f6;
  --status-working: #66bb6a;
  --status-idle: #9e9e9e;
  --status-attention: #ffa726;
  --status-finished: #78909c;
  --status-error: #ef5350;
  --titlebar-height: 38px;
  --sidebar-width: 30%;
  --font-mono: "SF Mono", "Menlo", "Monaco", "Courier New", monospace;
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #root {
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-family: var(--font-sans);
  font-size: 13px;
  -webkit-font-smoothing: antialiased;
}

/* Disable text selection on UI chrome (not in terminals) */
.no-select {
  -webkit-user-select: none;
  user-select: none;
}
```

- [ ] Replace `src/main.tsx` with a clean entry point (includes CSS import; scaffold CSS files were already deleted above):

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/main.tsx`**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

### Step 1A.8: Verify everything builds

- [ ] Run the Rust build check:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo check
```

**Expected output:** `Finished` with no errors.

- [ ] Run the frontend build:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx vite build
```

**Expected output:** Build completes, `dist/` directory created.

> **Note:** A full `tauri build --debug` is too heavyweight here (5-10 min). The `cargo check` + `npx vite build` above are sufficient for this step. The full `tauri build` is deferred to the final Verification Checklist.

### Step 1A.9: Verify .gitignore exists

- [ ] Verify the scaffold generated a `.gitignore`. If it does not exist, create one:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/.gitignore`** (create only if missing)

```
node_modules/
dist/
src-tauri/target/
```

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
cat .gitignore | grep -E 'node_modules|dist|target'
```

**Expected output:** Lines matching `node_modules`, `dist`, and `target` are present.

### Step 1A.10: Commit the scaffold

- [ ] Commit all scaffold files:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
git add -A
git commit -m "feat: scaffold Tauri + React project with dark theme and PTY module stub

- Tauri 2.x with frameless window (900x600 min, 1200x800 default)
- React 18 + TypeScript + Vite
- CSS Modules with dark theme CSS custom properties
- Dependencies: zustand, xterm.js, portable-pty, uuid
- Empty pty_manager module registered in main.rs"
```

---

## Task 1B: Rust PTY Module (TDD)

**Dependency:** Task 1A complete
**Can parallel with:** Task 1C
**Estimated time:** 40-55 minutes
**Outcome:** A fully unit-tested Rust PTY manager that can spawn processes in PTYs, read their output, write to their input, resize them, rename them, and kill them -- all via channel-based communication. No Tauri IPC wiring yet.

**Approach:** TDD -- write types first, then test stubs, then implement function by function with test verification between each step.

### Step 1B.1: Write the PTY manager types and enums

- [ ] Replace the placeholder `pty_manager.rs` with the public types, enums, and struct definitions. No implementation yet.

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/src/pty_manager.rs`**

```rust
//! PTY manager module.
//!
//! Owns all PTY state on a dedicated thread. Communicates with callers
//! via channel-based messages (PtyRequest / PtyResponse).
//!
//! Design: PTY handles from portable-pty are not Send/Sync, so they
//! cannot be shared across threads. All PTY state lives exclusively on
//! the manager thread. External code sends requests via an mpsc channel
//! and receives responses via oneshot channels.

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Unique identifier for a PTY session.
pub type SessionId = String;

/// Requests sent to the PTY manager thread.
pub enum PtyRequest {
    /// Spawn a new PTY running `command` in `cwd`.
    Create {
        name: String,
        cwd: PathBuf,
        command: String,
        args: Vec<String>,
        cols: u16,
        rows: u16,
        reply: mpsc::Sender<PtyResponse>,
    },
    /// Write bytes to a session's PTY stdin.
    Write {
        id: SessionId,
        data: Vec<u8>,
        reply: mpsc::Sender<PtyResponse>,
    },
    /// Resize a session's PTY.
    Resize {
        id: SessionId,
        cols: u16,
        rows: u16,
        reply: mpsc::Sender<PtyResponse>,
    },
    /// Rename a session.
    Rename {
        id: SessionId,
        name: String,
        reply: mpsc::Sender<PtyResponse>,
    },
    /// Kill a session's PTY process and clean up.
    Kill {
        id: SessionId,
        reply: mpsc::Sender<PtyResponse>,
    },
    /// List all active sessions.
    List {
        reply: mpsc::Sender<PtyResponse>,
    },
    /// Shut down the manager thread. All PTYs are killed.
    Shutdown,
}

/// Responses from the PTY manager thread.
#[derive(Debug)]
pub enum PtyResponse {
    /// Session was created successfully.
    Created { id: SessionId },
    /// A write operation completed.
    WriteOk,
    /// A resize operation completed.
    ResizeOk,
    /// A rename operation completed.
    RenameOk,
    /// A session was killed.
    Killed,
    /// List of active session IDs and names.
    Sessions(Vec<SessionListEntry>),
    /// An error occurred.
    Error(String),
}

/// Entry returned by the List request.
#[derive(Debug, Clone)]
pub struct SessionListEntry {
    pub id: SessionId,
    pub name: String,
    pub cwd: PathBuf,
    pub created_at_epoch_ms: u128,
}

/// Callback for PTY output. Called on a reader thread whenever bytes
/// are read from a session's PTY stdout.
pub type OutputCallback = Box<dyn Fn(SessionId, Vec<u8>) + Send + 'static>;

/// Callback for PTY exit. Called when a child process exits.
pub type ExitCallback = Box<dyn Fn(SessionId, Option<u32>) + Send + 'static>;
```

- [ ] Verify types compile:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo check
```

**Expected output:** `Finished` with no errors (unused import warnings are OK).

### Step 1B.2: Write PtyManagerHandle + request helper + start function

- [ ] Append the internal session state, handle, and `start()` function to `pty_manager.rs`:

**Append to: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/src/pty_manager.rs`**

```rust
// ---------------------------------------------------------------------------
// Internal session state (lives exclusively on the manager thread)
// ---------------------------------------------------------------------------

struct Session {
    id: SessionId,
    name: String,
    #[allow(dead_code)]
    cwd: PathBuf,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    #[allow(dead_code)]
    created_at: Instant,
    created_at_epoch_ms: u128,
    /// Handle to the reader thread so we can wait for it on cleanup.
    _reader_handle: thread::JoinHandle<()>,
    // NOTE: The child process is moved into the reader thread for wait().
    // This means we cannot send SIGTERM directly on kill -- instead, dropping
    // the master PTY fd causes the child to receive SIGHUP, which is sufficient
    // for Phase 1. Wave 4 task 4C implements proper SIGTERM/SIGKILL sequencing
    // by storing the child PID here and signaling it explicitly.
}

// ---------------------------------------------------------------------------
// PtyManager handle (clone-friendly, Send + Sync)
// ---------------------------------------------------------------------------

/// A handle to the PTY manager thread. Cloneable and thread-safe.
/// Send requests by calling methods; each blocks until the manager replies.
#[derive(Clone)]
pub struct PtyManagerHandle {
    tx: mpsc::Sender<PtyRequest>,
}

impl PtyManagerHandle {
    /// Send a request and wait for the response.
    fn request(&self, build: impl FnOnce(mpsc::Sender<PtyResponse>) -> PtyRequest) -> PtyResponse {
        let (reply_tx, reply_rx) = mpsc::channel();
        let req = build(reply_tx);
        if self.tx.send(req).is_err() {
            return PtyResponse::Error("PTY manager thread has shut down".into());
        }
        reply_rx
            .recv()
            .unwrap_or(PtyResponse::Error("PTY manager did not reply".into()))
    }

    /// Spawn a new PTY session.
    pub fn create(
        &self,
        name: String,
        cwd: PathBuf,
        command: String,
        args: Vec<String>,
        cols: u16,
        rows: u16,
    ) -> PtyResponse {
        self.request(|reply| PtyRequest::Create {
            name,
            cwd,
            command,
            args,
            cols,
            rows,
            reply,
        })
    }

    /// Write bytes to a session's stdin.
    pub fn write(&self, id: SessionId, data: Vec<u8>) -> PtyResponse {
        self.request(|reply| PtyRequest::Write { id, data, reply })
    }

    /// Resize a session's terminal.
    pub fn resize(&self, id: SessionId, cols: u16, rows: u16) -> PtyResponse {
        self.request(|reply| PtyRequest::Resize {
            id,
            cols,
            rows,
            reply,
        })
    }

    /// Rename a session.
    pub fn rename(&self, id: SessionId, name: String) -> PtyResponse {
        self.request(|reply| PtyRequest::Rename { id, name, reply })
    }

    /// Kill a session.
    pub fn kill(&self, id: SessionId) -> PtyResponse {
        self.request(|reply| PtyRequest::Kill { id, reply })
    }

    /// List all active sessions.
    pub fn list(&self) -> PtyResponse {
        self.request(|reply| PtyRequest::List { reply })
    }

    /// Shut down the manager thread and all sessions.
    pub fn shutdown(&self) {
        let _ = self.tx.send(PtyRequest::Shutdown);
    }
}

// ---------------------------------------------------------------------------
// Manager thread (stub -- filled in next steps)
// ---------------------------------------------------------------------------

/// Start the PTY manager thread. Returns a handle for sending requests.
///
/// - `on_output` is called whenever bytes are read from any session's PTY.
/// - `on_exit` is called when a child process exits.
///
/// Both callbacks are invoked on background reader threads (one per session),
/// NOT on the manager thread itself.
pub fn start(on_output: OutputCallback, on_exit: ExitCallback) -> PtyManagerHandle {
    let (tx, rx) = mpsc::channel::<PtyRequest>();

    let on_output = std::sync::Arc::new(on_output);
    let on_exit = std::sync::Arc::new(on_exit);

    thread::Builder::new()
        .name("pty-manager".into())
        .spawn(move || {
            manager_loop(rx, on_output, on_exit);
        })
        .expect("failed to spawn PTY manager thread");

    PtyManagerHandle { tx }
}

fn manager_loop(
    rx: mpsc::Receiver<PtyRequest>,
    on_output: std::sync::Arc<OutputCallback>,
    on_exit: std::sync::Arc<ExitCallback>,
) {
    let mut sessions: HashMap<SessionId, Session> = HashMap::new();
    let pty_system = native_pty_system();

    while let Ok(request) = rx.recv() {
        match request {
            // Handlers added in the next steps
            _ => { break; }
        }
    }
}
```

- [ ] Verify it compiles:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo check
```

**Expected output:** `Finished` with no errors (warnings about unused variables are OK).

### Step 1B.3: Write test stubs (all tests, before implementation)

- [ ] Append the test module to `pty_manager.rs`. These tests define the contract the implementation must satisfy.

**Append to: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/src/pty_manager.rs`**

```rust
// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Helper: start a manager with output/exit collectors.
    fn test_manager() -> (
        PtyManagerHandle,
        Arc<Mutex<Vec<(SessionId, Vec<u8>)>>>,
        Arc<Mutex<Vec<(SessionId, Option<u32>)>>>,
    ) {
        let output_log: Arc<Mutex<Vec<(SessionId, Vec<u8>)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let exit_log: Arc<Mutex<Vec<(SessionId, Option<u32>)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let ol = output_log.clone();
        let el = exit_log.clone();

        let handle = start(
            Box::new(move |id, data| {
                ol.lock().unwrap().push((id, data));
            }),
            Box::new(move |id, code| {
                el.lock().unwrap().push((id, code));
            }),
        );

        (handle, output_log, exit_log)
    }

    #[test]
    fn test_create_and_list() {
        let (handle, _output, _exit) = test_manager();

        let resp = handle.create(
            "test-session".into(),
            std::env::temp_dir(),
            "echo".into(),
            vec!["hello".into()],
            80,
            24,
        );

        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        thread::sleep(Duration::from_millis(200));

        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                assert!(
                    entries.iter().any(|e| e.id == id),
                    "Session {} not found in list: {:?}",
                    id,
                    entries
                );
                let entry = entries.iter().find(|e| e.id == id).unwrap();
                assert_eq!(entry.name, "test-session");
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }

        handle.shutdown();
    }

    #[test]
    fn test_output_received() {
        let (handle, output_log, _exit) = test_manager();

        let resp = handle.create(
            "echo-test".into(),
            std::env::temp_dir(),
            "echo".into(),
            vec!["hello world".into()],
            80,
            24,
        );

        let _id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        thread::sleep(Duration::from_millis(500));

        let log = output_log.lock().unwrap();
        let all_output: Vec<u8> = log.iter().flat_map(|(_, data)| data.clone()).collect();
        let output_str = String::from_utf8_lossy(&all_output);
        assert!(
            output_str.contains("hello world"),
            "Expected 'hello world' in output, got: {:?}",
            output_str
        );

        handle.shutdown();
    }

    #[test]
    fn test_exit_callback() {
        let (handle, _output, exit_log) = test_manager();

        let resp = handle.create(
            "exit-test".into(),
            std::env::temp_dir(),
            "true".into(),
            vec![],
            80,
            24,
        );

        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        thread::sleep(Duration::from_millis(500));

        let log = exit_log.lock().unwrap();
        assert!(
            log.iter().any(|(eid, code)| eid == &id && *code == Some(0)),
            "Expected exit code 0 for session {}, got: {:?}",
            id,
            *log
        );

        handle.shutdown();
    }

    #[test]
    fn test_write_to_session() {
        let (handle, output_log, _exit) = test_manager();

        let resp = handle.create(
            "cat-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
        );

        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        let resp = handle.write(id.clone(), b"ping\n".to_vec());
        match resp {
            PtyResponse::WriteOk => {}
            other => panic!("Expected WriteOk, got: {:?}", other),
        }

        thread::sleep(Duration::from_millis(500));

        let log = output_log.lock().unwrap();
        let all_output: Vec<u8> = log
            .iter()
            .filter(|(eid, _)| eid == &id)
            .flat_map(|(_, data)| data.clone())
            .collect();
        let output_str = String::from_utf8_lossy(&all_output);
        assert!(
            output_str.contains("ping"),
            "Expected 'ping' in output, got: {:?}",
            output_str
        );

        handle.shutdown();
    }

    #[test]
    fn test_resize() {
        let (handle, _output, _exit) = test_manager();

        let resp = handle.create(
            "resize-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
        );

        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        let resp = handle.resize(id.clone(), 120, 40);
        match resp {
            PtyResponse::ResizeOk => {}
            other => panic!("Expected ResizeOk, got: {:?}", other),
        }

        handle.shutdown();
    }

    #[test]
    fn test_rename_session() {
        let (handle, _output, _exit) = test_manager();

        let resp = handle.create(
            "original-name".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
        );

        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        let resp = handle.rename(id.clone(), "new-name".into());
        match resp {
            PtyResponse::RenameOk => {}
            other => panic!("Expected RenameOk, got: {:?}", other),
        }

        // Verify the name changed via list.
        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                let entry = entries.iter().find(|e| e.id == id).unwrap();
                assert_eq!(entry.name, "new-name");
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }

        handle.shutdown();
    }

    #[test]
    fn test_kill_session() {
        let (handle, _output, _exit) = test_manager();

        let resp = handle.create(
            "kill-test".into(),
            std::env::temp_dir(),
            "cat".into(),
            vec![],
            80,
            24,
        );

        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        let resp = handle.kill(id.clone());
        match resp {
            PtyResponse::Killed => {}
            other => panic!("Expected Killed, got: {:?}", other),
        }

        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                assert!(
                    !entries.iter().any(|e| e.id == id),
                    "Session should have been removed after kill"
                );
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }

        handle.shutdown();
    }

    #[test]
    fn test_write_to_nonexistent_session() {
        let (handle, _output, _exit) = test_manager();

        let resp = handle.write("nonexistent-id".into(), b"data".to_vec());
        match resp {
            PtyResponse::Error(msg) => {
                assert!(msg.contains("not found"), "Error should mention 'not found': {msg}");
            }
            other => panic!("Expected Error, got: {:?}", other),
        }

        handle.shutdown();
    }

    #[test]
    fn test_kill_nonexistent_session() {
        let (handle, _output, _exit) = test_manager();

        let resp = handle.kill("nonexistent-id".into());
        match resp {
            PtyResponse::Error(msg) => {
                assert!(msg.contains("not found"), "Error should mention 'not found': {msg}");
            }
            other => panic!("Expected Error, got: {:?}", other),
        }

        handle.shutdown();
    }

    #[test]
    fn test_nonzero_exit_code() {
        let (handle, _output, exit_log) = test_manager();

        let resp = handle.create(
            "fail-test".into(),
            std::env::temp_dir(),
            "false".into(),
            vec![],
            80,
            24,
        );

        let id = match resp {
            PtyResponse::Created { id } => id,
            other => panic!("Expected Created, got: {:?}", other),
        };

        thread::sleep(Duration::from_millis(500));

        let log = exit_log.lock().unwrap();
        assert!(
            log.iter().any(|(eid, code)| eid == &id && *code == Some(1)),
            "Expected exit code 1 for session {}, got: {:?}",
            id,
            *log
        );

        handle.shutdown();
    }

    #[test]
    fn test_shutdown_kills_all_sessions() {
        let (handle, _output, exit_log) = test_manager();

        // Create 3 sessions running long-lived processes.
        let mut ids = Vec::new();
        for i in 0..3 {
            let resp = handle.create(
                format!("session-{i}"),
                std::env::temp_dir(),
                "cat".into(),
                vec![],
                80,
                24,
            );
            match resp {
                PtyResponse::Created { id } => ids.push(id),
                other => panic!("Expected Created, got: {:?}", other),
            }
        }

        // Verify all 3 are listed.
        let resp = handle.list();
        match resp {
            PtyResponse::Sessions(entries) => {
                assert_eq!(entries.len(), 3, "Expected 3 sessions, got {}", entries.len());
            }
            other => panic!("Expected Sessions, got: {:?}", other),
        }

        // Shutdown should kill all sessions.
        handle.shutdown();

        // Wait for exit callbacks to fire.
        thread::sleep(Duration::from_millis(1000));

        let log = exit_log.lock().unwrap();
        for id in &ids {
            assert!(
                log.iter().any(|(eid, _)| eid == id),
                "Expected exit callback for session {}, got: {:?}",
                id,
                *log
            );
        }
    }
}
```

- [ ] Verify tests compile (they will **fail** at this point since `manager_loop` is a stub):

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo test --no-run 2>&1 | tail -3
```

**Expected output:** Compiles successfully (or with warnings). Tests are not run yet.

### Step 1B.4: Implement manager_loop -- Create, Write, Resize handlers

- [ ] Replace the stub `manager_loop` function with the Create, Write, and Resize handlers. Update the `match` block:

**In file: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/src/pty_manager.rs`**

Replace the `manager_loop` function with:

```rust
fn manager_loop(
    rx: mpsc::Receiver<PtyRequest>,
    on_output: std::sync::Arc<OutputCallback>,
    on_exit: std::sync::Arc<ExitCallback>,
) {
    let mut sessions: HashMap<SessionId, Session> = HashMap::new();
    let pty_system = native_pty_system();

    while let Ok(request) = rx.recv() {
        match request {
            PtyRequest::Create {
                name,
                cwd,
                command,
                args,
                cols,
                rows,
                reply,
            } => {
                let id = uuid::Uuid::new_v4().to_string();
                let size = PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                };

                let pair = match pty_system.openpty(size) {
                    Ok(pair) => pair,
                    Err(e) => {
                        let _ = reply.send(PtyResponse::Error(format!(
                            "Failed to open PTY: {e}"
                        )));
                        continue;
                    }
                };

                let mut cmd = CommandBuilder::new(&command);
                cmd.args(&args);
                cmd.cwd(&cwd);

                let child = match pair.slave.spawn_command(cmd) {
                    Ok(child) => child,
                    Err(e) => {
                        let _ = reply.send(PtyResponse::Error(format!(
                            "Failed to spawn command: {e}"
                        )));
                        continue;
                    }
                };

                // Drop the slave side -- the child owns it now.
                drop(pair.slave);

                let writer = match pair.master.take_writer() {
                    Ok(w) => w,
                    Err(e) => {
                        let _ = reply.send(PtyResponse::Error(format!(
                            "Failed to get PTY writer: {e}"
                        )));
                        continue;
                    }
                };

                let mut reader = match pair.master.try_clone_reader() {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = reply.send(PtyResponse::Error(format!(
                            "Failed to get PTY reader: {e}"
                        )));
                        continue;
                    }
                };

                // Spawn a reader thread for this session's stdout.
                let reader_id = id.clone();
                let cb = on_output.clone();
                let exit_cb = on_exit.clone();
                let mut child_for_wait = child;
                let reader_handle = thread::Builder::new()
                    .name(format!("pty-reader-{}", &id[..8]))
                    .spawn(move || {
                        let mut buf = [0u8; 4096];
                        loop {
                            match reader.read(&mut buf) {
                                Ok(0) => break, // EOF
                                Ok(n) => {
                                    cb(reader_id.clone(), buf[..n].to_vec());
                                }
                                Err(e) => {
                                    // EIO is expected when the child exits on macOS
                                    if e.kind() != std::io::ErrorKind::Other {
                                        eprintln!(
                                            "PTY read error for {}: {e}",
                                            &reader_id[..8]
                                        );
                                    }
                                    break;
                                }
                            }
                        }
                        // Wait for the child to fully exit and report the code.
                        let exit_code = child_for_wait
                            .wait()
                            .ok()
                            .and_then(|status| status.exit_code());
                        exit_cb(reader_id, exit_code);
                    })
                    .expect("failed to spawn PTY reader thread");

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();

                sessions.insert(
                    id.clone(),
                    Session {
                        id: id.clone(),
                        name,
                        cwd,
                        master: pair.master,
                        writer,
                        created_at: Instant::now(),
                        created_at_epoch_ms: now,
                        _reader_handle: reader_handle,
                    },
                );

                let _ = reply.send(PtyResponse::Created { id });
            }

            PtyRequest::Write { id, data, reply } => {
                if let Some(session) = sessions.get_mut(&id) {
                    match session.writer.write_all(&data) {
                        Ok(()) => {
                            let _ = session.writer.flush();
                            let _ = reply.send(PtyResponse::WriteOk);
                        }
                        Err(e) => {
                            let _ = reply.send(PtyResponse::Error(format!(
                                "Write failed: {e}"
                            )));
                        }
                    }
                } else {
                    let _ = reply.send(PtyResponse::Error(format!(
                        "Session not found: {id}"
                    )));
                }
            }

            PtyRequest::Resize {
                id,
                cols,
                rows,
                reply,
            } => {
                if let Some(session) = sessions.get(&id) {
                    let size = PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    };
                    match session.master.resize(size) {
                        Ok(()) => {
                            let _ = reply.send(PtyResponse::ResizeOk);
                        }
                        Err(e) => {
                            let _ = reply.send(PtyResponse::Error(format!(
                                "Resize failed: {e}"
                            )));
                        }
                    }
                } else {
                    let _ = reply.send(PtyResponse::Error(format!(
                        "Session not found: {id}"
                    )));
                }
            }

            // Remaining handlers in next step
            PtyRequest::Rename { reply, .. }
            | PtyRequest::Kill { reply, .. }
            | PtyRequest::List { reply, .. } => {
                let _ = reply.send(PtyResponse::Error("Not yet implemented".into()));
            }
            PtyRequest::Shutdown => { break; }
        }
    }
}
```

- [ ] Run the subset of tests that should now pass:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo test test_create_and_list test_output_received test_exit_callback test_write_to_session test_resize test_write_to_nonexistent_session test_nonzero_exit_code -- --nocapture
```

**Expected output:** These 7 tests pass. The remaining tests (kill, rename, shutdown) will still fail.

### Step 1B.5: Implement manager_loop -- Rename, Kill, List, Shutdown handlers

- [ ] Replace the placeholder arms in `manager_loop` with the full implementations:

**In file: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri/src/pty_manager.rs`**

Replace the placeholder arms (`PtyRequest::Rename`, `PtyRequest::Kill`, `PtyRequest::List`, `PtyRequest::Shutdown`) with:

```rust
            PtyRequest::Rename { id, name, reply } => {
                if let Some(session) = sessions.get_mut(&id) {
                    session.name = name;
                    let _ = reply.send(PtyResponse::RenameOk);
                } else {
                    let _ = reply.send(PtyResponse::Error(format!(
                        "Session not found: {id}"
                    )));
                }
            }

            PtyRequest::Kill { id, reply } => {
                if let Some(session) = sessions.remove(&id) {
                    // Dropping the master PTY and writer will close the PTY,
                    // causing the child to receive SIGHUP. This is a deliberate
                    // Phase 1 simplification: the child process ownership was
                    // moved to the reader thread, so we cannot send SIGTERM
                    // directly. SIGHUP via PTY drop is acceptable because most
                    // shells and well-behaved processes handle SIGHUP gracefully.
                    // Wave 4 task 4C implements the proper SIGTERM/SIGKILL
                    // sequence by storing the child PID in the Session struct.
                    drop(session.writer);
                    drop(session.master);
                    // The reader thread will see EOF and exit.
                    // We don't join it here to avoid blocking the manager.
                    let _ = reply.send(PtyResponse::Killed);
                } else {
                    let _ = reply.send(PtyResponse::Error(format!(
                        "Session not found: {id}"
                    )));
                }
            }

            PtyRequest::List { reply } => {
                let entries: Vec<SessionListEntry> = sessions
                    .values()
                    .map(|s| SessionListEntry {
                        id: s.id.clone(),
                        name: s.name.clone(),
                        cwd: s.cwd.clone(),
                        created_at_epoch_ms: s.created_at_epoch_ms,
                    })
                    .collect();
                let _ = reply.send(PtyResponse::Sessions(entries));
            }

            PtyRequest::Shutdown => {
                // Kill all sessions.
                let ids: Vec<SessionId> = sessions.keys().cloned().collect();
                for id in ids {
                    if let Some(session) = sessions.remove(&id) {
                        drop(session.writer);
                        drop(session.master);
                    }
                }
                break;
            }
```

- [ ] Run ALL tests:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo test -- --nocapture
```

**Expected output:** All 11 tests pass.

### Step 1B.6: Verify all tests pass

- [ ] Final test run with summary:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo test -- --nocapture
```

**Expected output:** All 11 tests pass:
- `test_create_and_list` -- PASS
- `test_output_received` -- PASS
- `test_exit_callback` -- PASS
- `test_write_to_session` -- PASS
- `test_resize` -- PASS
- `test_rename_session` -- PASS
- `test_kill_session` -- PASS
- `test_write_to_nonexistent_session` -- PASS
- `test_kill_nonexistent_session` -- PASS
- `test_nonzero_exit_code` -- PASS
- `test_shutdown_kills_all_sessions` -- PASS

If any test fails, debug and fix before proceeding.

### Step 1B.7: Commit the PTY module

- [ ] Commit:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
git add src-tauri/src/pty_manager.rs
git commit -m "feat: implement PTY manager with channel-based communication

- Dedicated manager thread owns all PTY state (not Send/Sync safe)
- PtyManagerHandle provides Clone + Send API for callers
- Supports create, write, resize, rename, kill, list, shutdown
- Output and exit callbacks on per-session reader threads
- 11 unit tests covering happy path, error cases, and shutdown"
```

---

## Task 1C: React App Shell

**Dependency:** Task 1A complete
**Can parallel with:** Task 1B
**Estimated time:** 25-35 minutes
**Outcome:** A dark-themed React shell with a custom title bar, two-pane layout (terminal area placeholder + sidebar placeholder), all styled with CSS Modules.

### Step 1C.1: Create the TitleBar component

- [ ] Create the components directory and TitleBar component:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/components/TitleBar/TitleBar.tsx`**

```tsx
import { getCurrentWindow } from "@tauri-apps/api/window";
import styles from "./TitleBar.module.css";

export function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <div className={styles.titleBar} data-tauri-drag-region>
      <div className={styles.title} data-tauri-drag-region>
        Agent Orchestrator
      </div>
      <div className={styles.windowControls}>
        <button
          className={`${styles.controlButton} ${styles.minimize}`}
          aria-label="Minimize"
          onClick={() => appWindow.minimize()}
        >
          <svg width="10" height="1" viewBox="0 0 10 1">
            <rect width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        <button
          className={`${styles.controlButton} ${styles.maximize}`}
          aria-label="Maximize"
          onClick={() => appWindow.toggleMaximize()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <rect
              x="0.5"
              y="0.5"
              width="9"
              height="9"
              fill="none"
              stroke="currentColor"
              strokeWidth="1"
            />
          </svg>
        </button>
        <button
          className={`${styles.controlButton} ${styles.close}`}
          aria-label="Close"
          onClick={() => appWindow.close()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line
              x1="0"
              y1="0"
              x2="10"
              y2="10"
              stroke="currentColor"
              strokeWidth="1.2"
            />
            <line
              x1="10"
              y1="0"
              x2="0"
              y2="10"
              stroke="currentColor"
              strokeWidth="1.2"
            />
          </svg>
        </button>
      </div>
    </div>
  );
}
```

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/components/TitleBar/TitleBar.module.css`**

```css
.titleBar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: var(--titlebar-height);
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  padding: 0 12px;
  -webkit-user-select: none;
  user-select: none;
}

.title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  letter-spacing: 0.3px;
  flex: 1;
}

.windowControls {
  display: flex;
  gap: 8px;
  align-items: center;
}

.controlButton {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--text-muted);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.controlButton:hover {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-primary);
}

.close:hover {
  background: rgba(239, 83, 80, 0.3);
  color: var(--status-error);
}
```

### Step 1C.2: Create the TerminalArea placeholder component

- [ ] Create the terminal area component that will later host xterm.js:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/components/TerminalArea/TerminalArea.tsx`**

```tsx
import styles from "./TerminalArea.module.css";

interface TerminalAreaProps {
  activeSessionId: string | null;
}

export function TerminalArea({ activeSessionId }: TerminalAreaProps) {
  return (
    <div className={styles.terminalArea}>
      {activeSessionId ? (
        <div className={styles.terminalContainer}>
          {/* xterm.js will be mounted here in Wave 2 */}
          <div className={styles.placeholder}>
            <span className={styles.placeholderText}>
              Terminal for session: {activeSessionId}
            </span>
          </div>
        </div>
      ) : (
        <div className={styles.emptyState}>
          <div className={styles.emptyIcon}>&#9654;</div>
          <h2 className={styles.emptyTitle}>No Active Session</h2>
          <p className={styles.emptyDescription}>
            Create a new session from the sidebar to start working with Claude
            Code.
          </p>
        </div>
      )}
    </div>
  );
}
```

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/components/TerminalArea/TerminalArea.module.css`**

```css
.terminalArea {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  background: var(--bg-primary);
  overflow: hidden;
}

.terminalContainer {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.placeholder {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #0d0d1a;
  margin: 4px;
  border-radius: 4px;
}

.placeholderText {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-muted);
}

.emptyState {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
}

.emptyIcon {
  font-size: 36px;
  color: var(--text-muted);
  opacity: 0.4;
}

.emptyTitle {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-secondary);
}

.emptyDescription {
  font-size: 13px;
  color: var(--text-muted);
  text-align: center;
  max-width: 300px;
  line-height: 1.5;
}
```

### Step 1C.3: Create the SessionPanel placeholder component

- [ ] Create the sidebar panel component:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/components/SessionPanel/SessionPanel.tsx`**

```tsx
import styles from "./SessionPanel.module.css";

interface SessionPanelProps {
  sessionCount: number;
}

export function SessionPanel({ sessionCount }: SessionPanelProps) {
  return (
    <div className={styles.sessionPanel}>
      <div className={styles.header}>
        <h2 className={styles.headerTitle}>Sessions</h2>
        <span className={styles.sessionCount}>{sessionCount}</span>
      </div>

      <button className={styles.newSessionButton}>
        <span className={styles.plusIcon}>+</span>
        New Session
      </button>

      <div className={styles.sessionList}>
        {sessionCount === 0 ? (
          <div className={styles.emptyList}>
            <p className={styles.emptyText}>
              No sessions yet. Click "New Session" to start.
            </p>
          </div>
        ) : (
          <div className={styles.placeholderCards}>
            {/* SessionCard components will be added in Wave 3 */}
            <p className={styles.emptyText}>
              Session cards will appear here.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
```

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/components/SessionPanel/SessionPanel.module.css`**

```css
.sessionPanel {
  width: var(--sidebar-width);
  min-width: 240px;
  max-width: 400px;
  display: flex;
  flex-direction: column;
  background: var(--bg-secondary);
  border-left: 1px solid var(--border-color);
  overflow: hidden;
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border-color);
}

.headerTitle {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
  letter-spacing: 0.3px;
}

.sessionCount {
  font-size: 11px;
  font-weight: 500;
  color: var(--text-muted);
  background: rgba(255, 255, 255, 0.06);
  padding: 2px 7px;
  border-radius: 10px;
}

.newSessionButton {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 10px 10px 0;
  padding: 8px 12px;
  background: rgba(79, 195, 247, 0.1);
  border: 1px solid rgba(79, 195, 247, 0.2);
  border-radius: 6px;
  color: var(--accent);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}

.newSessionButton:hover {
  background: rgba(79, 195, 247, 0.18);
  border-color: rgba(79, 195, 247, 0.35);
}

.plusIcon {
  font-size: 15px;
  font-weight: 300;
  line-height: 1;
}

.sessionList {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
}

.emptyList {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px 16px;
}

.emptyText {
  font-size: 12px;
  color: var(--text-muted);
  text-align: center;
  line-height: 1.5;
}

.placeholderCards {
  padding: 20px 0;
}
```

### Step 1C.4: Create barrel exports for components

- [ ] Create an index file for clean imports:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/components/index.ts`**

```typescript
export { TitleBar } from "./TitleBar/TitleBar";
export { TerminalArea } from "./TerminalArea/TerminalArea";
export { SessionPanel } from "./SessionPanel/SessionPanel";
```

### Step 1C.5: Wire everything into App.tsx

- [ ] Replace `src/App.tsx` with the full two-pane layout:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/App.tsx`**

```tsx
import styles from "./App.module.css";
import { TitleBar, TerminalArea, SessionPanel } from "./components";

function App() {
  return (
    <div className={styles.app}>
      <TitleBar />
      <div className={styles.mainContent}>
        <TerminalArea activeSessionId={null} />
        <SessionPanel sessionCount={0} />
      </div>
    </div>
  );
}

export default App;
```

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/App.module.css`**

```css
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: var(--bg-primary);
}

.mainContent {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
```

### Step 1C.6: Add TypeScript module declaration for CSS Modules

- [ ] Create a type declaration file so TypeScript understands `.module.css` imports:

**File: `/Users/stanton.borthwick/SProjects/Agent-Orchestrator/src/css-modules.d.ts`**

```typescript
declare module "*.module.css" {
  const classes: { [key: string]: string };
  export default classes;
}
```

### Step 1C.7: Verify the frontend builds

- [ ] Build the frontend to verify all components compile:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx vite build
```

**Expected output:** Build completes with no errors. Output shows the bundled files in `dist/`.

- [ ] Run TypeScript type checking:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
npx tsc --noEmit
```

**Expected output:** No type errors.

### Step 1C.8: Verify the full Tauri app builds

- [ ] Run a debug build of the full app to ensure Rust + frontend compile together:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri
cargo check
```

**Expected output:** `Finished` with no errors.

### Step 1C.9: Commit the React app shell

- [ ] Commit all frontend components:

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
git add src/components/ src/App.tsx src/App.module.css src/css-modules.d.ts
git commit -m "feat: add React app shell with dark-themed two-pane layout

- TitleBar with custom window controls (frameless window drag region)
- TerminalArea with empty state placeholder (xterm.js in Wave 2)
- SessionPanel sidebar with new session button placeholder
- CSS Modules throughout with dark theme custom properties
- All components use scoped styles, no global class pollution"
```

---

## Verification Checklist

After all three tasks are complete, verify:

- [ ] `cargo check` passes in `src-tauri/` with no errors
- [ ] `cargo test` passes in `src-tauri/` with all 11 PTY manager tests green
- [ ] `npx vite build` produces a `dist/` directory with no errors
- [ ] `npx tsc --noEmit` reports no type errors
- [ ] `npx tauri build --debug` produces a runnable macOS `.app` bundle (full build, expect 5-10 min)
- [ ] Running the app shows a frameless dark window with the two-pane layout and working window controls (minimize, maximize, close)
- [ ] Git log shows 3 clean commits (scaffold, PTY module, app shell)

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator
cargo test --manifest-path src-tauri/Cargo.toml && npx vite build && npx tsc --noEmit && echo "ALL CHECKS PASS"
```
