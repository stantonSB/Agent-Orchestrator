#[cfg(test)]
mod tests {
    use crate::status_parser::{SessionStatus, StatusTracker};

    // -----------------------------------------------------------------------
    // Initial state
    // -----------------------------------------------------------------------

    #[test]
    fn test_initial_status_is_starting() {
        let tracker = StatusTracker::new();
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    // -----------------------------------------------------------------------
    // notify_hook_event: idle_prompt transitions
    // -----------------------------------------------------------------------

    #[test]
    fn test_idle_prompt_from_starting_transitions_to_idle() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_hook_event("idle_prompt");
        assert_eq!(change, Some(SessionStatus::Idle));
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_idle_prompt_from_working_transitions_to_finished() {
        let mut tracker = StatusTracker::new();
        // Get to Working first
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let change = tracker.notify_hook_event("idle_prompt");
        assert_eq!(change, Some(SessionStatus::Finished));
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_idle_prompt_from_needs_attention_transitions_to_finished() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        tracker.notify_hook_event("permission_prompt"); // Working → NeedsAttention
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);

        let change = tracker.notify_hook_event("idle_prompt");
        assert_eq!(change, Some(SessionStatus::Finished));
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_idle_prompt_when_already_idle_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        assert_eq!(*tracker.status(), SessionStatus::Idle);

        let change = tracker.notify_hook_event("idle_prompt");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_idle_prompt_when_already_finished_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        tracker.notify_hook_event("idle_prompt"); // Working → Finished
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        let change = tracker.notify_hook_event("idle_prompt");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_duplicate_idle_prompts_in_a_row() {
        let mut tracker = StatusTracker::new();
        let first = tracker.notify_hook_event("idle_prompt");
        assert_eq!(first, Some(SessionStatus::Idle));

        let second = tracker.notify_hook_event("idle_prompt");
        assert_eq!(second, None);

        let third = tracker.notify_hook_event("idle_prompt");
        assert_eq!(third, None);
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    // -----------------------------------------------------------------------
    // notify_hook_event: permission_prompt transitions
    // -----------------------------------------------------------------------

    #[test]
    fn test_permission_prompt_from_working_transitions_to_needs_attention() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let change = tracker.notify_hook_event("permission_prompt");
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);
    }

    #[test]
    fn test_permission_prompt_from_starting_transitions_to_needs_attention() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_hook_event("permission_prompt");
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);
    }

    #[test]
    fn test_permission_prompt_when_already_needs_attention_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("permission_prompt"); // Starting → NeedsAttention
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);

        let change = tracker.notify_hook_event("permission_prompt");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);
    }

    // -----------------------------------------------------------------------
    // notify_hook_event: elicitation_dialog transitions
    // -----------------------------------------------------------------------

    #[test]
    fn test_elicitation_dialog_from_working_transitions_to_needs_attention() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let change = tracker.notify_hook_event("elicitation_dialog");
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);
    }

    #[test]
    fn test_elicitation_dialog_from_starting_transitions_to_needs_attention() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_hook_event("elicitation_dialog");
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);
    }

    #[test]
    fn test_elicitation_dialog_when_already_needs_attention_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("elicitation_dialog"); // Starting → NeedsAttention

        let change = tracker.notify_hook_event("elicitation_dialog");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);
    }

    // -----------------------------------------------------------------------
    // notify_hook_event: stop transitions (Stop hook fires immediately)
    // -----------------------------------------------------------------------

    #[test]
    fn test_stop_from_starting_transitions_to_idle() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_hook_event("stop");
        assert_eq!(change, Some(SessionStatus::Idle));
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_stop_from_working_transitions_to_finished() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let change = tracker.notify_hook_event("stop");
        assert_eq!(change, Some(SessionStatus::Finished));
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_stop_from_needs_attention_transitions_to_finished() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        tracker.notify_hook_event("permission_prompt"); // Working → NeedsAttention
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);

        let change = tracker.notify_hook_event("stop");
        assert_eq!(change, Some(SessionStatus::Finished));
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_stop_when_already_idle_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        assert_eq!(*tracker.status(), SessionStatus::Idle);

        let change = tracker.notify_hook_event("stop");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_stop_when_already_finished_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        tracker.notify_hook_event("stop"); // Working → Finished

        let change = tracker.notify_hook_event("stop");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_full_lifecycle_with_stop_hook() {
        let mut tracker = StatusTracker::new();

        // 1. Starting
        assert_eq!(*tracker.status(), SessionStatus::Starting);

        // 2. Stop hook fires on first response → Idle
        let change = tracker.notify_hook_event("stop");
        assert_eq!(change, Some(SessionStatus::Idle));

        // 3. User submits task → Working
        tracker.notify_user_input(b"fix the bug\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // 4. Claude finishes immediately (Stop hook) → Finished
        let change = tracker.notify_hook_event("stop");
        assert_eq!(change, Some(SessionStatus::Finished));
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        // 5. User starts new task → Working again
        tracker.notify_user_input(b"now tests\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // 6. Finished again via stop
        tracker.notify_hook_event("stop");
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    // -----------------------------------------------------------------------
    // notify_user_input: Escape key transitions (Working → Finished)
    // -----------------------------------------------------------------------

    #[test]
    fn test_escape_from_working_transitions_to_finished() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let change = tracker.notify_user_input(b"\x1b");
        assert_eq!(change, Some(SessionStatus::Finished));
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_escape_from_idle_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle

        let change = tracker.notify_user_input(b"\x1b");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_escape_from_starting_returns_none() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_user_input(b"\x1b");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_escape_from_finished_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        tracker.notify_hook_event("stop"); // Working → Finished

        let change = tracker.notify_user_input(b"\x1b");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_escape_sequence_does_not_trigger_transition() {
        // Arrow keys and other escape sequences are multi-byte, should not
        // be confused with a bare Escape press.
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working

        // Arrow Up: ESC [ A
        let change = tracker.notify_user_input(b"\x1b[A");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_escape_then_new_task() {
        // After interrupting, user can start a new task.
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working

        tracker.notify_user_input(b"\x1b"); // Working → Finished
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        let change = tracker.notify_user_input(b"new task\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    // -----------------------------------------------------------------------
    // notify_hook_event: unknown notification type
    // -----------------------------------------------------------------------

    #[test]
    fn test_unknown_notification_type_returns_none() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_hook_event("some_unknown_event");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_unknown_notification_type_does_not_change_state() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle

        let change = tracker.notify_hook_event("unrecognized_hook");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    // -----------------------------------------------------------------------
    // notify_user_input: Enter transitions
    // -----------------------------------------------------------------------

    #[test]
    fn test_enter_from_idle_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle

        let change = tracker.notify_user_input(b"hello\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_newline_from_idle_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle

        let change = tracker.notify_user_input(b"hello\n");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_enter_from_finished_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        tracker.notify_hook_event("idle_prompt"); // Working → Finished
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        let change = tracker.notify_user_input(b"new task\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_enter_from_needs_attention_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("permission_prompt"); // Starting → NeedsAttention

        let change = tracker.notify_user_input(b"y\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_enter_from_starting_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_user_input(b"hello\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_enter_from_working_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let change = tracker.notify_user_input(b"more input\r");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_enter_from_error_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_exit(1); // → Error
        assert_eq!(*tracker.status(), SessionStatus::Error);

        let change = tracker.notify_user_input(b"\r");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Error);
    }

    #[test]
    fn test_non_enter_input_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle

        let change = tracker.notify_user_input(b"typing without enter");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_empty_input_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle

        let change = tracker.notify_user_input(b"");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    // -----------------------------------------------------------------------
    // notify_exit: process exit transitions
    // -----------------------------------------------------------------------

    #[test]
    fn test_exit_code_zero_transitions_to_finished() {
        let mut tracker = StatusTracker::new();
        let status = tracker.notify_exit(0);
        assert_eq!(status, SessionStatus::Finished);
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_exit_code_nonzero_transitions_to_error() {
        let mut tracker = StatusTracker::new();
        let status = tracker.notify_exit(1);
        assert_eq!(status, SessionStatus::Error);
        assert_eq!(*tracker.status(), SessionStatus::Error);
    }

    #[test]
    fn test_exit_code_nonzero_various_codes() {
        for code in [2, 127, -1, 255] {
            let mut tracker = StatusTracker::new();
            let status = tracker.notify_exit(code);
            assert_eq!(
                status,
                SessionStatus::Error,
                "exit code {} should produce Error",
                code
            );
        }
    }

    #[test]
    fn test_exit_from_starting_state() {
        let mut tracker = StatusTracker::new();
        assert_eq!(*tracker.status(), SessionStatus::Starting);
        let status = tracker.notify_exit(0);
        assert_eq!(status, SessionStatus::Finished);
    }

    #[test]
    fn test_exit_from_working_state() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working
        let status = tracker.notify_exit(0);
        assert_eq!(status, SessionStatus::Finished);
    }

    #[test]
    fn test_exit_from_needs_attention_state() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("permission_prompt"); // Starting → NeedsAttention
        let status = tracker.notify_exit(0);
        assert_eq!(status, SessionStatus::Finished);
    }

    #[test]
    fn test_exit_from_idle_state() {
        let mut tracker = StatusTracker::new();
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        let status = tracker.notify_exit(0);
        assert_eq!(status, SessionStatus::Finished);
    }

    // -----------------------------------------------------------------------
    // as_str
    // -----------------------------------------------------------------------

    #[test]
    fn test_as_str() {
        assert_eq!(SessionStatus::Starting.as_str(), "starting");
        assert_eq!(SessionStatus::Working.as_str(), "working");
        assert_eq!(SessionStatus::Idle.as_str(), "idle");
        assert_eq!(SessionStatus::NeedsAttention.as_str(), "needs_attention");
        assert_eq!(SessionStatus::Finished.as_str(), "finished");
        assert_eq!(SessionStatus::Error.as_str(), "error");
    }

    // -----------------------------------------------------------------------
    // Full lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_lifecycle() {
        let mut tracker = StatusTracker::new();

        // 1. Starting
        assert_eq!(*tracker.status(), SessionStatus::Starting);

        // 2. idle_prompt hook fires → Idle
        let change = tracker.notify_hook_event("idle_prompt");
        assert_eq!(change, Some(SessionStatus::Idle));
        assert_eq!(*tracker.status(), SessionStatus::Idle);

        // 3. User submits task → Working
        let change = tracker.notify_user_input(b"fix the bug\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // 4. Claude requests permission → NeedsAttention
        let change = tracker.notify_hook_event("permission_prompt");
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);

        // 5. User answers → Working
        let change = tracker.notify_user_input(b"y\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // 6. Claude finishes → Finished
        let change = tracker.notify_hook_event("idle_prompt");
        assert_eq!(change, Some(SessionStatus::Finished));
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        // 7. User starts new task → Working
        let change = tracker.notify_user_input(b"now add tests\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_full_lifecycle_with_elicitation_dialog() {
        let mut tracker = StatusTracker::new();

        // Starting → Idle → Working → NeedsAttention (via elicitation) → Working → Finished
        tracker.notify_hook_event("idle_prompt");
        tracker.notify_user_input(b"task\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        tracker.notify_hook_event("elicitation_dialog");
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);

        tracker.notify_user_input(b"answer\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        tracker.notify_hook_event("idle_prompt");
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    // -----------------------------------------------------------------------
    // SubagentMap integration
    // -----------------------------------------------------------------------

    #[test]
    fn test_status_tracker_has_subagent_map() {
        let tracker = StatusTracker::new();
        assert_eq!(tracker.subagent_map().subagents().len(), 0);
    }

    #[test]
    fn test_subagent_map_mut_allows_modification() {
        let mut tracker = StatusTracker::new();
        tracker.subagent_map_mut().process_start("code-reviewer", None);
        assert_eq!(tracker.subagent_map().subagents().len(), 1);
    }

    // set_status tests

    #[test]
    fn test_set_status_from_starting_to_idle() {
        let mut tracker = StatusTracker::new();
        let change = tracker.set_status(SessionStatus::Idle);
        assert_eq!(change, Some(SessionStatus::Idle));
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_set_status_noop_when_already_in_state() {
        let mut tracker = StatusTracker::new();
        let change = tracker.set_status(SessionStatus::Starting);
        assert_eq!(change, None);
    }

    // -----------------------------------------------------------------------
    // worktree_cwd
    // -----------------------------------------------------------------------

    #[test]
    fn test_worktree_cwd_default_none() {
        let tracker = StatusTracker::new();
        assert_eq!(tracker.worktree_cwd(), None);
    }

    #[test]
    fn test_set_worktree_cwd() {
        let mut tracker = StatusTracker::new();
        let changed = tracker.set_worktree_cwd("/projects/app/.claude/worktrees/breezy-frog");
        assert!(changed);
        assert_eq!(tracker.worktree_cwd(), Some("/projects/app/.claude/worktrees/breezy-frog"));
    }

    #[test]
    fn test_set_worktree_cwd_returns_false_if_already_set() {
        let mut tracker = StatusTracker::new();
        tracker.set_worktree_cwd("/projects/app/.claude/worktrees/breezy-frog");
        let changed = tracker.set_worktree_cwd("/projects/app/.claude/worktrees/breezy-frog");
        assert!(!changed);
    }

    #[test]
    fn test_set_status_skipped_when_already_transitioned() {
        let mut tracker = StatusTracker::new();
        // Simulate hook event moving past Starting before timeout fires
        tracker.notify_hook_event("idle_prompt"); // Starting → Idle
        tracker.notify_user_input(b"task\r"); // Idle → Working

        // Timeout fires but tracker is already past Starting
        assert_eq!(*tracker.status(), SessionStatus::Working);
        // set_status to Idle would still change it — the caller checks for Starting first
    }
}
