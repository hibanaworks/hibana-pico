# plan.md -- Hibana/Pico Final Plan
This document is normative.
If a concept is not in this document, it is not part of Hibana/Pico.
Hibana/Pico is not designed by adding runtime intelligence.
Hibana/Pico is designed by deleting every runtime decision that hibana
choreography can express.
## 0. Definition
**Hibana/Pico** is a choreographic WASI Preview 1 microkernel swarm OS for
Raspberry Pi Pico-class boards.
It must run on:
```text
Raspberry Pi Pico      / RP2040
Raspberry Pi Pico W    / RP2040 + CYW43439
Raspberry Pi Pico 2 W  / RP2350 + CYW43439

It is:

a no_std / no_alloc WASI P1 syscall-to-choreography OS
a node-local microkernel capsule
a swarm OS substrate for typed object authority
a recoverable fail-safe lifecycle system

It is not:

a general Wasm runtime
a POSIX OS
a network stack
a WASI Preview 2 runtime
a bridge layer
a relay layer
a runtime recovery manager

The only protocol specification source is:

hibana choreography

The only control vocabulary is hibana control vocabulary:

RouteDecision
LoopContinue
LoopBreak
StateSnapshot
StateRestore
TopologyBegin
TopologyAck
TopologyCommit
CapDelegate
AbortBegin
AbortAck
Fence
TxCommit
TxAbort

Long-lived OS authority is Many-shot.

Concrete operation authority is One-shot.

1. Hibana-First Law

If a fact can be expressed by hibana, it must be expressed by hibana.

Hibana owns:

global choreography
role projection
legal message order
label legality
RouteDecision
LoopContinue
LoopBreak
StateSnapshot
StateRestore
TopologyBegin
TopologyAck
TopologyCommit
CapDelegate
AbortBegin
AbortAck
Fence
TxCommit
TxAbort
affine control tokens
capability control messages
endpoint decode
localside progression

Hibana/Pico must not reimplement these as:

Rust state machines
runtime protocol inference
manual phase flags
transport heuristics
fallback loops
shape-based request dispatch
stringly route selection
fd-number route selection
board-specific protocol branches
runtime recovery managers
runtime topology managers
runtime transaction managers

The allowed localside vocabulary is:

flow().send()
recv()
offer()
decode()

If code outside hibana needs to know "what phase is legal next," the design is
wrong.

2. Pico Responsibility Law

Hibana/Pico owns only what hibana cannot own:

WASI P1 import trampoline
bounded Wasm execution capacity
GuestLedger fact storage
memory lease table
ChoreoFS object storage
errno mapping
pending syscall token table
resolver readiness facts
MMIO / GPIO / timer / UART
CYW43439 reset / IRQ / gSPI / Wi-Fi byte movement
transport byte framing
firmware measurement gates
endpoint-free hard stop

None of these may become protocol authority.

They are implementation capacity behind hibana-projected roles.

3. NodeCapsule

A node is:

NodeCapsule =
  WASI P1 guest
  + Engine
  + GuestLedger
  + Kernel
  + ChoreoFS
  + Resolver
  + TransportPort

Each node owns its own:

Wasm memory
fd materialized view
memory lease table
pending syscall table
object store
resolver queue
endpoint state
transport port
lifecycle authority view

Swarm nodes never share:

memory
fd tables
leases
pending tokens
endpoint state
kernel state
object stores

Remote communication is always:

Kernel_i <-> Kernel_j

over hibana messages carried as transport bytes.

4. Microkernel Swarm OS Law

Hibana/Pico is a WASI P1 microkernel swarm OS.

The guest sees only:

fd
ptr
len
errno

The Kernel owns no protocol legality. The Kernel materializes and checks facts
required by the currently projected hibana phase.

The OS-level authority model is:

ActivationAuthority<Many>
ObjectAuthority<Many>
RouteAuthority<Many>
TopologyAuthority<Many>
TransactionAuthority<Many>

Concrete actions are One-shot:

Activation<One>
ObjectGrant<One>
RouteInstall<One>
TopologyCommit<One>
ObjectTx<One>
AbortTx<One>

Many is long-lived OS authority.

One is a concrete affine action.

Many never continues an old activation.

Many only authorizes fresh One-shot actions.

Hibana/Pico does not define separate runtime concepts named route replacement,
object piping, or recovery managers.

Hibana/Pico uses hibana control operations:

TopologyBegin / TopologyAck / TopologyCommit
Fence
RouteDecision
CapDelegate
TxCommit / TxAbort
AbortBegin / AbortAck
StateSnapshot / StateRestore
LoopContinue / LoopBreak

5. Runtime Shape

A local syscall has this shape:

ordinary WASI P1 app
  -> WASI P1 import trampoline
  -> Engine role
  -> hibana-projected Kernel role
  -> explicit object route
  -> local object / device / resolver fact
  -> typed return / typed reject / ENOSYS / trap

A remote object syscall has this shape:

ordinary WASI P1 app on node A
  -> Engine_A
  -> Kernel_A
  -> hibana message over byte transport
  -> Kernel_B
  -> object route on node B
  -> Kernel_B
  -> hibana message over byte transport
  -> Kernel_A
  -> Engine_A

No remote node receives authority over local Wasm memory.

Remote data is copied bytes under typed grants, generations, and choreography.

6. Authority Law

Runtime progress is legal only when all required facts intersect:

projected phase accepts label
  + payload decodes
  + control-message history materializes fd view
  + fd view names live object identity
  + object generation matches
  + route generation matches
  + explicit RouteDecision exists when a route is selected
  + memory lease matches when ptr/len is used
  + pending token matches when async completion is used
  + resolver fact exists when readiness is required
  + policy admits operation
  + linked implementation capacity exists
  => progress

Any missing fact gives:

typed reject
ENOSYS
trap
drop-by-policy

Not authority:

fd number
path string
network address
transport packet
label hint
lane
interrupt
core id
board type
Cargo feature
guest wrapper

Only hibana choreography decides protocol legality.

Kernel does not decide what is legal.
Kernel only checks whether the facts required by the currently projected hibana
phase are present.

7. WASI P1 Law

The guest may be:

Rust std
no_std Rust
C
Zig
TinyGo
handwritten Wasm

The choreography does not care.

The Engine sees only:

WASI P1 import -> EngineReq
EngineRet -> WASI result / errno / trap

Disabled imports fail closed.

Unsupported imports fail closed.

No import may:

fake success
choose a fallback route
bypass GuestLedger
bypass memory lease checks
bypass projected choreography

WASI Preview 2, WIT, Component Model, and P2 sockets/resources/streams are not
part of the runtime.

8. Syscall Stream Law

The syscall stream is guarded by hibana.

Example:

MemBorrowRead
  -> MemGrant
  -> WasiFdWrite
  -> WasiFdWriteRet
  -> MemRelease

The Engine may emit a syscall request only when the projected Engine role allows
that label.

The Kernel may answer only when the projected Kernel role allows that response.

Bad syscall order is not recovered.

Bad syscall order is rejected.

9. Memory Law

Choreography guards memory protocol order.

Leases guard memory authority.

A memory lease contains:

ptr
len
rights
memory generation
lease id

Pointer-backed syscalls require a matching lease.

memory.grow creates a fence.

After a fence:

old leases reject
old pending memory completions reject
new access must borrow again

Choreography does not inspect memory contents.

The lease table authorizes host access to guest memory.

Remote nodes never receive pointers.

10. GuestLedger Law

GuestLedger is app-local fact storage only.

It owns:

fd materialized view
memory lease table
pending syscall token table
quota
errno map
optional preopen manifest view

It does not own:

protocol order
RouteDecision
TopologyBegin
TopologyAck
TopologyCommit
CapDelegate
TxCommit
TxAbort
AbortBegin
AbortAck
Fence
filesystem authority
network authority
device authority
transport authority
scheduler policy
retry policy

A pending syscall token is linear.

Completion must match:

token id
token generation
syscall kind
fd
fd generation
lease id
lease generation
object generation
route generation
expected length / event / tick

A stale token is not recoverable.

11. ChoreoFS Law

ChoreoFS is a bounded resource identity store.

It is not POSIX.

Authority chain:

path string
  -> selector
  -> manifest entry
  -> object identity
  -> object generation
  -> explicit RouteDecision
  -> fd materialized view

Allowed object kinds:

StaticBlob
ConfigCell
AppendLog
ImageSlot
StateSnapshot
DirectoryView
GpioDevice
TimerDevice
UartDevice
NetworkDatagram
NetworkStream
NetworkListener
RemoteObject
ManagementObject
TelemetryObject

Forbidden:

ambient host filesystem passthrough
cwd authority
inode authority
implicit POSIX mutation
hidden socket authority
unbounded path buffers
unbounded iovec buffers
unbounded dirent buffers

12. Network / FD Law

Network is Kernel object routing.

Network is not WASI P2.
Network is not socket authority.
Network is not transport authority.

WASI P1 apps reach network objects through fd-visible imports:

sock_send     -> fd_write-like NetworkDatagram / NetworkStream route
sock_recv     -> fd_read-like NetworkDatagram / NetworkStream route
sock_shutdown -> fd_close / quiesce route
sock_accept   -> explicit NetworkListener accept route

The app sees:

fd
ptr
len
errno

The Kernel sees:

fd materialized view
object identity
object generation
route generation
RouteDecision
policy
lease
pending token
projected phase

The transport sees:

bytes

No network operation may progress from fd number alone.

No socket import may invent route authority.

No transport packet may become syscall authority.

13. RouteDecision Law

Every semantic route choice is a hibana RouteDecision.

Route selection must be represented as:

RouteDecision
or hibana-projected control message that carries an explicit route arm

Route selection must not be represented as:

if fd == ...
if path starts_with ...
if payload looks like ...
if remote address == ...
if ALPN == ...
if lane == ...
if board == ...

Remote object routing is:

fd materialized view
  -> object identity
  -> object generation
  -> route generation
  -> RouteDecision
  -> Kernel_i <-> Kernel_j

There is no bridge.

There is no relay.

There is no fd.is_remote semantic bypass.

14. Topology Transaction Law

Topology changes are hibana transactions.

Topology changes use:

TopologyBegin
TopologyAck
TopologyCommit
Fence
RouteDecision
CapDelegate when authority moves

A topology change may replace route generation, object generation, node
membership, or authority placement.

A topology change must not be:

hidden fallback
send-failed-then-use-another-route
transport reconnect as semantics
runtime heuristic

Old route/object authority is invalidated by:

Fence
generation mismatch

A guest fd number may remain stable.

The fd materialized view generation must change when authority changes.

Old pending operations reject.

New operations resolve against the current generation.

15. CapDelegate Law

CapDelegate is lower-layer authority movement.

It is not a guest API.

It is not a public WASI surface.

It is not a runtime recovery manager.

Guest-visible result is fd or object materialization, not exposure of raw
capability tokens.

CapDelegate may move:

object authority
route authority
topology authority
transaction authority
activation authority

CapDelegate must obey hibana lower-layer endpoint token rules.

Hibana/Pico must not invent:

ReAdmissionAllowed
RestartPermit
guest-visible capability object
app-level CapDelegate shim

Fresh One-shot authority is obtained through hibana lower-layer capability
delegation/minting from Many-shot authority.

16. Object Transaction Law

Object-to-object transfer, object update, log append, management install,
network object movement, and remote object exchange are typed object
transactions.

They use:

TxCommit
TxAbort
RouteDecision when a branch is selected
CapDelegate when authority moves
Fence when old authority must become stale

They are not:

bridge
relay
shared memory
transport forwarding
fd.is_remote dispatch
hidden retry
hidden fallback

A transaction requires:

source authority when a source exists
sink authority when a sink exists
bounded buffer
explicit choreography
TxCommit or TxAbort

Remote nodes never receive local pointers.

Transaction payloads are copied, framed, bounded bytes.

17. Loop Law

Normal loops use only:

LoopContinue
LoopBreak

Abort is not LoopBreak.

Abort is not a third loop arm.

The correct shape is:

Abort | Normal

and, inside Normal when a loop exists:

LoopContinue | LoopBreak

The following is forbidden:

Continue | Break | Abort as one loop decision
Abort as LoopBreak
abortable_loop
terminal_loop
LoopBreak used as fault terminal

Use ordinary binary hibana routes.

Example shape:

route(
  AbortBegin -> Fence -> SAFE -> AbortAck,
  Normal
)

If Normal contains a loop:

route(
  AbortBegin -> Fence -> SAFE -> AbortAck,
  route(
    LoopContinue -> Body,
    LoopBreak    -> NormalEnd
  )
)

18. Recoverable Fail-Safe Law

A running activation is One-shot.

Each activation is authorized by:

Activation<One>

A lifecycle owner may hold:

ActivationAuthority<Many>

Many does not continue an old session.

Many only authorizes fresh One-shot activations.

Guest-observed failure is choreography:

AbortBegin
Fence
SAFE
AbortAck

SAFE is the application-specific safe terminal choreography fragment.

SAFE is not:

a runtime manager
a board-specific global rule
a new hibana control op
a hidden generic safety policy

Abort is not LoopBreak.

Fence invalidates the current activation's facts by generation:

fd views
leases
pending tokens
object views
route views
async completions

Fence is not a cleanup loop.

The runtime must not perform:

for each fd close
for each lease revoke
for each pending clear

as a recovery manager.

After normal end or AbortAck, the activation is dead.

Recoverable fail-safe is fresh re-entry:

ActivationAuthority<Many>
  -> Activation<One>
  -> new generation
  -> new activation

Same-session recovery is forbidden.

19. StateSnapshot / StateRestore Law

Fresh restart does not require state restore.

State recovery exists only when explicitly choreographed.

State recovery uses:

StateSnapshot
StateRestore
Fence
ActivationAuthority<Many>
Activation<One>

State restore must not resurrect stale authority.

A restored activation still receives:

new generation
new fd materialized view
new lease epoch
new pending table
new object view generation

StateRestore is not:

same-session continuation
rollback heuristic
panic recovery
transport retry

20. Phase Invariant Law

Phase legality is owned by hibana projection.

Runtime code must not maintain independent phase state.

Runtime phase invariant violation is not protocol.

It is an implementation bug.

Host behavior:

test failure

Firmware behavior:

endpoint-free hard stop

Phase invariant violation must not become:

EngineAbort
LoopBreak
RouteDecision
retry
fallback
fake success

Development diagnostics may record fixed markers, but markers are observability
only and must not select routes, retries, fallbacks, or policies.

21. Hard Panic Law

Hard panic is not choreography.

Hard panic is:

panic_handler
machine invariant violation
endpoint corruption suspicion
fatal firmware fault

Hard panic must not call:

flow()
recv()
offer()
decode()
CapDelegate
Fence
AbortAck
transport send
format panic string
alloc

Hard panic may:

apply machine-local safe stop
write fixed failure marker
park
watchdog reset

Hard panic is endpoint-free.

This is not failure to use hibana.

This is respecting the boundary where hibana endpoint state can no longer be
trusted.

22. Resolver Law

Interrupts and readiness are evidence only.

ISR may:

clear hardware flag
capture bounded metadata
enqueue raw readiness
wake executor
return quickly

ISR must not:

call Endpoint methods
decode payloads
inspect fd authority
inspect leases
select routes
allocate
block

Resolver converts raw readiness into typed facts:

TimerSleepDone
GpioWaitSatisfied
TransportRxReady
TransportTxReady
BudgetExpired
LeaseFenceDue
NodeHealthChanged
CywReady

A resolver fact admits progress only when hibana-projected phase is open.

Resolver is not protocol authority.

23. Transport Law

Transport carries bytes only.

A swarm frame may contain:

source node
destination node
session id
session generation
lane
label hint
sequence
payload bytes
auth tag if enabled

Transport hints are not authority:

lane
label hint
source address
destination address
packet order
retry count

Payload authority begins only after endpoint decode.

Valid substrates:

RP2040 SIO FIFO
RP2350 local substrate
CYW43439 Wi-Fi byte transport
QEMU UDP mesh for proof
host queue for tests

Substrate is not semantics.

24. Raspberry Pi Pico Law

Raspberry Pi Pico is the smallest non-wireless target.

It must support:

RP2040
thumbv6m-none-eabi
no_std
no_alloc
dual-core role execution
SIO FIFO byte movement
GPIO device routes
timer resolver facts
UART debug sink
bounded WASI P1 guest execution
fd_write / poll_oneoff / proc_exit minimal profile
memory lease checks
memory.grow fence
bad syscall order fail-closed path
AbortBegin / Fence / SAFE / AbortAck proof

It must not require:

Wi-Fi
CYW43439
RP2350 capacity
heap allocation
host filesystem
ordinary host std capacity

25. Raspberry Pi Pico W Law

Raspberry Pi Pico W is the minimum physical wireless swarm target.

It must support:

RP2040 + CYW43439
thumbv6m-none-eabi
no_std
no_alloc
dual-core role execution
SIO FIFO local byte movement
GPIO device routes
timer resolver facts
UART debug sink
CYW43439 reset / IRQ / gSPI bring-up
CYW43439 firmware readiness
Wi-Fi byte transport
SwarmFrame exchange
remote object routing through fd materialized views
NetworkDatagram / NetworkStream / NetworkListener object routing
TopologyBegin / TopologyAck / TopologyCommit proof
TxCommit / TxAbort proof
bounded WASI P1 guest execution
memory lease checks
memory.grow fence
bad syscall order fail-closed path
recoverable fail-safe fresh activation proof

It must not require:

RP2350-only capacity
Pico 2 W-only assumptions
host UDP mesh
shared memory between nodes
WASI Preview 2
Component Model
P2 sockets/resources/streams
hidden relay
bridge object
heap allocation
unbounded packet buffers

Pico W is stricter than Pico 2 W.

If a wireless design cannot fit Pico W-class RP2040 capacity, it may be a
Pico 2 W extension, but it is not the minimum wireless Hibana/Pico design.

26. Raspberry Pi Pico 2 W Law

Raspberry Pi Pico 2 W is the higher-capacity wireless swarm target.

It must support:

RP2350 + CYW43439
thumbv8m.main-none-eabi
no_std
no_alloc
CYW43439 reset / IRQ / gSPI bring-up
CYW43439 firmware readiness
Wi-Fi byte transport
SwarmFrame exchange
remote object routing
network object routing
management object routing
multi-node choreography
TopologyBegin / TopologyAck / TopologyCommit
TxCommit / TxAbort
StateSnapshot / StateRestore when explicitly enabled
recoverable fail-safe fresh activation

It may use RP2350 capacity.

It may not change Hibana/Pico semantics.

A behavior that succeeds only because Pico 2 W has more capacity must be named
as Pico 2 W capacity, not core choreography meaning.

27. CYW43439 Law

CYW43439 bring-up has a choreography-visible readiness prefix.

Required order:

PowerOn
ResetAssert
ResetRelease
ProbeOk
FirmwareChunk*
FirmwareCommit
ClmNvramApply
CywReady
TransportOpen

Failure is fail-closed through explicit choreography when the endpoint is valid,
or endpoint-free hard stop when the machine substrate is not trustworthy.

bad image hash -> reject
out-of-order chunk -> reject
ready before commit -> reject
transport before CywReady -> reject
CywFailed -> no Wi-Fi fallback

gSPI byte traffic is not hibana choreography.

It is machine implementation capacity.

Wi-Fi is transport.

Wi-Fi is not route authority.

28. Pico Capacity Law

Every Pico firmware path must be bounded.

Bounded resources:

role programs
endpoint slab
tap buffer
transport frame queue
swarm frame payload
fragment buffer
memory lease table
pending syscall table
fd view
ChoreoFS object table
directory entries
path selectors
iovec copies
management image chunks
resolver readiness queue
UART/debug buffer
Topology transaction table if enabled
object transaction table if enabled
state snapshot table if enabled

Forbidden on Pico firmware paths:

Vec
Box
Rc
Arc
String as runtime storage
dynamic task spawning
recursive parser
unbounded guest-controlled loops
unbounded import table allocation
unbounded path expansion
unbounded packet reassembly
unbounded transaction buffering

Host-only proof capacity may not define Pico semantics.

Host success is not Pico success.

29. Feature Law

Cargo features select implementation capacity only.

Features may:

link implementation bodies
remove implementation bodies
select board substrate
select engine coverage
select syscall handler coverage
select proof/demo artifact embedding
select optional StateSnapshot / StateRestore capacity
select optional topology transaction capacity
select optional object transaction capacity

Features may not:

change choreography meaning
change route labels
change role ids
make disabled syscalls succeed
introduce fallback semantics
introduce compatibility names

Profile is capacity.

Choreography is meaning.

Important profiles:

profile-rp2040-pico-min:
  RP2040 non-wireless minimal WASI P1 device profile
profile-rp2040-picow-swarm-min:
  RP2040 + CYW43439 minimum wireless swarm profile
profile-rp2350-pico2w-swarm-min:
  RP2350 + CYW43439 higher-capacity wireless swarm profile
profile-host-linux-wasip1-full:
  host proof profile for wider ordinary WASI P1 coverage

30. Spec Generation Law

Repeated protocol facts must have one source.

Generate from single internal specs:

labels
WASI P1 import coverage
handler availability
typed ENOSYS / typed reject disposition
RouteDecision arm ids
profile capability matrix
projection accessors
ControlOp coverage tests
CapShot One/Many coverage tests

Forbidden:

manual duplicate label tables
manual duplicate syscall tables
manual duplicate profile truth tables
manual duplicate route ids
manual duplicate control-op tables

One meaning gets one source.

31. Verification Law

A release-quality tree must prove:

No-P2 surface
No-WIT surface
No-Component-Model surface
No-bridge surface
ordinary wasm32-wasip1 artifact path
import trampoline -> EngineReq path
Engine -> Kernel hibana choreography path
fd materialized view checks
memory lease checks
memory.grow fence
pending syscall token checks
ChoreoFS object authority
NetworkObject routing without P2
RemoteObject routing without bridge
resolver readiness admission
swarm nodes do not share memory
transport label_hint is demux only
endpoint decode owns payload authority
management update requires Fence / quiesce / generation
RouteDecision is explicit
TopologyBegin / TopologyAck / TopologyCommit are explicit when topology changes
CapDelegate is lower-layer authority movement
TxCommit / TxAbort close object transactions
AbortBegin / Fence / SAFE / AbortAck provide soft fail-safe
ActivationAuthority<Many> produces fresh Activation<One>
same-session recovery is impossible
hard panic never calls Endpoint methods

Raspberry Pi Pico verification:

thumbv6m-none-eabi build
real RP2040 hardware run
fd_write path
poll_oneoff path
proc_exit path
bad-order fail-closed path
AbortBegin / Fence / SAFE / AbortAck hardware proof
firmware size measurement
SRAM measurement
stack high-water measurement

Raspberry Pi Pico W verification:

thumbv6m-none-eabi build
real RP2040 + CYW43439 bring-up
CywReady reached
transport cannot open before CywReady
two physical Pico W nodes exchange SwarmFrame bytes
network object route works through fd
remote object route works through fd
Topology transaction proof
object transaction proof
recoverable fail-safe fresh activation proof
firmware size / SRAM / stack measurements

Raspberry Pi Pico 2 W verification:

thumbv8m.main-none-eabi build
real RP2350 + CYW43439 bring-up
two physical Pico 2 W nodes exchange SwarmFrame bytes
three or more nodes run composed choreography
remote object / network object / management object phases pass
Topology transaction proof
object transaction proof
optional StateSnapshot / StateRestore proof
recoverable fail-safe fresh activation proof
firmware size / SRAM / stack measurements

QEMU proof is useful.

QEMU proof is not physical success.

32. Publication Law

Do not claim Raspberry Pi Pico support until RP2040 physical gates pass.

Do not claim Raspberry Pi Pico W support until RP2040 + CYW43439 physical gates
pass.

Do not claim Raspberry Pi Pico 2 W Wi-Fi swarm completion until RP2350 +
CYW43439 physical swarm gates pass.

Do not claim recoverable fail-safe until:

AbortBegin / Fence / SAFE / AbortAck passes
fresh Activation<One> from ActivationAuthority<Many> passes
same-session recovery is rejected
hard panic endpoint-free stop is verified

Do not claim production management security while using demo authentication.

Demo auth must be named demo auth.

33. Non-Goals

WASI Preview 2
WIT
Component Model
P2 sockets/resources/streams
full POSIX filesystem
ambient host filesystem
general-purpose OS scheduler
hard-real-time arbitrary Wasm execution
transport-level semantic routing
bridge object
relay bypass
automatic protocol recovery
heap-required Pico runtime
BLE as swarm backbone
unauthenticated production management
same-session recovery
runtime recovery manager
runtime topology manager
runtime transaction manager

34. Final Principle

Only hibana choreography decides protocol legality.

Only hibana projection defines local progress.

Only hibana control operations express control:

RouteDecision
LoopContinue
LoopBreak
StateSnapshot
StateRestore
TopologyBegin
TopologyAck
TopologyCommit
CapDelegate
AbortBegin
AbortAck
Fence
TxCommit
TxAbort

Only CapShot expresses shot discipline:

Many = long-lived authority
One  = concrete affine action

Only endpoint decode gives payload meaning.

Only control-message history materializes fd views.

Only leases authorize guest memory access.

Only pending tokens authorize async completion.

Only resolver facts admit readiness.

Only object generations keep resources live.

Only route generations keep routes live.

Only transport carries bytes.

Only bounded static capacity is valid on Pico firmware.

No P2.
No bridge.
No relay.
No hidden fallback.
No heap-required Pico path.
No runtime protocol intelligence outside hibana.
No same-session recovery.
No fail_closed from localside.

Only choreography on real Pico-class hardware.
