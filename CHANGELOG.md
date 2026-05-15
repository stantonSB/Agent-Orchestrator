# Changelog

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
