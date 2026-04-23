#[cfg(test)]
mod tests {
    use crate::subagent_tracker::SubagentMap;

    #[test]
    fn test_first_session_id_becomes_parent() {
        let mut map = SubagentMap::new();
        map.process_event("parent-cc-id", "idle_prompt");
        assert_eq!(map.parent_session_id(), Some("parent-cc-id"));
        assert_eq!(map.subagents().len(), 0);
    }

    #[test]
    fn test_second_session_id_becomes_subagent() {
        let mut map = SubagentMap::new();
        map.process_event("parent-id", "idle_prompt");
        let changed = map.process_event("child-id", "idle_prompt");
        assert!(changed);
        assert_eq!(map.subagents().len(), 1);
        assert_eq!(map.subagents()[0].claude_session_id, "child-id");
        assert_eq!(map.subagents()[0].index, 1);
    }

    #[test]
    fn test_multiple_subagents_get_sequential_indexes() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child-a", "idle_prompt");
        map.process_event("child-b", "permission_prompt");
        map.process_event("child-c", "idle_prompt");

        let mut agents: Vec<_> = map.subagents().into_iter().collect();
        agents.sort_by_key(|a| a.index);
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].index, 1);
        assert_eq!(agents[1].index, 2);
        assert_eq!(agents[2].index, 3);
    }

    #[test]
    fn test_subagent_status_transitions() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child", "idle_prompt"); // initial: Idle

        let changed = map.process_event("child", "permission_prompt");
        assert!(changed);
        let child = map.subagents().into_iter().find(|a| a.claude_session_id == "child").unwrap();
        assert_eq!(child.status, crate::status_parser::SessionStatus::NeedsAttention);
    }

    #[test]
    fn test_subagent_stop_sets_finished_and_finished_at() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child", "permission_prompt"); // NeedsAttention

        let changed = map.process_event("child", "stop");
        assert!(changed);
        let child = map.subagents().into_iter().find(|a| a.claude_session_id == "child").unwrap();
        assert_eq!(child.status, crate::status_parser::SessionStatus::Finished);
        assert!(child.finished_at.is_some());
    }

    #[test]
    fn test_parent_event_returns_false() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        let changed = map.process_event("parent", "permission_prompt");
        assert!(!changed);
        assert_eq!(map.subagents().len(), 0);
    }

    #[test]
    fn test_any_needs_attention() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child-a", "idle_prompt");
        assert!(!map.any_needs_attention());

        map.process_event("child-b", "permission_prompt");
        assert!(map.any_needs_attention());
    }

    #[test]
    fn test_buffering_when_parent_unknown() {
        let mut map = SubagentMap::new();
        map.buffer_event("child-1", "idle_prompt");
        map.buffer_event("child-2", "permission_prompt");
        assert!(map.parent_unknown());
        assert_eq!(map.subagents().len(), 0);

        let changed = map.process_event("parent", "idle_prompt");
        assert!(changed);
        assert_eq!(map.subagents().len(), 2);
    }

    #[test]
    fn test_buffer_capacity_drops_oldest() {
        let mut map = SubagentMap::new();
        for i in 0..35 {
            map.buffer_event(&format!("child-{i}"), "idle_prompt");
        }
        // Buffer should be capped at 32
        map.process_event("parent", "idle_prompt");
        // First 3 dropped, 32 remain, but parent matches none of them
        assert_eq!(map.subagents().len(), 32);
    }

    #[test]
    fn test_payload_serialization() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child-a", "idle_prompt");
        map.process_event("child-b", "permission_prompt");

        let payload = map.payload();
        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0].index, 1); // sorted by index
        assert_eq!(payload[1].index, 2);

        // Verify it serializes to JSON
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("child-a"));
        assert!(json.contains("child-b"));
    }

    #[test]
    fn test_is_parent() {
        let mut map = SubagentMap::new();
        map.process_event("parent-id", "idle_prompt");
        assert!(map.is_parent("parent-id"));
        assert!(!map.is_parent("other-id"));
    }

    #[test]
    fn test_duplicate_subagent_event_no_change_returns_false() {
        let mut map = SubagentMap::new();
        map.process_event("parent", "idle_prompt");
        map.process_event("child", "idle_prompt"); // Idle
        let changed = map.process_event("child", "idle_prompt"); // Idle → Idle: no transition
        assert!(!changed);
    }
}
