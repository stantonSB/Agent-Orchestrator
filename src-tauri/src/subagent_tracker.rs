use crate::status_parser::SessionStatus;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Metadata for a single detected subagent.
#[derive(Debug, Clone)]
pub struct SubagentInfo {
    pub id: String,
    pub index: u16,
    pub status: SessionStatus,
    /// Claude Code `agent_id`, when the hook payload provides one. Used as the
    /// primary key for upsert/finish matching; `None` falls back to FIFO-by-type.
    pub agent_id: Option<String>,
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

    /// Register or refresh a subagent/teammate.
    ///
    /// When `agent_id` is `Some` and matches an existing record, upsert it:
    /// revive `Finished` -> `Working`, and fill in `display_name`/`agent_type`
    /// if they were previously unknown. Otherwise push a new `Working` record.
    /// Returns `true` only if something changed (so duplicate events are no-ops).
    pub fn process_start(
        &mut self,
        agent_id: Option<&str>,
        agent_type: &str,
        display_name: Option<String>,
    ) -> bool {
        if let Some(aid) = agent_id {
            if let Some(existing) = self
                .agents
                .iter_mut()
                .find(|a| a.agent_id.as_deref() == Some(aid))
            {
                let mut changed = false;
                if existing.status != SessionStatus::Working {
                    existing.status = SessionStatus::Working;
                    existing.finished_at = None;
                    changed = true;
                }
                if existing.display_name.is_none() && display_name.is_some() {
                    existing.display_name = display_name;
                    changed = true;
                }
                if existing.agent_type == "unknown" && agent_type != "unknown" {
                    existing.agent_type = agent_type.to_string();
                    changed = true;
                }
                return changed;
            }
        }

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
            agent_id: agent_id.map(|s| s.to_string()),
            agent_type: agent_type.to_string(),
            display_name,
            created_at,
            finished_at: None,
        });
        true
    }

    /// Mark a subagent/teammate `Finished`.
    ///
    /// When `agent_id` is `Some`, only a record with that exact id is finished;
    /// if none matches, nothing changes (we never finish an unrelated agent on a
    /// mismatched id). When `agent_id` is `None`, fall back to the oldest Working
    /// record of the same type, then the oldest Working of any type.
    /// Returns `true` if state changed.
    pub fn process_stop(&mut self, agent_id: Option<&str>, agent_type: &str) -> bool {
        // Resolve the target record's index using the precedence rules, then
        // mutate once — keeps a single "mark finished" site.
        let target = match agent_id {
            Some(aid) => self.agents.iter().position(|a| {
                a.agent_id.as_deref() == Some(aid) && a.status == SessionStatus::Working
            }),
            None => self
                .agents
                .iter()
                .enumerate()
                .filter(|(_, a)| a.agent_type == agent_type && a.status == SessionStatus::Working)
                .min_by_key(|(_, a)| a.index)
                .map(|(i, _)| i)
                .or_else(|| {
                    self.agents
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| a.status == SessionStatus::Working)
                        .min_by_key(|(_, a)| a.index)
                        .map(|(i, _)| i)
                }),
        };

        match target {
            Some(i) => {
                self.agents[i].status = SessionStatus::Finished;
                self.agents[i].finished_at = Some(Instant::now());
                true
            }
            None => false,
        }
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
        assert!(map.process_start(None, "code-reviewer", None));
        assert_eq!(map.subagents().len(), 1);
        assert_eq!(map.subagents()[0].agent_type, "code-reviewer");
        assert_eq!(map.subagents()[0].status, SessionStatus::Working);
        assert_eq!(map.subagents()[0].index, 1);
    }

    #[test]
    fn test_multiple_starts() {
        let mut map = SubagentMap::new();
        map.process_start(None, "code-reviewer", None);
        map.process_start(None, "code-reviewer", None);
        map.process_start(None, "Explore", None);
        assert_eq!(map.subagents().len(), 3);
        assert_eq!(map.subagents()[0].index, 1);
        assert_eq!(map.subagents()[1].index, 2);
        assert_eq!(map.subagents()[2].index, 3);
    }

    #[test]
    fn test_process_stop_marks_oldest_working() {
        let mut map = SubagentMap::new();
        map.process_start(None, "code-reviewer", None);
        map.process_start(None, "code-reviewer", None);

        assert!(map.process_stop(None, "code-reviewer"));
        // Oldest (index 1) should be finished
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
        assert_eq!(map.subagents()[1].status, SessionStatus::Working);
    }

    #[test]
    fn test_process_stop_fifo_ordering() {
        let mut map = SubagentMap::new();
        map.process_start(None, "code-reviewer", None);
        map.process_start(None, "code-reviewer", None);
        map.process_start(None, "code-reviewer", None);

        map.process_stop(None, "code-reviewer");
        map.process_stop(None, "code-reviewer");

        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
        assert_eq!(map.subagents()[1].status, SessionStatus::Finished);
        assert_eq!(map.subagents()[2].status, SessionStatus::Working);
    }

    #[test]
    fn test_process_stop_no_match_returns_false() {
        let mut map = SubagentMap::new();
        assert!(!map.process_stop(None, "code-reviewer"));
    }

    #[test]
    fn test_process_stop_fallback_to_any_working() {
        let mut map = SubagentMap::new();
        map.process_start(None, "code-reviewer", None);
        // Stop with different type — falls back to any working agent
        assert!(map.process_stop(None, "unknown-type"));
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
    }

    #[test]
    fn test_any_needs_attention() {
        let mut map = SubagentMap::new();
        map.process_start(None, "code-reviewer", None);
        assert!(!map.any_needs_attention());

        // Manually set to NeedsAttention for testing
        map.agents[0].status = SessionStatus::NeedsAttention;
        assert!(map.any_needs_attention());
    }

    #[test]
    fn test_payload_sorted_by_index() {
        let mut map = SubagentMap::new();
        map.process_start(None, "Explore", None);
        map.process_start(None, "code-reviewer", None);
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

        map.process_start(None, "code-reviewer", None);
        assert!(map.has_active());

        map.process_stop(None, "code-reviewer");
        assert!(!map.has_active());
    }

    #[test]
    fn test_process_start_with_display_name() {
        let mut map = SubagentMap::new();
        map.process_start(None, "general-purpose", Some("Review plan chunk 1".to_string()));
        assert_eq!(map.subagents()[0].agent_type, "general-purpose");
        assert_eq!(map.subagents()[0].display_name, Some("Review plan chunk 1".to_string()));
    }

    #[test]
    fn test_process_start_without_display_name() {
        let mut map = SubagentMap::new();
        map.process_start(None, "code-reviewer", None);
        assert_eq!(map.subagents()[0].display_name, None);
    }

    #[test]
    fn test_payload_uses_display_name_over_agent_type() {
        let mut map = SubagentMap::new();
        map.process_start(None, "general-purpose", Some("Review plan chunk 1".to_string()));
        let payload = map.payload();
        assert_eq!(payload[0].name, Some("Review plan chunk 1".to_string()));
    }

    #[test]
    fn test_payload_falls_back_to_agent_type() {
        let mut map = SubagentMap::new();
        map.process_start(None, "code-reviewer", None);
        let payload = map.payload();
        assert_eq!(payload[0].name, Some("code-reviewer".to_string()));
    }

    #[test]
    fn test_upsert_by_agent_id_dedupes() {
        let mut map = SubagentMap::new();
        assert!(map.process_start(Some("t1"), "deck-impl", None));
        // Second start for the same agent_id, already Working, no new info -> no-op.
        assert!(!map.process_start(Some("t1"), "deck-impl", None));
        assert_eq!(map.subagents().len(), 1);
    }

    #[test]
    fn test_distinct_agent_ids_are_separate_rows() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        map.process_start(Some("t2"), "deck-scan", None);
        assert_eq!(map.subagents().len(), 2);
    }

    #[test]
    fn test_stop_by_agent_id_finishes_correct_record() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        map.process_start(Some("t2"), "deck-scan", None);
        assert!(map.process_stop(Some("t1"), "deck-impl"));
        let a = map.subagents();
        let t1 = a.iter().find(|x| x.agent_id.as_deref() == Some("t1")).unwrap();
        let t2 = a.iter().find(|x| x.agent_id.as_deref() == Some("t2")).unwrap();
        assert_eq!(t1.status, SessionStatus::Finished);
        assert_eq!(t2.status, SessionStatus::Working);
    }

    #[test]
    fn test_stop_then_start_same_agent_id_revives() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        map.process_stop(Some("t1"), "deck-impl");
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
        // Teammate picks up new work -> revived, still one row.
        assert!(map.process_start(Some("t1"), "deck-impl", None));
        assert_eq!(map.subagents().len(), 1);
        assert_eq!(map.subagents()[0].status, SessionStatus::Working);
    }

    #[test]
    fn test_stop_unmatched_agent_id_is_noop() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        assert!(!map.process_stop(Some("does-not-exist"), "deck-impl"));
        assert_eq!(map.subagents()[0].status, SessionStatus::Working);
    }

    #[test]
    fn test_upsert_fills_in_display_name_later() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "deck-impl", None);
        assert!(map.process_start(Some("t1"), "deck-impl", Some("Redesign the deck".to_string())));
        assert_eq!(map.subagents()[0].display_name, Some("Redesign the deck".to_string()));
    }

    #[test]
    fn test_upsert_backfills_agent_type_from_unknown() {
        let mut map = SubagentMap::new();
        map.process_start(Some("t1"), "unknown", None);
        assert!(map.process_start(Some("t1"), "deck-impl", None));
        assert_eq!(map.subagents()[0].agent_type, "deck-impl");
    }
}
