#[cfg(test)]
mod tests {
    use crate::subagent_tracker::SubagentMap;
    use crate::status_parser::SessionStatus;

    #[test]
    fn test_process_start_registers_subagent() {
        let mut map = SubagentMap::new();
        let changed = map.process_start("code-reviewer", None);
        assert!(changed);
        assert_eq!(map.subagents().len(), 1);
        assert_eq!(map.subagents()[0].agent_type, "code-reviewer");
        assert_eq!(map.subagents()[0].status, SessionStatus::Working);
        assert_eq!(map.subagents()[0].index, 1);
    }

    #[test]
    fn test_multiple_subagents_get_sequential_indexes() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        map.process_start("code-reviewer", None);
        map.process_start("Explore", None);

        let agents = map.subagents();
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].index, 1);
        assert_eq!(agents[1].index, 2);
        assert_eq!(agents[2].index, 3);
    }

    #[test]
    fn test_process_stop_marks_oldest_working_of_same_type() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        map.process_start("code-reviewer", None);

        let changed = map.process_stop("code-reviewer");
        assert!(changed);
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
        assert!(map.subagents()[0].finished_at.is_some());
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
        let changed = map.process_stop("unknown-type");
        assert!(changed);
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
    }

    #[test]
    fn test_payload_serialization() {
        let mut map = SubagentMap::new();
        map.process_start("Explore", None);
        map.process_start("code-reviewer", None);

        let payload = map.payload();
        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0].index, 1);
        assert_eq!(payload[0].name, Some("Explore".to_string()));
        assert_eq!(payload[1].index, 2);
        assert_eq!(payload[1].name, Some("code-reviewer".to_string()));

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("Explore"));
        assert!(json.contains("code-reviewer"));
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
    fn test_mixed_agent_types_stop_correctly() {
        let mut map = SubagentMap::new();
        map.process_start("code-reviewer", None);
        map.process_start("Explore", None);
        map.process_start("code-reviewer", None);

        // Stop code-reviewer — should mark the first one (index 1)
        map.process_stop("code-reviewer");
        assert_eq!(map.subagents()[0].status, SessionStatus::Finished);
        assert_eq!(map.subagents()[1].status, SessionStatus::Working); // Explore
        assert_eq!(map.subagents()[2].status, SessionStatus::Working); // code-reviewer #2

        // Stop Explore
        map.process_stop("Explore");
        assert_eq!(map.subagents()[1].status, SessionStatus::Finished);
    }
}
