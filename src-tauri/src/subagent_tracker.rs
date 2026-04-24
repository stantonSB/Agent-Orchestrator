use crate::status_parser::SessionStatus;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Metadata for a single detected subagent.
#[derive(Debug, Clone)]
pub struct SubagentInfo {
    pub id: String,
    pub index: u16,
    pub status: SessionStatus,
    pub agent_type: String,
    pub display_name: Option<String>,
    pub created_at: u64,
    pub finished_at: Option<Instant>,
}

/// Serializable subagent info sent to the frontend via Tauri events.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubagentStatusPayload {
    pub id: String,
    pub index: u16,
    pub status: SessionStatus,
    pub name: Option<String>,
    pub created_at: u64,
}

impl From<&SubagentInfo> for SubagentStatusPayload {
    fn from(info: &SubagentInfo) -> Self {
        Self {
            id: info.id.clone(),
            index: info.index,
            status: info.status.clone(),
            name: Some(info.display_name.clone().unwrap_or_else(|| info.agent_type.clone())),
            created_at: info.created_at,
        }
    }
}

/// Tracks subagents for a single parent session.
///
/// Subagents are identified by `agent_type` from SubagentStart/SubagentStop
/// hook events. Since all subagents share the parent's `session_id`, we cannot
/// use session_id to distinguish them. Instead, we assign auto-incrementing
/// IDs and match SubagentStop to the oldest Working subagent of the same type.
pub struct SubagentMap {
    agents: Vec<SubagentInfo>,
    next_index: u16,
}

impl SubagentMap {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            next_index: 1,
        }
    }

    /// Returns a slice of all tracked subagents.
    pub fn subagents(&self) -> &[SubagentInfo] {
        &self.agents
    }

    /// Returns serializable payload for all subagents.
    pub fn payload(&self) -> Vec<SubagentStatusPayload> {
        let mut list: Vec<_> = self.agents.iter().map(SubagentStatusPayload::from).collect();
        list.sort_by_key(|s| s.index);
        list
    }

    /// Returns true if any subagent is in NeedsAttention status.
    pub fn any_needs_attention(&self) -> bool {
        self.agents.iter().any(|a| a.status == SessionStatus::NeedsAttention)
    }

    /// Register a new subagent when SubagentStart fires. Returns true (state changed).
    pub fn process_start(&mut self, agent_type: &str, display_name: Option<String>) -> bool {
        let index = self.next_index;
        self.next_index += 1;
        let id = format!("subagent-{}", index);
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.agents.push(SubagentInfo {
            id,
            index,
            status: SessionStatus::Working,
            agent_type: agent_type.to_string(),
            display_name,
            created_at,
            finished_at: None,
        });
        true
    }

    /// Mark the oldest Working subagent of the given type as Finished.
    /// Returns true if state changed.
    pub fn process_stop(&mut self, agent_type: &str) -> bool {
        // Find the oldest (lowest index) Working subagent with matching type
        if let Some(agent) = self.agents.iter_mut()
            .filter(|a| a.agent_type == agent_type && a.status == SessionStatus::Working)
            .min_by_key(|a| a.index)
        {
            agent.status = SessionStatus::Finished;
            agent.finished_at = Some(Instant::now());
            return true;
        }

        // Fallback: if no Working agent of that type, try any Working agent
        // (handles case where agent_type might differ between start/stop)
        if let Some(agent) = self.agents.iter_mut()
            .filter(|a| a.status == SessionStatus::Working)
            .min_by_key(|a| a.index)
        {
            agent.status = SessionStatus::Finished;
            agent.finished_at = Some(Instant::now());
            return true;
        }

        false
    }

    /// Returns true if there are any active (non-finished) subagents.
    pub fn has_active(&self) -> bool {
        self.agents.iter().any(|a| {
            a.status != SessionStatus::Finished && a.status != SessionStatus::Error
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_start_registers_subagent() {
        let mut map = SubagentMap::new();
        assert!(map.process_start("code-reviewer", None));
        assert_eq!(map.subagents().len(), 1);
        assert_eq!(map.subagents()[0].agent_type, "code-reviewer");
        assert_eq!(map.subagents()[0].status, SessionStatus::Working);
        assert_eq!(map.subagents()[0].index, 1);
    }

    #[test]
    fn test_multiple_starts() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        map.process_start("code-reviewer", None);
        map.process_start("Explore", None);
        assert_eq!(map.subagents().len(), 3);
        assert_eq!(map.subagents()[0].index, 1);
        assert_eq!(map.subagents()[1].index, 2);
        assert_eq!(map.subagents()[2].index, 3);
    }

    #[test]
    fn test_process_stop_marks_oldest_working() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        map.process_start("code-reviewer", None);

        assert!(map.process_stop("code-reviewer"));
        // Oldest (index 1) should be finished
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
        assert_eq!(map.subagents()[1].status, SessionStatus::Working);
    }

    #[test]
    fn test_process_stop_fifo_ordering() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        map.process_start("code-reviewer", None);
        map.process_start("code-reviewer", None);

        map.process_stop("code-reviewer");
        map.process_stop("code-reviewer");

        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
        assert_eq!(map.subagents()[1].status, SessionStatus::Finished);
        assert_eq!(map.subagents()[2].status, SessionStatus::Working);
    }

    #[test]
    fn test_process_stop_no_match_returns_false() {
        let mut map = SubagentMap::new();
        assert!(!map.process_stop("code-reviewer"));
    }

    #[test]
    fn test_process_stop_fallback_to_any_working() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        // Stop with different type — falls back to any working agent
        assert!(map.process_stop("unknown-type"));
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
    }

    #[test]
    fn test_any_needs_attention() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        assert!(!map.any_needs_attention());

        // Manually set to NeedsAttention for testing
        map.agents[0].status = SessionStatus::NeedsAttention;
        assert!(map.any_needs_attention());
    }

    #[test]
    fn test_payload_sorted_by_index() {
        let mut map = SubagentMap::new();
        map.process_start("Explore", None);
        map.process_start("code-reviewer", None);
        let payload = map.payload();
        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0].index, 1);
        assert_eq!(payload[0].name, Some("Explore".to_string()));
        assert_eq!(payload[1].index, 2);
        assert_eq!(payload[1].name, Some("code-reviewer".to_string()));
    }

    #[test]
    fn test_has_active() {
        let mut map = SubagentMap::new();
        assert!(!map.has_active());

        map.process_start("code-reviewer", None);
        assert!(map.has_active());

        map.process_stop("code-reviewer");
        assert!(!map.has_active());
    }

    #[test]
    fn test_process_start_with_display_name() {
        let mut map = SubagentMap::new();
        map.process_start("general-purpose", Some("Review plan chunk 1".to_string()));
        assert_eq!(map.subagents()[0].agent_type, "general-purpose");
        assert_eq!(map.subagents()[0].display_name, Some("Review plan chunk 1".to_string()));
    }

    #[test]
    fn test_process_start_without_display_name() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        assert_eq!(map.subagents()[0].display_name, None);
    }

    #[test]
    fn test_payload_uses_display_name_over_agent_type() {
        let mut map = SubagentMap::new();
        map.process_start("general-purpose", Some("Review plan chunk 1".to_string()));
        let payload = map.payload();
        assert_eq!(payload[0].name, Some("Review plan chunk 1".to_string()));
    }

    #[test]
    fn test_payload_falls_back_to_agent_type() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        let payload = map.payload();
        assert_eq!(payload[0].name, Some("code-reviewer".to_string()));
    }
}
