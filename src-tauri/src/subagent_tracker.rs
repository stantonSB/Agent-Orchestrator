use crate::status_parser::SessionStatus;
use std::collections::HashMap;
use std::time::Instant;

/// Metadata for a single detected subagent.
#[derive(Debug, Clone)]
pub struct SubagentInfo {
    pub claude_session_id: String,
    pub index: u16,
    pub status: SessionStatus,
    pub name: Option<String>,
    pub finished_at: Option<Instant>,
}

/// Serializable subagent info sent to the frontend via Tauri events.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubagentStatusPayload {
    pub id: String,
    pub index: u16,
    pub status: SessionStatus,
    pub name: Option<String>,
}

impl From<&SubagentInfo> for SubagentStatusPayload {
    fn from(info: &SubagentInfo) -> Self {
        Self {
            id: info.claude_session_id.clone(),
            index: info.index,
            status: info.status.clone(),
            name: info.name.clone(),
        }
    }
}

/// Tracks subagents for a single parent session.
///
/// The first `session_id` seen via `process_event` is recorded as the parent.
/// Subsequent new `session_id` values are registered as subagents.
pub struct SubagentMap {
    parent_claude_id: Option<String>,
    agents: HashMap<String, SubagentInfo>,
    next_index: u16,
    pending_events: Vec<(String, String)>, // (session_id, notification_type)
}

const MAX_PENDING: usize = 32;

impl SubagentMap {
    pub fn new() -> Self {
        Self {
            parent_claude_id: None,
            agents: HashMap::new(),
            next_index: 1,
            pending_events: Vec::new(),
        }
    }

    /// Returns the parent's Claude Code session_id, if established.
    pub fn parent_session_id(&self) -> Option<&str> {
        self.parent_claude_id.as_deref()
    }

    /// Returns a slice of all tracked subagents.
    pub fn subagents(&self) -> Vec<&SubagentInfo> {
        self.agents.values().collect()
    }

    /// Returns serializable payload for all subagents.
    pub fn payload(&self) -> Vec<SubagentStatusPayload> {
        let mut list: Vec<_> = self.agents.values().map(SubagentStatusPayload::from).collect();
        list.sort_by_key(|s| s.index);
        list
    }

    /// Returns true if any subagent is in NeedsAttention status.
    pub fn any_needs_attention(&self) -> bool {
        self.agents.values().any(|a| a.status == SessionStatus::NeedsAttention)
    }

    /// Process a hook event. Returns true if subagent state changed (triggering a re-emit).
    ///
    /// `claude_session_id`: the session_id from the hook JSON body
    /// `notification_type`: normalized event type (idle_prompt, permission_prompt, stop, etc.)
    pub fn process_event(&mut self, claude_session_id: &str, notification_type: &str) -> bool {
        // Establish parent identity on first event
        if self.parent_claude_id.is_none() {
            self.parent_claude_id = Some(claude_session_id.to_string());
            // Replay any pending events
            let pending = std::mem::take(&mut self.pending_events);
            let mut changed = false;
            for (sid, ntype) in pending {
                if sid != claude_session_id {
                    changed |= self.register_or_update_subagent(&sid, &ntype);
                }
            }
            return changed;
        }

        // Parent event — not our concern
        if self.parent_claude_id.as_deref() == Some(claude_session_id) {
            return false;
        }

        // Subagent event
        self.register_or_update_subagent(claude_session_id, notification_type)
    }

    /// Returns true if this session_id is the known parent.
    pub fn is_parent(&self, claude_session_id: &str) -> bool {
        self.parent_claude_id.as_deref() == Some(claude_session_id)
    }

    /// Returns true if parent identity has not been established yet.
    pub fn parent_unknown(&self) -> bool {
        self.parent_claude_id.is_none()
    }

    /// Buffer an event when parent identity is not yet established.
    pub fn buffer_event(&mut self, claude_session_id: &str, notification_type: &str) {
        if self.pending_events.len() >= MAX_PENDING {
            self.pending_events.remove(0);
        }
        self.pending_events.push((
            claude_session_id.to_string(),
            notification_type.to_string(),
        ));
    }

    fn register_or_update_subagent(&mut self, claude_session_id: &str, notification_type: &str) -> bool {
        if let Some(agent) = self.agents.get_mut(claude_session_id) {
            // Update existing subagent status
            let new_status = Self::notification_to_status(notification_type, &agent.status);
            if let Some(status) = new_status {
                let finished = status == SessionStatus::Finished;
                agent.status = status;
                if finished {
                    agent.finished_at = Some(Instant::now());
                }
                return true;
            }
            false
        } else {
            // New subagent detected
            let status = Self::initial_status(notification_type);
            let index = self.next_index;
            self.next_index += 1;
            let finished = status == SessionStatus::Finished;
            self.agents.insert(
                claude_session_id.to_string(),
                SubagentInfo {
                    claude_session_id: claude_session_id.to_string(),
                    index,
                    status,
                    name: None,
                    finished_at: if finished { Some(Instant::now()) } else { None },
                },
            );
            true
        }
    }

    fn notification_to_status(notification_type: &str, current: &SessionStatus) -> Option<SessionStatus> {
        match notification_type {
            "idle_prompt" | "stop" => match current {
                SessionStatus::Starting => Some(SessionStatus::Idle),
                SessionStatus::Working | SessionStatus::NeedsAttention | SessionStatus::Idle => {
                    Some(SessionStatus::Finished)
                }
                _ => None,
            },
            "permission_prompt" | "elicitation_dialog" => match current {
                SessionStatus::Working | SessionStatus::Starting | SessionStatus::Idle => {
                    Some(SessionStatus::NeedsAttention)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn initial_status(notification_type: &str) -> SessionStatus {
        match notification_type {
            "idle_prompt" | "stop" => SessionStatus::Idle,
            "permission_prompt" | "elicitation_dialog" => SessionStatus::NeedsAttention,
            _ => SessionStatus::Working,
        }
    }
}
