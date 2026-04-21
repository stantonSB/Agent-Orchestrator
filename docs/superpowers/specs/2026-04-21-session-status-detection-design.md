# Session Status Detection: Fix Premature Finished State

**Date**: 2026-04-21
**Status**: Approved

## Problem

Sessions show as "Finished" after just 3 seconds of no terminal output, even when Claude Code is still actively working (thinking, making API calls, running long commands). The current heuristic in `status_parser.rs` transitions `Working → Finished` purely on a 3-second output silence, which is too aggressive.

## Root Cause

In `src-tauri/src/status_parser.rs`, the `tick_with_time()` method (line 144) transitions to `Finished` whenever output has been quiet for 3 seconds with no question pattern detected:

```rust
} else if elapsed.as_secs() >= 3 {
    self.status = SessionStatus::Finished;
}
```

Claude Code regularly has output gaps >3s during normal operation — API calls, long tool executions, thinking phases all produce silence while actively working.

## Solution: Three-Signal Detection

Replace the simple timeout with a multi-signal approach that combines spinner keepalive, prompt detection, and expanded pattern matching.

### Signal 1: Spinner Keepalive

Claude Code displays animated spinner characters while actively working:
- macOS: `·`, `✢`, `✳`, `✶`, `✻`, `✽`
- Ghostty: `·`, `✢`, `✳`, `✶`, `✻`, `*`
- Linux: `·`, `✢`, `*`, `✶`, `✻`, `✽`
- Reduced motion: `●`

**Rule**: If spinner characters appeared in output recently (within ~1.5s), Claude is actively working — do not transition away from `Working` state regardless of any timeout.

**Implementation**: Track a `last_spinner_at: Option<Instant>` timestamp. In `feed_output()`, scan incoming bytes for spinner characters and update the timestamp. In `tick_with_time()`, check if the last spinner was within 1.5s — if so, skip all Working transitions.

### Signal 2: Idle Prompt Detection (for Finished)

When Claude Code completes a task and is ready for new input, it renders the idle prompt:
- `❯` (U+276F, "HEAVY RIGHT-POINTING ANGLE QUOTATION MARK ORNAMENT") on macOS/Unicode terminals
- `>` (U+003E) on non-Unicode/Windows fallback terminals

**Rule**: Transition `Working → Finished` when output has been quiet for ≥2s AND the idle prompt `❯` is detected in the buffer AND no question/permission pattern is present.

**Implementation**: Add a `check_idle_prompt()` method that scans the ANSI-stripped buffer for `❯` or `>` at the start of a line (after optional whitespace). Only transition to `Finished` via prompt detection or the 8s fallback.

### Signal 3: Expanded NeedsAttention Patterns (for Blocked)

Existing patterns (keep all):
- Line ends with `? ` (question)
- Line ends with `> ` (prompt)
- Contains `(y/n)`, `(yes/no)`, `[Y/n]`, `[y/N]`, `[Y/N]`
- Contains `AskUserQuestion`

New patterns to add:
- Contains `Do you want to proceed?`
- Contains `needs your permission`
- Contains `needs your approval`
- Contains `needs your attention`

### Signal 4: Fallback Timeout

**Rule**: If output has been quiet for ≥8s with no spinner activity, no prompt detected, and no question pattern — transition to `Finished`. This handles edge cases where both spinner and prompt detection fail.

## State Machine: Working Transitions

Priority order (evaluated top-to-bottom in `tick_with_time()`):

| Priority | Condition | Result |
|----------|-----------|--------|
| 1 | `last_spinner_at` within 1.5s | Stay **Working** (early return) |
| 2 | Output quiet ≥ 2s + question/permission pattern | **NeedsAttention** |
| 3 | Output quiet ≥ 2s + idle prompt detected + no question pattern | **Finished** |
| 4 | Output quiet ≥ 8s (no spinner, no prompt) | **Finished** |

## Unchanged Behavior

These transitions remain exactly as-is:
- `Starting → Idle` (output settles after 3s)
- `Idle → Working` (user presses Enter)
- `NeedsAttention → Working` (user presses Enter)
- `Finished → Working` (user presses Enter)
- `Any → Finished/Error` (process exits)

## Implementation Details

### StatusTracker Struct Changes

Add one new field:
```rust
pub struct StatusTracker {
    buffer: Vec<u8>,
    max_buffer_size: usize,
    status: SessionStatus,
    last_output_at: Option<Instant>,
    last_spinner_at: Option<Instant>,  // NEW
    has_received_output: bool,
}
```

### Spinner Character Detection

In `feed_output()`, after extending the buffer, scan the incoming `data` bytes for spinner characters. Since these are multi-byte UTF-8 characters, scan the data as a UTF-8 string:

```rust
const SPINNER_CHARS: &[char] = &['·', '✢', '✳', '✶', '✻', '✽', '●', '*'];
```

Note: `*` is also a common code character, but in the context of spinner detection it only matters if it appeared recently AND output then went quiet — a `*` in flowing code output won't trigger any transition since `last_output_at` keeps resetting.

### Idle Prompt Detection

New method `check_idle_prompt()`:
```rust
fn check_idle_prompt(&self) -> bool {
    let text = String::from_utf8_lossy(&self.buffer);
    let stripped = strip_ansi_escapes(&text);
    // Check if any non-empty line starts with ❯ or > (the idle prompt)
    stripped.lines().rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with('❯') || trimmed.starts_with('>')
        })
        .unwrap_or(false)
}
```

### Updated tick_with_time() for Working State

```rust
SessionStatus::Working => {
    // Signal 1: Spinner keepalive — Claude is actively working
    if let Some(last_spinner) = self.last_spinner_at {
        if now.duration_since(last_spinner).as_millis() < 1500 {
            return None; // Still spinning, definitely working
        }
    }

    // Signal 2: NeedsAttention (2s quiet + question pattern)
    if elapsed.as_secs() >= 2 && self.check_needs_attention() {
        self.status = SessionStatus::NeedsAttention;
    }
    // Signal 3: Finished via prompt detection (2s quiet + idle prompt)
    else if elapsed.as_secs() >= 2
        && self.check_idle_prompt()
        && !self.check_needs_attention()
    {
        self.status = SessionStatus::Finished;
    }
    // Signal 4: Fallback (8s quiet, no signals)
    else if elapsed.as_secs() >= 8 {
        self.status = SessionStatus::Finished;
    }
}
```

## Files Changed

| File | Changes |
|------|---------|
| `src-tauri/src/status_parser.rs` | Add `last_spinner_at` field, spinner detection in `feed_output()`, `check_idle_prompt()` method, expanded `check_needs_attention()` patterns, updated `tick_with_time()` Working branch |
| `src-tauri/src/status_parser_tests.rs` | Update existing timeout-based tests, add tests for spinner keepalive, prompt detection, expanded patterns, fallback timeout |

## Test Plan

1. **Spinner keepalive**: Feed spinner chars → verify stays Working even after 8s of output quiet (as long as spinner was recent)
2. **Prompt detection**: Feed idle prompt `❯ ` → verify transitions to Finished after 2s
3. **NeedsAttention priority**: Feed question pattern → verify NeedsAttention even when `❯` is also present
4. **Fallback timeout**: No spinner, no prompt → verify Finished after 8s
5. **No false Finished at 3s**: Feed non-spinner output, wait 3s → verify stays Working (regression test)
6. **Full lifecycle**: Starting → Idle → Working (with spinner) → NeedsAttention → Working → Finished (with prompt)
7. **Expanded patterns**: Test each new NeedsAttention pattern string
