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
}
