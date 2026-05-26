# Homebrew Cask Distribution for Agent Orchestrator

**Date:** 2026-05-26
**Status:** Draft

## Overview

Distribute Agent Orchestrator via Homebrew Cask so users can install with `brew tap stantonSB/agent-orchestrator && brew install --cask agent-orchestrator`. This requires fixing the existing release workflow's architecture naming bug, creating a tap repository, and automating cask formula updates on each release.

## Problem

1. The current release workflow builds DMGs for both aarch64 and x86_64 but both upload with the same filename (`AgentOrchestrator-v{VERSION}.dmg`), so the second overwrites the first.
2. There is no Homebrew distribution channel — users must manually download from GitHub Releases.

## Design

### 1. Fix Release Workflow — Distinct DMG Names per Architecture

**Current state:** A matrix build produces two DMGs with identical names. The `tauri-apps/tauri-action@v0` handles both build and upload in one step.

**Change:** Restructure `.github/workflows/release.yml` into two jobs:

- **`build` job** (matrix: aarch64, x86_64): Builds the DMG via tauri-action but does NOT create the release. Uploads the DMG as a GitHub Actions artifact with an arch-specific name.
- **`release` job** (depends on `build`): Downloads both artifacts, creates a single GitHub release, and attaches both DMGs with distinct names.

**Asset naming:**
- `AgentOrchestrator-v{VERSION}-aarch64.dmg`
- `AgentOrchestrator-v{VERSION}-x86_64.dmg`

The `.app.tar.gz` update bundles (used by Tauri's built-in updater) follow the same pattern if present.

### 2. Homebrew Tap Repository

**New repo:** `stantonSB/homebrew-agent-orchestrator`

**Structure:**
```
homebrew-agent-orchestrator/
  Casks/
    agent-orchestrator.rb
  README.md
```

**Cask formula** (`Casks/agent-orchestrator.rb`):

```ruby
cask "agent-orchestrator" do
  version "1.5.0"

  on_arm do
    sha256 "<aarch64-sha256>"
    url "https://github.com/stantonSB/Agent-Orchestrator/releases/download/v#{version}/AgentOrchestrator-v#{version}-aarch64.dmg"
  end

  on_intel do
    sha256 "<x86_64-sha256>"
    url "https://github.com/stantonSB/Agent-Orchestrator/releases/download/v#{version}/AgentOrchestrator-v#{version}-x86_64.dmg"
  end

  name "Agent Orchestrator"
  desc "Desktop app for running parallel Claude Code terminal sessions"
  homepage "https://github.com/stantonSB/Agent-Orchestrator"

  livecheck do
    url :url
    strategy :github_latest
  end

  app "Agent Orchestrator.app"

  zap trash: [
    "~/Library/Application Support/com.xbridge.agent-orchestrator",
    "~/Library/Caches/com.xbridge.agent-orchestrator",
    "~/Library/Preferences/com.xbridge.agent-orchestrator.plist",
    "~/Library/Saved Application State/com.xbridge.agent-orchestrator.savedState",
  ]
end
```

**User install:**
```bash
brew tap stantonSB/agent-orchestrator
brew install --cask agent-orchestrator
```

**Upgrade:**
```bash
brew upgrade --cask agent-orchestrator
```

### 3. Auto-Update Cask on Release

Add a third job to the existing release workflow in `Agent-Orchestrator/.github/workflows/release.yml`:

**`update-homebrew` job** (depends on `release`):

1. Downloads both DMGs from the just-created GitHub release using `gh release download`.
2. Computes SHA256 for each DMG.
3. Clones `stantonSB/homebrew-agent-orchestrator` using a PAT.
4. Rewrites `Casks/agent-orchestrator.rb` with the new version and SHA256 values using `sed`.
5. Commits and pushes to the tap repo.

**Authentication:** A GitHub Personal Access Token stored as `HOMEBREW_TAP_TOKEN` in the Agent-Orchestrator repo secrets. This token needs `repo` scope (write access to `homebrew-agent-orchestrator`).

**Workflow structure summary:**
```
release.yml
  build (matrix: aarch64, x86_64)
    → uploads arch-specific artifacts
  release (needs: build)
    → downloads artifacts, creates GitHub release with both DMGs
  update-homebrew (needs: release)
    → computes SHAs, updates cask formula, pushes to tap repo
```

## Out of Scope

- **Official homebrew-cask submission:** Deferred until the project builds sufficient notability (stars, downloads). The own-tap approach works identically for users.
- **Linux/Windows Homebrew support:** macOS only for now.
- **Tauri built-in updater changes:** The `.app.tar.gz` artifacts and updater endpoint are unaffected by this work.

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| PAT token expires or is revoked | Use a fine-grained PAT with long expiry scoped only to the tap repo. Workflow failure is non-blocking (release still succeeds, cask just doesn't auto-update). |
| DMG asset names change in future Tauri versions | Pin tauri-action version; test on upgrades. |
| Breaking change for users with bookmarked old DMG URLs | Old URL format only ever had one arch's DMG anyway (overwrite bug). Document the new URLs in release notes. |

## Testing

- After restructuring the release workflow, cut a test prerelease (`v1.5.1-rc.1`) and verify both DMGs appear with correct arch-specific names.
- After creating the tap repo, test `brew tap` + `brew install --cask` + `brew uninstall --cask` locally.
- After adding auto-update job, cut another test release and verify the cask formula is updated automatically.
