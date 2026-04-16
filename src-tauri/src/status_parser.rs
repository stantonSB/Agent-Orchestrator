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
pub struct StatusTracker {
    buffer: Vec<u8>,
    max_buffer_size: usize,
    status: SessionStatus,
    last_output_at: Option<Instant>,
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

        self.buffer.extend_from_slice(data);
        if self.buffer.len() > self.max_buffer_size {
            let drain_count = self.buffer.len() - self.max_buffer_size;
            self.buffer.drain(..drain_count);
        }

        let old_status = self.status.clone();
        self.status = SessionStatus::Working;

        if old_status != self.status {
            Some(self.status.clone())
        } else {
            None
        }
    }

    /// Called periodically to check for time-based status transitions.
    /// Accepts an optional `now` parameter for testability.
    pub fn tick_with_time(&mut self, now: Instant) -> Option<SessionStatus> {
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
            if self.check_needs_attention() {
                self.status = SessionStatus::NeedsAttention;
            } else {
                self.status = SessionStatus::Idle;
            }
        } else if elapsed.as_secs() >= 3 {
            self.status = SessionStatus::Idle;
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

        false
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
