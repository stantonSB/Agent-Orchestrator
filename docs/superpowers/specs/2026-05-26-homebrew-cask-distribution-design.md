# Homebrew Cask Distribution for Agent Orchestrator

**Date:** 2026-05-26
**Status:** Implemented

## Overview

Distribute Agent Orchestrator via Homebrew Cask so users can install with `brew tap stantonSB/agent-orchestrator && brew install --cask agent-orchestrator`. This requires fixing the existing release workflow's architecture naming bug, creating a tap repository, and automating cask formula updates on each release.

## Problem

1. The current release workflow builds DMGs for both aarch64 and x86_64 but both upload with the same filename (`AgentOrchestrator-v{VERSION}.dmg`), so the second overwrites the first.
2. There is no Homebrew distribution channel — users must manually download from GitHub Releases.

## Design

### 1. Fix Release Workflow — Distinct DMG Names per Architecture

**Current state:** A matrix build produces two DMGs with identical names. The `tauri-apps/tauri-action@v0` handles both build and upload in one step.

**Change:** Restructure `.github/workflows/release.yml` into three jobs:

#### `build` job (matrix: aarch64, x86_64)

Builds the DMG via tauri-action in **build-only mode** — omit `tagName`, `releaseName`, and `releaseBody` inputs so tauri-action builds but does not create a GitHub release.

Tauri produces DMGs named like `Agent Orchestrator_1.5.0_aarch64.dmg` (spaces, underscores, no `v` prefix). After the build step, add an explicit **rename step** to normalize the filename:

```bash
# Rename Tauri's default output to our convention
mv "Agent Orchestrator_${VERSION}_${ARCH}.dmg" "AgentOrchestrator-v${VERSION}-${ARCH}.dmg"
```

Then upload the renamed DMG as a GitHub Actions artifact using `actions/upload-artifact@v4` with a name like `dmg-aarch64` or `dmg-x86_64`.

#### `release` job (depends on `build`)

1. Downloads both artifacts using `actions/download-artifact@v4`
2. Creates the GitHub release using `gh release create $TAG --title "$TAG" --notes "..."`
3. Uploads both DMGs using `gh release upload $TAG AgentOrchestrator-v*-aarch64.dmg AgentOrchestrator-v*-x86_64.dmg`

Since `gh release upload` runs within the same job as `gh release create`, all assets are guaranteed to be present before the job completes.

#### `update-homebrew` job (depends on `release`)

See Section 3 below.

**Asset naming convention:**
- `AgentOrchestrator-v{VERSION}-aarch64.dmg`
- `AgentOrchestrator-v{VERSION}-x86_64.dmg`

The `.app.tar.gz` update bundles (used by Tauri's built-in updater) are not currently in use and are excluded from the rename/upload steps. If Tauri's auto-updater is enabled in the future, the same rename convention should be applied to those artifacts.

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
    sha256 "AARCH64_SHA256"
    url "https://github.com/stantonSB/Agent-Orchestrator/releases/download/v#{version}/AgentOrchestrator-v#{version}-aarch64.dmg"
  end

  on_intel do
    sha256 "X86_64_SHA256"
    url "https://github.com/stantonSB/Agent-Orchestrator/releases/download/v#{version}/AgentOrchestrator-v#{version}-x86_64.dmg"
  end

  name "Agent Orchestrator"
  desc "Desktop app for running parallel Claude Code terminal sessions"
  homepage "https://github.com/stantonSB/Agent-Orchestrator"

  livecheck do
    url "https://github.com/stantonSB/Agent-Orchestrator"
    strategy :github_latest
  end

  app "Agent Orchestrator.app"

  zap trash: [
    "~/Library/Application Support/com.xbridge.agent-orchestrator",
    "~/Library/Caches/com.xbridge.agent-orchestrator",
    "~/Library/Preferences/com.xbridge.agent-orchestrator.plist",
    "~/Library/Saved Application State/com.xbridge.agent-orchestrator.savedState",
  ]
  # Note: Agent Orchestrator installs hooks in ~/.claude/settings.json and
  # ~/.claude.json on first launch. These are shared config files used by
  # Claude Code and are intentionally NOT removed on uninstall to avoid
  # breaking other tools. Users can manually remove the
  # "agent-orchestrator-notify" hook entries if desired.
end
```

**README.md contents:** Brief install/upgrade/uninstall instructions and a link to the main Agent Orchestrator repo.

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

The `update-homebrew` job in the release workflow (depends on `release`):

1. Downloads both DMGs from the just-created GitHub release using `gh release download $TAG --pattern '*.dmg'`.
2. Computes SHA256 for each DMG: `shasum -a 256 <file> | awk '{print $1}'`.
3. Clones `stantonSB/homebrew-agent-orchestrator` using the PAT.
4. Updates the cask formula using a python3 script that understands the block structure:

```bash
VERSION="${TAG#v}"
AARCH64_SHA=$(shasum -a 256 "AgentOrchestrator-v${VERSION}-aarch64.dmg" | awk '{print $1}')
X86_64_SHA=$(shasum -a 256 "AgentOrchestrator-v${VERSION}-x86_64.dmg" | awk '{print $1}')

python3 -c "
import re
content = open('Casks/agent-orchestrator.rb').read()
content = re.sub(r'version \"[^\"]+\"', 'version \"${VERSION}\"', content, count=1)
# Replace first sha256 (on_arm block) and second sha256 (on_intel block)
# Matches both 64-char hex hashes and placeholder strings like AARCH64_SHA256
shas = ['${AARCH64_SHA}', '${X86_64_SHA}']
i = [0]
def replacer(m):
    result = f'sha256 \"{shas[i[0]]}\"'
    i[0] += 1
    return result
content = re.sub(r'sha256 \"[^\"]+\"', replacer, content)
open('Casks/agent-orchestrator.rb', 'w').write(content)
"
```

The regex `sha256 \"[^\"]+\"` matches any sha256 value — both 64-char hex hashes from previous releases and the initial placeholder strings (`AARCH64_SHA256`, `X86_64_SHA256`) on first run. The replacements are positional: first match is the `on_arm` block, second is `on_intel`.

5. Commits and pushes to the tap repo.

**Authentication:** A fine-grained GitHub Personal Access Token stored as `HOMEBREW_TAP_TOKEN` in the Agent-Orchestrator repo secrets. The fine-grained PAT must be scoped to the `homebrew-agent-orchestrator` repository only, with **Contents: Read and write** permission.

**Workflow structure summary:**
```
release.yml
  build (matrix: aarch64, x86_64)
    → tauri-action in build-only mode (no tagName/releaseName/releaseBody)
    → rename DMG to arch-specific convention
    → upload as GHA artifact
  release (needs: build)
    → download artifacts
    → gh release create + gh release upload both DMGs
  update-homebrew (needs: release)
    → gh release download both DMGs
    → compute SHA256s
    → clone tap repo, update formula via python3 script, commit, push
```

## Out of Scope

- **Official homebrew-cask submission:** Deferred until the project builds sufficient notability (stars, downloads). The own-tap approach works identically for users.
- **Linux/Windows Homebrew support:** macOS only for now.
- **Tauri built-in updater changes:** The `.app.tar.gz` artifacts and updater endpoint are unaffected by this work.
- **Hook cleanup on uninstall:** Agent Orchestrator writes entries to `~/.claude/settings.json` and `~/.claude.json`. These are shared config files and are intentionally left in place on uninstall. Documented in the cask formula comment.

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| PAT token expires or is revoked | Use a fine-grained PAT with long expiry scoped only to the tap repo with Contents: Read and write permission. Workflow failure is non-blocking (release still succeeds, cask just doesn't auto-update). |
| DMG asset names change in future Tauri versions | The rename step normalizes names regardless of Tauri's output format. Pin tauri-action version; test on upgrades. |
| Breaking change for users with bookmarked old DMG URLs | Old URL format only ever had one arch's DMG anyway (overwrite bug). Document the new URLs in release notes. |

## Testing

- After restructuring the release workflow, cut a test prerelease (`v1.5.1-rc.1`) and verify both DMGs appear on the GitHub release with correct arch-specific names.
- After creating the tap repo, test `brew tap` + `brew install --cask` + `brew uninstall --cask` locally.
- After adding the auto-update job, cut another test release and verify the cask formula is updated automatically in the tap repo.
