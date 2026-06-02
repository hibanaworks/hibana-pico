# EPF Final Design Plan

## 1. Core Principle

EPF is a VM appliance.

```text
EPF に入力を流す。
VM が走る。
Out を見る。
```

Observation, debugging, replay, and policy control all use the same execution model.
There is no EPF that is not VM execution.

```text
observe / debug / replay:
  TapEvent -> Epf VM -> Out

policy:
  policy::<ID>() -> Epf VM -> Out -> Decision
```

The input source and interpretation of `Out` differ, but the appliance model does not.

Pico class is not a profile. Pico class is the contract.

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

Larger hosts do not get a different EPF model. Host tools are renderers and
replay runners for the same compact records produced by the device.

```text
device:
  TapEvent -> epf.run(event) -> compact Out

host:
  TapEvent / compact Out stream -> render / explain / replay
```

RP2040/SIO is the hard proof target. If the design cannot explain SIO faults on
RP2040 without disturbing the FIFO path, it is not sufficiently refined.

```text
RP2040/SIO proof target:
  observe-only first
  native host/runtime side only
  no EPF inside the WASI P1 guest
  no EPF in the SIO FIFO word path
  no EPF in ISR
  compact Out -> RAM markers
```

## 2. Public Runtime Object

The only public EPF runtime object is `Epf`.

```rust
pub struct Epf<'a>;
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

EPF must not become a smaller eBPF. It does not expose hooks, maps, helpers, program
types, attach points, event channels, or pinning.

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
work without any policy storage.

```rust
type MinimalSioEpfStorage = EpfStorage<0, 256, 32, 4>;
```

`Target::Policy(ID)` is a logical target. Physical storage is a fixed policy slot
table.

```text
Target::Policy(ID):
  logical target

physical storage:
  fixed slot table
```

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

The EPF VM is intentionally smaller than Wasm and not modeled after eBPF.

```text
VM contract:
  typed input[4]
  compact Out
  fixed fuel
  fixed scratch
  no allocator
  no string ops
  no helper calls
  no map access
  no host callbacks
  no unbounded loops
```

## 3. Epf API

```rust
impl Epf<'_> {
    pub fn new<
        const POLICY_SLOTS: usize,
        const IMAGE_BYTES: usize,
        const SCRATCH_BYTES: usize,
        const HISTORY: usize,
    >(
        storage: &mut EpfStorage<POLICY_SLOTS, IMAGE_BYTES, SCRATCH_BYTES, HISTORY>,
    ) -> Self;

    pub fn load(&self, image: &[u8]) -> Result<Load, Error>;

    pub fn unload(&self, target: Target) -> Result<(), Error>;

    pub fn revert(&self, target: Target) -> Result<(), Error>;

    // Direct observe/debug/replay path only.
    pub fn run(&self, event: TapEvent) -> Out;

    // Policy path. Policy VM is only run through this resolver wrapper.
    pub fn resolver<'a, const POLICY_ID: u16>(
        &'a self,
        fallback: ResolverRef<'a, POLICY_ID>,
    ) -> ResolverRef<'a, POLICY_ID>;
}
```

API meaning:

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
  return a ResolverRef<ID> wrapper that runs the EPF policy VM at policy::<ID>()
```

`run(event)` is for direct observe/debug/replay only. It cannot directly execute a
`Target::Policy(ID)` VM. Policy VM execution is only available through
`resolver::<ID>(fallback)`.

`Epf` is a capacity-erased borrowed appliance view. `EpfStorage` owns typed fixed
arrays; `Epf` captures their erased capacities at construction.

```text
EpfStorage:
  owns typed fixed arrays
  carries POLICY_SLOTS / IMAGE_BYTES / SCRATCH_BYTES / HISTORY in the type

Epf:
  borrowed appliance view
  not const-generic
  stores erased capacity metadata captured from EpfStorage
  performs all capacity checks through erased capacities
```

This keeps the public runtime object as `Epf` rather than producing a family of
runtime object types.

## 4. Target

```rust
pub enum Target {
    Observe,
    Policy(u16),
}
```

`Target` is an image header attribute and an `unload` / `revert` value. It is not a
runtime object.

```text
Target::Observe:
  observation / debug / explanation VM

Target::Policy(ID):
  policy VM that runs when policy::<ID>() is reached
```

`epf.load(image)` reads the `Target` from the image header and installs the image in
the corresponding fixed physical slot.

## 5. Observe VM

`Epf::new` starts with a built-in observe VM.

```text
default observe VM:
  endpoint send/recv timeline
  route decision timeline
  transport reject/fault/hint explanation
  lane lifecycle check
  session/lane/role/label correlation
```

Load a `Target::Observe` image only when the observation logic should be replaced.

```rust
epf.load(&observe_image)?;
```

`unload(Target::Observe)` does not disable observation.

```text
unload(Target::Observe):
  return to built-in observe VM
```

`revert(Target::Observe)` restores the previous loaded observe image. If there is no
previous loaded observe image, it returns to the built-in observe VM.

## 6. Debug Path

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

RP2040/SIO proof target is device-run debug.

```text
TapEvent -> epf.run(event) -> compact Out -> RAM markers
```

Device-run debug:

```rust
let mut tap = rv.tap();
let epf = Epf::new(&mut epf_storage);

if let Some(event) = tap.next() {
    let out = epf.run(event);

    ram_markers.epf_epoch = out.epoch;
    ram_markers.epf_kind = out.kind as u32;
    ram_markers.epf_reason = out.reason as u32;
    ram_markers.epf_arg0 = out.arg0;
    ram_markers.epf_arg1 = out.arg1;
    ram_markers.epf_arg2 = out.arg2;
    ram_markers.epf_fuel_used = out.fuel_used as u32;
}
```

Host-run debug:

```bash
hibana-epf live --serial /dev/ttyACM0
hibana-epf replay trace.hbt
hibana-epf explain trace.hbt --session 0x5195d24c
```

Host-run tools use this core flow:

```text
TapEvent stream
  -> epf.run(event)
  -> compact Out
  -> render
```

No policy image is required for debugging. The observe VM always runs.
On device, `Out` is written to RAM markers, a compact uplink, or a resolver result.
String rendering is a host responsibility.

## 7. RP2040/SIO and WASI P1 Placement

Baker's core1 application role may be a WASI P1 guest, but EPF does not run inside
that guest.

```text
RP2040 core1:
  native firmware / runtime / SIO transport / WASI P1 engine
    -> WASI P1 guest logical image
```

The WASI P1 guest is a logical choreography image and a diagnosis subject. It is
not the owner of SIO, tap, RAM markers, or EPF.

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

The SIO FIFO word path must stay a carrier path.

```text
SIO FIFO word path:
  push/pop words
  assemble/stage frames
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

The first RP2040/SIO implementation is observe-only.

```text
initial storage:
  EpfStorage<0, 256, 32, 4>

initial capability:
  TapEvent -> epf.run(event) -> compact Out -> RAM marker

not required initially:
  policy slots
  image ingress
  hot-swap choreography
```

Policy hot-swap is a second-stage capability. It is not allowed to justify a
larger first implementation.

Minimum RP2040/SIO proof:

```text
core0 -> SIO -> core1
create deliberate session or lane mismatch
hibana runtime emits TransportReject Evidence
core1 native runtime calls epf.run(TapEvent)
core1 RAM markers contain:
  EPF_KIND        = TransportReject
  EPF_REASON      = SessionMismatch or LaneMismatch
  EPF_ARG0        = expected_session
  EPF_ARG1        = observed_session
  EPF_ARG2        = packed lane/source/peer/label
OpenOCD script reads the markers after the run
```

Capacity proof:

```text
force SIO demux/ring/queue capacity pressure
hibana runtime emits TransportFault Evidence
EPF Out contains:
  kind   = TransportFault
  reason = Capacity
  arg0   = fifo/status or capacity site
  arg1   = lane
  arg2   = pending/demux state
```

If this observe-only marker proof cannot be made reliable on RP2040/SIO, EPF is
not practical enough.

## 8. Policy Path

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

```text
intentional tradeoff:
  registration mismatch is prevented by ResolverRef<ID>
  image lifecycle is controlled by Target values to avoid more public API surface
```

## 9. ResolverRef

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

## 10. Fixed Storage and Hot-Swap Semantics

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

`load(Target::Observe image)`:

```text
verify image
copy into inactive observe bank
initialize inactive scratch
flip active observe image
observe_epoch += 1
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

`revert(Target::Observe)`:

```text
if previous loaded observe image exists:
  flip active observe image to previous
  observe_epoch += 1
else:
  return to built-in observe VM
  observe_epoch += 1
```

`revert(Target::Policy(ID))`:

```text
if ID has an assigned slot and previous image exists:
  flip active policy image to previous
  policy_epoch += 1
else:
  Err(NoPrevious)
```

`unload(Target::Observe)`:

```text
return to built-in observe VM
observe_epoch += 1
```

`unload(Target::Policy(ID))`:

```text
if ID has an assigned slot:
  clear active policy image for ID
  keep the physical slot reusable
  fallback resolver becomes active again
  policy_epoch += 1
else:
  fallback resolver is already active
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

## 11. Internal Evidence State

`Epf` keeps correlation and evidence state internally. This state is not public API.

```text
internal state:
  latest TapEvent-derived Evidence
  session/lane/role/label correlation
  route timeline
  transport reject/fault/hint history
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

Rationale: the observe VM is replaceable. A broken observe image must not block or
corrupt the canonical evidence ledger that policy VMs read.

Policy VM snapshots state at resolver entry.

```text
resolver entry:
  policy_epoch snapshot
  observe_epoch snapshot
  compact evidence summary snapshot
```

If `run(event)` advances while a resolver call is executing, that resolver call still
uses only its captured snapshot.

```text
same policy_epoch
same observe_epoch
same policy input
=> same Out
```

Policy VM may read the snapshot, but it must not mutate the observation state.

## 12. Transport Receive Path

The receive path is frame-first. A received carrier frame is the unit of
progress, rollback, route evidence, and reject evidence.

```rust
pub struct FrameHeader {
    pub session: SessionId,
    pub lane: Lane,
    pub source_role: u8,
    pub peer_role: u8,
    pub label: FrameLabel,
}

pub struct Incoming<'a> {
    pub header: FrameHeader,
    pub payload: Payload<'a>,
}

fn poll_recv<'a>(
    &'a self,
    rx: &'a mut Self::Rx<'a>,
    cx: &mut Context<'_>,
) -> Poll<Result<Incoming<'a>, Self::Error>>;
```

`Payload` is no longer returned alone. Returning payload without the carrier
header is not acceptable on Pico class hardware because it forces reject
evidence, route hints, and payload delivery to be correlated through a side
channel.

## 13. Receive Frame Peek

```rust
fn peek_recv_frame<'a>(
    &self,
    rx: &mut Self::Rx<'a>,
) -> Option<FrameHeader> {
    None
}
```

`peek_recv_frame` is optional and non-consuming.

```text
peek_recv_frame:
  returns the carrier-observed header for the same staged frame
  that a later poll_recv on the same Rx handle can return
  does not consume payload bytes
  does not requeue carrier state
  does not commit protocol progress
  is not route authority
```

Route hinting is just `FrameHeader.label` projected from the staged receive
frame. There is no separate `Hint` event and no receive-observation side
channel.

## 14. Receive Frame Lifecycle

```text
adapter:
  receives or stages one carrier frame
  can expose its FrameHeader through peek_recv_frame
  later returns the same FrameHeader + Payload through poll_recv

hibana runtime:
  compares Incoming.header with expected session/lane/role/label context

if match:
  descriptor/payload checks continue
  endpoint commit emits normal ENDPOINT_RECV evidence

if mismatch:
  emit TransportReject Evidence
  do not deliver payload to the app
  endpoint operation fails closed
```

Constraints:

```text
peek_recv_frame must point only to the staged frame
peek_recv_frame must not consume payload
peek_recv_frame must not commit progress
poll_recv must return Incoming for that same staged frame
route hint and reject evidence derive from the same FrameHeader
```

This keeps reject evidence aligned with payload delivery.

## 15. Reject

The adapter does not return Reject.

```text
adapter:
  returns Incoming { header, payload }

hibana runtime:
  compares expected context with Incoming.header

mismatch:
  emits TransportReject Evidence
```

Session and descriptor authority remain in hibana runtime.

## 16. Wait

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

`Capacity` remains as a diagnostic reason for ring overflow, queue full, or buffer
exhaustion. It does not add a new event kind; it is a `TransportFault` reason.

## 17. Transport Evidence

hibana runtime emits only three transport evidence kinds:

```text
TransportHint
TransportReject
TransportFault
```

Do not add:

```text
TransportParsed
TransportAccepted
TransportOpen
TransportRequeue
TransportWaitObservation
```

Reasons:

```text
Parsed:
  Incoming.header is checked against expected context. A standalone event is not needed.

Accepted:
  ENDPOINT_RECV / ENDPOINT_CONTROL commit is the evidence of acceptance.

Open:
  derivable from PortOpen / rendezvous / lane lifecycle.

Requeue:
  runtime action, not adapter observation.

Wait:
  diagnostic evidence derived from TransportError.
```

## 18. Evidence ABI

`TapEvent` is the physical record. `Evidence` is the semantic decode.

```rust
impl TapEvent {
    pub const fn evidence(self) -> Evidence;
}
```

`Evidence` stays in the integration observe surface:

```rust
hibana::integration::observe::Evidence
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

## 19. Image Header

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
  observation-state mutation forbidden
```

## 20. ReplayRecord

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

## 21. Remote Paths

Only two remote paths are allowed.

```text
TapEvent uplink:
  device -> host
  debug / replay

image ingress:
  manager / host / OTA -> device
  epf.load / unload / revert
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

## 22. Live Update

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

## 23. Acceptance Criteria

EPF:

```text
public runtime object is Epf only
EPF is VM appliance, not hook runtime
Pico class is the contract, not a profile
RP2040/SIO observe-only marker proof is the first acceptance gate
no_std / no_alloc / fixed storage by default
EpfStorage has fixed policy slots, image bytes, scratch bytes, and history capacity
EpfStorage<0, 256, 32, 4> is a valid observe-only starting point
Epf is a capacity-erased borrowed appliance view over typed fixed EpfStorage
Target::Policy(ID) is logical; physical storage is a fixed slot table
load(Target::Policy(ID)) returns Err(Capacity) when no policy slot is available
Out is a compact device record, not a formatted report
Out has target-specific interpretation without adding DecisionOut
Target::Policy(ID) interprets Out.kind/reason/arg0 as decision encoding
device code writes Out to RAM markers or compact uplink
string rendering is host/debug tooling only
debug has host-run and device-run paths; RP2040/SIO proof uses device-run
run(TapEvent) is direct observe/debug/replay only
policy VM runs only through resolver::<ID>(fallback)
EPF VM is not Wasm and not eBPF-shaped
EPF does not run inside a WASI P1 guest
EPF does not run in SIO FIFO push/pop, transport poll critical sections, or ISR
load/revert/unload are atomic with active unchanged on failure
observe VM trap reports Out and does not roll back canonical evidence state
policy VM trap fails closed and never silently falls back
```

hibana:

```text
app-facing API does not grow for EPF
hibana core does not know EPF images or VM
no EpfResolver
no ResolverRef::epf
no tap sink
no remote decision API
Evidence ABI is hibana integration authority
```

Transport:

```text
poll_recv returns Incoming { FrameHeader, Payload }
peek_recv_frame optionally exposes the same staged FrameHeader without consuming payload
there is no receive-observation side channel
route hinting derives from FrameHeader.label
adapter never returns Reject
WaitObservation does not exist
TransportError includes Offline, Deadline, Capacity, Failed
transport evidence kinds are TransportHint, TransportReject, TransportFault only
SIO carrier exposes observed frame metadata but does not decide mismatch authority
SIO FIFO word path remains carrier-only and does not invoke EPF
```

RP2040/SIO:

```text
core1 may host a WASI P1 engine, but EPF runs in native host/runtime side
WASI P1 guest is a diagnosis subject, not the EPF owner
observe-only must explain deliberate SIO session/lane mismatch through RAM markers
observe-only must explain SIO capacity pressure through TransportFault/Capacity markers
policy hot-swap is second-stage and cannot enlarge the first implementation
```

Remote:

```text
TapEvent uplink allowed
image ingress allowed
hidden remote decision forbidden
remote influence must be modeled as choreography facts
```

## 24. Final Usage

RP2040/SIO observe-only debug:

```rust
static mut EPF_STORAGE: EpfStorage<0, 256, 32, 4> = EpfStorage::new();

let epf = Epf::new(unsafe {
    &mut *core::ptr::addr_of_mut!(EPF_STORAGE)
});
```

Debug loop:

```rust
let mut tap = rv.tap();

if let Some(event) = tap.next() {
    let out = epf.run(event);

    ram_markers.epf_epoch = out.epoch;
    ram_markers.epf_kind = out.kind as u32;
    ram_markers.epf_reason = out.reason as u32;
    ram_markers.epf_arg0 = out.arg0;
    ram_markers.epf_arg1 = out.arg1;
    ram_markers.epf_arg2 = out.arg2;
    ram_markers.epf_fuel_used = out.fuel_used as u32;
}
```

Second-stage policy:

```rust
static mut EPF_POLICY_STORAGE: EpfStorage<1, 512, 64, 4> = EpfStorage::new();

let epf = Epf::new(unsafe {
    &mut *core::ptr::addr_of_mut!(EPF_POLICY_STORAGE)
});
```

```rust
let fallback7 = ResolverRef::<7>::decision_state(&state, choose);

rv.role(&role0)
    .set_resolver::<7>(epf.resolver::<7>(fallback7))?;

epf.load(&policy7_image)?;              // Policy(7) loaded: fallback ignored
epf.unload(Target::Policy(7))?;         // fallback restored
epf.revert(Target::Policy(7))?;         // previous Policy(7) VM restored
```

Host debug:

```bash
hibana-epf live --serial /dev/ttyACM0
```

Final shape:

```text
EPF に流す。
VM が走る。
Out を見る。
```

Only `Epf` is the public EPF runtime object.
