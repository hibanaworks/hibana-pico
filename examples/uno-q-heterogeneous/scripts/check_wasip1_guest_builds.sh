#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
GUEST_MANIFEST="${REPO_ROOT}/examples/uno-q-heterogeneous/wasip1/guest/Cargo.toml"
GUEST_DIR="${REPO_ROOT}/examples/uno-q-heterogeneous/wasip1/guest"
WASM="${REPO_ROOT}/target/wasip1-apps/wasm32-wasip1/release/uno-q-llm-face-shell.wasm"
SHELL_LOOP_WASM="${REPO_ROOT}/target/wasip1-apps/wasm32-wasip1/release/uno-q-llm-face-shell-loop.wasm"

cd "${GUEST_DIR}"
cargo build \
  --manifest-path "${GUEST_MANIFEST}" \
  --target wasm32-wasip1 \
  --release \
  --target-dir "${REPO_ROOT}/target/wasip1-apps"

for artifact in "${WASM}" "${SHELL_LOOP_WASM}"; do
  if [[ ! -s "${artifact}" ]]; then
    echo "missing WASI guest artifact: ${artifact}" >&2
    exit 1
  fi
done

python3 - "${WASM}" "${SHELL_LOOP_WASM}" <<'PY'
import sys
from pathlib import Path

for path in sys.argv[1:]:
    wasm = Path(path).read_bytes()
    if wasm[:4] != b"\0asm":
        raise SystemExit(f"{path}: not a wasm module")

    pos = 8

    def uleb(offset):
        value = 0
        shift = 0
        while True:
            byte = wasm[offset]
            offset += 1
            value |= (byte & 0x7F) << shift
            if byte < 0x80:
                return value, offset
            shift += 7

    def name(offset):
        length, offset = uleb(offset)
        return wasm[offset:offset + length].decode("utf-8"), offset + length

    imports = []
    exports = []
    memory = None
    while pos < len(wasm):
        section_id = wasm[pos]
        pos += 1
        size, pos = uleb(pos)
        end = pos + size
        if section_id == 2:
            count, pos = uleb(pos)
            for _ in range(count):
                module, pos = name(pos)
                item, pos = name(pos)
                kind = wasm[pos]
                pos += 1
                imports.append((module, item))
                if kind == 0:
                    _, pos = uleb(pos)
                elif kind in (1, 2):
                    flags, pos = uleb(pos)
                    _, pos = uleb(pos)
                    if flags & 1:
                        _, pos = uleb(pos)
                elif kind == 3:
                    pos += 2
        elif section_id == 5:
            count, cursor = uleb(pos)
            if count != 1:
                raise SystemExit(f"{path}: expected one memory, got {count}")
            flags, cursor = uleb(cursor)
            minimum, cursor = uleb(cursor)
            maximum = None
            if flags & 1:
                maximum, cursor = uleb(cursor)
            memory = (minimum, maximum)
        elif section_id == 7:
            count, pos = uleb(pos)
            for _ in range(count):
                item, pos = name(pos)
                kind = wasm[pos]
                pos += 1
                index, pos = uleb(pos)
                exports.append((item, kind, index))
        pos = end

    bad_imports = [
        f"{module}.{item}"
        for module, item in imports
        if module != "wasi_snapshot_preview1"
    ]
    if bad_imports:
        raise SystemExit(f"{path}: guest must import only WASI P1 functions: {bad_imports}")
    if memory != (1, 1):
        raise SystemExit(f"{path}: guest memory must be exactly one 64KiB page, got {memory}")
    if not any(item == "_start" and kind == 0 for item, kind, _ in exports):
        raise SystemExit(f"{path}: std WASI guest must export _start")
PY

echo "${WASM}"
echo "${SHELL_LOOP_WASM}"
