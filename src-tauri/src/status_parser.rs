use crate::subagent_tracker::SubagentMap;

/// Session status as determined by the status tracker.
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

/// Tracks the lifecycle state of a single session via hook events and user input.
///
/// State machine:
///   Starting       → Idle            (idle_prompt hook or stop hook)
///   Starting       → NeedsAttention  (permission_prompt / elicitation_dialog hook)
///   Working        → Finished        (idle_prompt hook or stop hook)
///   Working        → Finished        (user presses Escape to interrupt)
///   Working        → NeedsAttention  (permission_prompt / elicitation_dialog hook)
///   NeedsAttention → Finished        (idle_prompt hook)
///   Starting       → Idle            (5-second startup timeout)
///   Starting / Idle / Finished / NeedsAttention → Working (user presses Enter)
///   Any            → Finished / Error (process exits)
pub struct StatusTracker {
    status: SessionStatus,
    subagent_map: SubagentMap,
}

impl StatusTracker {
    pub fn new() -> Self {
        Self {
            status: SessionStatus::Starting,
            subagent_map: SubagentMap::new(),
        }
    }

    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    pub fn subagent_map(&self) -> &SubagentMap {
        &self.subagent_map
    }

    pub fn subagent_map_mut(&mut self) -> &mut SubagentMap {
        &mut self.subagent_map
    }

    /// Unconditionally set the status if it differs from the current value.
    /// Returns `Some(new_status)` on change, `None` if already in that state.
    pub fn set_status(&mut self, new_status: SessionStatus) -> Option<SessionStatus> {
        if self.status == new_status {
            return None;
        }
        self.status = new_status.clone();
        Some(new_status)
    }

    /// Process a notification hook event from Claude Code.
    ///
    /// Returns `Some(new_status)` if the status changed, `None` otherwise.
    pub fn notify_hook_event(&mut self, notification_type: &str) -> Option<SessionStatus> {
        let new_status = match notification_type {
            // idle_prompt fires after Claude Code's hardcoded 60-second idle timeout.
            // stop fires immediately when Claude's agent loop completes.
            "idle_prompt" | "stop" => match self.status {
                SessionStatus::Starting => Some(SessionStatus::Idle),
                SessionStatus::Working | SessionStatus::NeedsAttention => {
                    Some(SessionStatus::Finished)
                }
                _ => None,
            },
            "permission_prompt" | "elicitation_dialog" => match self.status {
                SessionStatus::Working | SessionStatus::Starting => {
                    Some(SessionStatus::NeedsAttention)
                }
                _ => None,
            },
            _ => None,
        };

        if let Some(ref s) = new_status {
            self.status = s.clone();
        }
        new_status
    }

    /// Notify the tracker that the user sent input to the PTY.
    ///
    /// If the input contains Enter (carriage return or newline), transitions
    /// to Working from Starting, Idle, Finished, or NeedsAttention.
    ///
    /// If the input is a bare Escape key (single `\x1b` byte, not part of an
    /// escape sequence), transitions from Working → Finished.  This handles
    /// the case where the user presses Escape to interrupt Claude Code — the
    /// Stop hook does not fire reliably on user-initiated interrupts.
    ///
    /// Starting is included because Claude Code does not fire an `idle_prompt`
    /// notification on initial startup — only after processing at least one
    /// message.  Without this transition the status would stay "Starting"
    /// for the entire first request/response cycle.
    pub fn notify_user_input(&mut self, data: &[u8]) -> Option<SessionStatus> {
        // Bare Escape key (not part of a multi-byte escape sequence like arrow keys).
        // When the user presses Escape while Claude is working, it interrupts the
        // agent and returns to the prompt.
        if data == [0x1b] {
            return match self.status {
                SessionStatus::Working => {
                    self.status = SessionStatus::Finished;
                    Some(SessionStatus::Finished)
                }
                _ => None,
            };
        }

        if !data.contains(&b'\r') && !data.contains(&b'\n') {
            return None;
        }
        match self.status {
            SessionStatus::Starting
            | SessionStatus::Idle
            | SessionStatus::Finished
            | SessionStatus::NeedsAttention => {
                self.status = SessionStatus::Working;
                Some(SessionStatus::Working)
            }
            _ => None,
        }
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
}
