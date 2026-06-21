# EPF Final Design Plan

## 1. Final Principle

EPF is a VM appliance.

```text
EPF に入力を流す。
VM が走る。
Out を見る。
```

Observation, debug, replay, and policy control use the same model. There is no
public EPF concept that is not VM execution.

```text
observe / debug / replay:
  TapEvent -> Epf VM -> Out

policy:
  policy::<ID>() -> Epf VM -> Out -> Decision
```

Pico class is not a small-device profile. Pico class is the contract.

```text
EPF default:
  no_std
  no_alloc
  fixed storage
  fixed policy slots
  compact Out
  no string rendering on device
  no dynamic map/helper/hook
  RAM-marker friendly
```

Larger hosts do not get a different EPF model. Host tools render, replay, and
explain the same compact records produced by the device.

```text
device:
  TapEvent -> epf.run(event) -> compact Out

host:
  TapEvent / compact Out stream -> render / explain / replay
```

## 2. Repository Authority

`hibana 0.8.0` is the authority. EPF and future QUIC work must follow the
current hibana shape, not preserve 0.6 compatibility.

```text
authority:
  hibana 0.8.0 public/integration surface
  hibana/AGENTS.md completion-form constraints

not authority:
  hibana 0.6.x
  recv_frame_hint compatibility
  hidden side-channel route hints
  protocol-specific pressure to grow app-facing hibana API
```

Do not add compatibility layers to make old `hibana-quic` compile. The correct
work is to move `hibana-quic` to hibana 0.8.0 and delete the 0.6-era concepts.

`hibana-pico` may use a local `../hibana` path while EPF needs unreleased
integration-surface additions such as `TapPort`, `TapEvent::evidence()`, and
transport mismatch evidence. That local path is an observability staging path,
not permission to change Hibana endpoint semantics. The local Hibana changes
must preserve the 0.8.0 progress model: expected-frame mismatch is observed and
discarded, not turned into a new endpoint reject/failure behavior.

`hibana` owns:

```text
choreography authority
endpoint progress authority
descriptor/session/lane/role/label authority
canonical TapEvent / Evidence ABI authority
transport integration contract
```

`hibana` does not own:

```text
EPF bytecode appliance
Pico-specific SIO transport
WASI P1 engine or guest resource table
QUIC protocol state
H3 stream manager
management image delivery protocol
```

`hibana-epf` owns:

```text
Epf VM appliance
fixed storage and image lifecycle
observe VM
policy VM
compact Out
generic bytecode envelope parser
TapEvent retention spool
compact Out binary record
replay verification format
host render/explain codec core
common image recipes
```

## 3. Public EPF Runtime Object

The only public EPF runtime object is `Epf`. Const capacity parameters are
allowed because Pico class is fixed-storage/no-alloc; they are capacities, not
profiles and not separate runtime concepts.

```rust
pub struct Epf<
    'a,
    const POLICY_SLOTS: usize,
    const IMAGE_BYTES: usize,
    const SCRATCH_BYTES: usize,
    const HISTORY: usize,
>;
```

The following are value or resource types, not runtime objects:

```text
EpfStorage
TapEvent
TapPort
Out
Load
Error
Target
ReplayRecord
ImageIngress
TapSpool
CompactOutRecord
ImageSpec
```

The following public concepts are removed or kept internal:

```text
Probe
EvidenceCell
PolicyBank
ObserverBank
EpfObserver
EpfPolicy
Slot
Action
run_with
HostSlots
helper
map
attach point
hook
```

EPF must not become a smaller eBPF. It does not expose hooks, maps, helpers,
program types, attach points, event channels, or pinning.

## 4. Storage

`EpfStorage` is fixed at construction time. There is no heap-backed expansion.

```rust
pub struct EpfStorage<
    const POLICY_SLOTS: usize,
    const IMAGE_BYTES: usize,
    const SCRATCH_BYTES: usize,
    const HISTORY: usize,
>;
```

`POLICY_SLOTS` may be zero. Observe/debug is the minimum EPF capability and must
work without policy storage.

```rust
type MinimalSioEpfStorage = EpfStorage<0, 256, 32, 4>;
```

`EpfStorage` owns typed fixed arrays. `Epf` is a borrowed appliance view over
that storage. Carrying the same capacity constants on `Epf` is acceptable for
Pico class because it does not create a second runtime object or a host/device
profile split.

```text
EpfStorage:
  owns typed fixed arrays
  carries POLICY_SLOTS / IMAGE_BYTES / SCRATCH_BYTES / HISTORY in the type

Epf:
  borrowed appliance view
  may carry the same storage capacity constants
  performs all capacity checks against fixed storage
  can also be constructed from raw storage for static RP2040 RAM ownership
```

This keeps the public runtime object as `Epf`; capacity constants do not become
EPF profiles.

## 5. Out

`Out` is a compact device record, not a formatted report.

```rust
pub struct Out {
    pub epoch: u32,
    pub kind: u16,
    pub reason: u16,
    pub arg0: u32,
    pub arg1: u32,
    pub arg2: u32,
    pub fuel_used: u16,
    pub flags: u16,
}
```

String summaries are host/debug renderers, not the device contract.

`Out` is interpreted by target. Do not add a separate public `DecisionOut`.

```text
Target::Observe:
  Out = report / diagnostic record

Target::Policy(ID):
  Out.kind / Out.reason / Out.arg0 = decision encoding
```

Policy decision encoding:

```text
Out.kind:
  0 = Defer
  1 = Choose
  2 = Reject

Out.arg0:
  chosen arm for Choose

Out.reason:
  reject reason for Reject
```

Policy conversion:

```text
Defer:
  DecisionResolution::Defer

Choose:
  DecisionResolution::Arm(decode_arm(Out.arg0))

Reject:
  ResolverError::reject(Out.reason)
```

## 6. Epf API

```rust
impl<
    const POLICY_SLOTS: usize,
    const IMAGE_BYTES: usize,
    const SCRATCH_BYTES: usize,
    const HISTORY: usize,
> Epf<'_, POLICY_SLOTS, IMAGE_BYTES, SCRATCH_BYTES, HISTORY> {
    pub fn new(
        storage: &mut EpfStorage<'_, POLICY_SLOTS, IMAGE_BYTES, SCRATCH_BYTES, HISTORY>,
    ) -> Self;

    pub unsafe fn from_raw_storage(
        storage: *mut EpfStorage<'_, POLICY_SLOTS, IMAGE_BYTES, SCRATCH_BYTES, HISTORY>,
    ) -> Self;

    pub fn load(&self, image: &[u8]) -> Result<Load, Error>;

    pub fn unload(&self, target: Target) -> Result<(), Error>;

    pub fn revert(&self, target: Target) -> Result<(), Error>;

    // Direct observe/debug/replay path only.
    pub fn run(&self, event: TapEvent) -> Out;

    pub fn resolver<const POLICY_ID: u16>(
        &self,
        fallback: ResolverRef<'_, POLICY_ID>,
    ) -> ResolverRef<'_, POLICY_ID>;
}
```

Meaning:

```text
load:
  install a VM image

unload:
  remove the VM image for a target

revert:
  restore the previous VM image for a target

run:
  feed one TapEvent into the observe/debug/replay path and return Out

resolver:
  return a typed Hibana ResolverRef<ID> wrapper for policy::<ID>()
  run Target::Policy(ID) VM when loaded
  call fallback only while Target::Policy(ID) is unloaded
  fail closed on policy VM trap or invalid policy Out
```

Do not publish a no-op `resolver()` that merely returns fallback. The resolver
API is valid only because the returned resolver actually runs
`Target::Policy(ID)` VM when loaded, calls fallback only when unloaded, and
fails closed on trap.

`run(event)` cannot directly execute a `Target::Policy(ID)` VM.

## 7. Target

```rust
pub enum Target {
    Observe,
    Policy(u16),
}
```

`Target` is an image header attribute and an `unload` / `revert` value. It is not
a runtime object.

```text
Target::Observe:
  observation / debug / explanation VM

Target::Policy(ID):
  policy VM that runs when policy::<ID>() is reached
```

`epf.load(image)` reads the `Target` from the image header and installs the image
in the corresponding fixed physical slot.

## 8. Observe VM

`Epf::new` starts with a built-in observe VM.

```text
default observe VM:
  endpoint send/recv timeline
  route decision timeline
  transport mismatch/fault/frame explanation
  lane lifecycle check
  session/lane/role/label correlation
```

Load a `Target::Observe` image only when the observation logic should be
replaced.

```rust
epf.load(&observe_image)?;
```

`unload(Target::Observe)` returns to the built-in observe VM. It does not disable
observation.

## 9. Debug Paths

There are two debug paths. Both use the same compact `Out`; they differ only in
where the VM runs.

```text
VM runs wherever Epf::run or the resolver wrapper is called.
```

Host-run debug:

```text
device streams TapEvent
host runs epf.run(event)
host renders Out
```

Device-run debug:

```text
device runs epf.run(event)
device writes compact Out to RAM markers or compact uplink
host reads markers or compact records
```

`TapPort` is created from the Hibana rendezvous witness. It is read-only, drains
the actual Hibana tap ring, and cannot write tap events, choose routes, recover
sessions, or mutate endpoint state.

RP2040/SIO proof target is device-run debug.

```text
TapEvent -> epf.run(event) -> compact Out -> RAM markers
```

Device-run debug:

```rust
let mut tap = rv.tap();
let epf = Epf::new(&mut epf_storage);

let mut drained = 0;
while drained < 8 {
    drained += 1;
    let Some(event) = tap.next() else { break };
    let out = epf.run(event);

    if out.emitted() {
        ram_markers.epf_epoch += 1;
        ram_markers.epf_kind = out.kind();
        ram_markers.epf_reason = out.reason();
        ram_markers.epf_arg0 = out.arg0();
        ram_markers.epf_arg1 = out.arg1();
        ram_markers.epf_arg2 = out.arg2();
        ram_markers.epf_fuel_used = out.fuel_used() as u32;
        break;
    }
}
```

In appkit firmware, the native scheduler may expose this as a capsule observe
tick:

```rust
fn observe(tap: &mut hibana::runtime::tap::TapPort<'_>) {
    let mut drained = 0;
    while drained < 8 {
        drained += 1;
        let Some(event) = tap.next() else { break };
        let out = epf.run(event);
        if out.emitted() {
            write_compact_out(out);
            break;
        }
    }
}
```

This is not an EPF hook, not a transport hook, and not a remote decision path.
It is only a native runtime service point that drains the read-only Hibana tap
ring after endpoint polls.

The drain is bounded. It may skip quiet VM outputs from unrelated TapEvents in
the same scheduler tick, but it must not loop until the ring is empty.

Host-run tools:

```bash
hibana-epf live --serial /dev/ttyACM0
hibana-epf replay trace.hbt
hibana-epf explain trace.hbt --session 0x5195d24c
```

No policy image is required for debugging. The observe VM always runs. On
device, `Out` is written to RAM markers, compact uplink, or resolver result.
String rendering is a host responsibility.

## 10. RP2040/SIO and WASI P1 Placement

Baker's core1 application role may be a WASI P1 guest, but EPF does not run
inside that guest.

```text
RP2040 core1:
  native firmware / runtime / SIO transport / WASI P1 engine
    -> WASI P1 guest logical image
```

The WASI P1 guest is a logical choreography image and a diagnosis subject. It is
not the owner of SIO, tap, RAM markers, or EPF.

The minimum practical proof must co-reside with a real core1 WASI P1 guest, not
with `NoWasi`.

```text
session-mismatch proof:
  EngineArtifact = WasiImage
  core1 native localside drives the WASI P1 guest until it stops making bounded progress
  guest is a normal Rust std WASI P1 binary using hibana-wasip1-guest::choreofs
  guest attempts choreofs::open_write(...).write_once_exact(...)
  guest first emits path_open on choreography lane 1
  core0 receives path_open with the unmodified projected session
  core0 sends path_open_ret over SIO with deliberate Tx-header-only session skew
  core1 hibana runtime emits TransportMismatch/session_mismatch TapEvent
  core1 discards the mismatched frame without committing endpoint progress
  core1 does not deliver the payload to the WASI guest and does not create a new endpoint reject
  the guest remains pending before fd_write because the expected path_open_ret never arrives
  SIO operational deadline may return the blocking native wait to the scheduler
  deadline is used as a scheduler-yield boundary here, not as the diagnosis itself
  observe tick copies drained TapEvents into a RAM spool up to spool capacity
  probe/UART writes observe bytecode into the reserved EPF RAM image area while the system is stuck
  native observe tick runs loaded EPF observe bytecode over retained TapEvents
  session-mismatch diagnosis is expected on core1 because the mismatched reply is consumed there
  core0 has its own EPF storage/spool and may also drain local TapEvents without sharing EPF mutable state
  EPF Out is copied to per-core RAM markers
```

The guest must be minimal enough for RP2040 RAM:

```text
guest:
  Rust std is allowed and required for the proof
  no allocator-heavy behavior
  uses hibana-wasip1-guest::choreofs as the normal app-facing API
  imported WASI calls are supported by explicit baker-firmware feature gates
  first useful ChoreoFS operation is path_open
  fd_write is the guest's intended next operation, but the ignored mismatched reply prevents reaching it
```

```text
bad:
  WASI P1 guest runs EPF
  guest touches SIO / tap ring / RAM markers
  EPF runs while FIFO words are pushed or popped

good:
  core1 native host/runtime owns SIO, tap, and RAM markers
  core1 native host/runtime runs the WASI P1 guest
  core1 native host/runtime calls epf.run(event)
  core1 native host/runtime writes compact Out to RAM markers
```

The proof must run through a runtime path that calls the capsule observe tick
while the role task is pending. A bare-metal canonical WASI loop that bypasses
the role scheduler is not sufficient for this EPF proof because no `TapPort`
drain occurs and the retained TapEvent spool remains empty. This is not a
separate profile: Pico-class runtime must make TapPort observation available on
the ordinary stuck/pending path. A carrier-local deadline may be used to return
from a blocking native wait to that scheduler path, but the mismatch TapEvent is
the proof input. Panic parking may also consume the retained spool for other
failures, but session mismatch itself is not a panic proof.

The SIO FIFO word path must stay a carrier path.

```text
SIO FIFO word path:
  push/pop words
  assemble/stage Hibana frames
  demux lanes
  expose observed frame metadata

hibana runtime:
  compare observed metadata with expected context
  emit canonical TapEvent / Evidence

EPF:
  run only after a TapEvent exists
  return compact Out
```

EPF must not run in:

```text
SIO FIFO push/pop loop
transport poll critical section
interrupt handler
WASI P1 guest
```

The first RP2040/SIO implementation is observe-only, but it is not a hard-coded
marker writer. It must include a bytecode ingress path and must prove that the
loaded observe VM produced the result.

```text
initial storage:
  EpfStorage<0, IMAGE_BYTES, SCRATCH_BYTES, HISTORY>

initial capability:
  write EPF bytecode image into reserved device memory
  runtime verifies and loads the image
  TapEvent -> epf.run(event) -> loaded observe VM -> compact Out
  compact Out -> RAM marker or UART compact record

not required initially:
  policy slots
  hot-swap choreography
```

Policy hot-swap is a second-stage capability. Observe bytecode loading is not
second-stage; it is the first meaningful proof.

## 11. Pico Diagnostic Bytecode Ingress

Pico-class diagnosis must support this workflow:

```text
device is running and misbehaving
host/probe/user writes an EPF observe bytecode image into a reserved bytecode area
runtime detects a commit word
runtime verifies and loads the image
future TapEvents run through the loaded observe VM
VM output is written to RAM markers or compact UART records
```

The byte stream format is owned by `hibana-epf::ImageIngress<N>`, not by a
board. Debug probe UART, mailbox RAM, USB, files, or future management
choreography may all feed the same envelope parser.

```text
b"HEPF" | image_len:u16-le | image_hash:u32-le | image_bytes
```

The reserved bytecode area is device-owned RAM. It may be written by a debug
probe, UART loader, or future management choreography, but the runtime consumes
the same committed image semantics.

```text
EPF bytecode area:
  magic
  sequence
  state
  target
  image_len
  image_hash
  image_bytes[IMAGE_BYTES]
  result_epoch
  result_digest
  result_reason
```

State machine:

```text
Empty:
  no pending image

Writing:
  host/probe/UART is filling image_bytes

Commit:
  image_len and image_hash are final
  runtime may verify and load

Loaded:
  epf.load(image) succeeded
  result_digest and epoch are visible

Rejected:
  verification or load failed
  result_reason is visible
```

The commit word is written last. The runtime must never load a partially written
image.

```text
write order:
  header without Commit
  image_bytes
  image_len
  image_hash
  memory fence if available
  state = Commit
```

Runtime polling:

```text
allowed:
  native runtime idle/debug tick
  after endpoint poll returns
  explicit diagnostic service point

forbidden:
  SIO FIFO push/pop loop
  interrupt handler
  inside transport poll critical section
  WASI P1 guest
```

Output sinks:

```text
RAM markers:
  always supported on Baker/Pico proof builds
  used by probe scripts

UART compact records:
  optional first-stage output
  binary compact Out records, not formatted strings
  generic record format is hibana_epf::CompactOutRecord
```

UART is a sink, not a decision path. It may carry compact records and may carry
image ingress bytes, but it must not ask a remote actor for a route decision.

Direct marker synthesis is forbidden as an EPF proof.

```text
bad:
  detect mismatch
  hand-write EPF_KIND / EPF_REASON markers
  call it EPF

good:
  detect mismatch
  emit TapEvent / Evidence
  loaded observe VM runs
  VM Out is copied to RAM markers / UART record
```

Minimum RP2040/SIO proof:

```text
core0 -> SIO -> core1
host/probe writes observe bytecode image into EPF bytecode area
runtime loads the image and records image digest/epoch
core1 std WASI P1 guest emits ChoreoFS path_open on lane 1
core0 returns path_open_ret with deliberate Tx-header-only session mismatch
hibana runtime emits TransportMismatch Evidence
hibana runtime discards that frame and keeps endpoint progress unchanged
core1 native runtime calls epf.run(TapEvent)
loaded observe VM produces Out
core1 RAM markers contain:
  EPF_IMAGE_DIGEST = loaded observe image digest
  EPF_LOAD_EPOCH   = non-zero
  EPF_KIND        = compact report kind for TransportMismatch
  EPF_REASON      = SessionMismatch or LaneMismatch
  EPF_ARG0        = expected_session
  EPF_ARG1        = observed_session
  EPF_ARG2        = packed lane/source/peer/label
  EPF_FUEL_USED   = non-zero
OpenOCD/probe script reads the markers after the run
```

The loaded observe bytecode for this proof must filter the typed Evidence input,
not inspect raw transport state.

```text
required typed input filter:
  input[3] == (TRANSPORT_MISMATCH << 16)
           | (expected_lane << 8)
           | TRANSPORT_MISMATCH_SESSION

for WASI path_open_ret session-mismatch:
  expected_lane = 0
  input[3] = 0x02050001
```

The Baker proof must pass with a std WASI P1 ChoreoFS guest and pending-state
EPF bytecode loading:

```text
features:
  wasm-engine-core
  embed-wasip1-artifacts

observed markers:
  EPF_LOAD_EPOCH   = 1
  EPF_ACTIVE_TARGET = Observe
  CORE1_EPF_KIND    = 1
  CORE1_EPF_REASON  = 1
  CORE1_EPF_ARG0    = expected_session
  CORE1_EPF_ARG1    = observed_session = expected_session ^ 0x11110000
  CORE1_EPF_ARG2    = observed_lane/source_role/peer_role/transport_frame_label metadata
  CORE1_EPF_FUEL_USED = 7
  CORE1_EPF_TAP_SPOOL_LEN  >= the TransportMismatch event position
  CORE1_EPF_TAP_SPOOL_READ >= CORE1_EPF_TAP_SPOOL_LEN when loaded-bytecode replay drains retained events
```

This is the core1 WASI ChoreoFS `path_open_ret` reply mismatch, not a synthetic
marker and not a direct no_std syscall stub. The EPF bytecode reads the actual
Hibana TapEvent through TapPort after the application is already stuck or no
longer making bounded progress. Session mismatch does not require a panic
marker and must not be implemented by forcing an endpoint recv failure.
Deadline remains a separate transport proof, not the session-mismatch proof.
The SIO skew must not mutate the receiver's expected session. It mutates only
the selected sender's frame header so the first request can complete normally
and the reply can pass through Hibana's ordinary `FrameHeader` mismatch
observation path.
The EPF proof must not depend on the application making progress after the
mismatch. If TapEvent already exists, the device must retain it in RAM and must
be able to run loaded EPF bytecode over that retained event stream while the
role task is pending. If an unrelated panic later occurs, the panic park loop
may also replay the retained spool, but that is not the session-mismatch gate.

`transport_frame_label` is Hibana's compact demux label, not the application
message label constant such as `LABEL_WASI_PATH_OPEN_RET`. The proof must not
interpret that byte as a WASI call label. The proof must not use ChoreoFS
operation counters or object markers as diagnostic evidence; those are app/demo
semantics, not Hibana transport evidence.

Capacity proof:

```text
force SIO demux/ring/queue capacity pressure
hibana runtime emits TransportFault Evidence
loaded observe bytecode filters typed Evidence input, not SIO internals
EPF Out contains:
  kind   = TransportFault
  reason = Capacity
  arg0   = capacity site
  arg1   = lane
  arg2   = (TRANSPORT_FAULT << 16)
         | (lane << 8)
         | TRANSPORT_FAULT_CAPACITY
  fuel   = non-zero
```

Deadline / panic-marker proof:

```text
SIO carrier-local deadline is enabled by the capsule facts
recv/send wait reaches the carrier deadline
SIO returns TransportError::Deadline
Hibana surfaces EndpointError { operation, file, line, column, kind: Transport(Deadline) }
Baker panic handler writes RAM panic markers:
  panic file hash
  panic line/column
  panic message hash
  panic message bytes
```

Deadline diagnostics are allowed to be panic-marker-first. They already explain
the terminal wait cause well enough for many Pico-class failures. EPF is still
needed when the cause is structural and not just temporal: session mismatch,
lane mismatch, capacity pressure, route/progress mismatch, or cases where the
operator loads bytecode after the system is already stuck and wants the TapEvent
stream explained.

If this loaded-bytecode observe proof cannot be made reliable on RP2040/SIO, EPF
is not practical enough.

## 12. Policy Path Contract

A fallback resolver is always registered.

```rust
let fallback7 = ResolverRef::<7>::decision_state(&state, choose);

rv.role(&role0)
    .set_resolver::<7>(epf.resolver::<7>(fallback7))?;
```

Semantics:

```text
policy::<7>() reached

if Target::Policy(7) image is loaded:
  run EPF VM
  ignore fallback resolver

else:
  call fallback resolver
```

If a loaded policy VM traps:

```text
fail closed
do not silently fall back
```

Restore fallback explicitly:

```rust
epf.unload(Target::Policy(7))?;
```

Restore the previous policy VM explicitly:

```rust
epf.revert(Target::Policy(7))?;
```

`resolver::<ID>()` is type checked. `unload(Target::Policy(ID))` and
`revert(Target::Policy(ID))` use a value target intentionally, so image lifecycle
operations share one small API.

This API must never regress into a fallback-preserving no-op resolver. That is
worse than no resolver because it makes a loaded policy image appear active when
it is not.

## 13. ResolverRef

```rust
pub struct ResolverRef<'cfg, const POLICY_ID: u16>;
```

```rust
fn set_resolver<const POLICY_ID: u16>(
    self,
    resolver: ResolverRef<'cfg, POLICY_ID>,
) -> Result<Self, ResolverError>;
```

This removes policy point mismatch at the type level.

```rust
// compile error
set_resolver::<8>(epf.resolver::<7>(fallback7));
```

hibana core does not know EPF. `hibana-epf` only returns a `ResolverRef<ID>`.

Do not add:

```text
EpfResolver
ResolverRef::epf
EPF hook
EPF slot
remote decision API
```

## 14. Fixed Storage and Hot-Swap Semantics

`Epf` internally keeps fixed physical storage. This is the default contract, not
a Pico-only profile.

```text
internal storage:
  observe bank:
    bank A: image + scratch
    bank B: image + scratch
    active: built-in or A or B
    previous: optional loaded image
    epoch: u32

  policy slots[POLICY_SLOTS]:
    policy_id: Option<u16>
    bank A: image + scratch
    bank B: image + scratch
    active: A or B
    previous: optional loaded image
    epoch: u32

  history[HISTORY]:
    compact evidence summary records

  busy: Cell<bool>
```

`load(Target::Policy(ID) image)`:

```text
if ID is already assigned to a policy slot:
  use that slot
else if a free policy slot exists:
  assign ID to that slot
else:
  Err(Capacity)

verify image
copy into inactive bank for that slot
initialize inactive scratch
flip active policy image
policy_epoch += 1
```

Concurrent rule:

```text
load/revert/unload during run/resolver:
  Err(Busy)
  active unchanged

load failure:
  active unchanged

run:
  max 1 TapEvent
  max 1 observe VM run

device observe tick:
  bounded drain of a small fixed number of TapEvents
  calls run(event) once per drained TapEvent
  stops after first emitted Out

resolver:
  max 1 policy VM run or max 1 fallback resolver call
```

VM trap:

```text
Target::Observe:
  Out contains trap report

Target::Policy(ID):
  fail closed
  no silent fallback
```

## 15. Internal Evidence State

`Epf` keeps correlation and evidence state internally. This state is not public
API.

```text
internal state:
  latest TapEvent-derived Evidence
  session/lane/role/label correlation
  route timeline
  transport mismatch/fault/frame history
  observe_epoch
```

`run(event)` updates canonical evidence state before running the observe VM:

```text
run(event):
  decode TapEvent -> Evidence
  update canonical internal evidence state
  observe_epoch += 1
  run active Observe VM against event + evidence snapshot
  return Out
```

Observe VM trap does not roll back canonical evidence state.

```text
Observe VM trap:
  Out contains trap report
  canonical evidence state remains updated
```

Policy VM snapshots state at resolver entry.

```text
resolver entry:
  policy_epoch snapshot
  observe_epoch snapshot
  compact evidence summary snapshot
```

If `run(event)` advances while a resolver call is executing, that resolver call
still uses only its captured snapshot.

```text
same policy_epoch
same observe_epoch
same policy input
=> same Out
```

Policy VM may read the snapshot, but it must not mutate the observation state.

## 16. Hibana Frame Contract

The receive path is frame-first. A received Hibana frame is the unit of progress,
rollback, route evidence, mismatch evidence, and EPF observation.

```rust
pub struct FrameHeader(/* compact observed semantic frame header */);

impl FrameHeader {
    pub fn session(self) -> SessionId;
    pub fn lane(self) -> Lane;
    pub fn source_role(self) -> u8;
    pub fn peer_role(self) -> u8;
    pub fn label(self) -> FrameLabel;
}

fn poll_recv<'a>(
    &'a self,
    rx: &'a mut Self::Rx<'a>,
    cx: &mut Context<'_>,
) -> Poll<Result<ReceivedFrame<'a>, TransportError>>;
```

`ReceivedFrame` is the receive boundary. Payload bytes and the optional
carrier-observed `FrameHeader` cross the transport boundary as one value returned
by `poll_recv`. Do not split the same receive unit across `poll_recv` plus a
separate observation hook. Do not add an `Incoming` wrapper, a compatibility
receive type, or a post-poll peek API.

Important definition:

```text
Hibana frame = route/progress-visible receive unit
not necessarily a physical carrier packet
not necessarily a QUIC wire frame
not necessarily non-empty payload
```

Therefore a control-only receive unit is still a Hibana frame if it affects
route/progress.

## 17. ReceivedFrame Boundary

```rust
pub struct ReceivedFrame<'f>(/* private */);

impl<'f> ReceivedFrame<'f> {
    pub const fn deterministic(payload: Payload<'f>) -> Self;
    pub const fn framed(header: FrameHeader, payload: Payload<'f>) -> Self;
    pub fn payload(&self) -> Payload<'f>;
}
```

`ReceivedFrame::framed(header, payload)` binds observed frame metadata to the
exact payload view returned by the same `poll_recv` call.

```text
ReceivedFrame:
  returns carrier-observed header and payload as the same receive value
  does not expose metadata through a second transport method
  does not let the runtime read a later or earlier frame header
  does not commit protocol progress by construction
  is not route authority
```

Route evidence is `FrameHeader::label()` projected from the staged receive frame.
There is no `recv_frame_hint` compatibility path, no post-poll receive metadata
peek API, and no receive-observation side channel.

`ReceivedFrame::deterministic(payload)` is only for payload-only transports that cannot
observe Hibana frame metadata. In that case the runtime must not synthesize a
`FrameHeader` from expected context. No observed header means no transport
mismatch evidence can be derived from that receive; Pico/SIO-class transports
must use `ReceivedFrame::framed`.

## 18. Receive Frame Lifecycle

```text
adapter:
  receives or stages one Hibana frame
  returns ReceivedFrame::framed(FrameHeader, Payload) from poll_recv
  or returns ReceivedFrame::deterministic(Payload) only when no frame metadata is observable

hibana runtime:
  reads the FrameHeader from the same ReceivedFrame value
  compares observed session/lane/role/label with expected context

if match:
  descriptor/payload checks continue
  endpoint commit emits normal ENDPOINT_RECV evidence

if mismatch:
  emit TransportMismatch Evidence
  do not deliver payload to the app
  discard the mismatched frame
  keep endpoint progress unchanged
  keep the receive operation waiting/pending
```

Constraints:

```text
ReceivedFrame::framed must contain metadata for the same staged frame as its payload
observed ReceivedFrame header must not consume payload
observed ReceivedFrame header must not commit progress
poll_recv must return header and payload together for observed transports
route evidence and mismatch evidence derive from the same ReceivedFrame header
missing header must remain unknown and must not be replaced with expected context
```

This keeps mismatch evidence aligned with payload delivery without adding
endpoint rejection semantics.

The validation owner is singular. Do not let direct receive, passive offer,
route decode, or control receive each invent its own partial check.

```text
classify_staged_frame(expected, frame_header):
  if session / lane / source_role / peer_role / label all match:
    Accept

  if any expected frame context field does not match:
    DiscardAndWait(reason)

  if transport returned Err:
    TerminalFault(error)
```

All receive paths must use the same expected-frame classifier before delivering
fresh transport payload to user/control decode. Structural route/materialization
invariants remain fail-closed.

```text
direct recv:
  poll_recv -> classify -> accept/discard+pending/fault

passive offer materialization:
  selected branch and staged payload must remain internally consistent
  route/materialization contradictions are PhaseInvariant
  fresh frame context mismatch is TransportMismatch only before it becomes structural route authority

route branch decode:
  staged frame -> classify against DecodeRuntimeDesc / RecvMeta context
  match is required before payload decode

control receive:
  staged frame -> classify against control expected context
  match is required before control commit
```

The classifier is a pre-progress frame-context classifier only. It must not
absorb endpoint phase, route frontier, descriptor, control-op, or policy
invariants.

```text
Stage 1: classify_frame_context(expected_receive_context, staged FrameHeader)
  Match:
    continue to endpoint phase application

  Mismatch(Session/Lane/SourceRole/PeerRole/Label):
    emit TransportMismatch
    discard only that frame
    keep endpoint progress unchanged
    keep operation pending

Stage 2: apply accepted frame to endpoint phase
  payload decode failure:
    codec/decode error, fail-closed

  route frontier / descriptor / control op / phase contradiction:
    PhaseInvariant, fail-closed

  policy reject:
    policy reject, fail-closed
```

Required order:

```text
1. verify session / lane / source_role / peer_role / label
2. on mismatch: emit TransportMismatch, discard only that frame, keep progress unchanged
3. only after match: treat FrameHeader::label() as route evidence
4. only after match: decode payload or commit control progress
```

The same apparent label mismatch is classified by authority, not by spelling.

```text
label mismatch before endpoint progress:
  direct recv is waiting for message M
  staged FrameHeader::label() is not M's expected frame label
  -> TransportMismatch / discard + pending

label mismatch after route/materialization authority exists:
  selected route branch / phase / descriptor says one frame label
  materialized branch, control evidence, or descriptor relation says another
  -> PhaseInvariant / fail-closed
```

Forbidden partial validation:

```text
session mismatch only handled in direct recv
fresh receive path checks label but ignores session/source/peer
decode path accepts payload before header context matches
wrong source_role falls through to codec failure
```

`TransportMismatch` is not a generic error bucket. It is only the disposition for
an observed Hibana frame whose expected frame context does not match. This is the
only receive failure class that may discard one frame and continue waiting.

`PhaseInvariant` must still be used when hibana's own route/materialization
state contradicts itself.

```text
PhaseInvariant examples:
  selected route arm expects one frame label but materialized branch carries another
  staged branch metadata no longer matches the descriptor used for decode
  binding/control evidence contradicts the already selected route authority
  descriptor/control invariants are violated after Hibana has committed internal authority
```

Do not downgrade these to `TransportMismatch`. Hibana's purpose is to fail
closed on impossible internal protocol state.

## 19. Mismatch and Fault

The adapter does not return Reject.

```text
adapter:
  returns ReceivedFrame through poll_recv
  binds staged FrameHeader to payload in that same value when metadata is observable

hibana runtime:
  compares expected context with the observed header bound to ReceivedFrame

mismatch:
  emits TransportMismatch Evidence
  discards the mismatched frame
  continues waiting for the expected frame
```

Session and descriptor authority remain in hibana runtime.

`TransportMismatch` reasons are limited to expected-vs-observed `FrameHeader`
context:

```text
Session
Lane
SourceRole
PeerRole
Label
```

Do not encode pre-progress expected-vs-observed context mismatches as endpoint
rejects, codec failures, phase invariants, or policy rejects. The endpoint
operation remains pending unless a separate terminal transport fault occurs.

Do not put unrelated failures into `TransportMismatch`:

```text
payload codec failure
control handle decode failure
descriptor invariant failure
policy reject
application-level value mismatch
QUIC authentication failure before local Hibana frame staging
QUIC packet parse failure before local Hibana frame staging
capacity pressure
deadline
offline
failed carrier
```

TransportMismatch emission owner is singular:

```text
only classify_frame_context / ExpectedFrame classifier may emit TransportMismatch
PhaseInvariant must not be converted into TransportMismatch
TransportError must not be converted into TransportMismatch
codec/decode errors must not be converted into TransportMismatch
```

This is a gate, not just documentation.

## 20. TransportFrame Emit Policy

`TransportFrame` is staged/observed frame-header evidence. It is not accepted
evidence and it is not commit evidence.

Canonical policy:

```text
accepted/staged observation needed:
  emit TransportFrame

mismatch:
  emit TransportMismatch only
  TransportMismatch includes observed metadata
  do not also emit duplicate TransportFrame for the same mismatched frame

commit:
  ENDPOINT_RECV / ENDPOINT_CONTROL remains the acceptance evidence
```

If a future timeline VM needs every staged frame, that must be an explicit plan
revision with a canonical `TransportFrame -> TransportMismatch` order. The
current Pico-class default is the lower-event-count policy above.

There is no `WaitObservation`.

```text
poll_send / poll_recv
  -> Err(TransportError)
  -> hibana runtime emits TransportFault Evidence
  -> endpoint operation returns terminal error
```

```rust
pub enum TransportError {
    Offline,
    Deadline,
    Capacity,
    Failed,
}
```

`Capacity` remains as a diagnostic reason for ring overflow, queue full, or
buffer exhaustion. It does not add a new event kind; it is a `TransportFault`
reason.

`TransportFault` is terminal for the current endpoint operation. It is not
`discard + pending`.

```text
TransportMismatch:
  observed frame context mismatch
  emit evidence
  discard mismatched frame
  progress unchanged
  wait continues

TransportFault:
  carrier-local terminal condition
  emit evidence
  return endpoint transport error
```

## 21. Transport Evidence

hibana runtime emits only canonical transport evidence:

```text
TransportFrame
TransportMismatch
TransportFault
```

`TransportFrame` is the semantic evidence corresponding to staged/observed
`FrameHeader` route evidence. It replaces the old "hint" wording.
It is not acceptance evidence.

```text
TransportFrame:
  staged/observed FrameHeader exists
  payload has not necessarily been delivered
  endpoint progress has not necessarily committed

TransportMismatch:
  staged/observed FrameHeader did not match expected context
  mismatched frame was discarded
  endpoint progress stayed unchanged

TransportFault:
  poll_send / poll_recv returned TransportError
  endpoint operation returned terminal transport error
```

Acceptance remains derivable from endpoint commit events:

```text
ENDPOINT_RECV
ENDPOINT_CONTROL
```

Minimum Evidence ABI:

```text
TRANSPORT_FRAME:
  arg0 = session
  arg1 = lane/source/peer/label packed as canonical metadata
  arg2 = optional adapter-local compact status or zero

TRANSPORT_MISMATCH:
  causal_hi = expected_lane
  causal_lo = reason
  arg0 = expected_session
  arg1 = observed_session
  arg2 = observed_lane << 24
       | source_role   << 16
       | peer_role     << 8
       | frame_label

TRANSPORT_FAULT:
  causal_hi = lane or zero
  causal_lo = reason
  arg0 = session or zero if unavailable
  arg1 = carrier-local compact status or zero
  arg2 = lane/source/peer/label or zero if no staged frame exists
```

`TapEvent::evidence()` must decode all three transport kinds into typed
`input[4]`. EPF bytecode must read typed Evidence input, not raw transport
adapter state.

Do not add:

```text
TransportParsed
TransportAccepted
TransportOpen
TransportRequeue
TransportWaitObservation
TransportHint
receive-observation side channel
```

Reasons:

```text
Parsed:
  staged FrameHeader is checked against expected context. A standalone event is not needed.

Accepted:
  ENDPOINT_RECV / ENDPOINT_CONTROL commit is the evidence of acceptance.

Open:
  derivable from PortOpen / rendezvous / lane lifecycle.

Requeue:
  runtime action, not adapter observation.

Wait:
  diagnostic evidence derived from TransportError.

Hint:
  obsolete 0.6-era wording. Route evidence is a staged frame header.
```

## 22. Future QUIC Boundary

The EPF-driven hibana changes must not make future `hibana-quic` impossible, so
the boundary is documented here. It is not a current implementation gate for
this plan. The current gate is hibana + hibana-pico Pico/SIO proof.

```text
QUIC wire frame != Hibana frame
Hibana frame label is local demux/progress evidence
Hibana frame label must never become QUIC wire format
```

This is not a custom QUIC design. Peer-visible QUIC remains RFC QUIC/H3.

Forbidden:

```text
put Hibana frame labels into QUIC packets
require peers to send Hibana-specific QUIC frames
extend H3 frame types for Hibana routing
make 0-RTT decisions through peer-visible non-standard control frames
```

Allowed and required:

```text
standard QUIC packet / frame / stream / H3 frame
  -> hibana-quic decodes and updates QUIC state
  -> hibana-quic stages a local Hibana frame or binding evidence
  -> hibana compares descriptor/session/lane/role/label context
  -> EPF observes the resulting TapEvent/Evidence
```

`FrameHeader` is local semantic evidence returned from the transport adapter to
hibana runtime. It is not a QUIC wire header.

```text
peer-visible wire:
  standard QUIC packet
  standard QUIC frame
  standard QUIC stream
  standard H3 frame

hibana-quic local staging:
  QUIC connection -> local rendezvous/session binding
  stream id / direction / stream class -> local lane/role binding
  CRYPTO / H3 readiness -> local control frame or binding evidence
  stream payload readiness -> local Hibana frame
```

QUIC packet parse failure, decrypt failure, or authentication failure before a
local Hibana frame is staged is not `TransportMismatch`. It is either
transport-local drop/telemetry or `TransportFault` if it becomes terminal to the
Hibana endpoint operation.

Examples:

```text
Retry packet observed:
  local Hibana control frame/evidence

Version Negotiation observed:
  local Hibana control frame/evidence

ServerHello CRYPTO parsed:
  local Hibana control frame/evidence

H3 request stream has complete HEADERS frame:
  local Hibana receive frame/evidence

0-RTT break:
  local Hibana control frame/evidence with empty payload if needed
```

`hibana-quic` must not preserve `recv_frame_hint` as a compatibility shim. It
must stage route/progress-visible facts as Hibana frames or binding evidence.

Future QUIC compatibility requires the same receive classifier as Pico/SIO. A
staged local Hibana frame from `hibana-quic` must be validated against expected
session/lane/source/peer/label before route materialization, route decode, or
control commit can consume it.

## 23. Evidence ABI

`TapEvent` is the physical record. `Evidence` is the semantic decode.

```rust
impl TapEvent {
    pub const fn evidence(self) -> Evidence;
}
```

`Evidence` stays in the runtime tap surface:

```rust
hibana::runtime::tap::Evidence
```

Users should not need to memorize raw packing.

VM input:

```text
TapEvent
  -> Evidence
  -> typed input[4]
  -> VM
  -> Out
```

Policy input:

```text
policy context + internal evidence snapshot
  -> typed input[4]
  -> VM
  -> Out
```

Transport Evidence decoding is mandatory for first-class EPF diagnosis:

```text
TransportFrame:
  typed staged frame observation

TransportMismatch:
  typed expected-vs-observed frame context mismatch

TransportFault:
  typed terminal carrier fault
```

Do not ship a plan or implementation where session mismatch is observable but
deadline/capacity/offline/failed transport conditions are invisible to EPF.

## 24. Image Header

```rust
pub struct Header {
    pub abi: u16,
    pub target: Target,
    pub schema_hash: u32,
    pub code_len: u16,
    pub fuel_max: u16,
    pub mem_len: u16,
    pub code_hash: u32,
}
```

Verification:

```text
schema_hash == hibana Evidence ABI hash
target is supported
code_hash matches
code_len <= selected fixed slot image capacity
mem_len <= selected fixed slot scratch capacity
fuel_max != 0
instructions valid

Target::Observe:
  decision output forbidden

Target::Policy(ID):
  ID must be a valid Hibana dynamic policy id
  ID must not be the static/no-policy sentinel
  observation-state mutation forbidden
```

Policy output is validated before conversion to Hibana decision:

```text
Out.kind = Defer:
  DecisionResolution::Defer

Out.kind = Choose:
  Out.arg0 == 0 -> Left
  Out.arg0 == 1 -> Right
  otherwise -> invalid policy VM output -> fail closed

Out.kind = Reject:
  fail closed
```

`Out.arg0` must never be treated as an arbitrary arm integer.

## 25. ReplayRecord

`ReplayRecord` is not VM input. It is runner input and a verification log.

```rust
pub enum ReplayRecord {
    Tap(TapEvent),

    ImageLoaded {
        target: Target,
        epoch: u32,
        digest: u32,
    },

    ImageReverted {
        target: Target,
        epoch: u32,
        digest: u32,
    },

    ImageUnloaded {
        target: Target,
        epoch: u32,
    },

    ImageRejected {
        target: Target,
        reason: LoadRejectReason,
    },

    Run {
        target: Target,
        observe_epoch: u32,
        policy_epoch: Option<u32>,
        engine: Engine,
        out_digest: u32,
        fuel_used: u16,
    },
}
```

Replay semantics:

```text
ReplayRecord::Tap:
  epf.run(event)

ImageLoaded / ImageReverted / ImageUnloaded:
  replay runner reproduces image lifecycle

ImageRejected:
  verifies rejection

Run:
  verifies observed output / decision / fuel
```

Image lifecycle records are not fed into the VM.

## 26. Remote Paths

Only these remote/debug data paths are allowed.

```text
TapEvent uplink:
  device -> host
  debug / replay

image ingress:
  manager / host / OTA -> device
  epf.load / unload / revert

debug bytecode ingress:
  debug probe / UART -> reserved EPF bytecode area
  runtime verifies and calls epf.load(image)
```

Forbidden:

```text
remote decision
```

If a remote actor participates in a decision, model it as a choreography role.

```text
remote role -> fact message
device role -> receives fact
Epf observes fact
policy::<ID>() resolver uses local snapshot
```

## 27. Live Update

Live update is not hidden fetch.

```text
g::par(
  epf_update_service_loop,
  APP
)
```

Alternatively, use an explicit update point inside `APP`.

The update service is an image delivery protocol.

```text
Manager -> Device: ImageHeader
Manager -> Device: ImageChunk*
Manager -> Device: Commit

Device:
  epf.load(buffer)

Device -> Manager:
  Loaded { target, epoch, digest }
  or Rejected { target, reason }
```

## 28. Acceptance Criteria

EPF:

```text
public runtime object is Epf only
EPF is VM appliance, not hook runtime
Pico class is the contract, not a profile
RP2040/SIO loaded-bytecode observe proof is the first acceptance gate
no_std / no_alloc / fixed storage by default
EpfStorage has fixed policy slots, image bytes, scratch bytes, and history capacity
EpfStorage<0, IMAGE_BYTES, SCRATCH_BYTES, HISTORY> supports observe bytecode loading
Epf is a borrowed appliance view over typed fixed EpfStorage
Epf capacity constants are storage capacities, not profiles
Target::Policy(ID) is logical; physical storage is a fixed slot table
Target::Policy(ID) rejects Hibana static/no-policy sentinel ids at image verification
load(Target::Policy(ID)) returns a slot-unavailable error when no policy slot is available
Out is a compact device record, not a formatted report
Out has target-specific interpretation without adding DecisionOut
Target::Policy(ID) interprets Out.kind/reason/arg0 as decision encoding
invalid policy Out.kind or Choose arg0 fails closed and never silently falls back
device code writes Out to RAM markers or compact uplink
device code never hand-synthesizes EPF success markers as proof
string rendering is host/debug tooling only
debug has host-run and device-run paths; RP2040/SIO proof uses device-run
run(TapEvent) is direct observe/debug/replay only
device-run proof drains actual Hibana TapPort, not pending compact evidence
hibana-epf exposes resolver() only as a real policy VM wrapper
policy VM runs only through resolver::<ID>(fallback)
EPF VM is not Wasm and not eBPF-shaped
EPF does not run inside a WASI P1 guest
EPF does not run in SIO FIFO push/pop, transport poll critical sections, or ISR
load/revert/unload are atomic with active unchanged on failure
observe VM trap reports Out and does not roll back canonical evidence state
policy VM trap fails closed and never silently falls back
loaded observe image digest/epoch/fuel_used are externally inspectable
```

hibana:

```text
hibana 0.8.0 is authority
0.6 compatibility layers are forbidden
app-facing API does not grow for EPF
hibana core does not know EPF images or VM
no EpfResolver
no ResolverRef::epf
no tap sink
no remote decision API
Evidence ABI is hibana integration authority
Hibana frame is the route/progress-visible receive unit
control-only receive evidence can be represented as a Hibana frame or binding evidence
one incoming-frame classifier owns expected-vs-observed header validation
fresh transport payload delivery paths use that classifier before user/control decode
session/lane/source_role/peer_role/label must all match before fresh payload delivery
TransportMismatch is discard+pending only, never a new endpoint reject/failure semantic
TransportMismatch emission owner is the ExpectedFrame classifier path only
TransportMismatch is never emitted for PhaseInvariant, TransportError, codec/decode errors, or policy reject
TransportFault is terminal transport error evidence
PhaseInvariant remains mandatory for impossible internal route/materialization/descriptor state
```

Transport:

```text
poll_recv returns ReceivedFrame
ReceivedFrame binds payload bytes and optional FrameHeader in the same value
ReceivedFrame::framed header must describe the exact same staged frame as its payload
observed ReceivedFrame header must not consume payload or commit progress
missing header remains unknown and must not be synthesized from expected context
post-poll receive metadata peek API does not exist
there is no recv_frame_hint compatibility path
there is no receive-observation side channel
route evidence derives from FrameHeader::label()
route evidence must not bypass full payload-delivery validation
adapter never returns Reject
WaitObservation does not exist
TransportError includes Offline, Deadline, Capacity, Failed
transport evidence kinds are TransportFrame, TransportMismatch, TransportFault
TransportFrame is staged/observed FrameHeader evidence, not accepted evidence
TransportFrame is not emitted for a frame that is emitted as TransportMismatch
TransportMismatch reasons are only Session, Lane, SourceRole, PeerRole, Label
TransportFault reasons include Offline, Deadline, Capacity, Failed
SIO carrier exposes observed frame metadata but does not decide mismatch authority
SIO FIFO word path remains carrier-only and does not invoke EPF
test transports must preserve actual source_role/peer_role metadata instead of hard-coding source_role = 0
```

Future QUIC boundary, non-blocking for this hibana/hibana-pico implementation:

```text
hibana-quic targets hibana 0.8.0, not 0.6.x
QUIC wire remains standard RFC QUIC/H3
Hibana frame labels never appear on the wire
FrameHeader is local semantic evidence, not QUIC wire format
Retry / VN / ServerHello / 0-RTT break / H3 readiness become local Hibana frame or binding evidence
recv_frame_hint is deleted, not preserved as a shim
QUIC route/progress evidence is commit/requeue/discard capable
QUIC parse/decrypt/auth failure before local Hibana frame staging is not TransportMismatch
staged hibana-quic frames use the same header classifier as Pico/SIO
```

RP2040/SIO:

```text
core1 may host a WASI P1 engine, but EPF runs in native host/runtime side
WASI P1 guest is a diagnosis subject, not the EPF owner
session-mismatch acceptance must use EngineArtifact = WasiImage, not NoWasi
session-mismatch guest must be a Rust std WASI P1 guest using hibana-wasip1-guest::choreofs
session-mismatch must first become stuck at the path_open_ret reply when only the SIO Tx frame header is skewed
session-mismatch TapEvent filter must use lane 1 TransportMismatch/session_mismatch
session-mismatch must not require a RAM panic marker or a new endpoint recv failure
session-mismatch may use carrier-local deadline only to yield from blocking wait back to observe
session-mismatch proof must retain TapEvents in an EPF RAM spool and consume them after bytecode load
core0 and core1 must use separate EPF storage and separate TapEvent spools; shared EPF mutable state across cores is not accepted
stuck-state observe drain must read up to the spool capacity, not a smaller arbitrary demo limit
observe bytecode image can be written into a reserved RAM area by probe/UART
runtime loads committed observe bytecode without rebooting
loaded observe VM must explain deliberate SIO session/lane mismatch through RAM markers or UART
loaded observe VM must explain SIO capacity pressure through TransportFault/Capacity markers or UART
capacity-fault proof must load observe bytecode and produce kind=TransportFault reason=Capacity from typed TapEvent/Evidence input
policy proof must load Target::Policy bytecode and make a route decision through hibana ResolverRef, not by local branch flags
policy timer proof must accept Target::Policy bytecode from debug-probe UART into a fixed staging area
UART RX interrupt may only stage bytes and set image-ready fact; it must not call epf.load, run EPF, or select a route
policy timer proof must use a Hibana route resolver to read the UART image-ready fact and select the image-load choreography branch
policy timer proof must deliver the staged Target::Policy bytecode through normal Hibana/SIO choreography after that branch is selected
policy timer proof must register an EPF-wrapped default resolver; default resolver reads the RP2040 timer IRQ-ready fact and selects timer-expired
policy timer proof must feed the same timer IRQ-ready fact to EPF as a Hibana TapEvent at resolver entry on each endpoint
policy timer bytecode must read typed VM input from that timer TapEvent and select response-ready only when input[0] == 1
policy timer proof must copy compact EPF Out to RAM markers and UART compact records after EPF runs
policy timer proof must record active target Policy(ID), image_ingress=UART_CHOREOGRAPHY, uart_rx_ready=1, uart_tx_records>0, timer_irq_ready=1, timer_fact_kind/arg/fuel, and non-zero EPF observe markers on both core0 and core1
carrier-local deadline must produce RAM panic markers with Transport(Deadline)
panic-marker diagnostics and loaded-bytecode diagnostics are complementary, not competing paths
EPF_FUEL_USED must be non-zero for loaded-bytecode proofs
```

Remote:

```text
TapEvent uplink allowed
image ingress allowed
debug-probe/RAM bytecode ingress allowed
UART bytecode ingress allowed
hidden remote decision forbidden
remote influence must be modeled as choreography facts
```

Process gates:

```text
do not push hibana without explicit user instruction
hibana final gate must pass before claiming hibana is complete
hibana maintainability budget must pass; observe/core.rs must not exceed its owner budget
hibana-pico loaded-bytecode hardware proof must pass before claiming RP2040/SIO EPF proof
hibana-quic work is outside the current implementation gate; run a local-hibana smoke only before claiming QUIC compatibility
```

## 29. Final Usage

RP2040/SIO loaded-bytecode observe debug:

```rust
static mut EPF_CORE0_STORAGE: EpfStorage<1, IMAGE_BYTES, SCRATCH_BYTES, HISTORY> =
    EpfStorage::new();
static mut EPF_CORE1_STORAGE: EpfStorage<1, IMAGE_BYTES, SCRATCH_BYTES, HISTORY> =
    EpfStorage::new();
static mut EPF_BYTECODE_AREA: EpfBytecodeArea<IMAGE_BYTES> =
    EpfBytecodeArea::empty();

let epf = if core_id() == 0 {
    Epf::new(unsafe { &mut *core::ptr::addr_of_mut!(EPF_CORE0_STORAGE) })
} else {
    Epf::new(unsafe { &mut *core::ptr::addr_of_mut!(EPF_CORE1_STORAGE) })
};
```

Diagnostic load tick:

```rust
if let Some(image) = unsafe { (&mut *core::ptr::addr_of_mut!(EPF_BYTECODE_AREA)).take_commit() } {
    match epf.load(image.bytes()) {
        Ok(load) => {
            ram_markers.epf_load_epoch = load.epoch;
            ram_markers.epf_image_digest = load.digest;
            ram_markers.epf_load_reason = 0;
        }
        Err(error) => {
            ram_markers.epf_load_reason = error.code();
        }
    }
}
```

Debug loop:

```rust
let mut tap = rv.tap();

let mut drained = 0;
while drained < 8 {
    drained += 1;
    let Some(event) = tap.next() else { break };
    let out = epf.run(event);

    if out.emitted() {
        ram_markers.epf_epoch += 1;
        ram_markers.epf_kind = out.kind();
        ram_markers.epf_reason = out.reason();
        ram_markers.epf_arg0 = out.arg0();
        ram_markers.epf_arg1 = out.arg1();
        ram_markers.epf_arg2 = out.arg2();
        ram_markers.epf_fuel_used = out.fuel_used() as u32;
        break;
    }
}
```

The proof is valid only when:

```text
epf_load_epoch != 0
epf_image_digest == expected observe image digest
epf_fuel_used != 0
epf_kind/reason/args are copied from VM Out
each core owns its own EpfStorage and TapEvent spool
```

Generic retained-event replay:

```rust
let _ = spool.push(event);

if let Some(load) = epf.load_ingress(&mut ingress)? {
    ram_markers.epf_load_epoch = load.epoch();
}

if let Some(out) = epf.run_spool(&mut spool) {
    let record = hibana_epf::CompactOutRecord::from_out(core_id, out);
    record.encode(&mut bytes);
    // board firmware decides RAM marker, UART, USB, or SIO sink
}
```

Loaded policy resolver:

```rust
let fallback7 = ResolverRef::<7>::decision_state(&state, choose);

rv.role(&role0)
    .set_resolver::<7>(epf.resolver::<7>(fallback7))?;

epf.load(&policy7_image)?;              // Policy(7) loaded: fallback ignored
epf.unload(Target::Policy(7))?;         // fallback restored
epf.revert(Target::Policy(7))?;         // previous Policy(7) VM restored
```

The Baker `epf-policy-timer` proof specializes this shape:

```text
debug probe UART sends framed Target::Policy(57) bytecode to core0 UART0
UART0 RX interrupt stages bytes into fixed RAM and sets only uart_rx_ready fact
first Hibana route resolver reads uart_rx_ready and selects image-load branch
image-load branch sends Target::Policy(57) as a normal role0 -> role1 SIO choreography payload
image_ingress=UART_CHOREOGRAPHY proves source=UART and load path=choreography
default resolver reads RP2040 timer IRQ-ready fact
timer_irq_ready=1 would make default resolver choose Right / timer-expired
each resolver entry feeds timer_irq_ready as TapEvent{id=0x57,arg0=1} into Epf::run
loaded EPF policy VM requires typed input[0] == 1
loaded EPF policy VM => Left / response-ready
compact EPF Out is written to RAM markers and UART compact records
success requires UART bytecode ingress, resolver-selected image-load choreography, timer TapEvent ingestion, and the Hibana ResolverRef path to call the loaded EPF VM
```

Host debug:

```bash
hibana-epf image session-mismatch --wrap -o session-mismatch.hepf
hibana-epf image session-mismatch > observe.epf
hibana-epf image deadline-fault > observe.epf
hibana-epf image capacity-fault > observe.epf
hibana-epf image policy-choose-on-input 57 0 1 0 > policy.epf
hibana-epf wrap-image policy.epf > policy.hepf
hibana-epf diagnose session-mismatch trace.hbt
hibana-epf explain-tap trace.hbt
hibana-epf replay --image observe.epf --out out.hepo trace.hbt
hibana-epf stream --recipe session-mismatch < trace.hbt
hibana-epf explain-out out.hepo
```

`stream` reads canonical 20-byte TapEvent records from stdin. Serial, USB CDC,
TCP, and debug-probe capture are board/host runner responsibilities layered on
top of this codec, not separate EPF runtime models. A board runner may connect
`policy.hepf`, TapEvent records, and `out.hepo` to Debug Probe UART, USB CDC,
TCP, files, or a Hibana management choreography.

Final shape:

```text
EPF に流す。
VM が走る。
Out を見る。
```

Only `Epf` is the public EPF runtime object.
