# UNO Q Heterogeneous Hibana-Pico Plan

## Goal

Build a hardware-backed heterogeneous sample that shows hibana-pico running one
choreography across different compute classes:

- Arduino UNO Q Linux ARM runs a choreography kernel process plus sandboxed
  WASI P1 application cells.
- Arduino UNO Q Linux runs a separate local LLM proposal process behind a
  choreography-controlled bridge.
- Arduino UNO Q STM32U585 Cortex-M33 runs the physical choreography kernel
  image for LED/actuator authority.
- Optional Challenger+ RP2350 NB-IoT extension runs core0 as choreography kernel
  and core1 as a WASI P1 image.

The sample must make every WASI P1 operation cross the projected Hibana
Endpoint/carrier boundary. No direct guest-host syscall shortcut or
shared-process bypass is allowed.

The authority source is always choreography. The LLM never authorizes device
state, network state, LED animation, or final commits.

WASI fd values are treated as general capabilities. That is intentional:
ChoreoFS can make an fd represent a local file, an LED command stream, a local
LLM request queue, or a remote Challenger link. The rule is that fds are minted
only by projected choreography and driver facts; fd numeric values never carry
ambient authority by themselves.

## Design Direction

UNO Q should be treated as a dual-brain board:

- Linux ARM side: high-level choreography kernel process, WASI P1 guest
  execution, local LLM sidecar integration, logging, and network orchestration.
- STM32U585 Cortex-M33 side: deterministic physical choreography kernel, I/O
  witness, LED face renderer, and route/fence authority.

This keeps the real-time MCU as the choreography authority and lets the Linux
side run the richer WASI P1 workload.

Linux is not a raw tool host in this sample. The LLM sidecar is a separate
proposal process, but the Linux bridge that talks to it is also a choreography
participant. Its job is to convert local model output into typed proposal facts,
not to bypass the choreography.

The local LLM runtime should be hidden behind an OpenAI-style local
sidecar boundary. First backend target:

- `llama.cpp` `llama-server` with a small GGUF model, because it is a compact
  single-process server shape and fits the edge-device story.

Secondary backend:

- Ollama, if the UNO Q image already has it or installing it is easier than
  building `llama.cpp`.

The sample must not depend on a specific model for correctness. Model output is
untrusted proposal text until the WASI P1 cell and projected choreography turn
it into accepted route evidence.

Challenger+ RP2350 NB-IoT should be treated as a field node:

- core0: choreography kernel and carrier owner.
- core1: WASI P1 image.
- NB-IoT/GNSS/Wi-Fi-positioning capability is represented as device facts first;
  live network integration is a second step after local proof passes.

## First Sample

Create `examples/uno-q-heterogeneous` as a Cargo example package with:

- `uno-q-linux-kernel`: Linux choreography kernel process with
  `Artifact = NoWasi`.
- `uno-q-wasi-llm-cell`: Linux logical image with `Artifact = WasiImage`.
- `uno-q-llm-sidecar`: Linux process boundary for the local LLM proposal
  generator.
- `ios-prompt-ingress`: Linux-side Wi-Fi prompt ingress boundary for production
  iOS input.
- `uno-q-m33-led-kernel`: STM32U585 Cortex-M33 logical image with
  `Artifact = NoWasi`.
- `challenger-net-kernel`: Challenger+ RP2350 network choreography kernel image
  with `Artifact = NoWasi`.
- `challenger-wasi-radio-cell`: Challenger+ RP2350 core1 WASI P1 image used for
  remote-node packet shaping and telemetry.
- `host-loopback-proof`: local executable that runs both logical images in one
  process for deterministic CI-style proof before flashing.
- `uno-q-hardware-proof`: UNO Q executable that runs the Linux-side split
  logical image and talks to the flashed STM32U585 logical image through a real
  Hibana `Endpoint` carrier over the board-local serial bridge.
- `wasip1/guest`: Rust WASI P1 guest crate used by `uno-q-wasi-llm-cell`.

Initial choreography:

1. iOS sends a prompt to the UNO Q Linux Wi-Fi ingress service.
2. Linux ingress service admits the prompt only as an `ios-prompt-ingress`
   choreography boundary fact.
3. Linux WASI P1 guest opens ChoreoFS object `ios/prompt/inbox`.
4. Guest reads the bounded prompt through the fd.
5. Guest opens ChoreoFS object `llm/prompt`.
6. Guest writes a normalized bounded prompt request.
7. Linux choreography kernel forwards the request to the local LLM sidecar only
   through a projected route.
8. LLM sidecar returns proposal text plus coarse emotion intent.
9. WASI P1 guest normalizes the proposal into a typed face command candidate:
   neutral, happy, sad, angry, surprised, thinking, or speaking.
10. STM32U585 choreography kernel validates the candidate and selects the final
   route.
11. If the route requires remote evidence, the guest opens a ChoreoFS network fd
   such as `net/challenger/tx` or `net/challenger/rx`.
12. Linux choreography kernel and Challenger choreography kernel exchange the
   remote fact over the selected network carrier.
13. STM32U585 renders the UNO Q face on the LED matrix/RGB LEDs.
14. During speaking, STM32U585 owns mouth-frame timing. The LLM can propose text,
   but not LED pixels.
15. Guest writes a final acknowledgement.
16. STM32U585 sends a fence/commit marker back to Linux.
17. Guest exits through WASI P1 `proc_exit(0)`.

Required WASI P1 imports for the first pass:

- `path_open`
- `fd_write`
- `fd_read`
- `fd_fdstat_get`
- `fd_close`
- `poll_oneoff`
- `proc_exit`
- `args_get` / `args_sizes_get` if the guest needs role configuration

## ChoreoFS Network FD Plan

Challenger communication is exposed to WASI as capability fds, not as raw
sockets.

ChoreoFS objects:

- `ios/prompt/inbox`: read-only iOS prompt receipt fd.
- `net/challenger/tx`: write-only packet proposal fd.
- `net/challenger/rx`: read-only packet receipt fd.
- `net/challenger/control`: write-only route/control fd for join, heartbeat,
  and link reset requests.
- `net/challenger/status`: read-only status fd for link state and last observed
  Challenger marker.

The fd lifecycle is:

1. WASI guest calls `path_open` on a specific `net/challenger/*` object.
2. Linux choreography kernel validates the object, rights, and route state.
3. The kernel returns a bounded fd with object ID and rights recorded in the
   ledger.
4. Guest uses `fd_write` or `fd_read` against that fd.
5. Each operation becomes a typed Hibana message.
6. The network carrier moves only the projected packet fact.
7. Challenger choreography kernel validates the packet and either commits,
   rejects, or asks for retry through a typed route.

Packet shape:

- magic/version
- choreography session id
- source image id
- destination image id
- lane
- monotonic packet id
- payload kind
- payload bytes
- compact witness/status byte

Network bearer:

- Preferred API shape is a transport-neutral `net/challenger/*` ChoreoFS fd.
- If the connected Challenger firmware exposes Wi-Fi association, map the fd to
  Wi-Fi UDP first.
- For the Challenger+ RP2350 NB-IoT board, the official module capability is
  embedded TCP/UDP/IP over NB-IoT/cellular plus Wi-Fi positioning scans. If Wi-Fi
  association is not exposed, keep the same fd API and map the carrier to the
  module TCP/UDP/IP path instead.

This keeps the WASI guest code unchanged while the choreography kernel chooses
the physical bearer.

## Choreography-Only Wiring Contract

The implementation must make every interesting edge visible in the choreography:

- iOS Wi-Fi ingress to Linux choreography kernel: typed prompt ingress fact.
- WASI P1 guest to fd authority: `EngineReq` / `EngineRet`.
- fd authority to Linux choreography kernel: typed prompt and face candidate
  facts.
- Linux choreography kernel to LLM sidecar: typed proposal request/reply facts.
- fd authority to Challenger choreography kernel: typed network packet and
  receipt facts.
- fd authority to STM32U585 LED renderer: typed face commit facts.

There must be no direct Rust call from guest-normalized data to the LLM sidecar,
from LLM output to LED pixels, or from a ChoreoFS network fd to a raw socket.
The host loopback proof may run all roles in one process for convenience, but it
must still move data only through the Hibana carrier.

Production iOS prompt ingress:

- iOS may connect over Wi-Fi to a Linux listener on UNO Q.
- The listener is an ingress adapter, not an authority source.
- The adapter writes into choreography as an `ios-prompt-ingress` boundary fact.
- The WASI P1 guest receives the prompt only by opening and reading the
  `ios/prompt/inbox` ChoreoFS fd.
- Backpressure, prompt truncation, and rejection are choreography-visible route
  outcomes.

Test posture:

- Local and hardware smoke tests must run without waiting for a human prompt.
- The proof harness supplies deterministic iOS prompt input through the
  `ios-prompt-ingress` role.
- Manual iOS input is a production UX path, not a required verification step.

## LED Face Plan

UNO Q face rendering is a physical authority surface, so it belongs to the
STM32U585 choreography kernel image.

Face states:

- `Neutral`: idle eyes, closed mouth.
- `Happy`: raised eyes, smile.
- `Sad`: lowered eyes, small frown.
- `Angry`: narrowed eyes, flat mouth.
- `Surprised`: round eyes, open mouth.
- `Thinking`: asymmetric eyes, small animated dot or side glance.
- `Speaking`: eye state plus mouth-open/mouth-mid/mouth-closed loop.

Rules:

- LLM proposal may include `emotion` and `utterance`.
- WASI P1 cell maps free text into a bounded face command candidate.
- Choreography selects the final face route.
- STM32U585 owns frame timing and LED writes.
- Linux cannot write raw LED pixels.
- A safe-state route must blank or neutralize the face.

## Carrier Plan

Use stages so the sample can prove value before board-specific drivers are
complete:

1. Host loopback carrier for deterministic proof.
2. Linux process-to-process carrier between `uno-q-linux-kernel`,
   `uno-q-wasi-llm-cell`, `ios-prompt-ingress`, and `uno-q-llm-sidecar`.
3. UNO Q Linux-to-M33 serial carrier implemented as a concrete
   `hibana::integration::Transport`, not as a hand-written face-command bridge.
4. `net/challenger/*` ChoreoFS fd carrier between UNO Q Linux and Challenger,
   first over Wi-Fi UDP if association is exposed, otherwise over the
   Challenger module TCP/UDP/IP bearer.

Carrier rules:

- Preserve Hibana lane metadata.
- Demultiplex by lane before payload delivery.
- Treat receive hints as observations only, not authority.
- Never allow same-board co-location to skip Endpoint/carrier traffic.
- Put operational deadline fuses in the concrete carrier. Deadline expiry is a
  session-generation fault that reaches the top-level logical image panic
  boundary; it is not a route choice and not a retry/fallback mechanism.
- Respect the physical UART receive depth with transport pacing. On UNO Q the
  host-to-M33 serial carrier paces bytes so the STM32U585 appkit image can poll
  and decode a complete projected carrier frame instead of losing bytes after
  the UART FIFO fills.

## Flash And Run Plan

1. Build WASI P1 guest for `wasm32-wasip1`.
2. Build Linux AArch64 choreography kernel process for UNO Q.
3. Build Linux AArch64 LLM sidecar adapter for UNO Q.
4. Build STM32U585 M33 image for `thumbv8m.main-none-eabi`.
5. Identify UNO Q flash path:
   - Board-local OpenOCD path for STM32U585 flash programming.
   - Serial deployment for Linux app if SSH or USB network is available.
6. Flash/deploy M33 image.
7. Copy Linux choreography process, LLM sidecar adapter, and guest artifact to
   UNO Q Linux.
8. Run the sample and collect:
   - role/image IDs
   - WASI import counters
   - carrier TX/RX counters
   - LLM proposal counters
   - selected face route
   - final commit marker

Current local discovery:

- UNO Q is visible over USB as Arduino device serial `3819862432`.
- Challenger+ RP2350 NB-IoT is visible over USB as iLabs device serial
  `8782E0373C82B466`.
- `adb` is installed on this Mac and sees the UNO Q Linux side.
- UNO Q Linux is Debian 13 on AArch64 and exposes board-local OpenOCD tooling
  under `/opt/openocd/bin/openocd`.
- Board metadata identifies the STM32U585 target as `b_u585i_iot02a` /
  `stm32u585zitxq`.
- UNO Q Linux exposes M33 serial command/ack on `/dev/ttyHS1` at 115200 baud.
- `probe-rs list` still finds no local debug probe and no UF2 mass-storage
  volume is currently mounted.

## Verification Gates

Local gates:

- `cargo check -p uno-q-heterogeneous`
- build WASI P1 guest
- host loopback proof exits successfully and reports both logical images
- no `NoWasi` image leases WASI guest storage
- LLM sidecar output is accepted only as proposal evidence
- iOS prompt input is accepted only as ingress evidence and then read by WASI
  through `ios/prompt/inbox`
- proof input is deterministic and does not require a human operator
- LED face commands are selected by choreography, not by the LLM

Hardware gates:

- M33 kernel image boots through `appkit::run::<site::Local<M33LedKernelImage>,
  UnoQCapsule>(NoWasi)` and exposes an `APPKIT_READY` transport observation.
- Linux WASI P1 app sends at least one `path_open` and two `fd_write` requests
  through Hibana.
- M33 kernel observes and validates every request through projected
  `Endpoint` receive/flow/send progress.
- UNO Q displays at least neutral, happy, sad, angry, surprised, and speaking
  face states from choreography-selected routes.
- Speaking mode shows at least three mouth frames owned by the M33 timing loop.
- WASI P1 guest opens `net/challenger/tx` and `net/challenger/rx` through
  ChoreoFS and exchanges at least one typed remote fact with Challenger.
- iOS can submit a prompt over Wi-Fi, and the prompt appears in WASI only through
  a ChoreoFS fd read.
- Final commit marker is present on Linux and M33.
- A transport operational deadline can intentionally poison a stuck session
  generation for diagnosis; normal successful proof runs must complete without
  relying on deadline expiry.
- If Challenger is enabled, RP2350 core0/core1 both expose ready markers and at
  least one cross-device Hibana frame is observed.

## Current Implementation Status

Implemented in this workspace:

- `host-loopback-proof` runs six roles in one deterministic proof carrier:
  M33 LED choreography kernel, Linux choreography kernel, WASI P1 LLM cell,
  LLM sidecar, iOS prompt ingress, and Challenger network kernel.
- The WASI P1 guest is built from `wasip1/guest` and embedded into the host
  proof with `embed-wasip1-artifacts`.
- The proof uses only ChoreoFS fds for guest I/O:
  - `ios/prompt/inbox`
  - `llm/prompt`
  - `net/challenger/tx`
  - `net/challenger/rx`
  - `face/ack`
- The iOS role is deterministic for tests, so no human prompt entry is needed.
- For production ingress, `ios-prompt-ingress` can read a raw TCP or HTTP prompt
  from iOS when `UNO_Q_IOS_PROMPT_TCP=1` is set. The default bind address is
  `0.0.0.0:7105`, overridable with `UNO_Q_IOS_PROMPT_ADDR`.
- The LLM sidecar can only emit a typed proposal. The accepted face route and
  final commit are selected by choreography.
- The Challenger link is represented as a ChoreoFS network fd capability. The
  host proof verifies the full request/receipt/read path.
- The WASI engine fd-write inline payload now uses the protocol stream chunk
  capacity, so ChoreoFS writes can carry the same bounded payload size as
  `Wasip1StreamChunk`.

Verified locally:

- WASI P1 guest build:
  `bash ./scripts/check_wasip1_guest_builds.sh`
  This script applies the core WASM profile flags
  `--initial-memory=65536 --max-memory=65536 -zstack-size=4096`; do not replace
  it with a raw `cargo build`, because that can emit a guest memory section
  outside the embedded interpreter profile.
- Example check:
  `cargo check -p uno-q-heterogeneous --features 'runtime-wasip1 embed-wasip1-artifacts' --bins`
- End-to-end host proof:
  `cargo run -p uno-q-heterogeneous --features 'runtime-wasip1 embed-wasip1-artifacts' --bin host-loopback-proof`
- iOS TCP ingress smoke proof:
  run the same proof with `UNO_Q_IOS_PROMPT_TCP=1
  UNO_Q_IOS_PROMPT_ADDR=127.0.0.1:7105` and POST
  any bounded non-empty prompt such as `face speaking from ios` to
  `http://127.0.0.1:7105/`.
- Target checks:
  - `cargo check -p uno-q-heterogeneous --target aarch64-unknown-linux-gnu --bin uno-q-linux-kernel --bin uno-q-llm-sidecar --bin uno-q-wasi-llm-cell --bin ios-prompt-ingress`
  - `cargo check -p uno-q-heterogeneous --target thumbv8m.main-none-eabi --bin uno-q-m33-led-kernel`
  - `cargo check -p uno-q-heterogeneous --target thumbv8m.main-none-eabi --bin challenger-net-kernel`
- M33/RP2350 generic release image builds:
  - `cargo build -p uno-q-heterogeneous --target thumbv8m.main-none-eabi --release --bin uno-q-m33-led-kernel`
  - `cargo build -p uno-q-heterogeneous --target thumbv8m.main-none-eabi --release --bin challenger-net-kernel`

Hardware flash status:

- UNO Q and Challenger are visible over USB serial.
- UNO Q Linux deployment via `adb push` works.
- UNO Q static AArch64 build works with `aarch64-unknown-linux-musl` and
  `rust-lld`.
- `uno-q-m33-native-kernel` is a standalone Rust `no_std` STM32U585 image with
  its own vector table, linker map, clock setup, UART setup, SysTick interrupt,
  GPIOF charlieplex LED matrix driver, and an appkit-attached M33 logical image.
- Board behavior is represented below the projected role as local board
  resolvers:
  - timer interrupt resolver for scan refresh and speaking mouth-frame timing
  - UART carrier byte resolver feeding `hibana::integration::Transport`
  - LED render resolver driven only after projected face candidate/final commit
- The M33 image no longer accepts a hand-written face-command protocol. The
  physical UART is a Hibana carrier (`HBU1` wire envelope carrying session,
  lane, source, peer, frame label, and payload bytes), and the M33 role receives
  typed facts only through `Endpoint::recv` / `Endpoint::flow`.
- Native M33 build and flash entrypoints are fixed as:
  - `examples/uno-q-heterogeneous/scripts/build_m33_native.sh`
  - `examples/uno-q-heterogeneous/scripts/flash_m33_native.sh`
- The previous STM32U585 flash contents were backed up on UNO Q at
  `/tmp/uno_q_m33_flash_before_native.bin` before programming the native image.
- The native STM32U585 ELF was programmed and verified through board-local
  OpenOCD at flash origin `0x08000000`.
- OpenOCD register inspection after reset shows the M33 running from the native
  image in flash, with PC inside the native LED/kernel loop and GPIO/UART
  peripheral registers configured by the image.
- The old board-core-generated M33 image is not part of this sample. Runtime
  authority is the native choreographic kernel image plus the Linux choreography
  process.
- Native STM32U585 transport observation is visible on Linux through
  `/dev/ttyHS1` with the board serial router service stopped. The successful
  appkit boot marker is `HIBANA_M33:APPKIT_READY`.
- Linux host appkit execution uses a wall-time millisecond clock for endpoint
  operational deadlines. The default no_std counter clock remains for embedded
  images, but host hardware proofs must not burn endpoint deadlines by busy
  polling unrelated pending roles.
- The appkit-attached native STM32U585 image is flashed and verified on UNO Q.
- `uno-q-hardware-proof` now runs a split hardware proof:
  - M33 image `715` requests only role `ROLE_M33_LED_KERNEL`.
  - Linux hardware peer image `717` requests roles `ROLE_WASI_LLM_CELL`,
    `ROLE_LINUX_KERNEL`, `ROLE_LLM_SIDECAR`, `ROLE_CHALLENGER_KERNEL`, and
    `ROLE_IOS_PROMPT_INGRESS`.
  - Both images attach to the same raw `hibana::g` choreography through
    `appkit::run`.
  - Every WASI P1 import completion crosses projected `EngineReq` / `EngineRet`
    over the real UART carrier when it needs the M33 authority role.
- `uno-q-hardware-proof` reset-runs M33, waits for the appkit ready observation,
  and then starts Linux-side choreography. No human prompt is required.
- Repeated unattended `uno-q-hardware-proof` runs pass on UNO Q with projected
  Endpoint/carrier frames over `/dev/ttyHS1`.
- A diagnostic run with host deadline set low showed the expected
  `EndpointError { operation: "recv", kind: SessionFault(DeadlineExceeded) }`,
  proving that stalled carrier waits fail at the Hibana endpoint boundary. RAM
  counters then showed `RX_BYTES=4`, `RX_FRAMES=0`, `TX_FRAMES=0`, which
  identified UART FIFO overflow before frame decode. The root fix was transport
  byte pacing, not a retry or compatibility layer. The Linux-to-M33 carrier
  defaults to 10ms per byte and can be overridden with
  `UNO_Q_HIBANA_UART_BYTE_US` for diagnostics.

Next hardware step:

- Replace the deterministic LLM sidecar with a real local `llama.cpp` or Ollama
  sidecar while keeping it a proposal-only boundary.
- Move the Challenger role from host proof carrier to the connected physical
  Challenger+ RP2350 NB-IoT node.

## Open Questions

- Which local LLM backend is practical on the connected UNO Q image:
  `llama.cpp`, Ollama, or a smaller custom proposal process?
- Does the connected Challenger firmware expose Wi-Fi association, or only
  Wi-Fi scan/positioning plus NB-IoT TCP/UDP/IP?

## Next Step

Attach the real local LLM sidecar and the physical Challenger node without
giving either of them authority over the final route. The M33 path is now an
appkit-attached Hibana carrier path, so future work should extend that shape
rather than adding board-local protocol shortcuts.
