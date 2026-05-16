#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if git ls-files --error-unmatch plan.md >/dev/null 2>&1; then
  echo "plan gate failed: plan.md must be local-only and untracked" >&2
  exit 1
fi

if cargo package --list --allow-dirty 2>/dev/null | rg -n '(^|/)plan\.md$'; then
  echo "plan gate failed: plan.md must not be included in the Cargo package" >&2
  exit 1
fi

bash ./scripts/check_wasip1_guest_builds.sh
cargo check --workspace --all-targets
cargo check --workspace --all-targets --all-features
cargo check -p heterogeneous-split-example --all-targets
cargo check -p heterogeneous-split-example --target thumbv6m-none-eabi --bin rp2040-io
cargo check -p heterogeneous-split-example --target thumbv8m.main-none-eabihf --bin m33-realtime
cargo test --test host_architecture_boundaries
cargo test --test host_capsule_api --features wasm-engine-core,wasip1-sys-fd-write,wasip1-sys-path-open,wasip1-sys-poll-oneoff,wasip1-sys-proc-exit
cargo test -p hibana-pico --features wasm-engine-core,wasip1-sys-fd-write --lib drive_wasi_guest_completes_import_only_through_endpoint_carrier
cargo test -p hibana-pico --features wasm-engine-core,wasip1-sys-fd-write --lib run_drives_wasi_guest_import_completion_through_endpoint_carrier
cargo test -p hibana-pico --all-features --lib

if rg -n -S 'pub mod (kernel|machine|port|projects|proof);' src/lib.rs; then
  echo "plan gate failed: forbidden public root module" >&2
  exit 1
fi

if rg -n -S 'mod (machine|port|projects);' src/lib.rs || test -e src/machine || test -e src/port || test -e src/projects || test -e artifacts; then
  echo "plan gate failed: empty private placeholder modules and root artifacts/ are not permitted" >&2
  exit 1
fi

if rg -n -S --glob '!scripts/check_plan_pico_gates.sh' 'appkit::(Choreo\b|Program\b|support\b)|pub mod proof|proof::|NetworkRoute|RemoteRoute|PicoFdRoute|with_policy|cap_grant_remote|apply_cap_grant_with_policy|AttachedImage|RunCtx|I::launch|fn run\(attached|project_role|AttachOnlyTransport|materialized_role_count|macro_rules!|g::steps|Program<steps::|wasm-engine-tiny|TinyWasm|CoreWasm|CoreWasip1' src examples guest Cargo.toml README.md scripts; then
  echo "plan gate failed: forbidden legacy public/runtime surface" >&2
  exit 1
fi

if rg -n -S 'appkit build|proc_macro choreography|choreo!|placement!|xtask required|external projection generator' src examples guest Cargo.toml; then
  echo "plan gate failed: forbidden build or DSL surface" >&2
  exit 1
fi

if rg -n -S 'wasi:(cli|clocks|filesystem|http|io|random|sockets)|wasi_snapshot_preview2|wasm32-wasip2|wasip2|wit-bindgen|wit_component|component-model' Cargo.toml README.md src examples guest --glob '!src/appkit.rs'; then
  echo "plan gate failed: forbidden WASI P2 / WIT / Component Model surface" >&2
  exit 1
fi

if rg -n -S '#\[allow|#!\[allow|allow\((dead_code|unused|warnings)' src tests examples guest; then
  echo "plan gate failed: dead-code/unused allowances are not permitted" >&2
  exit 1
fi

if rg -n -S 'as _\b|let _[A-Za-z0-9_]*\b|for _[A-Za-z0-9_]*\b|(^|[(,])\s*_[A-Za-z0-9_]+\s*:' src tests examples guest; then
  echo "plan gate failed: capsule/appkit code must not hide unused values behind underscore bindings" >&2
  exit 1
fi

if rg -n -S 'platform-(host-native|linux|cortex-m)' Cargo.toml examples guest scripts src --glob '!scripts/check_plan_pico_gates.sh'; then
  echo "plan gate failed: std/no_std and site family behavior must follow Rust target and site types, not platform feature flags" >&2
  exit 1
fi

if git ls-files --others --exclude-standard | rg -n '/target/'; then
  echo "plan gate failed: generated target/ artifacts must stay ignored" >&2
  exit 1
fi

if sed -n '/^\[features\]/,/^\[/p' Cargo.toml | rg -n -S 'embed-wasip1-artifacts'; then
  echo "plan gate failed: artifact embedding is example/physical-artifact packaging, not a hibana-pico core feature" >&2
  exit 1
fi

if rg -n -S 'Box<dyn Future|Vec<ScheduledTask|Box::pin|std::vec!\[' src/appkit.rs; then
  echo "plan gate failed: appkit scheduler/storage must use bounded in-place storage, not host heap shortcuts" >&2
  exit 1
fi

if rg -n -S 'pub mod (carrier|host|linux|mcu|rp2040|swarm|process|bare)|pub struct (Native|Core)|pub const (IN_PROCESS|TCP|UDP|UART|USB)|SioTransport|core_id\(\)' src/site.rs; then
  echo "plan gate failed: core site must stay generic; board/carrier-specific site families belong in examples or user crates" >&2
  exit 1
fi

if rg -n -S 'site::carrier|appkit::InProcessCarrier|pub struct InProcess(Carrier|Tx|Rx)|has_in_process_carrier' src examples tests README.md --glob '!tests/host_architecture_boundaries.rs'; then
  echo "plan gate failed: in-process carrier vocabulary must be user/example implementation, not a core public path" >&2
  exit 1
fi

if rg -n -S 'RefCell|CarrierAttachState|AttachedCarrierFrame|push_attached_frame|pop_attached_frame|requeue_attached_frame' src/appkit.rs; then
  echo "plan gate failed: appkit core must not carry local queue/refcell carrier implementation details" >&2
  exit 1
fi

if rg -n -S 'AtomicBool|compare_exchange\(false, true|unsafe impl Sync for WasiGuestArena|pub fn storage<.*&'\''static self|pub unsafe fn storage_from_owner|storage_from_owner\(|WasiGuestStorage|wasi_guest_storage' src/appkit.rs; then
  echo "plan gate failed: WASI guest arena must be single-owner storage, not a shared atomic lease" >&2
  exit 1
fi

if rg -n -S 'Atomic(Bool|U|I|Ptr)|Ordering|compare_exchange|fetch_|load\(Ordering|store\(.*Ordering' examples/baker-firmware/src; then
  echo "plan gate failed: Baker examples must not use shared atomic readiness/state flags" >&2
  exit 1
fi

if rg -n -S 'rp2040-boot2|rp2040_boot2' examples/baker-firmware/Cargo.toml examples/baker-firmware/src; then
  echo "plan gate failed: Baker boot code must live in the Baker example, not an external boot crate" >&2
  exit 1
fi

echo "plan_pico gates ok"
