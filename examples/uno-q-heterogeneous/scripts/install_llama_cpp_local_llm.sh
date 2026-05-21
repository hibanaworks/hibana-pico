#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

ADB="${ADB:-adb}"
LLAMA_CPP_TAG="${LLAMA_CPP_TAG:-b9263}"
LLAMA_ARCHIVE="llama-${LLAMA_CPP_TAG}-bin-ubuntu-arm64.tar.gz"
LLAMA_URL="${LLAMA_CPP_RELEASE_URL:-https://github.com/ggml-org/llama.cpp/releases/download/${LLAMA_CPP_TAG}/${LLAMA_ARCHIVE}}"
MODEL_NAME="${UNO_Q_LOCAL_LLM_MODEL_NAME:-Qwen2.5-0.5B-Instruct-Q4_K_M.gguf}"
MODEL_URL="${UNO_Q_LOCAL_LLM_MODEL_URL:-https://huggingface.co/bartowski/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/${MODEL_NAME}}"
CACHE_DIR="${REPO_ROOT}/target/uno-q-local-llm"
REMOTE_ROOT="${UNO_Q_LOCAL_LLM_REMOTE_ROOT:-/data/local/tmp/uno-q-local-llm}"

mkdir -p "${CACHE_DIR}"

download_if_missing() {
  local url="$1"
  local out="$2"
  if [[ -s "${out}" ]]; then
    return
  fi
  curl --fail --location --retry 3 --continue-at - --output "${out}" "${url}"
}

download_if_missing "${LLAMA_URL}" "${CACHE_DIR}/${LLAMA_ARCHIVE}"
download_if_missing "${MODEL_URL}" "${CACHE_DIR}/${MODEL_NAME}"

DIST_DIR="${CACHE_DIR}/llama-${LLAMA_CPP_TAG}"
mkdir -p "${DIST_DIR}"
tar -xzf "${CACHE_DIR}/${LLAMA_ARCHIVE}" -C "${DIST_DIR}"
BIN_DIR="$(dirname "$(find "${DIST_DIR}" -type f -name llama-completion | head -n 1)")"
if [[ ! -x "${BIN_DIR}/llama-completion" ]]; then
  echo "llama-completion not found in ${DIST_DIR}" >&2
  exit 1
fi
if [[ ! -x "${BIN_DIR}/llama-server" ]]; then
  echo "llama-server not found in ${DIST_DIR}" >&2
  exit 1
fi

"${ADB}" shell "mkdir -p '${REMOTE_ROOT}/bin' '${REMOTE_ROOT}/lib' '${REMOTE_ROOT}/models'"
for bin in llama-completion llama-cli llama-server; do
  if [[ -f "${BIN_DIR}/${bin}" ]]; then
    "${ADB}" push "${BIN_DIR}/${bin}" "${REMOTE_ROOT}/bin/${bin}" >/dev/null
  fi
done
for lib in "${BIN_DIR}"/*.so*; do
  [[ -e "${lib}" ]] || continue
  "${ADB}" push "${lib}" "${REMOTE_ROOT}/lib/$(basename "${lib}")" >/dev/null
done
"${ADB}" push "${CACHE_DIR}/${MODEL_NAME}" "${REMOTE_ROOT}/models/${MODEL_NAME}" >/dev/null
"${ADB}" shell "cp '${REMOTE_ROOT}'/lib/*.so* '${REMOTE_ROOT}/bin/' 2>/dev/null || true; chmod 755 '${REMOTE_ROOT}/bin'/llama-*"

"${ADB}" shell "cd '${REMOTE_ROOT}/bin' && LD_LIBRARY_PATH=. ./llama-completion -m '../models/${MODEL_NAME}' --no-display-prompt --simple-io --no-warmup -t 4 -n 8 --temp 0 -p 'You are controlling a shell. Return one terminal input line: ls' 2>/tmp/uno-q-local-llm-smoke.err" |
  grep -q '^ls$'

echo "installed llama.cpp local LLM:"
echo "  server: ${REMOTE_ROOT}/bin/llama-server"
echo "  fallback completion: ${REMOTE_ROOT}/bin/llama-completion"
echo "  model:  ${REMOTE_ROOT}/models/${MODEL_NAME}"
