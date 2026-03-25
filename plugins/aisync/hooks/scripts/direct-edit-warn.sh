#!/bin/bash
set -euo pipefail

# Require jq for JSON parsing
if ! command -v jq &>/dev/null; then
  exit 0
fi

input=$(cat)
tool_name=$(echo "$input" | jq -r '.tool_name // empty')
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')

# Only care about Edit/Write operations
if [[ "$tool_name" != "Edit" && "$tool_name" != "Write" ]]; then
  exit 0
fi

# Check if this is an aisync-managed project
if [ ! -f "$CLAUDE_PROJECT_DIR/aisync.toml" ]; then
  exit 0
fi

# Synced target files that shouldn't be edited directly
synced_targets=(
  "CLAUDE.md"
  "AGENTS.md"
  ".cursor/rules/project.mdc"
  ".cursor/hooks.json"
  ".cursor/mcp.json"
  ".opencode/plugins/aisync-hooks.js"
)

# Get relative path from project root
rel_path="${file_path#"$CLAUDE_PROJECT_DIR"/}"

for target in "${synced_targets[@]}"; do
  if [[ "$rel_path" == "$target" ]]; then
    echo "{\"systemMessage\": \"WARNING: '$rel_path' is managed by AgentSync and will be overwritten on next sync. Edit the canonical source in .ai/ instead (e.g., .ai/instructions.md for CLAUDE.md/AGENTS.md).\"}" >&2
    exit 2
  fi
done

exit 0
