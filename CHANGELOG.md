# Changelog

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
