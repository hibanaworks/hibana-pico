#!/usr/bin/env bash
set -euo pipefail

ADB="${ADB:-adb}"
REMOTE_PROMPT="${UNO_Q_LOCAL_LLM_USER_PROMPT_FILE:-/data/local/tmp/uno-q-local-llm/user-prompt.txt}"

if [[ "$#" -gt 0 ]]; then
  prompt="$*"
else
  prompt="$(cat)"
fi

"${ADB}" shell "mkdir -p '$(dirname "${REMOTE_PROMPT}")'"
printf '%s\n' "${prompt}" | "${ADB}" shell "cat > '${REMOTE_PROMPT}'"

echo "wrote human prompt to ${REMOTE_PROMPT}"
