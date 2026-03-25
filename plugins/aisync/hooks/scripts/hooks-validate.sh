#!/bin/bash
set -euo pipefail

# Require jq for JSON parsing
if ! command -v jq &>/dev/null; then
  exit 0
fi

input=$(cat)
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')

# Only trigger for hooks.toml edits
rel_path="${file_path#"$CLAUDE_PROJECT_DIR"/}"
if [[ "$rel_path" != ".ai/hooks.toml" ]]; then
  exit 0
fi

# Check if aisync is available
if ! command -v aisync &>/dev/null; then
  exit 0
fi

# Validate by attempting translation
output=$(aisync hooks translate 2>&1) || {
  jq -n --arg msg "hooks.toml validation failed: $output" '{"systemMessage": $msg}' >&2
  exit 2
}

echo "hooks.toml validated successfully"
exit 0
