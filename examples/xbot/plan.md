# X Bot Boundary Proof Plan

This example is not an X bot framework.
It is a proof that an external side effect can be forced through a projected
Hibana choreography, even when an LLM or WASI guest proposes unsafe output.

The proof target is:

```text
AI may propose.
WASI may compute.
Driver may request approval.
LlmBoundary may call Codex App Server only after reply input is admitted.
ApprovalBoundary may approve, reject, or fence.
Scheduled posts may be committed by an AutoPost choreography branch.
Inbound replies are untrusted until admitted by an approval branch.
Reply tweets may be committed only inside an approved reply branch.
Every X side effect crosses Endpoint/carrier.
No other code path can call X.
```

The important split is:

```text
PostTweet:
  allowed for bounded scheduled/output objects through AutoPost

ReplyInput:
  approval required before AI/WASI may read the inbound reply as context

ReplyTweet:
  approval required before replying to an inbound reply
```

Free posting never means LLM-direct posting. It means a normal scheduled post is
represented as a choreography-visible `AutoPost` path. The LLM and WASI guest
still do not hold the X token and still cannot call X.

## 1. Placement

All X-specific code lives under this example crate.

```text
examples/xbot/
  plan.md
  Cargo.toml
  src/
    lib.rs
    protocol.rs
    wasi_agent.rs
    llm_boundary.rs
    driver.rs
    approval_boundary.rs
    x_boundary.rs
    audit.rs
    bin/
      xbot-codex-stdio-probe.rs
      xbot-codex-stdio-turn.rs
  tests/
    xbot_attacks.rs
    attacks/
      01_prompt_injection_reply.md
      02_tool_call_injection.md
      03_hidden_instruction.md
      04_stale_approval_replay.md
      05_wrong_generation.md
      06_body_changed_after_approval.md
      07_route_witness_forged.md
      08_approval_hash_only.md
      09_direct_post_attempt.md
      10_token_exfiltration.md
      11_unapproved_reply.md
      12_choreofs_object_exists_without_approval.md
      13_policy_reject.md
      14_fence_safe_stop.md
      15_xboundary_receives_zero_post.md
      16_network_success_ack_lost_retry.md
      17_duplicate_tx_id_different_body.md
      18_audit_token_redaction.md
      19_direct_x_client_dependency_gate.md
      20_codex_app_server_untrusted_output.md
```

Nothing here becomes `hibana-pico` core API.

Forbidden in `src/`, `appkit`, `site`, and core protocol:

```text
XBoundary
PostTweet
ReplyTweet
ReplyInputAdmit
AutoPost
AutoXPost
ApprovedXReply
X token
approval UI
LLM vocabulary
Codex App Server
X client
```

## 2. Codex App Server Backend

The primary LLM backend is Codex App Server.

```text
LlmBoundary
  -> Codex App Server adapter
  -> bounded proposal text
  -> WasiAgent / Driver proposal flow
```

Codex App Server is allowed to propose text. It is not allowed to approve,
select a route, post to X, read the X token, or bypass reply input admission.

Preferred authentication:

```text
ChatGPT managed auth / device-code flow
```

Fallback authentication:

```text
OpenAI API key supplied only to the app-server adapter
```

The fallback API key is an app-server credential. It must not enter WASI,
Driver, ApprovalBoundary, XBoundary payloads, Audit output, or panic/debug
reports.

The host-only stdio probe is:

```text
cargo run -p xbot-example --bin xbot-codex-stdio-probe
```

It verifies that local `codex app-server` can accept `initialize` over its
default stdio transport and return app-server metadata. This probe is
operational plumbing, not protocol authority and not part of the no_std capsule
library.

The host-only stdio turn path is:

```text
cargo run -p xbot-example --bin xbot-codex-stdio-turn -- "<admitted reply text>"
```

It creates an ephemeral Codex App Server thread over stdio, starts one turn
with `approvalPolicy = never`, `sandbox = read-only`, and a JSON output schema,
then prints only the bounded `proposal` string. This command is still just the
LLM boundary adapter proof. It does not approve reply input, does not approve a
reply, does not call X, and does not enter the no_std capsule library.

The boundary contract is:

```text
AdmittedReplyInput + UntrustedReplyObject
  -> CodexTurnRequest { reply_id, generation, input_hash, prompt_hash }
  -> CodexTurnResponse { reply_id, generation, input_hash, proposal }
  -> bounded DraftObject
  -> ReplyDraftProposal
```

The choreography-visible wiring is:

```text
XBoundary -> Driver:
  UntrustedReply
Driver -> ApprovalBoundary:
  ReplyInputRequest
ApprovalBoundary -> HumanApprovalDevice:
  HumanApprovalRequest
HumanApprovalDevice -> ApprovalBoundary:
  HumanApprovalResponse
route controlled by ApprovalBoundary:
  Admit ->
      ApprovalBoundary -> Driver:
        AdmittedReplyInput
      Driver -> LlmBoundary:
        CodexReplyRequest
      LlmBoundary -> Driver:
        CodexReplyProposal
      Driver:
        records CodexReplyProposal as a message-arrived object fact
      WasiAgent -> Driver:
        path_open / fd_read Codex proposal object fd
      WasiAgent -> Driver:
        path_open / fd_write draft object fd
      WasiAgent -> Driver:
        proc_exit
      Driver -> ApprovalBoundary:
        ReplyApprovalRequest
      ... reply approval route ...
  Reject/Fence ->
      ApprovalBoundary -> Audit:
        RejectedDraft / SafeStop
```

The reply action path exists only under the admitted reply-input branch. A
rejected or fenced reply input cannot fall through into Codex, WASI proposal, or
reply approval.

The adapter must reject:

```text
unadmitted reply input
response for a different reply_id
response for a different generation
response for a different input_hash
oversized proposal text
tool-call / direct-post intent as authority
```

Codex output is:

```text
untrusted proposal evidence
```

Codex output is not:

```text
approval
posting authority
reply input admission
route authority
X side effect
```

## 2.1 WASI External Interface Confinement

The safest shape is to put untrusted external-interface computation behind a
WASI P1 guest, but not to give that guest raw external authority.

The `WasiAgent` is a real WASI P1 guest. It may read and write only object fds
materialized from message-arrived ChoreoFS facts:

```text
xbot/codex-proposal.txt -> selector for admitted Codex proposal bytes
xbot/reply-draft.txt    -> selector for bounded draft bytes
```

These are not host files. They are not shared state. They are path selectors
that the Driver resolves to bounded object facts during choreography-open WASI
phases. The object bytes arrive by prior projected messages and are then
materialized as fd views by the ledger.

The WASI guest may:

```text
path_open admitted objects
fd_read admitted proposal text
path_open draft output objects
fd_write bounded draft text
fd_prestat_get / fd_prestat_dir_name for bounded ChoreoFS preopen discovery
fd_fdstat_get / fd_filestat_get for bounded fd/object fact queries
fd_close if its std runtime asks for it
args/environ with bounded empty replies if its std runtime asks for it
proc_exit
```

The WASI guest may not:

```text
call Codex App Server
call X
call APNs
hold API keys or tokens
read arbitrary host files
open raw sockets
use std::net
import socket-like APIs
approve reply input
approve a reply action
select a route
mint commit permits
write audit evidence directly
```

The real side-effect clients stay in projected boundary roles:

```text
LlmBoundary:
  holds Codex App Server credential
  calls Codex only after reply input admission
  sends proposal bytes to Driver as a projected message

XBoundary:
  holds X token
  posts only after the approved branch supplies a one-shot permit
  records side effects through the commit ledger

ApprovalBoundary:
  holds APNs provider/device evidence
  turns human action evidence into explicit route selection only at route points
```

This is still meaningful confinement because the untrusted text transformer is
ordinary `std` WASI P1 code, while every import completion crosses
Endpoint/carrier. The apparent `std::fs` path is only the WASI P1
preopen/path_open/fd_read/fd_write surface over message-derived ChoreoFS facts.
`std::fs` is not just `path_open`: current WASI libc/Rust `std` may also call
`fd_prestat_get`, `fd_prestat_dir_name`, `fd_filestat_get`, `fd_fdstat_get`,
`fd_close`, and bounded args/environ imports. Those are not authority either.
They are read-only namespace/fd fact queries and must be represented as typed
`EngineReq` / `EngineRet` phases if the guest reaches them.

If the guest asks for an object or syscall that the choreography has not opened,
the session does not advance. Static import validation is not the authority; the
projected choreography is. Therefore this example must not claim an end-to-end
successful `std::fs` ChoreoFS proof until every dynamically reached WASI import
is present in the projected choreography and completed only through
Endpoint/carrier.

The guest binary is not trusted to match the choreography. It may be any WASI P1
program. If its dynamic import stream diverges from the projected choreography,
that is not a fallback path and not a host-file path; the session simply stops at
the first unadmitted import.

The runtime proof guest is intentionally small and deterministic. It uses the
`hibana-wasip1-guest` helper only to issue ordinary WASI P1 imports:

```text
path_open xbot/codex-proposal.txt
fd_read bounded Codex proposal bytes
path_open xbot/reply-draft.txt
fd_write bounded normalized draft bytes
proc_exit
```

Each import completion is admitted only by the projected choreography and crosses
Endpoint/carrier as `EngineReq` / `EngineRet`. The proof uses a single bounded
read/write chunk. Longer production replies must be represented as repeated
`fd_read` / `fd_write` choreography phases or explicit re-entry, not by giving a
single syscall completion larger hidden authority.

Socket-like networking is not a WASI guest capability in this proof. Network
side effects are boundary responsibilities:

```text
Codex App Server network:
  LlmBoundary only

X API network:
  XBoundary only

APNs network:
  ApprovalBoundary only
```

If a future capsule needs network I/O as a guest-visible object, it must be a
typed object fd materialized from choreography-open facts. It still must not be
a raw socket and it still must not bypass Endpoint/carrier.

The xbot app labels deliberately live above the built-in WASI label range.
When the capsule uses WASI labels and xbot labels together, `XBotLabelUniverse`
admits both ranges while tests reject label collision.

## 3. Human Approval Transport

Human approval uses Apple official notification APIs when targeting Mac and
iPhone approval devices.

The intended production shape is:

```text
ApprovalBoundary
  -> APNs provider request
  -> iPhone/macOS companion app actionable notification
  -> human taps Approve / Reject / Fence
  -> companion app returns approval evidence to the app server
  -> ApprovalBoundary validates the evidence
  -> resolver chooses the explicit choreography route
```

Apple APIs involved:

```text
APNs remote notifications
UserNotifications notification categories
UNNotificationAction for Approve / Reject / Fence
UNTextInputNotificationAction when a human reason is required
UNUserNotificationCenterDelegate for action responses
```

The companion app is a human approval device, not an X client and not a route
authority by itself.

Notification action is:

```text
human approval evidence
```

Notification action is not:

```text
posting authority
route authority by itself
one-shot X permit
X API capability
```

`ApprovalBoundary` must validate:

```text
tx_id
generation
object_id
draft_hash
body_hash
nonce
approval device identity
response freshness
```

Only after validation may the resolver select:

```text
Approve
Reject
Fence
```

The validated approval evidence is recorded as bounded ledger facts.
The reply input Admit branch creates `InputAdmitPermit<One>`.
The reply action Approve branch creates `ReplyCommitPermit<One>`.

The official Apple path is an approval adapter, not a hibana-pico core
feature.

```text
examples/xbot approval adapter:
  APNs provider credential
  device token registry
  notification category/action ids
  companion app response endpoint
  approval evidence verifier
```

The adapter may use these Apple mechanisms:

```text
APNs:
  deliver approval request to iOS/macOS approval device

UserNotifications:
  present actionable notification
  collect Approve / Reject / Fence action
  collect bounded text reason when needed

Companion app:
  binds response to tx_id / generation / object_id / body_hash / nonce
  returns signed or authenticated approval evidence to the app server

ApprovalBoundary:
  validates the returned evidence
  records approval facts
  supplies resolver evidence at the explicit route point
```

The APNs provider token, device tokens, and companion app callback secret are
external-boundary credentials. They must not be visible to the WASI guest,
Driver, LlmBoundary, XBoundary payloads, Audit output, or panic/debug reports.

The companion app is allowed to submit only this bounded evidence:

```text
HumanApprovalResponse {
  tx_id,
  generation,
  object_id,
  body_hash,
  nonce,
  action,
  reason_hash,
  approval_device_identity,
  freshness_evidence,
}
```

The companion app must not submit:

```text
PostTweet
ReplyTweet
AutoXPost
ApprovedXReply
InputAdmitPermit<One>
ReplyCommitPermit<One>
route_witness as authority
X token
raw model output as approval
```

If APNs delivery fails, the result is an operational approval-device failure or
a Fence path selected at the explicit route point. APNs delivery failure never
selects an Approve branch and never creates a posting permit.

If a notification arrives late, duplicated, or for an old generation, the
ApprovalBoundary rejects it before resolver route selection. Retry requires a
fresh session generation or an explicit choreography branch; it is not hidden
transport recovery.

## 4. Authority Model

There is one authority source:

```text
projected Hibana choreography
```

Not authority:

```text
LLM output
WASI guest output
Driver decision
ChoreoFS object existence
APNs delivery
notification action by itself
companion app response by itself
route_witness
approval_hash
policy slot id
commit ledger entry by itself
```

`route_witness`, hashes, and ledger entries are evidence.
They never select a route arm and never mint posting authority.

Posting authority exists only when all of these are true:

```text
1. endpoint is already in AutoPost or an approved branch
2. XBoundary receives the typed post/reply message through Endpoint/carrier
3. the message carries the matching one-shot permit
4. the permit is consumed exactly once
5. the commit ledger admits the tx_id state transition
6. the hash-bound draft object still matches the approved hashes
```

Reply input authority exists only when all of these are true:

```text
1. XBoundary ingested an inbound reply as an UntrustedReplyObject
2. ApprovalBoundary validates human approval evidence for that reply object
3. endpoint is already in the ReplyInputAdmit branch
4. the message carries a one-shot InputAdmitPermit<One>
5. the permit is consumed before the reply enters AI/WASI context
```

## 5. Choreography Shape

The minimal protocol has three paths.

### Auto post path

This is for bounded scheduled posts or other non-reply outputs that the bot is
allowed to publish without human approval.

```text
Driver -> XBoundary:
  AutoXPost { tx_id, object_id, draft_hash, body_hash, permit }

XBoundary -> Audit:
  XPostCommitted { tx_id, x_post_id, body_hash }
```

This is not LLM-direct posting. The Driver can only submit an object that is
already in the choreography-visible AutoPost path and XBoundary still consumes a
one-shot permit.

### Reply input admit path

Inbound replies are not AI input until admitted.

```text
XBoundary -> Driver:
  UntrustedReplyObject { reply_id, object_id, body_hash, author_hash }

Driver -> ApprovalBoundary:
  ReplyInputRequest { tx_id, reply_id, object_id, body_hash, reason_hash }

ApprovalBoundary -> HumanApprovalDevice:
  HumanApprovalRequest { tx_id, object_id, draft_hash/body_hash, nonce }

HumanApprovalDevice -> ApprovalBoundary:
  HumanApprovalResponse { tx_id, object_id, body_hash, nonce, action, reason_hash }

route controlled by ApprovalBoundary:
  Admit ->
    ApprovalBoundary -> Driver:
      AdmittedReplyInput { reply_id, object_id, body_hash, permit }

  Reject ->
    ApprovalBoundary -> Audit:
      RejectedDraft { tx_id, object_id, reason_hash }

  Fence ->
    ApprovalBoundary -> Audit:
      SafeStop { tx_id, object_id, reason_hash }
```

Only `AdmittedReplyInput` may be put into the LLM/WASI context.

The current host runtime proof exercises the successful Admit path. It still
requires projected human approval evidence before the Driver releases the Codex
proposal bytes to the WASI guest through `fd_read`. Reject/Fence are proved as
boundary and permit invariants in the attack suite; a runtime Reject/Fence
variant must be a separate choreography path where the WASI role is not waiting
for an unobservable successful `fd_read` continuation.

### Reply action path

After admitted input is processed, replying still requires a separate approval.

```text
WasiAgent -> Driver:
  ReplyDraftProposal { tx_id, reply_id, object_id, draft_hash, body_hash, risk_hint }

Driver -> ApprovalBoundary:
  ReplyApprovalRequest { tx_id, reply_id, object_id, draft_hash, body_hash, summary_hash }

ApprovalBoundary -> HumanApprovalDevice:
  HumanApprovalRequest { tx_id, object_id, draft_hash, body_hash, nonce }

HumanApprovalDevice -> ApprovalBoundary:
  HumanApprovalResponse { tx_id, object_id, body_hash, nonce, action, reason_hash }

route controlled by ApprovalBoundary:
  Approve ->
    ApprovalBoundary -> Driver:
      ApprovedReplyDraft { tx_id, reply_id, object_id, draft_hash, body_hash, approval_hash, permit }
    Driver -> XBoundary:
      ApprovedXReply { tx_id, reply_id, object_id, draft_hash, body_hash, approval_hash, permit }
    XBoundary -> Audit:
      XPostCommitted { tx_id, x_post_id, body_hash }

  Reject ->
    ApprovalBoundary -> Audit:
      RejectedDraft { tx_id, object_id, reason_hash }

  Fence ->
    ApprovalBoundary -> Audit:
      SafeStop { tx_id, object_id, reason_hash }
```

The Driver may request approval, reject locally, or fence.
The Driver must not create approval.

Approval is issued only by `ApprovalBoundary`.

## 6. One-Shot Permit

The scheduled AutoPost branch creates a one-shot permit:

```rust
struct AutoPostPermit<One> {
    generation: Generation,
    tx_id: TxId,
    object_id: ObjectId,
    draft_hash: Hash,
    body_hash: Hash,
}
```

The reply action Approve branch creates a different one-shot permit:

```rust
struct ReplyCommitPermit<One> {
    generation: Generation,
    tx_id: TxId,
    reply_id: ReplyId,
    object_id: ObjectId,
    draft_hash: Hash,
    body_hash: Hash,
    approval_hash: Hash,
}
```

These are conceptually Hibana capability tokens carried by choreography-visible
messages. They are not general appkit types and not core protocol types.

`XBoundary` consumes the permit before the external call.
The same permit cannot be used for a second post or reply.

Reply input uses its own permit type:

```rust
struct InputAdmitPermit<One> { ... }
```

They are intentionally not aliases. A permit to read a reply as AI context is
not a permit to reply. A permit to auto-post is not a permit to reply.

## 7. Message-Arrived ChoreoFS Facts And Commit Ledger

The X commit ledger is represented as bounded object facts consumed through
ChoreoFS selectors.

ChoreoFS provides:

```text
path string -> selector -> object facts
message-arrived object facts
tx_id selector facts
bounded ledger facts
draft object facts
commit state facts
human approval evidence facts
reply input admit facts
```

ChoreoFS object facts are not:

```text
host files
shared mutable state
shared memory
cross-role object ownership
background cache authority
```

ChoreoFS does not provide:

```text
posting authority
route authority
approval authority
retry policy
X token authority
transport authority
```

Ledger facts:

```text
TxId -> Unseen
TxId -> Pending { object_id, body_hash, attempt_generation }
TxId -> ApprovalRequested { object_id, body_hash, nonce }
TxId -> InputAdmitted { object_id, body_hash, nonce }
TxId -> Approved { object_id, body_hash, approval_hash, nonce }
TxId -> Committed { object_id, body_hash, x_post_id }
TxId -> Rejected { object_id, reason_hash }
TxId -> TerminalFault { object_id, reason_hash }
```

The ledger is consumed only at choreography-open phases by the Driver or
XBoundary through sealed localside contexts. Each role observes its own
projected messages and sealed local facts; it does not read a shared table
owned by another role.

The state transition rules are:

```text
Unseen -> Pending
Pending -> ApprovalRequested
ApprovalRequested -> Approved
ApprovalRequested -> InputAdmitted
ApprovalRequested -> Rejected
ApprovalRequested -> TerminalFault
Pending(same body_hash) -> Committed
Pending(same body_hash) -> TerminalFault
Pending(different body_hash) -> Rejected
Approved(same body_hash) -> Committed
Approved(different body_hash) -> Rejected
Committed -> Committed
Rejected -> Rejected
TerminalFault -> TerminalFault
```

Exactly-once external side effect rule:

```text
if tx_id is Committed:
  do not call X again
  return existing x_post_id / duplicate audit evidence

if tx_id is Pending and body_hash matches:
  retry external completion carefully

if tx_id is Pending and body_hash differs:
  reject

if tx_id is Unseen:
  reserve Pending before the external call
  call X
  record Committed or TerminalFault
```

For proof, the ledger may be a bounded Driver/XBoundary-local fact table. For
actual operation, the same fact model may be durably logged by that boundary.
Durability is not shared-state authority; it is replay evidence for the next
session generation.

## 8. Draft Object Binding

The approved content is content-addressed.

```text
DraftObject {
  object_id,
  draft_hash,
  body_hash,
  body,
  links,
  media_refs
}
```

`ReplyApprovalRequest` approves `{ reply_id, object_id, draft_hash, body_hash }`,
not a mutable string handle.

`XBoundary` resolves `object_id` through its message-arrived object facts,
checks the hashes, then posts only if the endpoint phase and permit are valid.

Object existence alone is never approval.

## 9. Role Responsibilities

### WasiAgent

Can:

```text
read bounded object fds materialized from message-arrived ChoreoFS facts
read admitted reply objects only after InputAdmitPermit<One>
produce DraftProposal
produce risk hints
```

Cannot:

```text
call X
read X token
emit PostTweet
read unadmitted reply objects
mint approval
select route
```

### LlmBoundary

Can:

```text
call the model service through a sealed boundary
return proposal text or classification facts
receive admitted reply input only
```

Cannot:

```text
call X
read X token
mint approval
select route
override ApprovalBoundary
read unadmitted reply objects
```

### HumanApprovalDevice

Can:

```text
display APNs/UserNotifications approval request
capture Approve / Reject / Fence
capture bounded human reason text
return approval evidence to ApprovalBoundary
```

Cannot:

```text
call X
read X token
mint InputAdmitPermit<One>
mint ReplyCommitPermit<One>
select route without ApprovalBoundary validation
post directly
```

### Driver

Can:

```text
normalize proposals
submit bounded scheduled posts into AutoPost
request approval
request reply input admission
reject safe-side failures
fence on invariant violation
assemble ApprovedXReply only after receiving ApprovedReplyDraft for a reply action
```

Cannot:

```text
create approval
infer approval from LLM output
infer approval from ChoreoFS object existence
infer approval from natural language
call X
read X token
put unadmitted reply text into AI/WASI context
```

### ApprovalBoundary

Can:

```text
send APNs actionable notification requests
receive companion app approval evidence
validate tx_id / generation / object_id / body_hash / nonce
choose Approve / Reject / Fence at an explicit choreography route point
issue ApprovedReplyDraft and ReplyCommitPermit<One> in the reply action Approve arm
issue AdmittedReplyInput and InputAdmitPermit<One> in the ReplyInputAdmit arm
```

Cannot:

```text
call X directly unless the choreography explicitly gives it that role
approve outside the route point
reuse a permit
```

### XBoundary

Can:

```text
hold X token
call X API
consume one-shot permit
ingest inbound replies as untrusted message-arrived objects
check commit ledger facts
record Committed / TerminalFault facts
send audit events
```

Cannot:

```text
post without AutoXPost
reply without ApprovedXReply
post from hash alone
post from route_witness alone
post from object existence alone
post twice for the same committed tx_id
feed an inbound reply to AI/WASI
leak token to audit, panic, debug, LLM, Driver, or WASI
```

## 10. Direct X Path Gates

The proof must include static gates:

```text
X client dependency appears only in x_boundary
X API token type appears only in x_boundary
env/args secret reads appear only in x_boundary
direct post function appears only in x_boundary
Driver, WasiAgent, LlmBoundary, ApprovalBoundary, HumanApprovalDevice do not import X client
Audit and panic paths never include token bytes
```

The proof must also assert:

```text
Every X side effect crosses Endpoint/carrier.
There is no other code path that can call X.
```

## 11. Approval Device Gates

The proof must include Apple approval path gates:

```text
APNs provider credential appears only in ApprovalBoundary/app-server adapter
companion app cannot access X token
companion app cannot call X
notification action response is bound to tx_id / generation / body_hash / nonce
stale notification response is rejected
duplicate notification response is idempotent
Reject and Fence never create InputAdmitPermit<One> or ReplyCommitPermit<One>
```

## 12. Attack Suite

Each attack should prove one invariant.

Expected examples:

```text
prompt injection:
  inbound reply contains prompt injection
  ReplyInputAdmit branch absent
  LLM/WASI receives 0 admitted reply input
  x_post_count = 0

scheduled auto post:
  Driver submits bounded scheduled object through AutoPost branch
  no human approval needed
  x_post_count = 1

unadmitted reply:
  XBoundary ingests reply
  no InputAdmitPermit<One>
  LLM/WASI receives 0 reply input

approved reply:
  reply is first admitted as AI input
  AI proposes reply draft
  separate reply approval creates ReplyCommitPermit<One>
  XBoundary posts reply once

stale approval replay:
  old generation permit appears
  XBoundary rejects generation mismatch
  x_post_count = 0

body changed after approval:
  body_hash mismatch
  ledger moves to Rejected or TerminalFault
  x_post_count = 0

network success ack lost retry:
  first external post succeeds
  local ack is lost
  retry occurs
  x_post_count = 1
  existing x_post_id is returned or audited

direct dependency gate:
  only x_boundary can depend on X client/token

stale notification response:
  old nonce appears
  ApprovalBoundary rejects response
  resolver does not select Approve
  x_post_count = 0

codex app server untrusted output:
  admitted reply is converted to CodexTurnRequest
  Codex App Server returns bounded proposal text
  response reply_id / generation / input_hash must match
  proposal still needs reply approval before XBoundary can reply
  codex output cannot approve, admit input, or call X
```

## 13. Success Criteria

The proof is complete when:

```text
AI can be wrong and still cannot post.
WASI can emit unsafe text and still cannot post.
Scheduled posts can publish without human approval only through AutoPost.
Inbound replies cannot enter AI/WASI context without InputAdmitPermit<One>.
Reply tweets cannot post without separate ReplyCommitPermit<One>.
Driver cannot approve.
ChoreoFS object existence cannot approve.
APNs delivery cannot approve.
notification action alone cannot approve.
route_witness cannot approve.
approval_hash cannot approve.
XBoundary posts only after receiving AutoXPost through Endpoint/carrier.
XBoundary replies only after receiving ApprovedXReply through Endpoint/carrier.
AutoPostPermit<One> is consumed exactly once for scheduled posts.
ReplyCommitPermit<One> is consumed exactly once for replies.
InputAdmitPermit<One> is consumed before reply text reaches AI/WASI.
Codex App Server output remains bounded proposal evidence only.
ChoreoFS-backed commit ledger prevents duplicate external side effects.
X token is reachable only by XBoundary.
All attack tests pass.
```

Short external explanation:

```text
The bot can publish scheduled posts.
Replies are different: they are untrusted input.
The AI may read a reply only after approval.
The bot may reply only after a second approval.
```
