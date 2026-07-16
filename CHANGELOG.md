# Changelog

## [1.13.0] - 2026-07-16

### Added
- **Teammates in the fleet view** — In-process teammates spawned by Claude Code's task system now appear in a session's subagent list, tracked via the TaskCreated/TeammateIdle hooks. Subagent tracking is also more robust overall: agents are matched by ID instead of oldest-first guessing (#128)

### Fixed
- **Claude Code hooks no longer leak into non-AO sessions** — Older versions merged five hook entries into the global `~/.claude/settings.json`, causing the notify script to run before every tool call in every Claude Code session on the machine. Hooks are now injected per-session when Agent Orchestrator launches Claude Code, and on startup the app cleans up any global entries left behind by older versions, preserving unrelated hooks and settings (#126)
- **Cmd+Click file links in worktree sessions** — Relative file paths in terminal output now resolve against the session's git worktree instead of the base repository, so Cmd+clicking a file created inside a worktree opens it correctly (#127)

## [1.12.0] - 2026-06-17

### Changed
- **Faster, smoother output with many concurrent sessions** — Reworked the terminal-output pipeline so the UI stays responsive when several sessions stream output at once: PTY output is sent as compact base64 instead of a JSON integer array, the reader coalesces output into fewer, larger chunks, and xterm.js renders via GPU-accelerated WebGL with automatic fallback to the DOM renderer (#125)
- **Fewer wasted re-renders** — A session status change now re-renders only that session's card instead of the whole session tree, hidden terminals skip needless cursor-blink/render work, and all duration timers share a single ticker (#125)

## [1.11.0] - 2026-06-11

### Fixed
- **Cmd+Q quit confirmation** — Cmd+Q now reliably shows the quit confirmation dialog instead of exiting the app immediately (#122)
- **Cmd+Click file links** — Cmd+clicking file paths now opens them in VS Code; paths containing spaces are handled correctly (#122)
- **/voice microphone access** — Declared microphone usage so `/voice` works in sessions; macOS now prompts for mic permission instead of silently denying it (#122)

## [1.10.0] - 2026-06-09

### Added
- **Cmd+Q quit confirmation** — Pressing Cmd+Q on macOS now routes through the quit confirmation dialog instead of exiting immediately, matching the behaviour of the window close button (#121)

## [1.9.1] - 2026-05-28

### Added
- **Worktree CWD discovery** — PreToolUse hook captures the working directory early when Claude Code operates in git worktrees, ensuring sessions are correctly grouped by project

### Changed
- **Updated installation docs** — Removed Gatekeeper quarantine workaround now that the app is properly signed and notarized; added worktree-linked terminals and cleanup features to README

## [1.9.0] - 2026-05-28

### Added
- **Worktree cleanup on session close** — New checkbox in the close/dismiss session dialog to automatically clean up the git worktree when closing a session (#116)

### Fixed
- **Worktree cwd set eagerly** — Worktree working directory is now set at session creation time, preventing race conditions (#115)
- **App quit behavior** — Use `window.close()` instead of `window.destroy()` for proper quit handling
- **Worktree path isolation** — Worktree paths no longer contaminate the last-used directory preference
- **Worktree dropdown visibility** — Worktree dropdown now shows correctly for finished/errored sessions and in the new session modal
- **Close confirmation permission** — Added missing window destroy permission for the close confirmation dialog

## [1.8.0] - 2026-05-28

### Added
- **Worktree-linked terminal sessions** — Create terminal sessions linked to an active Claude session's git worktree, with parent-child nesting in the sidebar and cascading close (#109)

### Fixed
- **Quit confirmation not closing app** — The quit confirmation dialog now properly closes the app after user confirms, instead of re-triggering the close prevention (#108)
- **Sidebar text clipping** — Reserve scrollbar space to prevent content from shifting when the scrollbar appears

### Changed
- **Documentation overhaul** — Expanded README from 4 to 14 feature entries, added screenshots for all major features, and updated all doc pages for accuracy

## [1.7.1] - 2026-05-27

### Fixed
- **Unicode grapheme handling** — Use correct Unicode version for xterm graphemes addon, fixing potential text rendering issues

## [1.7.0] - 2026-05-27

### Added
- **Image drag-and-drop** — Drag images from Finder or a web browser onto the active terminal to paste the file path, replicating copy-paste behavior with Claude Code. Includes a visual drop overlay during drag.

### Fixed
- **Shift+Enter newline** — Shift+Enter now correctly inserts a newline instead of submitting input.
- **Emoji spacing** — Fixed emoji spacing in terminal status bar.

## [1.6.0] - 2026-05-27

### Added
- **Settings modal** — Configurable session names via a new settings modal, accessible with Cmd+, or the gear icon in the title bar
- **Quit confirmation dialog** — Prompts before closing the app to prevent accidental session loss
- **Session cycling keybindings** — Navigate between sessions with Cmd+Shift+[ and Cmd+Shift+]
- **Default session names** — New sessions get auto-incrementing placeholder names (Session 1, Session 2, etc.)
- **Directory persistence** — Last used directory is remembered across app restarts
- **Auto-focus confirm button** — Close session dialog now auto-focuses the confirm button for faster keyboard workflows
- **Homebrew Cask distribution** — Agent Orchestrator can now be installed and updated via `brew install --cask agent-orchestrator` (#95)

## [1.4.1] - 2026-05-21

### Fixed
- **Escape key in terminal sessions** — ESC keypresses now pass through to the PTY, fixing navigation in Claude Code settings screens (e.g. `/mcp`). The search bar close-on-ESC continues to work via its own DOM-level handlers.

## [1.4.0] - 2026-05-19

### Changed
- **macOS code signing & notarization** — App is now signed with an Apple Developer ID certificate and notarized, eliminating Gatekeeper warnings on first launch

## [1.3.0] - 2026-05-15

### Added
- **Session persistence** — Sessions are automatically saved when the app closes and restored on startup, preserving terminal scrollback, session names, and project context across restarts (#85)
- **Exited session status** — Sessions that were restored from a previous app session display an "exited" status with distinct styling, and terminals are set to read-only mode
- **Scrollback text capture** — Added ability to extract terminal scrollback content for persistence via `getScrollbackText`

### Fixed
- **Missing search dependency** — Installed the missing `@xterm/addon-search` package

### Changed
- **Session `created_at` field** — Migrated `created_at_epoch_ms` from `i64` to `u64` and added `is_git_repo` to the session list response

## [1.2.3] - 2026-05-14

### Fixed
- **Claude path resolution** — Resolve `claude` to an absolute path to prevent directory name collisions when a project folder is named "claude" (#82)
- **Project path tooltip** — Show full project directory path on hover in the new session modal for better disambiguation (#81)

## [1.2.2] - 2026-05-13

### Fixed
- **Remote default branch detection** — `git pull` now detects the remote's default branch instead of hardcoding "main", fixing issues for repos with different default branch names (#80)
- **VSCode file link support** — Cmd+clicking file paths in the terminal now opens them in VSCode with correct line and column position

## [1.2.1] - 2026-04-30

### Fixed
- **Shift+Enter newline behavior** — Shift+Enter now correctly inserts a newline instead of submitting input (#77)

### Security
- **IPC hardening** — Removed `command` and `args` parameters from `create_session` IPC, eliminating arbitrary command execution via a compromised webview. The backend now derives commands from `session_type` and `session_mode` enums. Folded `git_pull_main` into `create_session` as a boolean flag, removing the path-traversal vector. Added strict validation that rejects unknown values instead of silent fallback (#76)

## [1.2.0] - 2026-04-29

### Added
- **Terminal search** — Press Cmd+F to open a floating search bar for finding text within any terminal session (#75)
- **Claude (auto) session mode** — New session mode that lets Claude run autonomously without manual intervention (#74)

### Fixed
- **Terminal padding color** — Matched terminal padding/margin color to the terminal background, eliminating visual seams (#73)

## [1.1.0] - 2026-04-29

### Added
- **Session mode dropdown** — Replaced individual session checkboxes with a unified mode dropdown for cleaner workflow control (#71)
- **Release management skill** — Added automated release workflow for version bumping, changelog generation, and GitHub releases (#66)

### Fixed
- **Terminal padding background** — Matched terminal padding background color to the active theme, eliminating visual artifacts (#68)

### Changed
- **Open-source preparation** — Added MIT license, contributing guidelines, and cleaned up repository for public release (#72)
- **Documentation revamp** — Redesigned README with visual layout and split documentation into 6 standalone guides (#69)

## [1.0.0] - 2026-04-29 — Public Release

### Added
- **Clickable file paths in terminal** — Cmd+click on file paths in terminal output to open them directly in your editor (#63)

### Fixed
- **Non-git directory session stability** — Fixed session timeout and crash when working in directories without a git repository (#65)
- **Terminal last row clipping** — Fixed the bottom row of terminal output being cut off by moving padding to the .xterm element (#64)

## [0.3.1] - 2026-04-24

### Fixed

- **Terminal resize stability** — Added defensive guards to prevent `fitAddon.fit()` from calculating bogus column/row dimensions during layout transitions, which could permanently break PTY output formatting.

## [0.3.0] - 2026-04-24

### Added

- **Nested subagent status tracking** — The sidebar now shows real-time status of Claude Code subagents spawned within each session. Subagent activity is tracked via a new `SubagentStop` hook and displayed inline with `SubagentList` components. Finished subagents are automatically cleaned up after 30 seconds.
- **Custom app logo** — Replaced the default Tauri logo with a custom "Parallel Streams" logo for the app icon and window chrome.
- **Demo video** — Embedded a demo video in the README.

### Fixed

- **Auto-select next session on close** — When closing the active running session, the app now automatically selects the next available session instead of leaving the view empty.
- **Escape key resets working status** — Pressing Escape in a terminal now correctly resets the session status from "Working" back to "Idle".
- **Hook settings format** — Updated hooks to use the new matcher-based schema format, fixing compatibility with recent Claude Code versions.
- **SessionStart hook matcher** — Changed from object to string matcher to match the expected hook format.

### Changed

- **Documentation overhaul** — Comprehensive update across all project docs, moved dev guide to `DEVELOPMENT.md`, added documentation index to README, and removed all references to the old "Scape" name.

## [0.2.0]

- Initial versioned release.

## [0.1.1]

- Patch release.

## [0.1.0]

- Initial release.
