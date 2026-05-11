# Baker Link Dev Rev1 Bring-Up Notes

This note is the practical hardware path for the Baker Link Dev Rev1 / RP2040
LED smoke. It records the exact `probe-rs` and GDB workflow used during the
first successful board run, plus the failure modes that looked like hibana
phase errors but were actually board bring-up issues.

## What The Demo Proves

The Baker LED demo is the smallest physical proof of the Hibana/Pico shape:

```text
Rust-built WASI P1 app
  -> fd_write(fd=3, "1")
  -> Engine role
  -> Kernel role
  -> ChoreoFS GpioDevice object fd
  -> GuestLedger fd + read lease check
  -> GPIO role
  -> GP22 green on
  -> poll_oneoff(250 ticks)
  -> GuestLedger pending poll token
  -> Timer role
  -> TIMER0 IRQ top-half records raw readiness
  -> resolver admits TimerSleepDone
  -> GuestLedger consumes matching pending token
  -> fd_write(fd=4, "1")
  -> GPIO role
  -> GP21 orange blink on/off three times
  -> fd_write(fd=5, "1")
  -> GP20 red on
  -> poll_oneoff(250 ticks)
```

The app sees only `fd`, `ptr`, `len`, and errno-like results. It does not see
GP22/GP21/GP20, route labels, lanes, ChoreoFS object ids, the resolver, SIO
FIFO, `GuestLedger`, or hibana endpoints. Baker LED fds are materialized by the
Baker project runtime after ChoreoFS `GpioDevice` object facts pass the
projected route; `GuestLedger` is app-local capability state only: fds, leases,
pending syscall tokens, quotas, and errno mapping. The
choreography is still the protocol authority. The visible LED route table is
`fd=3 -> GP22`, `fd=4 -> GP21`, and `fd=5 -> GP20`; the GPIO device role treats
the bank as active-high and keeps it one-hot when a selected LED is turned on.
The Baker page also mentions an MCU LED on GP25, but this hardware demo
intentionally leaves GP25 out until the visible free LED set is confirmed on the
board in hand.

Source map:

| Layer | File |
| --- | --- |
| Choreography | `src/projects/baker_link_led/choreography.rs::{traffic_light_roles,choreofs_traffic_light_roles,abort_safe_terminal_roles,abort_safe_linear_roles}` |
| Firmware entry | `src/projects/baker_link_led/main.rs` |
| Firmware localside | `src/projects/baker_link_led/runtime.rs` |
| WASI guest ledger | `src/kernel/guest_ledger.rs` |
| fd/object route check | `src/kernel/fd_object.rs` |
| Baker board pin/safe-state facts | `src/machine/rp2040/baker_link.rs` |
| Baker LED project object/fd/route manifest | `src/projects/baker_link_led/manifest.rs` |
| Timer top-half / raw readiness | `src/machine/rp2040/timer.rs` |
| Resolver admission | `src/kernel/resolver.rs` |
| Host parity tests | `tests/host_baker_led_fd.rs` |

## Tools

Install the host tools:

```bash
cargo install probe-rs-tools --locked
cargo install elf2uf2-rs
brew install arm-none-eabi-gdb
```

`probe-rs` talks to the Baker Link CMSIS-DAP probe. `arm-none-eabi-gdb` is only
needed when using `probe-rs gdb`; without it, flashing can still work, but GDB
attach cannot.

Check tool availability:

```bash
probe-rs --version
arm-none-eabi-gdb --version
elf2uf2-rs --version
```

## Build

Build the WASI P1 guest artifacts first, then build the RP2040 firmware:

```bash
bash ./scripts/check_wasip1_guest_builds.sh
cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts"
```

The script rebuilds the Baker min LED guests with `--initial-memory=65536` and
`-zstack-size=4096`, so the embedded `CoreWasip1Instance` is executing a real
`wasm32-wasip1` artifact whose initial memory fits the RP2040 profile.

The ELF is:

```text
target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo
```

## Flash With probe-rs

The probe-rs path is preferred for debugging because it also gives RAM reads and
GDB attach:

```bash
probe-rs download \
  --chip RP2040 \
  --non-interactive \
  --verify \
  --disable-progressbars \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo

probe-rs reset --chip RP2040 --non-interactive
```

With `--verify`, flashing can take around 50 seconds and may print very little.
Wait for `Finished ...` before assuming it is stuck.

## UF2 Alternative

UF2 is useful when using BOOTSEL mass storage instead of SWD:

```bash
elf2uf2-rs \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo.uf2
```

Copy the UF2 to the BOOTSEL volume. Use `probe-rs` when you need status reads or
GDB.

## Read The Result From RAM

The demo exports two status symbols:

```text
HIBANA_DEMO_FAILURE_STAGE
HIBANA_DEMO_RESULT
```

In the current Baker smoke the stage/result marker is read from `0x20030ae0`,
but do not hard-code this forever. Confirm with the Rust-bundled `llvm-nm` if
needed:

```bash
SYSROOT="$(rustc --print sysroot)"
HOST="$(rustc -Vv | sed -n 's/^host: //p')"
"$SYSROOT/lib/rustlib/$HOST/bin/llvm-nm" -n \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo \
  | rg 'HIBANA_DEMO_(FAILURE_STAGE|RESULT)'
```

Read the status words:

```bash
probe-rs read --chip RP2040 --non-interactive b32 0x20030ae0 8
```

Expected success shape:

```text
00000000 48494f4b
```

`0x48494f4b` means success. If you read a wider range, adjacent words include
runtime readiness fields and may be non-zero after a clean run.

Useful values:

| Value | Meaning |
| --- | --- |
| `0x48494f4b` | success |
| `0x48494641` | typed-reject |
| `0x48490001` | Core0 entered `core0_main` |
| `0x4849000b` | first traffic-light `fd_write("1")` completed, GP22 green path reached |
| `0x4849000d` | final traffic-light `fd_write` completed for the current app activation |
| `0x48490025` | Kernel received explicit `proc_exit(0)` from Engine |
| `0x48490026` | Reserved; loop break is now Engine-owned control, followed by `proc_exit` |
| `0x48490020` | Kernel entered `kernel_fd_write` |
| `0x48490021` | Kernel received `MemBorrow` |
| `0x48490414` | `RecvError::PhaseInvariant` diagnostic |

## GDB Through probe-rs

Use this non-reset attach when the board is already running and you want to see
where it stopped:

```bash
probe-rs gdb \
  --chip RP2040 \
  --non-interactive \
  --gdb arm-none-eabi-gdb \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo \
  -- \
  --batch \
  -ex "symbol-file target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo" \
  -ex "info registers pc sp xpsr" \
  -ex "bt" \
  -ex "x/8wx 0x20030ae0" \
  -ex "detach" \
  -ex "quit"
```

Avoid `--reset-halt` when preserving failure evidence matters. It resets the
chip and can erase the timing/state you were trying to inspect.

For an interactive session:

```bash
probe-rs gdb \
  --chip RP2040 \
  --non-interactive \
  --gdb arm-none-eabi-gdb \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo \
  -- \
  -ex "symbol-file target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo"
```

Then inside GDB:

```gdb
info registers pc sp xpsr
bt
x/8wx 0x20030ae0
break hibana_pico_baker_led_demo::hard_stop
continue
detach
quit
```

## UART Expectations

Do not rely on UART logs appearing on the USB debug connection. The canonical
debug channel for this smoke is:

```text
probe-rs read status words
probe-rs gdb backtrace
visible GP22/GP21/GP20 traffic-light sequence
```

UART exists in the tree as a choreographed local device proof and as
fail-diagnostic output, but the Baker LED success criterion is not "UART text
appeared in the terminal." It is `HIBANA_DEMO_RESULT == 0x48494f4b` plus the
visible LED route completing.

## Handoff Checklist

Run these before changing the Baker firmware:

```bash
cargo fmt -- --check
cargo test --test host_baker_led_fd
cargo test --test host_measurement_gates
cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts"
```

Then flash and check:

```bash
probe-rs download --chip RP2040 --non-interactive --verify --disable-progressbars \
  target/thumbv6m-none-eabi/release/hibana-pico-baker-led-demo
probe-rs reset --chip RP2040 --non-interactive
sleep 1
probe-rs read --chip RP2040 --non-interactive b32 0x20030ae0 8
```

## Pitfalls From The First Bring-Up

### Missing `arm-none-eabi-gdb`

Symptom:

```text
probe-rs gdb ... --gdb arm-none-eabi-gdb ...
```

cannot spawn GDB.

Fix:

```bash
brew install arm-none-eabi-gdb
```

This only fixes debug attach. It does not by itself fix firmware behavior.

### `.bss` Was Not Zeroed

Symptom:

```text
HIBANA_DEMO_FAILURE_STAGE retains old values
RP2040_LOCAL_STATE / requeue slots / wakers contain stale frames
recv(MemBorrow) can report PhaseInvariant or wait on impossible state
```

Root cause:

The no-std reset path jumped into Rust without clearing `.bss` or copying
`.data`.

Fix:

`linker/rp2040/pico_demo.ld` now exports `__data_*` and `__bss_*`, and
`src/projects/baker_link_led/main.rs::init_ram()` initializes RAM on Core0
before launching Core1.

### SIO FIFO Boot Residue

Symptom:

The first hibana receive on Core0 can see a bogus SIO word from the Core1 launch
handshake instead of an Engine frame. This can surface as `RecvError::PhaseInvariant`.

Root cause:

RP2040 SIO FIFO is used both for multicore boot handoff and for the local
hibana transport. The boot protocol is not a hibana frame stream.

Fix:

Drain the boot FIFO before starting the hibana SIO transport on both cores.

### PhaseInvariant Does Not Always Mean Choreography Is Wrong

`RecvError::PhaseInvariant` is a real hibana protocol error, but on a physical
board it can also expose lower-level bring-up mistakes:

```text
stale RAM
boot FIFO residue
wrong endpoint attached to a role
wrong projected program
transport frame label hint not matching the actual payload
```

Check the stage word and GDB backtrace before changing choreography.

### `probe-rs read` Can Halt Progress Briefly

Reading memory through the probe is a debugger operation. It is fine for this
smoke, but do not treat probe timing as representative of normal interrupt
latency or Core0 blocked time. During the first bring-up, repeated
`probe-rs read` calls also left the target halted often enough to make the
timer demo appear stuck. For timing-sensitive checks, either:

```text
reset/run without reads, then attach GDB once at the end
```

or attach GDB, use `continue`, interrupt once, and then inspect the marker
words.

### SIO FIFO Wakeups Must Be Bidirectional

Symptom:

```text
Core0 waits in Rp2040SioBackend::enqueue
Core1 waits in Rp2040SioBackend::dequeue
GDB attach or single-step makes progress resume
```

Root cause:

The SIO transport originally emitted `sev` after FIFO writes only. If the writer
slept waiting for `FIFO_RDY`, a receiver read could create space without waking
the writer. GDB accidentally supplied an external event, so the bug looked like
"GDB fixes it."

Fix:

`src/substrate/host_queue.rs::rp2040_fifo_try_read()` now emits `sev` after a
successful FIFO read. FIFO writes still emit `sev` for the receiver.

### Timer Ticks And Traffic-Light Timing

Symptom:

```text
long visible delays behave differently from the Pico SDK tutorial
TIMER0_IRQ_COUNT increases, but only after a long wait
```

Root cause:

The Baker demo does not use the Pico SDK HAL timer wrapper. It drives TIMER0
directly and treats the alarm tick as the machine-owned readiness source. That
keeps the interrupt path visible.

Fix:

```text
poll_oneoff -> TIMER0 IRQ top-half -> resolver TimerSleepDone -> PollReady
```

The current pattern mirrors the Baker traffic-light tutorial route order, but
uses bring-up-friendly TIMER ticks for visual confirmation: GP22 green,
GP21 orange blink phases, and GP20 red. These are LED proof manifest constants,
not Baker Link board authority. If the board clock setup changes, keep the
choreography and resolver path fixed and adjust only
`src/projects/baker_link_led/manifest.rs`.

## Final Known-Good Result

The first successful hardware run reached:

```text
HIBANA_DEMO_RESULT = 0x48494f4b
```

That run validated:

```text
WASI P1 import trap fd_write(fd=3, "1")
  -> memory borrow/grant
  -> Kernel fd resolver
  -> GPIO role
  -> GP22 green on
  -> WASI P1 import trap poll_oneoff
  -> TIMER0 top-half raw readiness
  -> resolver TimerSleepDone
  -> next fd_write / poll_oneoff phase
  -> GP22 green off
  -> fd_write(fd=4, "1")
  -> GP21 orange blink on
  -> fd_write(fd=4, "0")
  -> GP21 orange blink off
  -> fd_write(fd=5, "1")
  -> GP20 red on
  -> success
```

No bridge object, relay path, or WASI P2 surface is involved.

## Bad-Order WASI P1 Proof

The positive demo embeds the Rust-built `wasip1-led-blink.wasm` artifact. The
negative demo embeds `wasip1-led-bad-order.wasm`, which deliberately calls
`poll_oneoff` before the first `fd_write`.

That is outside the Baker traffic-light choreography:

```text
Kernel -> Engine: MsgRun
Engine -> Kernel: MsgMemBorrowRead
Kernel -> Engine: MsgMemGrant
Engine -> Kernel: MsgFdWrite
Kernel -> GPIO: MsgGpioSet
...
Engine -> Kernel: MsgPollOneoff
```

So the bad app tries this illegal order:

```text
Kernel -> Engine: MsgRun
Engine -> Kernel: MsgPollOneoff   # rejected: projected localside is not here
```

Host proof:

```bash
cargo test --test host_baker_led_fd \
  baker_link_bad_order_wasip1_poll_oneoff_is_rejected_before_fd_write_phase
```

Firmware build for the typed-reject hardware variant:

```bash
CARGO_TARGET_DIR=$PWD/target/wasip1-apps \
RUSTFLAGS="-C link-arg=--initial-memory=65536 -C link-arg=-zstack-size=4096" \
  cargo build --manifest-path apps/wasip1/wasip1-smoke-apps/Cargo.toml \
  --target wasm32-wasip1 --release --bin wasip1-led-bad-order

cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-bad-order-demo"
```

The expected outcome is not LED success. It is a typed-reject stop before any
hidden timer progress is created. On hardware the expected RAM markers are:

```text
HIBANA_DEMO_RESULT        = 0x4849524a
HIBANA_DEMO_FAILURE_STAGE = 0x48490043
```

`0x4849524a` means the bad app was rejected by the projected localside at the
expected point; it is not a generic panic marker.

## Resolver-Reject WASI P1 Proofs

Two additional negative hardware patterns keep the choreography order legal and
fail only at the fd/resource resolver.

`wasip1-led-invalid-fd.wasm` issues the expected first `fd_write` phase, but
uses `fd=6`. That fd is not in the Baker LED route arm:

```text
Engine -> Kernel: MsgMemBorrowRead
Kernel -> Engine: MsgMemGrant
Engine -> Kernel: MsgFdWrite { fd=6, "1" }
Kernel resolver rejects before Kernel -> GPIO
```

`wasip1-led-bad-payload.wasm` uses valid `fd=3`, but writes `"2"` instead of
the allowed `"0"` / `"1"` control byte:

```text
Engine -> Kernel: MsgMemBorrowRead
Kernel -> Engine: MsgMemGrant
Engine -> Kernel: MsgFdWrite { fd=3, "2" }
Kernel resolver rejects before Kernel -> GPIO
```

Host proofs:

```bash
cargo test --test host_baker_led_fd \
  baker_link_invalid_fd_wasip1_app_is_rejected_by_fd_object_without_gpio_progress

cargo test --test host_baker_led_fd \
  baker_link_bad_payload_wasip1_app_is_rejected_by_fd_object_without_gpio_progress
```

Firmware builds:

```bash
cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-invalid-fd-demo"

cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-bad-payload-demo"
```

Expected RAM markers:

| Pattern | `HIBANA_DEMO_RESULT` | `HIBANA_DEMO_FAILURE_STAGE` |
| --- | --- | --- |
| bad order | `0x4849524a` | `0x48490043` |
| invalid fd | `0x4849524a` | `0x48490044` |
| bad payload | `0x4849524a` | `0x48490045` |

These failures prove a different boundary from bad-order. Bad-order is rejected
by the projected localside because the app asks for a syscall phase that is not
open. Invalid-fd and bad-payload are rejected by the resolver after the legal
`fd_write` phase arrives, and before any GPIO role progress is created.

## ChoreoFS Path-Open Hardware Proof

`wasip1-led-choreofs-open.wasm` is the physical Baker ChoreoFS proof. It is the
small RP2040 artifact: a `#![no_main]` `wasm32-wasip1` app exporting
`__main_void`. The app opens LED objects by path, receives materialized fds, then
drives the same GPIO/timer choreography:

```text
path_open(9, "/device/led/green")  -> ChoreoFS GpioDevice -> fd 3
path_open(9, "/device/led/orange") -> ChoreoFS GpioDevice -> fd 4
path_open(9, "/device/led/red")    -> ChoreoFS GpioDevice -> fd 5
fd_write(fd=3, "1")                -> Kernel -> GPIO -> GP22
poll_oneoff(...)                   -> TIMER0 IRQ -> resolver -> PollReady
```

The choreography is
`src/projects/baker_link_led/choreography.rs::choreofs_traffic_light_roles`: three path-open
cycles, then the same Engine-owned `LoopContinue` / `LoopBreak` route as the
traffic-light proof. The bad-path proof uses
`src/projects/baker_link_led/choreography.rs::choreofs_bad_path_roles`, a
terminal one-path-open choreography, so the reject is not hidden behind a later
traffic loop. Baker-specific ChoreoFS opens map `GpioDevice` objects to the
explicit Baker GPIO route. Generic ChoreoFS object routes do not get to pretend
to be GPIO.

The negative ChoreoFS hardware patterns are:

| Pattern | Meaning | Expected stage |
| --- | --- | --- |
| `choreofs-bad-path` | `/not/allowed` has no manifest object | `0x4849004b` |
| `choreofs-bad-payload` | `fd_write("on")` reaches the LED object but violates LED payload policy | `0x4849004c` |
| `choreofs-wrong-object` | `/device/not-gpio` materializes an fd, then `fd_write("1")` rejects because the object is not GPIO | `0x4849004d` |

The verified hardware commands are:

```bash
scripts/run_baker_link_hardware_pattern.sh choreofs
scripts/run_baker_link_hardware_pattern.sh choreofs-bad-path
scripts/run_baker_link_hardware_pattern.sh choreofs-bad-payload
scripts/run_baker_link_hardware_pattern.sh choreofs-wrong-object
scripts/run_baker_link_hardware_pattern.sh fail-safe
```

## Fail-Safe Hardware Proof

The fail-safe profile proves the soft abort path as hibana choreography, not as
a hard-stop or panic escape hatch. The full route is host-tested in
`src/projects/baker_link_led/choreography.rs::abort_safe_terminal_roles`:

```text
Abort | Normal
```

The physical Baker profile runs the selected terminal fragment from
`abort_safe_linear_roles` so the proof fits the same two-core RP2040 mapping:

```text
EngineAbort
  -> AbortBegin
  -> Fence
  -> GpioSet(GP22 inactive) -> GpioSetDone
  -> GpioSet(GP21 inactive) -> GpioSetDone
  -> GpioSet(GP20 inactive) -> GpioSetDone
  -> AbortAck
```

`Fence` is materialized by `GuestLedger::apply_abort_fence`: old fd views,
leases, pending syscall tokens, and object views become stale by generation.
The safe state uses the existing GPIO device choreography. Hard runtime panic
still bypasses endpoints and applies the same Baker board safe state by direct
MMIO.

Host proof:

```bash
cargo test --test host_baker_led_fd baker_link_abort
```

Firmware build:

```bash
cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-abort-safe-demo"
```

Hardware proof:

```bash
scripts/run_baker_link_hardware_pattern.sh fail-safe
```

Expected result is `HIBANA_DEMO_RESULT = 0x48494653`.

## Chaser And Std-Source Variants

The traffic-light choreography does not encode the LED order. The WASI app does.
To prove that split, `wasip1-led-chaser.wasm` changes only the guest syscall
sequence:

```text
fd=3 "1", wait 250
fd=4 "1", wait 50
fd=5 "1", wait 50
fd=4 "1", wait 50
fd=3 "1", wait 50
fd=4 "1", wait 50
fd=5 "1", wait 250
```

The Kernel still drives the same projected `baker_led_blink_roles()` program.
The Baker loop is a hibana route:

```text
BudgetRun;
route(
  LoopContinue + fd_write/poll_oneoff body,
  LoopBreak + proc_exit(0)
)
```

Engine sends `LoopContinue` once for each fd_write/poll step produced by the
WASI guest. Passive roles, including Kernel, GPIO, and Timer, enter the selected
arm with `offer().decode()`. When the WASI app returns, Engine sends
`LoopBreak + proc_exit(0)`. A non-looping guest is not restarted by Kernel.

Host proof:

```bash
HIBANA_WASIP1_GUEST_DIR=$PWD/target/wasip1-apps/wasm32-wasip1/release \
  cargo test --test host_baker_led_fd \
  baker_link_chaser_wasip1_app_changes_fd_order_without_choreography_changes
```

Firmware build:

```bash
cargo build --target thumbv6m-none-eabi --release \
  --bin hibana-pico-baker-led-demo \
  --features "profile-rp2040-pico-min embed-wasip1-artifacts baker-chaser-demo"
```

`wasip1-led-ordinary-std-chaser.wasm` is the ordinary Rust std variant. It is a
normal `fn main()` `wasm32-wasip1` artifact using Rust std `_start`, `File`
fd writes, and `thread::sleep`; the source has no hand-written `__main_void`
trampoline. It also has a Baker hardware profile when built with the 64 KiB
initial-memory flags used by `scripts/check_wasip1_guest_builds.sh`.

```bash
cargo test --test host_baker_led_fd \
  baker_link_ordinary_std_wasip1_app_is_host_full_profile_artifact
```

## Hardware Pattern Runner

The hardware runner builds the selected WASI P1 artifact and firmware variant,
flashes the Baker Link board through `probe-rs`, resets it, then reads the RAM
markers by symbol:

```bash
scripts/run_baker_link_hardware_pattern.sh traffic
scripts/run_baker_link_hardware_pattern.sh chaser
scripts/run_baker_link_hardware_pattern.sh ordinary-std
scripts/run_baker_link_hardware_pattern.sh choreofs
scripts/run_baker_link_hardware_pattern.sh bad-order
scripts/run_baker_link_hardware_pattern.sh invalid-fd
scripts/run_baker_link_hardware_pattern.sh bad-payload
scripts/run_baker_link_hardware_pattern.sh choreofs-bad-path
scripts/run_baker_link_hardware_pattern.sh choreofs-bad-payload
scripts/run_baker_link_hardware_pattern.sh choreofs-wrong-object
```

The positive patterns expect `HIBANA_DEMO_RESULT = 0x48494f4b`. The negative
patterns expect `HIBANA_DEMO_RESULT = 0x4849524a` and the stage values listed
above.
