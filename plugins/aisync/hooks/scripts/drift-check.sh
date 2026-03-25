#!/bin/bash
set -euo pipefail

# Check if aisync is available
if ! command -v aisync &>/dev/null; then
  echo "aisync CLI not found — skipping drift check"
  exit 0
fi

# Check if this is an aisync-managed project
if [ ! -f "$CLAUDE_PROJECT_DIR/aisync.toml" ]; then
  exit 0
fi

# Require jq for JSON parsing
if ! command -v jq &>/dev/null; then
  # Fallback: run check without JSON parsing
  aisync check 2>/dev/null || echo "AgentSync: Config drift detected. Run /aisync:sync to fix."
  exit 0
fi

# Run drift check
check_status=0
output=$(aisync check --json 2>/dev/null) || check_status=$?

if [ $check_status -ne 0 ]; then
  drifted=$(echo "$output" | jq -r '.drifted // [] | .[] // empty' 2>/dev/null || true)
  if [ -n "$drifted" ]; then
    echo "AgentSync: Config drift detected. Run /aisync:sync to fix."
    echo "Drifted tools: $drifted"
  fi
fi

exit 0
