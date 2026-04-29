---
name: release
description: Manage releases for Agent Orchestrator — bump versions, generate release notes from git commits, build artifacts, tag, and publish GitHub releases. Use this skill whenever the user mentions "release", "new version", "bump version", "cut a release", "ship it", "publish a release", "make a release", "version bump", "patch release", "minor release", "major release", or anything related to preparing and shipping a new version of Agent Orchestrator. Even casual phrases like "let's ship" or "time for a new release" should trigger this skill.
---

# Release Skill — Agent Orchestrator

This skill guides the full release workflow for Agent Orchestrator, a Tauri 2 desktop app. It covers version bumping, release note generation, building, tagging, and GitHub release creation.

## Why this workflow exists

Releasing Agent Orchestrator requires coordinating version numbers across three files, generating meaningful release notes from conventional commits, building platform-specific artifacts (.app + .dmg), and publishing everything to GitHub. Doing this manually is error-prone — it's easy to forget a version file, miss a commit in the notes, or name an artifact wrong. This skill makes the process reliable and repeatable.

## Release Workflow

### Step 1: Determine current version and release type

Read the current version from `package.json` (the source of truth). Confirm it matches `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`. If they're out of sync, flag this to the user before proceeding.

Then ask the user what type of release this is:

| Type | When to use | Example |
|------|------------|---------|
| **patch** | Bug fixes, small tweaks | 0.5.1 → 0.5.2 |
| **minor** | New features, non-breaking changes | 0.5.1 → 0.6.0 |
| **major** | Breaking changes, major milestones | 0.5.1 → 1.0.0 |

Use AskUserQuestion to let them pick. Show the current version and what the new version would be for each option.

### Step 2: Gather commits for release notes

Get all commits between the last release tag and HEAD:

```bash
git log v<current-version>..HEAD --oneline --no-merges
```

If there's no tag for the current version, fall back to the most recent `v*` tag.

### Step 3: Generate release notes

Categorize commits using their conventional commit prefixes into a changelog entry that follows the existing CHANGELOG.md format (Keep a Changelog style):

```markdown
## [<new-version>] - <YYYY-MM-DD>

### Added
- **Feature name** — Description (from `feat:` commits)

### Fixed
- **Fix name** — Description (from `fix:` commits)

### Changed
- **Change name** — Description (from `refactor:`, `chore:`, `perf:` commits)
```

Guidelines for writing good release notes:
- Transform terse commit messages into user-friendly descriptions — the audience is someone deciding whether to update
- Group related commits into a single bullet when they're part of the same logical change
- Bold the feature/fix name, then use an em dash before the description (matching existing CHANGELOG.md style)
- Skip commits that are purely internal (CI tweaks, typo fixes in comments) unless the user wants them included
- If a commit references a PR number like `(#65)`, keep it

Present the draft release notes to the user for review before applying them. Let them edit or approve.

### Step 4: Update version numbers

Update the version string in all three files:

1. **`package.json`** — the `"version"` field
2. **`src-tauri/Cargo.toml`** — the `version` field under `[package]`
3. **`src-tauri/tauri.conf.json`** — the `"version"` field

Also update `src-tauri/Cargo.lock` by running:
```bash
cd src-tauri && cargo check
```
This regenerates the lock file with the new version without doing a full build.

### Step 5: Update CHANGELOG.md

Prepend the new release entry to CHANGELOG.md, right after the `# Changelog` heading. Keep all existing entries intact.

### Step 6: Commit the version bump

Stage all changed files and create a commit:
```
chore: release v<new-version>
```

### Step 7: Build release artifacts

Run the Tauri build:
```bash
npm run tauri build
```

The build produces artifacts in `src-tauri/target/release/bundle/`:
- `dmg/Agent Orchestrator_<version>_aarch64.dmg` (or similar)
- `macos/Agent Orchestrator.app` (inside a directory)

After the build completes, verify the artifacts exist. Rename/copy them to follow the release naming convention:
```
AgentOrchestrator-v<version>.dmg
AgentOrchestrator-v<version>.app.tar.gz
```

For the .app, create a compressed tarball since .app is a directory:
```bash
cd src-tauri/target/release/bundle/macos
tar -czf AgentOrchestrator-v<version>.app.tar.gz "Agent Orchestrator.app"
```

### Step 8: Tag and push

```bash
git tag v<new-version>
git push origin main
git push origin v<new-version>
```

Confirm with the user before pushing — this is a shared-state operation.

### Step 9: Create GitHub release

```bash
gh release create v<new-version> \
  --title "v<new-version>" \
  --notes-file <path-to-release-notes> \
  <path-to-dmg> \
  <path-to-app-tarball>
```

The `--notes-file` should contain just the release notes for this version (not the full changelog). Write them to a temp file for this purpose.

### Step 10: Confirm completion

Tell the user:
- The new version number
- Link to the GitHub release
- Summary of what was included

## Error handling

- **Build failure**: If `npm run tauri build` fails, do not proceed with tagging or releasing. Show the error and help debug.
- **Version mismatch**: If the three version files don't agree at the start, stop and fix that first.
- **No new commits**: If there are no commits since the last tag, ask the user if they really want an empty release.
- **Tag already exists**: If `v<new-version>` tag already exists, stop and ask the user what to do.

## Rollback

If something goes wrong after committing but before pushing:
```bash
git reset --soft HEAD~1  # Undo the commit, keep changes staged
git tag -d v<new-version>  # Delete the local tag
```

If already pushed, the user will need to handle this manually — don't force-push or delete remote tags without explicit user approval.
