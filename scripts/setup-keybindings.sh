#!/usr/bin/env bash
# Ensures ~/.claude/keybindings.json has the project's recommended keybindings.
# Runs automatically via Claude Code SessionStart hook.

set -euo pipefail

KEYBINDINGS_FILE="$HOME/.claude/keybindings.json"

mkdir -p "$HOME/.claude"

if [ ! -f "$KEYBINDINGS_FILE" ]; then
  cat > "$KEYBINDINGS_FILE" << 'KEYBINDINGS'
{
  "$schema": "https://www.schemastore.org/claude-code-keybindings.json",
  "$docs": "https://code.claude.com/docs/en/keybindings",
  "bindings": [
    {
      "context": "Chat",
      "bindings": {
        "shift+enter": "chat:newline"
      }
    }
  ]
}
KEYBINDINGS
  exit 0
fi

# File exists — check if shift+enter is already bound
if grep -q '"shift+enter"' "$KEYBINDINGS_FILE" 2>/dev/null; then
  exit 0
fi

# Add shift+enter binding using node (available in this project)
node -e "
const fs = require('fs');
const cfg = JSON.parse(fs.readFileSync('$KEYBINDINGS_FILE', 'utf8'));
if (!cfg.bindings) cfg.bindings = [];
let chatCtx = cfg.bindings.find(b => b.context === 'Chat');
if (!chatCtx) {
  chatCtx = { context: 'Chat', bindings: {} };
  cfg.bindings.push(chatCtx);
}
chatCtx.bindings['shift+enter'] = 'chat:newline';
fs.writeFileSync('$KEYBINDINGS_FILE', JSON.stringify(cfg, null, 2) + '\n');
"
