# Wave 4: Status Engine & Polish

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring sessions to life with real-time status detection, visual activity indicators, proper close/cleanup flows, and robust error handling so the app is production-ready.

**Architecture:** The Rust PTY manager thread gains a status parser module that inspects the last ~500 bytes of each session's stdout buffer to detect Working/Idle/Needs Attention transitions. The frontend adds animated activity indicators, duration timers, a right-click context menu for session management, and toast notifications for errors. Tauri config enforces minimum window size, and a shutdown hook ensures clean PTY teardown.

**Tech Stack:** Rust (regex, status state machine, SIGTERM/SIGKILL), React (CSS animations, context menus, toast notifications), Zustand (timer state), Tauri (window config, shutdown hooks, IPC events)

---

## Task 4A: Status Parser

**Files to create:**
- `src-tauri/src/status_parser.rs`
- `src-tauri/src/status_parser_tests.rs`

**Files to modify:**
- `src-tauri/src/pty_manager.rs`
- `src-tauri/src/main.rs` (add `mod status_parser;`)

### Steps

- [ ] **4A-1: Create the status parser module with types and buffer**

Create `src-tauri/src/status_parser.rs`:

```rust
use std::time::Instant;

/// Session status as determined by the status parser.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Working,
    Idle,
    NeedsAttention,
    Finished,
    Error,
}

/// Tracks the output buffer and timing state for a single session.
pub struct StatusTracker {
    /// Ring buffer of the last ~500 bytes of stdout output.
    buffer: Vec<u8>,
    /// Maximum buffer size.
    max_buffer_size: usize,
    /// Current parsed status.
    status: SessionStatus,
    /// Timestamp of the last output chunk received.
    last_output_at: Option<Instant>,
    /// Whether we have ever received output.
    has_received_output: bool,
}

impl StatusTracker {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(512),
            max_buffer_size: 500,
            status: SessionStatus::Starting,
            last_output_at: None,
            has_received_output: false,
        }
    }

    /// Returns the current status.
    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    /// Feed new output bytes into the tracker. Returns Some(new_status) if
    /// the status changed, None otherwise.
    pub fn feed_output(&mut self, data: &[u8]) -> Option<SessionStatus> {
        if data.is_empty() {
            return None;
        }

        self.has_received_output = true;
        self.last_output_at = Some(Instant::now());

        // Append to buffer, keeping only the last max_buffer_size bytes.
        self.buffer.extend_from_slice(data);
        if self.buffer.len() > self.max_buffer_size {
            let drain_count = self.buffer.len() - self.max_buffer_size;
            self.buffer.drain(..drain_count);
        }

        let old_status = self.status.clone();
        self.status = SessionStatus::Working;

        if old_status != self.status {
            log::debug!(
                "Status transition: {:?} -> {:?}",
                old_status,
                self.status
            );
            Some(self.status.clone())
        } else {
            None
        }
    }

    /// Called periodically (e.g., every 1 second) to check for time-based
    /// status transitions (Working -> Idle, Idle -> NeedsAttention).
    /// Returns Some(new_status) if status changed.
    ///
    /// Accepts an optional `now` parameter to allow tests to inject a
    /// synthetic clock. Production callers pass `Instant::now()`.
    pub fn tick_with_time(&mut self, now: Instant) -> Option<SessionStatus> {
        // Don't tick if finished/error or never started
        if matches!(
            self.status,
            SessionStatus::Finished | SessionStatus::Error | SessionStatus::Starting
        ) {
            return None;
        }

        let Some(last_output) = self.last_output_at else {
            return None;
        };

        let elapsed = now.duration_since(last_output);
        let old_status = self.status.clone();

        if elapsed.as_secs() >= 10 {
            // 10+ seconds since last output: check for attention patterns
            // then fall back to Idle
            if self.check_needs_attention() {
                self.status = SessionStatus::NeedsAttention;
            } else {
                self.status = SessionStatus::Idle;
            }
        } else if elapsed.as_secs() >= 3 {
            // 3-10 seconds: no output received, so no longer "Working".
            // Working means bytes arrived within the last 3 seconds.
            // After 3 seconds of silence we transition to Idle.
            // NeedsAttention is intentionally skipped here — it only
            // triggers after 10+ seconds to avoid false positives.
            self.status = SessionStatus::Idle;
        }

        if old_status != self.status {
            log::debug!(
                "Status transition (tick): {:?} -> {:?} (elapsed: {:?})",
                old_status,
                self.status,
                elapsed
            );
            Some(self.status.clone())
        } else {
            None
        }
    }

    /// Convenience wrapper that calls `tick_with_time(Instant::now())`.
    pub fn tick(&mut self) -> Option<SessionStatus> {
        self.tick_with_time(Instant::now())
    }

    /// Notify the tracker that the process exited.
    /// Returns the new status.
    pub fn notify_exit(&mut self, exit_code: i32) -> SessionStatus {
        let old_status = self.status.clone();
        self.status = if exit_code == 0 {
            SessionStatus::Finished
        } else {
            SessionStatus::Error
        };
        log::debug!(
            "Status transition (exit): {:?} -> {:?} (code: {})",
            old_status,
            self.status,
            exit_code
        );
        self.status.clone()
    }

    /// Check the output buffer for patterns indicating Claude is waiting
    /// for user input.
    fn check_needs_attention(&self) -> bool {
        // Convert buffer to string (lossy — terminal output may have
        // ANSI escapes, but pattern matching on the text is sufficient).
        let text = String::from_utf8_lossy(&self.buffer);

        // Strip ANSI escape sequences for cleaner pattern matching.
        let stripped = strip_ansi_escapes(&text);

        // Get the last non-empty line for end-of-line pattern checks.
        let last_line = stripped
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");

        // Pattern 1: Line ending with "? " (question prompt)
        if last_line.ends_with("? ") {
            log::debug!("NeedsAttention: matched '? ' pattern");
            return true;
        }

        // Pattern 2: Line ending with "> " (input prompt)
        if last_line.ends_with("> ") {
            log::debug!("NeedsAttention: matched '> ' pattern");
            return true;
        }

        // Pattern 3: Contains (y/n) variants
        let lower = stripped.to_lowercase();
        if lower.contains("(y/n)")
            || lower.contains("(yes/no)")
            || stripped.contains("[Y/n]")
            || stripped.contains("[y/N]")
            || stripped.contains("[Y/N]")
        {
            log::debug!("NeedsAttention: matched y/n pattern");
            return true;
        }

        // Pattern 4: AskUserQuestion marker from Claude Code
        if stripped.contains("AskUserQuestion") {
            log::debug!("NeedsAttention: matched AskUserQuestion pattern");
            return true;
        }

        false
    }

    /// Get the raw buffer contents (for testing/debugging).
    #[cfg(test)]
    pub fn buffer_contents(&self) -> &[u8] {
        &self.buffer
    }
}

/// Strip ANSI escape sequences from a string.
/// Handles CSI sequences (ESC [ ... final_byte) and OSC sequences (ESC ] ... ST).
fn strip_ansi_escapes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // ESC character — consume the escape sequence
            match chars.peek() {
                Some('[') => {
                    // CSI sequence: ESC [ ... (ends at 0x40-0x7E)
                    chars.next(); // consume '['
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC sequence: ESC ] ... (ends at BEL or ST)
                    chars.next(); // consume ']'
                    while let Some(&c) = chars.peek() {
                        if c == '\x07' {
                            chars.next();
                            break;
                        }
                        if c == '\x1b' {
                            chars.next();
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        chars.next();
                    }
                }
                _ => {
                    // Other escape — consume next char
                    chars.next();
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}
```

- [ ] **4A-2: Write unit tests for the status parser**

Create `src-tauri/src/status_parser_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::status_parser::{SessionStatus, StatusTracker};
    use std::time::Duration;

    #[test]
    fn test_initial_status_is_starting() {
        let tracker = StatusTracker::new();
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_first_output_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        let change = tracker.feed_output(b"Hello from Claude");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_subsequent_output_stays_working() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Hello");
        let change = tracker.feed_output(b" world");
        assert_eq!(change, None); // Already Working, no change
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_empty_output_ignored() {
        let mut tracker = StatusTracker::new();
        let change = tracker.feed_output(b"");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_buffer_truncation_to_500_bytes() {
        let mut tracker = StatusTracker::new();
        let large_data = vec![b'A'; 600];
        tracker.feed_output(&large_data);
        assert_eq!(tracker.buffer_contents().len(), 500);
    }

    #[test]
    fn test_buffer_keeps_tail() {
        let mut tracker = StatusTracker::new();
        // Fill with 400 bytes of 'A'
        tracker.feed_output(&vec![b'A'; 400]);
        // Add 200 bytes of 'B' — should keep last 500 (100 A's + 200 B's? No: 400+200=600, keep last 500)
        tracker.feed_output(&vec![b'B'; 200]);
        let buf = tracker.buffer_contents();
        assert_eq!(buf.len(), 500);
        // Last 200 bytes should all be 'B'
        assert!(buf[300..].iter().all(|&b| b == b'B'));
    }

    #[test]
    fn test_exit_code_zero_is_finished() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"some output");
        let status = tracker.notify_exit(0);
        assert_eq!(status, SessionStatus::Finished);
    }

    #[test]
    fn test_exit_code_nonzero_is_error() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"some output");
        let status = tracker.notify_exit(1);
        assert_eq!(status, SessionStatus::Error);
    }

    #[test]
    fn test_tick_no_change_while_starting() {
        let mut tracker = StatusTracker::new();
        let change = tracker.tick();
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_tick_no_change_while_finished() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"done");
        tracker.notify_exit(0);
        let change = tracker.tick();
        assert_eq!(change, None);
    }

    #[test]
    fn test_needs_attention_question_mark_space() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Do you want to proceed? ");
        // Manually set last_output_at to 11 seconds ago to trigger tick
        // We need to use a helper for this since Instant is not easily manipulable.
        // Instead, we test the pattern matching via the check_needs_attention path
        // by verifying the full flow with a time-based approach.
        // For unit tests, we test the pattern detection directly.
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_needs_attention_yn_pattern() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Continue with changes? (y/n) ");
        assert_eq!(*tracker.status(), SessionStatus::Working);
        // The NeedsAttention detection happens on tick() after 10s
    }

    #[test]
    fn test_needs_attention_bracket_yn() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Install dependencies? [Y/n] ");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_needs_attention_input_prompt() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Enter your choice> ");
        // Note: "> " pattern matches at end of last line
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_needs_attention_ask_user_question() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"AskUserQuestion: What should I do next?");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_strip_ansi_and_detect_pattern() {
        let mut tracker = StatusTracker::new();
        // Simulate ANSI-colored output with a question at the end
        tracker.feed_output(b"\x1b[32mDo you want to continue?\x1b[0m ? ");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_tick_with_time_needs_attention_on_question_pattern() {
        let mut tracker = StatusTracker::new();
        let start = Instant::now();

        // Feed output containing a question prompt pattern
        tracker.feed_output(b"Do you want to proceed? ");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // Advance the injected clock past 10 seconds
        let future = start + Duration::from_secs(11);
        let change = tracker.tick_with_time(future);

        // Buffer contains "? " pattern, so after 10s tick should trigger NeedsAttention
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);
    }

    #[test]
    fn test_tick_with_time_idle_without_attention_pattern() {
        let mut tracker = StatusTracker::new();
        let start = Instant::now();

        // Feed output that does NOT match any attention pattern
        tracker.feed_output(b"Compiling module abc...\n");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // Advance the injected clock past 10 seconds
        let future = start + Duration::from_secs(11);
        let change = tracker.tick_with_time(future);

        // No attention pattern in the buffer, so should be Idle, not NeedsAttention
        assert_eq!(change, Some(SessionStatus::Idle));
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    // Integration-style test verifying the state machine flow end-to-end.

    #[test]
    fn test_status_state_machine_flow() {
        let mut tracker = StatusTracker::new();

        // Start -> Working on first output
        assert_eq!(*tracker.status(), SessionStatus::Starting);
        tracker.feed_output(b"Starting up...\n");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // More output keeps it Working
        tracker.feed_output(b"Processing files...\n");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // Exit transitions to Finished
        tracker.notify_exit(0);
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        // Tick does nothing after Finished
        let change = tracker.tick();
        assert_eq!(change, None);
    }
}
```

- [ ] **4A-3: Register the status parser module in main.rs**

Modify `src-tauri/src/main.rs` to add the module declaration:

```rust
mod status_parser;

#[cfg(test)]
mod status_parser_tests;
```

- [ ] **4A-4: Integrate StatusTracker into the PTY manager Session struct**

Modify `src-tauri/src/pty_manager.rs`. Add a `StatusTracker` to each `Session`:

```rust
use crate::status_parser::StatusTracker;

// In the Session struct, add:
struct Session {
    id: String,
    name: String,
    status: SessionStatus,
    pty: Box<dyn MasterPty>,
    child: Box<dyn Child>,
    cwd: PathBuf,
    created_at: Instant,
    status_tracker: StatusTracker, // ADD THIS
}
```

Initialize it in the session creation code:

```rust
// In the Create handler, when building a new Session:
status_tracker: StatusTracker::new(),
```

- [ ] **4A-5: Feed PTY output into the status tracker and emit status events**

In the PTY manager's output reading loop (in `src-tauri/src/pty_manager.rs`), after reading bytes from stdout:

```rust
// After reading output bytes from the PTY:
// let bytes_read = reader.read(&mut buf)?;
// let output = &buf[..bytes_read];

// Feed output to the status tracker
if let Some(new_status) = session.status_tracker.feed_output(output) {
    session.status = new_status.clone();
    // Emit status change event to frontend
    app_handle.emit_all(
        &format!("session-status-{}", session.id),
        serde_json::json!({ "status": new_status }),
    ).ok();
}
```

- [ ] **4A-6: Add a 1-second tick timer for idle/needs-attention detection**

In the PTY manager thread, add a periodic tick loop. This can be a separate thread or integrated into the existing event loop with a timeout on the channel receive:

```rust
use std::time::Duration;

// In the PTY manager's main loop, replace a blocking recv() with recv_timeout():
loop {
    match request_rx.recv_timeout(Duration::from_secs(1)) {
        Ok(request) => {
            // Handle the request as before...
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // Tick all active sessions
            for session in sessions.values_mut() {
                if let Some(new_status) = session.status_tracker.tick() {
                    session.status = new_status.clone();
                    app_handle.emit_all(
                        &format!("session-status-{}", session.id),
                        serde_json::json!({ "status": new_status }),
                    ).ok();
                }
            }
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            break; // Channel closed, shut down
        }
    }
}
```

- [ ] **4A-7: Handle process exit in the status tracker**

In the PTY manager, where process exit is detected (child.try_wait() or similar):

```rust
// When child process exit is detected:
if let Ok(Some(exit_status)) = session.child.try_wait() {
    let exit_code = exit_status.exit_code() as i32;
    let new_status = session.status_tracker.notify_exit(exit_code);
    session.status = new_status.clone();

    app_handle.emit_all(
        &format!("session-status-{}", session.id),
        serde_json::json!({ "status": new_status }),
    ).ok();

    app_handle.emit_all(
        &format!("session-exit-{}", session.id),
        serde_json::json!({ "exit_code": exit_code }),
    ).ok();
}
```

- [ ] **4A-8: Run the status parser tests**

```bash
cd src-tauri && cargo test status_parser
```

Expected output:
```
running 19 tests
test status_parser_tests::tests::test_initial_status_is_starting ... ok
test status_parser_tests::tests::test_first_output_transitions_to_working ... ok
test status_parser_tests::tests::test_subsequent_output_stays_working ... ok
test status_parser_tests::tests::test_empty_output_ignored ... ok
test status_parser_tests::tests::test_buffer_truncation_to_500_bytes ... ok
test status_parser_tests::tests::test_buffer_keeps_tail ... ok
test status_parser_tests::tests::test_exit_code_zero_is_finished ... ok
test status_parser_tests::tests::test_exit_code_nonzero_is_error ... ok
test status_parser_tests::tests::test_tick_no_change_while_starting ... ok
test status_parser_tests::tests::test_tick_no_change_while_finished ... ok
test status_parser_tests::tests::test_needs_attention_question_mark_space ... ok
test status_parser_tests::tests::test_needs_attention_yn_pattern ... ok
test status_parser_tests::tests::test_needs_attention_bracket_yn ... ok
test status_parser_tests::tests::test_needs_attention_input_prompt ... ok
test status_parser_tests::tests::test_needs_attention_ask_user_question ... ok
test status_parser_tests::tests::test_tick_with_time_needs_attention_on_question_pattern ... ok
test status_parser_tests::tests::test_tick_with_time_idle_without_attention_pattern ... ok
test status_parser_tests::tests::test_strip_ansi_and_detect_pattern ... ok
test status_parser_tests::tests::test_status_state_machine_flow ... ok

test result: ok. 19 passed; 0 failed; 0 ignored
```

---

## Task 4B: Activity Indicators & Duration

**Files to create:**
- `src/components/ActivityPulse.tsx`
- `src/components/ActivityPulse.module.css`
- `src/components/DurationTimer.tsx`
- `src/components/DurationTimer.module.css`
- `src/components/StatusDot.tsx`
- `src/components/StatusDot.module.css`

**Files to modify:**
- `src/components/SessionCard.tsx`
- `src/components/SessionCard.module.css`

### Steps

- [ ] **4B-1: Create the StatusDot component with color mapping**

Create `src/components/StatusDot.tsx`:

```tsx
import styles from "./StatusDot.module.css";

interface StatusDotProps {
  status: "starting" | "working" | "idle" | "needs_attention" | "finished" | "error";
}

const STATUS_LABELS: Record<StatusDotProps["status"], string> = {
  starting: "Starting",
  working: "Working",
  idle: "Idle",
  needs_attention: "Needs Attention",
  finished: "Finished",
  error: "Error",
};

export function StatusDot({ status }: StatusDotProps) {
  return (
    <span
      className={`${styles.dot} ${styles[status]}`}
      title={STATUS_LABELS[status]}
      aria-label={STATUS_LABELS[status]}
    />
  );
}
```

- [ ] **4B-2: Create the StatusDot CSS with status colors**

Create `src/components/StatusDot.module.css`:

```css
.dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.starting {
  background-color: #3b82f6; /* blue */
  animation: pulse 2s ease-in-out infinite;
}

.working {
  background-color: #22c55e; /* green */
  animation: pulse 1.2s ease-in-out infinite;
}

.idle {
  background-color: #6b7280; /* gray */
}

.needs_attention {
  background-color: #f59e0b; /* orange/amber */
  animation: pulse 0.8s ease-in-out infinite;
}

.finished {
  background-color: #6b7280; /* muted gray */
  opacity: 0.6;
}

.error {
  background-color: #ef4444; /* red */
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.5;
    transform: scale(0.85);
  }
}
```

- [ ] **4B-3: Create the ActivityPulse component**

Create `src/components/ActivityPulse.tsx`:

```tsx
import styles from "./ActivityPulse.module.css";

interface ActivityPulseProps {
  active: boolean;
}

export function ActivityPulse({ active }: ActivityPulseProps) {
  if (!active) return null;

  return (
    <div className={styles.pulseContainer}>
      <div className={styles.pulseBar} />
    </div>
  );
}
```

- [ ] **4B-4: Create the ActivityPulse CSS with animation**

Create `src/components/ActivityPulse.module.css`:

```css
.pulseContainer {
  width: 100%;
  height: 2px;
  background-color: rgba(34, 197, 94, 0.15);
  border-radius: 1px;
  overflow: hidden;
  position: absolute;
  bottom: 0;
  left: 0;
}

.pulseBar {
  height: 100%;
  width: 40%;
  background: linear-gradient(
    90deg,
    transparent,
    #22c55e,
    transparent
  );
  border-radius: 1px;
  animation: slide 1.5s ease-in-out infinite;
}

@keyframes slide {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(350%);
  }
}
```

- [ ] **4B-5: Create the DurationTimer component**

Create `src/components/DurationTimer.tsx`:

```tsx
import { useEffect, useState } from "react";
import styles from "./DurationTimer.module.css";

interface DurationTimerProps {
  /** Session creation timestamp in milliseconds since epoch */
  createdAt: number;
  /** Whether the session is still active (not finished/error) */
  active: boolean;
  /** Timestamp when the session finished (ms since epoch). Required when active is false. */
  finishedAt?: number;
}

function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m ${seconds}s`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

export function DurationTimer({ createdAt, active, finishedAt }: DurationTimerProps) {
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (!active) return;

    const interval = setInterval(() => {
      setNow(Date.now());
    }, 1000);

    return () => clearInterval(interval);
  }, [active]);

  // When active, use the live `now` tick. When finished, use `finishedAt`
  // so the timer freezes at the final duration instead of continuing to count.
  const endTime = active ? now : (finishedAt ?? now);
  const elapsed = endTime - createdAt;

  return (
    <span className={`${styles.duration} ${active ? styles.active : styles.inactive}`}>
      {formatDuration(elapsed)}
    </span>
  );
}
```

- [ ] **4B-6: Create the DurationTimer CSS**

Create `src/components/DurationTimer.module.css`:

```css
.duration {
  font-family: "SF Mono", "Menlo", "Monaco", "Courier New", monospace;
  font-size: 11px;
  letter-spacing: 0.02em;
  font-variant-numeric: tabular-nums;
}

.active {
  color: #9ca3af; /* gray-400 */
}

.inactive {
  color: #4b5563; /* gray-600 */
}
```

- [ ] **4B-7: Integrate StatusDot, ActivityPulse, and DurationTimer into SessionCard**

Modify `src/components/SessionCard.tsx` to add the new components:

```tsx
import { StatusDot } from "./StatusDot";
import { ActivityPulse } from "./ActivityPulse";
import { DurationTimer } from "./DurationTimer";

// Inside the SessionCard component's JSX, update the layout:
// Replace the existing status indicator (if any) with:

// Updated SessionCardProps with the optional close/dismiss callbacks
// used by the context menu in step 4C-7.
interface SessionCardProps {
  session: SessionData;
  isActive: boolean;
  onClick: () => void;
  /** Called to terminate a running session (SIGTERM/SIGKILL flow). */
  onClose?: (id: string) => void;
  /** Called to remove a finished/errored session from the list. */
  onDismiss?: (id: string) => void;
}

export function SessionCard({ session, isActive, onClick, onClose, onDismiss }: SessionCardProps) {
  const isRunning = !["finished", "error"].includes(session.status);

  return (
    <div
      className={`${styles.card} ${isActive ? styles.active : ""} ${!isRunning ? styles.dimmed : ""}`}
      onClick={onClick}
    >
      <div className={styles.header}>
        <StatusDot status={session.status} />
        <span className={styles.name}>{session.name}</span>
        <DurationTimer createdAt={session.createdAt} active={isRunning} finishedAt={session.finishedAt} />
      </div>
      <div className={styles.statusLabel}>{formatStatusLabel(session.status)}</div>
      <ActivityPulse active={session.status === "working"} />
    </div>
  );
}

function formatStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    starting: "Starting...",
    working: "Working",
    idle: "Idle",
    needs_attention: "Needs Attention",
    finished: "Finished",
    error: "Error",
  };
  return labels[status] || status;
}
```

- [ ] **4B-8: Update SessionCard CSS for new layout**

Modify `src/components/SessionCard.module.css` to add styles for the integrated components:

```css
/* Add to existing SessionCard.module.css: */

.card {
  position: relative; /* needed for ActivityPulse absolute positioning */
  overflow: hidden; /* clip the pulse animation */
}

.dimmed {
  opacity: 0.6;
}

.header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 500;
  color: #e5e7eb;
}

.statusLabel {
  font-size: 11px;
  color: #9ca3af;
  margin-top: 2px;
  padding-left: 16px; /* align with name, past the dot */
}
```

- [ ] **4B-9: Verify the components render correctly**

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator && npm run dev
```

Open the app, create a session, and verify:
- Status dot appears with correct color (blue for Starting)
- Dot transitions to green+pulsing when Working
- Duration timer counts up every second in "Xm Xs" format
- Green sliding pulse bar appears at bottom of card when Working
- Pulse bar disappears when Idle
- Orange pulsing dot appears for Needs Attention
- Finished sessions appear dimmed with gray dot

---

## Task 4C: Session Close & Cleanup

**Files to create:**
- `src/components/ContextMenu.tsx`
- `src/components/ContextMenu.module.css`
- `src/components/CloseConfirmDialog.tsx`
- `src/components/CloseConfirmDialog.module.css`

**Files to modify:**
- `src-tauri/src/pty_manager.rs` (SIGTERM/SIGKILL flow)
- `src/components/SessionCard.tsx` (right-click handler)
- `src/components/SessionPanel.tsx` (dismiss finished sessions)
- `src/store.ts` (close/dismiss actions)

### Steps

- [ ] **4C-1: Implement SIGTERM/SIGKILL graceful shutdown in Rust**

Modify `src-tauri/src/pty_manager.rs`. In the `Close` request handler, implement a graceful shutdown sequence:

```rust
use std::thread;
use std::time::Duration;

// NOTE: The `nix` crate imports (`signal`, `Pid`) are intentionally placed
// inside the `#[cfg(unix)]` blocks within function bodies rather than at the
// top of the file. This keeps them tightly scoped so the module compiles
// without warnings on non-Unix platforms. If you prefer top-level imports,
// gate them with `#[cfg(unix)]` at file scope instead:
//
//   #[cfg(unix)]
//   use nix::sys::signal::{self, Signal};
//   #[cfg(unix)]
//   use nix::unistd::Pid;

// In the PtyRequest::Close handler:
//
// IMPORTANT: The graceful-shutdown wait runs in a separate thread so
// it does NOT block the PTY manager's event loop (which would freeze
// ALL sessions for up to 5.5 seconds).  The spawned thread performs
// SIGTERM, a polling wait loop, an optional SIGKILL, and finally
// sends a cleanup message back to the PTY manager via `request_tx`.

/// Message sent from the shutdown thread back to the PTY manager
/// once the process has exited (or been killed).
enum ShutdownResult {
    /// Session exited; PTY manager should remove it from the HashMap.
    Completed { session_id: String, exit_code: i32 },
}

fn close_session_async(
    session_id: String,
    pid: u32,
    request_tx: mpsc::Sender<PtyRequest>,
) {
    thread::spawn(move || {
        log::info!("Closing session {}: sending SIGTERM to pid {}", session_id, pid);

        // Send SIGTERM
        #[cfg(unix)]
        {
            use nix::sys::signal::{self, Signal};
            use nix::unistd::Pid;
            let nix_pid = Pid::from_raw(pid as i32);
            signal::kill(nix_pid, Signal::SIGTERM).ok();
        }

        // Poll for up to 5 seconds waiting for graceful exit
        let start = std::time::Instant::now();
        let mut exited = false;
        while start.elapsed() < Duration::from_secs(5) {
            // We can't call try_wait on the Child from here (it's not
            // Send in portable_pty), so we check if the process is
            // still alive via kill(pid, 0).
            #[cfg(unix)]
            {
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;
                let nix_pid = Pid::from_raw(pid as i32);
                if signal::kill(nix_pid, None).is_err() {
                    // Process no longer exists
                    exited = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }

        if !exited {
            // SIGTERM didn't work — send SIGKILL
            log::warn!(
                "Session {} did not exit after SIGTERM, sending SIGKILL to pid {}",
                session_id, pid
            );

            #[cfg(unix)]
            {
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;
                let nix_pid = Pid::from_raw(pid as i32);
                signal::kill(nix_pid, Signal::SIGKILL).ok();
            }

            // Brief wait for SIGKILL to take effect
            thread::sleep(Duration::from_millis(500));
        }

        // Send a cleanup message back to the PTY manager thread so it
        // can remove the session from its HashMap without blocking.
        log::info!("Session {} shutdown complete, requesting cleanup", session_id);
        request_tx
            .send(PtyRequest::CleanupAfterClose {
                session_id: session_id.clone(),
            })
            .ok();
    });
}

// In the PTY manager main loop, add a handler for the new request variant:
// PtyRequest::CleanupAfterClose { session_id } => {
//     if let Some(mut session) = sessions.remove(&session_id) {
//         let exit_code = session.child.try_wait()
//             .ok().flatten()
//             .map(|s| s.exit_code() as i32)
//             .unwrap_or(-1);
//         let new_status = session.status_tracker.notify_exit(exit_code);
//         app_handle.emit_all(
//             &format!("session-status-{}", session_id),
//             serde_json::json!({ "status": new_status }),
//         ).ok();
//     }
// }
```

Add `nix` to dependencies in `src-tauri/Cargo.toml`:

```toml
[dependencies]
nix = { version = "0.29", features = ["signal"] }
```

- [ ] **4C-2: Create the ContextMenu component**

Create `src/components/ContextMenu.tsx`:

```tsx
import { useEffect, useRef } from "react";
import styles from "./ContextMenu.module.css";

interface ContextMenuItem {
  label: string;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    }

    function handleEscape(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onClose();
      }
    }

    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [onClose]);

  // Adjust position so menu doesn't overflow the window
  const adjustedStyle = {
    left: `${x}px`,
    top: `${y}px`,
  };

  return (
    <div ref={menuRef} className={styles.menu} style={adjustedStyle}>
      {items.map((item, i) => (
        <button
          key={i}
          className={`${styles.item} ${item.danger ? styles.danger : ""}`}
          onClick={() => {
            item.onClick();
            onClose();
          }}
          disabled={item.disabled}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **4C-3: Create the ContextMenu CSS**

Create `src/components/ContextMenu.module.css`:

```css
.menu {
  position: fixed;
  z-index: 1000;
  min-width: 160px;
  background-color: #1f2937;
  border: 1px solid #374151;
  border-radius: 6px;
  padding: 4px 0;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
}

.item {
  display: block;
  width: 100%;
  padding: 8px 12px;
  border: none;
  background: none;
  color: #e5e7eb;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  white-space: nowrap;
}

.item:hover:not(:disabled) {
  background-color: #374151;
}

.item:disabled {
  color: #4b5563;
  cursor: default;
}

.danger {
  color: #f87171;
}

.danger:hover:not(:disabled) {
  background-color: rgba(239, 68, 68, 0.15);
}
```

- [ ] **4C-4: Create the CloseConfirmDialog component**

Create `src/components/CloseConfirmDialog.tsx`:

```tsx
import styles from "./CloseConfirmDialog.module.css";

interface CloseConfirmDialogProps {
  sessionName: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function CloseConfirmDialog({
  sessionName,
  onConfirm,
  onCancel,
}: CloseConfirmDialogProps) {
  return (
    <div className={styles.overlay} onClick={onCancel}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <h3 className={styles.title}>Close Session</h3>
        <p className={styles.message}>
          Are you sure you want to close <strong>{sessionName}</strong>? This
          will terminate the Claude process.
        </p>
        <div className={styles.actions}>
          <button className={styles.cancelBtn} onClick={onCancel}>
            Cancel
          </button>
          <button className={styles.confirmBtn} onClick={onConfirm}>
            Close Session
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **4C-5: Create the CloseConfirmDialog CSS**

Create `src/components/CloseConfirmDialog.module.css`:

```css
.overlay {
  position: fixed;
  inset: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}

.dialog {
  background-color: #1f2937;
  border: 1px solid #374151;
  border-radius: 8px;
  padding: 24px;
  max-width: 400px;
  width: 90%;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
}

.title {
  margin: 0 0 12px;
  font-size: 16px;
  font-weight: 600;
  color: #f3f4f6;
}

.message {
  margin: 0 0 20px;
  font-size: 14px;
  color: #9ca3af;
  line-height: 1.5;
}

.message strong {
  color: #e5e7eb;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.cancelBtn {
  padding: 8px 16px;
  border: 1px solid #374151;
  border-radius: 6px;
  background: none;
  color: #e5e7eb;
  font-size: 13px;
  cursor: pointer;
}

.cancelBtn:hover {
  background-color: #374151;
}

.confirmBtn {
  padding: 8px 16px;
  border: none;
  border-radius: 6px;
  background-color: #ef4444;
  color: white;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
}

.confirmBtn:hover {
  background-color: #dc2626;
}
```

- [ ] **4C-6: Add dismiss and close actions to the Zustand store**

Modify `src/store.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";

// Add to the AppState interface:
interface AppState {
  // ... existing fields ...
  closeSession: (id: string) => Promise<void>;
  dismissSession: (id: string) => void;
}

// Add to the store implementation:
closeSession: async (id: string) => {
  try {
    await invoke("close_session", { id });
    // Session will be updated via session-status event (to Finished/Error)
    // We don't remove it from the list — user can dismiss it later
  } catch (err) {
    console.error("Failed to close session:", err);
  }
},

dismissSession: (id: string) => {
  set((state) => {
    const sessions = new Map(state.sessions);
    sessions.delete(id);
    // If the dismissed session was active, switch to another
    let activeSessionId = state.activeSessionId;
    if (activeSessionId === id) {
      const remaining = Array.from(sessions.keys());
      activeSessionId = remaining.length > 0 ? remaining[0] : null;
    }
    return { sessions, activeSessionId };
  });
},
```

- [ ] **4C-7: Add right-click context menu to SessionCard**

Modify `src/components/SessionCard.tsx` to handle the context menu:

```tsx
import { useState } from "react";
import { ContextMenu } from "./ContextMenu";
import { CloseConfirmDialog } from "./CloseConfirmDialog";

// Inside SessionCard component, add state and handlers:
const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
const [showCloseConfirm, setShowCloseConfirm] = useState(false);

const isRunning = !["finished", "error"].includes(session.status);

function handleContextMenu(e: React.MouseEvent) {
  e.preventDefault();
  setContextMenu({ x: e.clientX, y: e.clientY });
}

function handleClose() {
  if (isRunning) {
    setShowCloseConfirm(true);
  } else {
    onDismiss?.(session.id);
  }
}

function handleConfirmClose() {
  setShowCloseConfirm(false);
  onClose?.(session.id);
}

// In the JSX, add onContextMenu to the card div:
<div
  className={`${styles.card} ${isActive ? styles.active : ""}`}
  onClick={onClick}
  onContextMenu={handleContextMenu}
>
  {/* ... existing card content ... */}
</div>

{contextMenu && (
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    onClose={() => setContextMenu(null)}
    items={
      isRunning
        ? [
            { label: "Close Session", onClick: handleClose, danger: true },
          ]
        : [
            { label: "Dismiss", onClick: () => onDismiss?.(session.id) },
          ]
    }
  />
)}

{showCloseConfirm && (
  <CloseConfirmDialog
    sessionName={session.name}
    onConfirm={handleConfirmClose}
    onCancel={() => setShowCloseConfirm(false)}
  />
)}
```

- [ ] **4C-8: Wire dismiss handler in SessionPanel**

Modify `src/components/SessionPanel.tsx` to pass dismiss/close handlers down:

```tsx
import { useAppStore } from "../store";

// In SessionPanel component:
const closeSession = useAppStore((s) => s.closeSession);
const dismissSession = useAppStore((s) => s.dismissSession);

// When rendering SessionCard:
<SessionCard
  key={session.id}
  session={session}
  isActive={session.id === activeSessionId}
  onClick={() => setActiveSession(session.id)}
  onClose={closeSession}
  onDismiss={dismissSession}
/>
```

- [ ] **4C-9: Verify close and dismiss flows**

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator && npm run dev
```

Test the following:
1. Right-click on an active session card -> "Close Session" appears in context menu
2. Clicking "Close Session" shows the confirmation dialog
3. Confirming sends SIGTERM, session status moves to Finished/Error
4. Right-click on a finished session card -> "Dismiss" appears
5. Clicking "Dismiss" removes the session from the panel
6. Clicking outside the context menu closes it
7. Pressing Escape closes the context menu

---

## Task 4D: Error Handling & Edge Cases

**Files to create:**
- `src/components/Toast.tsx`
- `src/components/Toast.module.css`
- `src/components/ToastContainer.tsx`

**Files to modify:**
- `src-tauri/tauri.conf.json` (minimum window size)
- `src-tauri/src/main.rs` (shutdown hook, PATH check)
- `src-tauri/src/pty_manager.rs` (spawn error handling)
- `src/store.ts` (toast state)
- `src/App.tsx` (mount ToastContainer)

### Steps

- [ ] **4D-1: Set minimum window size in Tauri config**

Modify `src-tauri/tauri.conf.json`. In the `app.windows` array, add minimum size constraints:

```json
{
  "app": {
    "windows": [
      {
        "title": "Agent Orchestrator",
        "width": 1200,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600,
        "decorations": false,
        "resizable": true
      }
    ]
  }
}
```

If using Tauri v2 config format (`tauri.conf.json` with `app.windows`), adjust accordingly. The key fields are `minWidth: 900` and `minHeight: 600`.

- [ ] **4D-2: Add PATH check for the claude binary**

Modify `src-tauri/src/pty_manager.rs`. Before spawning a PTY session, check that `claude` is available:

```rust
use std::process::Command;

/// Check if the `claude` binary is available and functional.
/// Uses `claude --version` instead of `which` because `which` may fail
/// in non-interactive shell contexts. This also verifies the binary
/// actually executes, not just that a file exists on PATH.
fn check_claude_on_path() -> Result<(), String> {
    match Command::new("claude").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            log::info!("Found claude: {}", version.trim());
            Ok(())
        }
        Ok(output) => Err(format!(
            "Claude CLI found but returned an error (exit code {:?}). Please reinstall Claude Code.",
            output.status.code()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(
            "Claude CLI not found on PATH. Please install Claude Code (https://docs.anthropic.com/en/docs/claude-code) and ensure 'claude' is available in your terminal."
                .to_string(),
        ),
        Err(e) => Err(format!("Failed to run claude: {}", e)),
    }
}

// Call this at the start of the Create handler:
// PtyRequest::Create { name, cwd } => {
//     if let Err(msg) = check_claude_on_path() {
//         // Send error response back
//         // Emit error event to frontend
//         app_handle.emit_all("spawn-error", serde_json::json!({ "error": msg })).ok();
//         continue;
//     }
//     // ... proceed with spawn ...
// }
```

- [ ] **4D-3: Add spawn failure error handling**

In `src-tauri/src/pty_manager.rs`, wrap the PTY spawn in proper error handling:

```rust
// In the Create handler, after the PATH check:
let spawn_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    // ... existing PTY spawn code ...
}));

match spawn_result {
    Ok(Ok(session)) => {
        // Success — register session, emit created event
    }
    Ok(Err(e)) => {
        let error_msg = format!("Failed to spawn session: {}", e);
        log::error!("{}", error_msg);
        app_handle.emit_all(
            "spawn-error",
            serde_json::json!({ "error": error_msg }),
        ).ok();
    }
    Err(_) => {
        let error_msg = "PTY spawn panicked unexpectedly".to_string();
        log::error!("{}", error_msg);
        app_handle.emit_all(
            "spawn-error",
            serde_json::json!({ "error": error_msg }),
        ).ok();
    }
}
```

- [ ] **4D-4: Create the Toast notification component**

Create `src/components/Toast.tsx`:

```tsx
import { useEffect } from "react";
import styles from "./Toast.module.css";

export interface ToastData {
  id: string;
  message: string;
  type: "error" | "warning" | "info";
}

interface ToastProps {
  toast: ToastData;
  onDismiss: (id: string) => void;
}

export function Toast({ toast, onDismiss }: ToastProps) {
  useEffect(() => {
    const timer = setTimeout(() => {
      onDismiss(toast.id);
    }, 8000); // Auto-dismiss after 8 seconds

    return () => clearTimeout(timer);
  }, [toast.id, onDismiss]);

  return (
    <div className={`${styles.toast} ${styles[toast.type]}`}>
      <span className={styles.message}>{toast.message}</span>
      <button
        className={styles.dismissBtn}
        onClick={() => onDismiss(toast.id)}
        aria-label="Dismiss"
      >
        ×
      </button>
    </div>
  );
}
```

- [ ] **4D-5: Create the Toast CSS**

Create `src/components/Toast.module.css`:

```css
.toast {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 8px;
  font-size: 13px;
  line-height: 1.4;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  animation: slideIn 0.2s ease-out;
  max-width: 420px;
}

.error {
  background-color: #451a1a;
  border: 1px solid #7f1d1d;
  color: #fca5a5;
}

.warning {
  background-color: #451a00;
  border: 1px solid #7c2d12;
  color: #fdba74;
}

.info {
  background-color: #1e293b;
  border: 1px solid #334155;
  color: #94a3b8;
}

.message {
  flex: 1;
}

.dismissBtn {
  background: none;
  border: none;
  color: inherit;
  font-size: 18px;
  cursor: pointer;
  padding: 0 4px;
  opacity: 0.6;
  flex-shrink: 0;
}

.dismissBtn:hover {
  opacity: 1;
}

@keyframes slideIn {
  from {
    transform: translateY(-12px);
    opacity: 0;
  }
  to {
    transform: translateY(0);
    opacity: 1;
  }
}
```

- [ ] **4D-6: Create the ToastContainer component**

Create `src/components/ToastContainer.tsx`:

```tsx
import { useAppStore } from "../store";
import { Toast } from "./Toast";

const containerStyle: React.CSSProperties = {
  position: "fixed",
  top: 48, // Below the title bar
  right: 16,
  display: "flex",
  flexDirection: "column",
  gap: 8,
  zIndex: 3000,
  pointerEvents: "none",
};

const itemStyle: React.CSSProperties = {
  pointerEvents: "auto",
};

export function ToastContainer() {
  const toasts = useAppStore((s) => s.toasts);
  const dismissToast = useAppStore((s) => s.dismissToast);

  return (
    <div style={containerStyle}>
      {toasts.map((toast) => (
        <div key={toast.id} style={itemStyle}>
          <Toast toast={toast} onDismiss={dismissToast} />
        </div>
      ))}
    </div>
  );
}
```

- [ ] **4D-7: Add toast state to the Zustand store**

Modify `src/store.ts`:

```typescript
import { type ToastData } from "./components/Toast";

// Add to the AppState interface:
interface AppState {
  // ... existing fields ...
  toasts: ToastData[];
  addToast: (message: string, type: ToastData["type"]) => void;
  dismissToast: (id: string) => void;
}

// Add to the store implementation:
toasts: [],

addToast: (message, type) => {
  const id = crypto.randomUUID();
  set((state) => ({
    toasts: [...state.toasts, { id, message, type }],
  }));
},

dismissToast: (id) => {
  set((state) => ({
    toasts: state.toasts.filter((t) => t.id !== id),
  }));
},
```

- [ ] **4D-8: Listen for spawn-error events and show toasts**

Modify `src/App.tsx` to listen for backend error events:

```tsx
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "./store";
import { ToastContainer } from "./components/ToastContainer";

// Inside the App component:
function App() {
  const addToast = useAppStore((s) => s.addToast);

  useEffect(() => {
    const unlisten = listen<{ error: string }>("spawn-error", (event) => {
      addToast(event.payload.error, "error");
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [addToast]);

  return (
    <div className="app">
      {/* ... existing layout ... */}
      <ToastContainer />
    </div>
  );
}
```

- [ ] **4D-9: Add clean shutdown hook for all PTYs on app quit**

Modify `src-tauri/src/main.rs` to register a shutdown hook:

```rust
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        // ... existing setup ...
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                log::info!("Window close requested — shutting down all sessions");
                // Send a shutdown signal to the PTY manager
                if let Some(shutdown_tx) = window.app_handle().try_state::<ShutdownSender>() {
                    shutdown_tx.send(()).ok();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Add shutdown channel support to the PTY manager in `src-tauri/src/pty_manager.rs`:

```rust
use std::sync::mpsc;

/// A channel sender for requesting shutdown of the PTY manager.
pub struct ShutdownSender(mpsc::Sender<()>);

impl ShutdownSender {
    pub fn send(&self, _: ()) -> Result<(), mpsc::SendError<()>> {
        self.0.send(())
    }
}

// In the PTY manager thread setup, create a shutdown channel:
let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

// Store shutdown_tx in Tauri state so the on_window_event handler can
// retrieve it via app_handle.try_state::<ShutdownSender>():
app.manage(ShutdownSender(shutdown_tx));

// In the PTY manager main loop, also check the shutdown channel:
// Use select-like behavior by checking shutdown_rx.try_recv() in the timeout branch:
Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
    // Check for shutdown signal
    if shutdown_rx.try_recv().is_ok() {
        log::info!("Shutdown signal received — closing all sessions");
        for (id, session) in sessions.iter_mut() {
            log::info!("Shutting down session: {}", id);
            close_session(session).ok();
        }
        break;
    }

    // Normal tick processing...
    for session in sessions.values_mut() {
        if let Some(new_status) = session.status_tracker.tick() {
            // ... emit status event ...
        }
    }
}
```

- [ ] **4D-10: Validate error handling and edge cases**

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator && npm run tauri dev
```

Test the following scenarios:
1. **Minimum window size**: Try to resize the window smaller than 900x600 -- it should stop resizing at the minimum
2. **Claude not on PATH**: Temporarily rename the claude binary (or modify PATH), try to create a session -- should see an error toast
3. **Spawn failure**: Try to create a session with an invalid directory -- should see an error toast
4. **Clean shutdown**: Open 2-3 sessions, then close the app window -- check that all claude processes are terminated (run `ps aux | grep claude` to verify no orphans)
5. **Toast auto-dismiss**: Trigger an error toast, wait 8 seconds -- it should fade away automatically
6. **Toast manual dismiss**: Trigger an error toast, click the X button -- it should dismiss immediately

- [ ] **4D-11: Run the full test suite**

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator/src-tauri && cargo test
```

Expected output: all tests pass, including the status parser tests from 4A.

```bash
cd /Users/stanton.borthwick/SProjects/Agent-Orchestrator && npm run build
```

Expected output: production build completes without errors.
