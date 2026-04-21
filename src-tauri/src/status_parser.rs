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

impl SessionStatus {
    /// Returns the snake_case string representation matching the serde output.
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Starting => "starting",
            SessionStatus::Working => "working",
            SessionStatus::Idle => "idle",
            SessionStatus::NeedsAttention => "needs_attention",
            SessionStatus::Finished => "finished",
            SessionStatus::Error => "error",
        }
    }
}

/// Tracks the output buffer and timing state for a single session.
///
/// State machine:
///   Starting  → Idle      (output settles after startup, user hasn't submitted)
///   Idle      → Working   (user presses Enter)
///   Working   → Finished  (idle prompt detected after 2s, or fallback 8s timeout)
///   Working   → NeedsAttention (output stops for 2s, question/approval pattern)
///   NeedsAttention → Working (user presses Enter to answer)
///   Finished  → Working   (user presses Enter for new task)
///   Any       → Finished/Error (process exits)
/// Spinner characters used by Claude Code to indicate active work.
/// Excludes `*` (U+002A) which appears in code comments, globs, etc.
const SPINNER_CHARS: &[char] = &['·', '✢', '✳', '✶', '✻', '✽', '●'];

pub struct StatusTracker {
    buffer: Vec<u8>,
    max_buffer_size: usize,
    status: SessionStatus,
    last_output_at: Option<Instant>,
    last_spinner_at: Option<Instant>,
    has_received_output: bool,
}

impl StatusTracker {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(512),
            max_buffer_size: 500,
            status: SessionStatus::Starting,
            last_output_at: None,
            last_spinner_at: None,
            has_received_output: false,
        }
    }

    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    /// Feed new output bytes into the tracker with an explicit timestamp.
    ///
    /// Updates the buffer and timestamp but does NOT change status.
    /// Status transitions are driven by user input and tick polling.
    /// Also scans for spinner characters to track active work.
    pub fn feed_output_with_time(&mut self, data: &[u8], now: Instant) -> Option<SessionStatus> {
        if data.is_empty() {
            return None;
        }

        self.has_received_output = true;
        self.last_output_at = Some(now);

        self.buffer.extend_from_slice(data);
        if self.buffer.len() > self.max_buffer_size {
            let drain_count = self.buffer.len() - self.max_buffer_size;
            self.buffer.drain(..drain_count);
        }

        // Scan incoming data for spinner characters
        if let Ok(text) = std::str::from_utf8(data) {
            if text.chars().any(|c| SPINNER_CHARS.contains(&c)) {
                self.last_spinner_at = Some(now);
            }
        } else {
            // Lossy fallback for non-UTF-8 data
            let text = String::from_utf8_lossy(data);
            if text.chars().any(|c| SPINNER_CHARS.contains(&c)) {
                self.last_spinner_at = Some(now);
            }
        }

        // Output alone does not change status.
        None
    }

    /// Feed new output bytes into the tracker.
    ///
    /// Convenience wrapper that calls `feed_output_with_time(data, Instant::now())`.
    pub fn feed_output(&mut self, data: &[u8]) -> Option<SessionStatus> {
        self.feed_output_with_time(data, Instant::now())
    }

    /// Notify the tracker that the user sent input to the PTY.
    ///
    /// If the input contains Enter (carriage return or newline), transitions
    /// to Working. This is the ONLY way to enter the Working state from
    /// Idle, Finished, or NeedsAttention.
    pub fn notify_user_input(&mut self, data: &[u8]) -> Option<SessionStatus> {
        let has_enter = data.iter().any(|&b| b == b'\r' || b == b'\n');
        if !has_enter {
            return None;
        }

        // Don't transition out of terminal states caused by process exit.
        if matches!(self.status, SessionStatus::Error) {
            return None;
        }

        let old_status = self.status.clone();
        self.status = SessionStatus::Working;
        // Reset the idle timer so we don't immediately transition to
        // Finished before Claude has a chance to start outputting.
        self.last_output_at = Some(Instant::now());
        // Clear spinner timestamp so stale spinner data from a previous
        // work cycle doesn't affect transitions in the next cycle.
        self.last_spinner_at = None;
        // Clear buffer so stale patterns (from previous prompts) don't
        // cause false NeedsAttention detections.
        self.buffer.clear();

        if old_status != self.status {
            Some(self.status.clone())
        } else {
            None
        }
    }

    /// Called periodically to check for time-based status transitions.
    /// Accepts an optional `now` parameter for testability.
    pub fn tick_with_time(&mut self, now: Instant) -> Option<SessionStatus> {
        // Terminal states: no transitions.
        if matches!(self.status, SessionStatus::Finished | SessionStatus::Error) {
            return None;
        }

        let Some(last_output) = self.last_output_at else {
            return None;
        };

        let elapsed = now.duration_since(last_output);
        let old_status = self.status.clone();

        match self.status {
            SessionStatus::Starting => {
                // After startup output settles (3s), transition to Idle.
                // The user hasn't submitted anything yet.
                if self.has_received_output && elapsed.as_secs() >= 3 {
                    self.status = SessionStatus::Idle;
                }
            }
            SessionStatus::Working => {
                // Signal 1: Spinner keepalive — Claude is actively working
                if let Some(last_spinner) = self.last_spinner_at {
                    if now.duration_since(last_spinner).as_millis() < 1500 {
                        return None; // Still spinning, definitely working
                    }
                }

                // Signal 2: NeedsAttention (2s quiet + question pattern)
                if elapsed.as_secs() >= 2 && self.check_needs_attention() {
                    self.status = SessionStatus::NeedsAttention;
                }
                // Signal 3: Finished via prompt detection (2s quiet + idle prompt)
                else if elapsed.as_secs() >= 2
                    && self.check_idle_prompt()
                    && !self.check_needs_attention()
                {
                    self.status = SessionStatus::Finished;
                }
                // Signal 4: Fallback (8s quiet, no signals)
                else if elapsed.as_secs() >= 8 {
                    self.status = SessionStatus::Finished;
                }
            }
            SessionStatus::Idle | SessionStatus::NeedsAttention => {
                // These states are stable:
                //   Idle: waiting for user to submit first task
                //   NeedsAttention: waiting for user to answer question/approval
                // Both exit via notify_user_input() → Working.
            }
            _ => {}
        }

        if old_status != self.status {
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
    pub fn notify_exit(&mut self, exit_code: i32) -> SessionStatus {
        self.status = if exit_code == 0 {
            SessionStatus::Finished
        } else {
            SessionStatus::Error
        };
        self.status.clone()
    }

    /// Check the output buffer for patterns indicating the agent is waiting
    /// for user input.
    fn check_needs_attention(&self) -> bool {
        let text = String::from_utf8_lossy(&self.buffer);
        let stripped = strip_ansi_escapes(&text);

        let last_line = stripped
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");

        if last_line.ends_with("? ") {
            return true;
        }

        if last_line.ends_with("> ") {
            return true;
        }

        let lower = stripped.to_lowercase();
        if lower.contains("(y/n)")
            || lower.contains("(yes/no)")
            || stripped.contains("[Y/n]")
            || stripped.contains("[y/N]")
            || stripped.contains("[Y/N]")
        {
            return true;
        }

        if stripped.contains("AskUserQuestion") {
            return true;
        }

        if lower.contains("do you want to proceed?")
            || lower.contains("needs your permission")
            || lower.contains("needs your approval")
            || lower.contains("needs your attention")
        {
            return true;
        }

        false
    }

    /// Check if the last non-empty line in the buffer is the idle prompt character.
    fn check_idle_prompt(&self) -> bool {
        let text = String::from_utf8_lossy(&self.buffer);
        let stripped = strip_ansi_escapes(&text);
        stripped
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| {
                let trimmed = line.trim();
                trimmed == "❯" || trimmed == "❯ "
            })
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub fn buffer_contents(&self) -> &[u8] {
        &self.buffer
    }
}

/// Strip ANSI escape sequences from a string.
fn strip_ansi_escapes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
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
                    chars.next();
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}
