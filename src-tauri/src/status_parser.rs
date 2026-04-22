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
///   Starting       → Idle            (idle_prompt hook)
///   Starting       → NeedsAttention  (permission_prompt / elicitation_dialog hook)
///   Working        → Finished        (idle_prompt hook)
///   Working        → NeedsAttention  (permission_prompt / elicitation_dialog hook)
///   NeedsAttention → Finished        (idle_prompt hook)
///   Idle / Finished / NeedsAttention → Working (user presses Enter)
///   Any            → Finished / Error (process exits)
pub struct StatusTracker {
    status: SessionStatus,
}

impl StatusTracker {
    pub fn new() -> Self {
        Self {
            status: SessionStatus::Starting,
        }
    }

    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    /// Process a notification hook event from Claude Code.
    ///
    /// Returns `Some(new_status)` if the status changed, `None` otherwise.
    pub fn notify_hook_event(&mut self, notification_type: &str) -> Option<SessionStatus> {
        let new_status = match notification_type {
            "idle_prompt" => match self.status {
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
    /// to Working from Idle, Finished, or NeedsAttention.
    pub fn notify_user_input(&mut self, data: &[u8]) -> Option<SessionStatus> {
        if !data.contains(&b'\r') && !data.contains(&b'\n') {
            return None;
        }
        match self.status {
            SessionStatus::Idle | SessionStatus::Finished | SessionStatus::NeedsAttention => {
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
