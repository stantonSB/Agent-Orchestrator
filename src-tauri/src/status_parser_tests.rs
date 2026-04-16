#[cfg(test)]
mod tests {
    use crate::status_parser::{SessionStatus, StatusTracker};
    use std::time::Duration;
    use std::time::Instant;

    // -----------------------------------------------------------------------
    // Initial state
    // -----------------------------------------------------------------------

    #[test]
    fn test_initial_status_is_starting() {
        let tracker = StatusTracker::new();
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    // -----------------------------------------------------------------------
    // feed_output: no longer transitions status
    // -----------------------------------------------------------------------

    #[test]
    fn test_feed_output_does_not_change_status() {
        let mut tracker = StatusTracker::new();
        let change = tracker.feed_output(b"Hello from Claude");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_empty_output_ignored() {
        let mut tracker = StatusTracker::new();
        let change = tracker.feed_output(b"");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_buffer_truncation_to_500_bytes() {
        let mut tracker = StatusTracker::new();
        let large_data = vec![b'A'; 600];
        tracker.feed_output(&large_data);
        assert_eq!(tracker.buffer_contents().len(), 500);
    }

    #[test]
    fn test_buffer_keeps_tail() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(&vec![b'A'; 400]);
        tracker.feed_output(&vec![b'B'; 200]);
        let buf = tracker.buffer_contents();
        assert_eq!(buf.len(), 500);
        assert!(buf[300..].iter().all(|&b| b == b'B'));
    }

    // -----------------------------------------------------------------------
    // notify_user_input: transitions to Working on Enter
    // -----------------------------------------------------------------------

    #[test]
    fn test_user_input_with_enter_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_user_input(b"hello\r");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_user_input_with_newline_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_user_input(b"hello\n");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_user_input_without_enter_no_change() {
        let mut tracker = StatusTracker::new();
        let change = tracker.notify_user_input(b"typing...");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_user_input_clears_buffer() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Do you want to proceed? ");
        assert!(!tracker.buffer_contents().is_empty());

        tracker.notify_user_input(b"\r");
        assert!(tracker.buffer_contents().is_empty());
    }

    #[test]
    fn test_user_input_when_already_working_returns_none() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let change = tracker.notify_user_input(b"\r");
        assert_eq!(change, None);
    }

    #[test]
    fn test_user_input_does_not_transition_from_error() {
        let mut tracker = StatusTracker::new();
        tracker.notify_exit(1);
        assert_eq!(*tracker.status(), SessionStatus::Error);

        let change = tracker.notify_user_input(b"\r");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Error);
    }

    // -----------------------------------------------------------------------
    // tick: Starting → Idle (after startup output settles)
    // -----------------------------------------------------------------------

    #[test]
    fn test_tick_starting_to_idle_after_output_settles() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Welcome to Claude Code...\n");
        assert_eq!(*tracker.status(), SessionStatus::Starting);

        let future = Instant::now() + Duration::from_secs(4);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, Some(SessionStatus::Idle));
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_tick_no_change_starting_without_output() {
        let mut tracker = StatusTracker::new();
        let change = tracker.tick();
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_tick_starting_stays_if_output_recent() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Loading...");

        // Only 1 second later — should stay Starting.
        let future = Instant::now() + Duration::from_secs(1);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    // -----------------------------------------------------------------------
    // tick: Working → Finished (output stops, no question pattern)
    // -----------------------------------------------------------------------

    #[test]
    fn test_tick_working_to_finished() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"fix the bug\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // Simulate Claude outputting then going quiet.
        tracker.feed_output(b"I'll fix the bug now.\nDone!\n");

        let future = Instant::now() + Duration::from_secs(4);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, Some(SessionStatus::Finished));
        assert_eq!(*tracker.status(), SessionStatus::Finished);
    }

    #[test]
    fn test_tick_no_change_while_finished() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        tracker.feed_output(b"done");
        // Simulate time passing to reach Finished.
        let future = Instant::now() + Duration::from_secs(4);
        tracker.tick_with_time(future);
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        // Further ticks should not change state.
        let later = future + Duration::from_secs(10);
        let change = tracker.tick_with_time(later);
        assert_eq!(change, None);
    }

    // -----------------------------------------------------------------------
    // tick: Working → NeedsAttention (output stops with question pattern)
    // -----------------------------------------------------------------------

    #[test]
    fn test_tick_working_to_needs_attention_question() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"do something\r");
        tracker.feed_output(b"Do you want to proceed? ");

        let future = Instant::now() + Duration::from_secs(3);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
    }

    #[test]
    fn test_tick_working_to_needs_attention_yn() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        tracker.feed_output(b"Continue? (y/n) ");

        let future = Instant::now() + Duration::from_secs(3);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
    }

    #[test]
    fn test_tick_working_to_needs_attention_bracket_yn() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        tracker.feed_output(b"Install deps? [Y/n] ");

        let future = Instant::now() + Duration::from_secs(3);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
    }

    #[test]
    fn test_tick_working_to_needs_attention_prompt() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        tracker.feed_output(b"Enter your choice> ");

        let future = Instant::now() + Duration::from_secs(3);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
    }

    #[test]
    fn test_tick_working_to_needs_attention_ask_user_question() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        tracker.feed_output(b"AskUserQuestion: What should I do next?");

        let future = Instant::now() + Duration::from_secs(3);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
    }

    #[test]
    fn test_tick_working_to_needs_attention_with_ansi() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        tracker.feed_output(b"\x1b[32mDo you want to continue?\x1b[0m ? ");

        let future = Instant::now() + Duration::from_secs(3);
        let change = tracker.tick_with_time(future);
        assert_eq!(change, Some(SessionStatus::NeedsAttention));
    }

    // -----------------------------------------------------------------------
    // Idle is stable — only exits via user input
    // -----------------------------------------------------------------------

    #[test]
    fn test_idle_stays_idle_on_tick() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Ready.\n");
        let t1 = Instant::now() + Duration::from_secs(4);
        tracker.tick_with_time(t1); // Starting → Idle
        assert_eq!(*tracker.status(), SessionStatus::Idle);

        let t2 = t1 + Duration::from_secs(30);
        let change = tracker.tick_with_time(t2);
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_idle_to_working_on_user_input() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Ready.\n");
        let t1 = Instant::now() + Duration::from_secs(4);
        tracker.tick_with_time(t1); // Starting → Idle

        let change = tracker.notify_user_input(b"fix this\r");
        assert_eq!(change, Some(SessionStatus::Working));
    }

    // -----------------------------------------------------------------------
    // NeedsAttention is stable — only exits via user input
    // -----------------------------------------------------------------------

    #[test]
    fn test_needs_attention_stays_on_tick() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        tracker.feed_output(b"Proceed? (y/n) ");
        let t1 = Instant::now() + Duration::from_secs(3);
        tracker.tick_with_time(t1); // Working → NeedsAttention
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);

        let t2 = t1 + Duration::from_secs(30);
        let change = tracker.tick_with_time(t2);
        assert_eq!(change, None);
    }

    #[test]
    fn test_needs_attention_to_working_on_user_answer() {
        let mut tracker = StatusTracker::new();
        tracker.notify_user_input(b"\r");
        tracker.feed_output(b"Proceed? (y/n) ");
        let t1 = Instant::now() + Duration::from_secs(3);
        tracker.tick_with_time(t1);
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);

        let change = tracker.notify_user_input(b"y\r");
        assert_eq!(change, Some(SessionStatus::Working));
    }

    // -----------------------------------------------------------------------
    // Finished → Working on new submission
    // -----------------------------------------------------------------------

    #[test]
    fn test_finished_to_working_on_new_submission() {
        let mut tracker = StatusTracker::new();
        // First task cycle.
        tracker.notify_user_input(b"task one\r");
        tracker.feed_output(b"Done.\n");
        let t1 = Instant::now() + Duration::from_secs(4);
        tracker.tick_with_time(t1); // → Finished
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        // New submission.
        let change = tracker.notify_user_input(b"task two\r");
        assert_eq!(change, Some(SessionStatus::Working));
    }

    // -----------------------------------------------------------------------
    // Process exit
    // -----------------------------------------------------------------------

    #[test]
    fn test_exit_code_zero_is_finished() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"some output");
        let status = tracker.notify_exit(0);
        assert_eq!(status, SessionStatus::Finished);
    }

    #[test]
    fn test_exit_code_nonzero_is_error() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"some output");
        let status = tracker.notify_exit(1);
        assert_eq!(status, SessionStatus::Error);
    }

    #[test]
    fn test_tick_no_change_after_exit() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"done");
        tracker.notify_exit(0);
        let change = tracker.tick();
        assert_eq!(change, None);
    }

    // -----------------------------------------------------------------------
    // Full lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_lifecycle() {
        let mut tracker = StatusTracker::new();

        // 1. Starting
        assert_eq!(*tracker.status(), SessionStatus::Starting);

        // 2. Startup output → still Starting
        tracker.feed_output(b"Welcome to Claude Code\n");
        assert_eq!(*tracker.status(), SessionStatus::Starting);

        // 3. Output settles → Idle
        let t1 = Instant::now() + Duration::from_secs(4);
        tracker.tick_with_time(t1);
        assert_eq!(*tracker.status(), SessionStatus::Idle);

        // 4. User submits → Working
        tracker.notify_user_input(b"fix the bug\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // 5. Claude streams output
        tracker.feed_output(b"I'll analyze the code...\n");
        tracker.feed_output(b"Found the issue.\n");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // 6. Claude asks a question
        tracker.feed_output(b"Should I apply the fix? (y/n) ");
        let t2 = Instant::now() + Duration::from_secs(3);
        tracker.tick_with_time(t2);
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);

        // 7. User answers
        tracker.notify_user_input(b"y\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        // 8. Claude finishes
        tracker.feed_output(b"Fix applied successfully.\n");
        let t3 = Instant::now() + Duration::from_secs(4);
        tracker.tick_with_time(t3);
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        // 9. User submits new task
        tracker.notify_user_input(b"now add tests\r");
        assert_eq!(*tracker.status(), SessionStatus::Working);
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
}
