#[cfg(test)]
mod tests {
    use crate::status_parser::{SessionStatus, StatusTracker};
    use std::time::Duration;
    use std::time::Instant;

    #[test]
    fn test_initial_status_is_starting() {
        let tracker = StatusTracker::new();
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_first_output_transitions_to_working() {
        let mut tracker = StatusTracker::new();
        let change = tracker.feed_output(b"Hello from Claude");
        assert_eq!(change, Some(SessionStatus::Working));
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_subsequent_output_stays_working() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Hello");
        let change = tracker.feed_output(b" world");
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Working);
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
    fn test_tick_no_change_while_starting() {
        let mut tracker = StatusTracker::new();
        let change = tracker.tick();
        assert_eq!(change, None);
        assert_eq!(*tracker.status(), SessionStatus::Starting);
    }

    #[test]
    fn test_tick_no_change_while_finished() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"done");
        tracker.notify_exit(0);
        let change = tracker.tick();
        assert_eq!(change, None);
    }

    #[test]
    fn test_needs_attention_question_mark_space() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Do you want to proceed? ");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_needs_attention_yn_pattern() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Continue with changes? (y/n) ");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_needs_attention_bracket_yn() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Install dependencies? [Y/n] ");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_needs_attention_input_prompt() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"Enter your choice> ");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_needs_attention_ask_user_question() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"AskUserQuestion: What should I do next?");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_strip_ansi_and_detect_pattern() {
        let mut tracker = StatusTracker::new();
        tracker.feed_output(b"\x1b[32mDo you want to continue?\x1b[0m ? ");
        assert_eq!(*tracker.status(), SessionStatus::Working);
    }

    #[test]
    fn test_tick_with_time_needs_attention_on_question_pattern() {
        let mut tracker = StatusTracker::new();
        let start = Instant::now();

        tracker.feed_output(b"Do you want to proceed? ");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let future = start + Duration::from_secs(11);
        let change = tracker.tick_with_time(future);

        assert_eq!(change, Some(SessionStatus::NeedsAttention));
        assert_eq!(*tracker.status(), SessionStatus::NeedsAttention);
    }

    #[test]
    fn test_tick_with_time_idle_without_attention_pattern() {
        let mut tracker = StatusTracker::new();
        let start = Instant::now();

        tracker.feed_output(b"Compiling module abc...\n");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        let future = start + Duration::from_secs(11);
        let change = tracker.tick_with_time(future);

        assert_eq!(change, Some(SessionStatus::Idle));
        assert_eq!(*tracker.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_status_state_machine_flow() {
        let mut tracker = StatusTracker::new();

        assert_eq!(*tracker.status(), SessionStatus::Starting);
        tracker.feed_output(b"Starting up...\n");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        tracker.feed_output(b"Processing files...\n");
        assert_eq!(*tracker.status(), SessionStatus::Working);

        tracker.notify_exit(0);
        assert_eq!(*tracker.status(), SessionStatus::Finished);

        let change = tracker.tick();
        assert_eq!(change, None);
    }

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
