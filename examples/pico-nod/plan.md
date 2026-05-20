# Pico Nod by Hibana Plan

Pico Nod by Hibana is a human approval device and a minimal Hibana-wired server
for external actions.

```text
AI asks.
Pico Nod notifies you.
You nod, reject, or fence.
Only approved choreography can act.
```

The technical shape is:

```text
public bytes
  -> WASI P1 ingress
  -> candidate facts
  -> Hibana choreography
  -> human approval route
  -> one-shot commit permit
  -> CommitBoundary
```

Everything else is evidence.

## 1. Authority

There is one protocol authority.

```text
Hibana choreography.
```

Not authority:

```text
HTTP request
JSON shape
WASI guest output
APNs delivery result
app notification tap
LLM output
ChoreoFS object existence
signed ticket by itself
signed receipt by itself
external API response by itself
```

Forbidden:

```text
database as protocol state
shared memory as protocol state
hidden retry branch
timeout as approval
notification delivery as approval
direct external API call outside CommitBoundary
credential inside WASI guest
credential inside approval app
admin direct commit path
```

## 2. No Database

Pico Nod has no database.
Production also has no database.

Allowed state:

```text
portable signed capability tickets
portable signed receipts
bounded in-memory ChoreoFS facts
bounded in-memory commit facts
bounded in-memory audit ring
```

No database means:

```text
no immediate global revocation promise
no global mutable abuse counter
no durable server-side session table
no hidden persistence authority
```

Recovery after restart uses portable signed evidence supplied by the app, issuer,
or external service, then materializes it as bounded in-memory facts for a new
choreography instance.

## 3. Keys

Keys authenticate evidence.
Keys do not mint protocol progress.

```text
issuer key:
  signs issuer capability tickets

device key:
  signs human approval responses

receipt key:
  signs audit and outcome receipts

delivery key:
  signs APNs delivery capability tickets
```

Key rules:

```text
purpose-separated keys
fixed signature suite
kid on every signed object
short overlap window during rotation
old keys verify old unexpired evidence only
short TTL on capability tickets
small bounded clock skew
```

## 4. Roles

Each important role is a logical image and may be a separate process.
Co-location is only a proof convenience.

```text
Ingress:
  HTTP/TLS byte acceptor
  WASI P1 ingress guest

Router:
  maps admitted candidate facts into choreography messages

ApprovalBoundary:
  validates device-signed human evidence
  supplies resolver evidence at explicit route points

ApnsBoundary:
  owns APNs provider credential
  sends notification requests

CommitBoundary:
  owns one external action credential family
  performs side effects only after ApprovedIntent

AuditBoundary:
  emits signed receipts and bounded audit facts
```

No role may call another role directly.
Every cross-role step uses Endpoint/carrier.

## 5. WASI Ingress

Public ingress should be WASI P1 whenever practical.

```text
HTTP/TLS acceptor
  -> bounded request bytes
  -> WASI P1 ingress guest
  -> normalized candidate object
  -> Endpoint/carrier
  -> choreography
```

The HTTP/TLS acceptor can:

```text
terminate TLS
allowlist method/path
enforce header/body size limits
forward bounded bytes to WASI ingress
```

It cannot:

```text
hold credentials
contain business logic
contain approval logic
contain issuer policy logic
call external action clients
select routes
```

Acceptor compromise model:

```text
compromised acceptor may inject bounded bytes
compromised acceptor may drop requests
compromised acceptor may cause denial of service
compromised acceptor still cannot mint approval
compromised acceptor still cannot commit side effects
```

Where available, the acceptor runs as a separate process with only a listener and
one carrier to the WASI ingress role.

WASI guests can:

```text
parse bounded request bodies
normalize JSON/form payloads
read through fd_read
emit candidate facts through fd_write
compute summaries or risk hints as evidence
```

WASI guests cannot:

```text
own sockets
perform TLS
hold credentials
approve
select routes
call APNs
call external action APIs
write audit directly
mutate commit authority
```

If a WASI guest diverges from choreography, the session stops at the first
unadmitted import. Static import validation is not authority.

## 6. ChoreoFS

ChoreoFS is a bounded path/object fact resolver.

It may provide:

```text
issuer facts
device facts
intent body facts
approval nonce facts
approval evidence facts
commit facts
audit facts
external target facts
entitlement facts
rate-window ticket facts
```

It never provides:

```text
approval authority
route authority
external execution authority
APNs authority
retry policy
host filesystem fallback
shared mutable authority
```

Exact split:

```text
ChoreoFS:
  selector -> bounded object facts

Ledger/materialized view:
  facts -> fd/session/permit view

Choreography:
  legal order, route decision, phase authority
```

Facts are consumed only at choreography-open phases.

## 7. Intent And Approval

Intent is proposal evidence.

```text
IntentRequest {
  issuer_id
  workspace_id
  tx_id
  action_kind
  object_id
  body_hash
  summary_hash
}
```

Approval is device-signed human evidence.

```text
ApprovalEvidence {
  tx_id
  generation
  workspace_id
  object_id
  body_hash
  summary_hash
  nonce
  device_id
  action: Nod | Reject | Fence
  displayed_version
  displayed_hash
  signature
}
```

The app signs exactly what it displayed.
If the UI truncates, summarizes, redacts, or paginates an intent, that displayed
view must have its own `displayed_hash`.
Approval of unseen content is invalid.

The minimal route:

```text
IntentIssuerIngress -> IntentRouter:
  IntentRequest

IntentRouter -> ApprovalBoundary:
  ApprovalRequest

ApprovalBoundary -> ApnsBoundary:
  NotifyApprovalDevice

ApprovalIngress -> ApprovalBoundary:
  ApprovalEvidence

explicit route point:
  Nod ->
    ApprovalBoundary -> CommitBoundary:
      ApprovedIntent { tx_id, object_id, body_hash, permit: IntentCommitPermit<One> }

  Reject ->
    ApprovalBoundary -> AuditBoundary:
      IntentRejected

  Fence ->
    ApprovalBoundary -> AuditBoundary:
      IntentFenced
```

`Nod` is a product word.
The protocol meaning is an approved route arm.

## 8. APNs And App

APNs is delivery evidence, not approval.

APNs success means only:

```text
APNs accepted the notification request.
```

It does not mean:

```text
user saw it
user approved it
route selected
commit allowed
```

APNs payloads must be bounded and must not contain secrets, full prompts,
external API tokens, raw issuer credentials, or private audit data.

Raw APNs tokens are boundary-local secrets.
The server should receive delivery capability evidence, not treat raw tokens as
durable protocol state.

```text
DeviceDeliveryCap {
  user_id
  workspace_id
  device_id
  apns_token_hash
  topic
  expires_at
  kid
  signature
}
```

The app can display evidence and submit signed Nod / Reject / Fence evidence.
The app cannot call external action APIs, hold external action tokens, or select
routes without server-side choreography.

Device compromise model:

```text
compromised device key can forge that device's human evidence
compromised device cannot bypass choreography
compromised device cannot reach CommitBoundary directly
future device tickets may be refused
live sessions may be fenced
quorum choreography may reduce single-device risk
```

Device compromise is a root-trust loss for that device, not a protocol recovery
branch.

## 9. CommitBoundary

CommitBoundary is the only side-effect boundary.

It must check:

```text
current endpoint phase is approved
permit is one-shot
generation is current
object hash matches approval-bound hash
tx_id is not already committed in current in-memory commit facts
action kind matches issuer policy
destination is allowlisted
```

External side effects are not perfectly transactional.
Without a database, exactly-once is bounded by external idempotency evidence.

```text
reserve Pending before external call
use external idempotency key when available
emit signed OutcomeReceipt on success
on lost ACK, require receipt replay or external idempotency evidence
if no idempotency evidence exists, Fence
```

No blind retry.
No retry route without explicit choreography.

Commit outcome contract:

```text
idempotency evidence exists:
  retry may be represented by explicit choreography

idempotency evidence missing:
  no retry route exists
  result is Fence / manual reconciliation
```

Pico Nod does not promise exactly-once for external services that cannot provide
idempotency evidence.

## 10. Billing

Billing is service access evidence.
Billing is not approval.

BillingBoundary may verify:

```text
StoreKit transaction evidence
App Store Server JWS evidence
App Store Server Notifications V2
```

It emits bounded entitlement facts:

```text
EntitlementActive
EntitlementGrace
EntitlementExpired
EntitlementRevoked
EntitlementUnknown
```

`EntitlementUnknown` fails closed for paid-only service features.
No entitlement state approves or commits an external action.

## 11. Expiry, Deadline, And Fault

Two meanings must stay separate.

```text
protocol-visible expiry:
  Timer role
  explicit Expired/Fence route
  typed continuation exists

operational deadline:
  wait-site fuse
  session generation poisoned
  no continuation
```

Never implement `approval_timeout -> auto reject` as a hidden runtime branch.
If expiry should be a product state, write it as choreography.

## 12. Abuse Control

Abuse control is evidence and choreography, not a hidden global counter.

Allowed:

```text
short-lived issuer tickets
signed rate-window ticket facts
workspace pending-intent cap
device notification cap
manual Fence for live sessions
future ticket refusal
```

Not promised without a database:

```text
instant global revoke
durable global rate counter
server-side infinite audit history
```

Deployments requiring instant global revocation or durable global abuse counters
are outside the no-database Pico Nod contract.

## 13. Support

Support actions are intents too.

Allowed support intents:

```text
fence workspace
revoke future issuer tickets
revoke future device tickets
rotate key
export signed receipts
mark incident
reconcile external commit
```

Forbidden:

```text
admin direct commit path
admin direct route selection
admin direct external API call
```

## 14. Tests

Protocol tests:

```text
intent_request_reaches_approval_boundary
nod_branch_creates_one_shot_commit_permit
reject_branch_creates_no_commit_permit
fence_branch_creates_no_commit_permit
timeout_does_not_select_nod
```

Boundary tests:

```text
http_tls_acceptor_has_no_business_logic
http_tls_acceptor_has_no_credential_access
http_tls_acceptor_compromise_cannot_commit
only_wasi_ingress_normalizes_public_request_body
apns_delivery_success_does_not_approve
app_response_is_evidence_not_route_authority
billing_entitlement_is_fact_not_approval_or_commit
support_actions_are_intents_not_admin_direct_paths
commit_boundary_is_only_external_token_owner
issuer_cannot_call_commit_boundary_directly
```

Security tests:

```text
intent_object_existence_is_not_approval
approval_display_hash_mismatch_rejected
stale_approval_replay_rejected
wrong_generation_rejected
duplicate_tx_id_different_body_rejected
lost_commit_ack_does_not_commit_twice
external_unknown_outcome_fences_without_idempotency_evidence
expired_ticket_rejected_without_global_revocation_db
old_kid_verifies_only_unexpired_evidence
raw_apns_token_is_not_route_or_device_identity
token_redaction_in_audit_and_panic
compromised_device_key_requires_future_refusal_or_fence
no_database_contract_rejects_instant_global_revoke_requirement
```

Runtime proof:

```text
one Capsule
multiple LogicalImages
roles may be separate processes
Endpoint/carrier for every cross-role message
ChoreoFS facts consumed only during choreography-open phases
CommitBoundary is the only external side-effect path
```

## 15. Implementation Order

```text
1. Protocol vocabulary: IntentRequest / ApprovalEvidence / ApprovedIntent.
2. WASI P1 public ingress guest.
3. Host proof carrier with separated logical images.
4. Local approval adapter without APNs.
5. APNs boundary and app delivery capability evidence.
6. CommitBoundary for one external action family.
7. BillingBoundary as entitlement facts only.
8. Receipt export and support intents.
9. Minimal HTTP byte acceptor for local/server deployment.
10. Real APNs provider, Apple app, provisioning, and release packaging.
```

## 16. Release Readiness

The repository is not App Store ready until these artifacts exist and pass
their gates:

```text
Apple Developer Program team configured
Bundle ID and entitlement profile
minimal iOS/macOS approval app
Xcode first launch setup completed
APNs provider token/key integration
APNs provider private key material
StoreKit / App Store Server evidence integration
App Store Server private key material
external action credential material
signed release build
privacy labels and App Review metadata
```

Release readiness is not inferred from unit tests or local Swift builds.
It is audited by:

```bash
scripts/check_pico_nod_app.sh
scripts/check_pico_nod_release_readiness.sh
```

That script must fail closed when production identifiers, signing material,
APNs provider configuration, StoreKit evidence configuration, or the external
action endpoint are absent.

When those facts are present, the release gate runs:

```bash
scripts/archive_pico_nod_app.sh
```

This uses the checked-in Xcode project and shared scheme:

```text
examples/pico-nod/apple/PicoNodApp/PicoNodApp.xcodeproj
examples/pico-nod/apple/PicoNodApp/PicoNodApp.xcodeproj/xcshareddata/xcschemes/PicoNodApp.xcscheme
```

The app package also carries the minimal release resources:

```text
Assets.xcassets/AppIcon.appiconset
PrivacyInfo.xcprivacy
PicoNod.entitlements
LaunchScreen.storyboard
```

`scripts/check_pico_nod_app.sh` must build the iOS Release target without
signing and fail on actionable Xcode warnings. The known AppIntents metadata
skip emitted when no AppIntents dependency exists is ignored because Pico Nod
does not expose AppIntents. A local app build is still not App Store readiness;
signing, provisioning, APNs, StoreKit, external action credentials, and review
metadata remain required release facts.

Release review evidence is also checked in as explicit artifacts:

```text
examples/pico-nod/release/app-store-review.md
examples/pico-nod/release/privacy-labels.md
examples/pico-nod/release/operations-runbook.md
```

These files are not authority. They are release audit evidence. Choreography
still owns protocol progress, and the production server still fails closed when
credentials or concrete boundary configuration are absent.

The repository is not production-server ready until these artifacts exist and
pass their gates:

```text
HTTP/TLS byte acceptor deployment shape
WASI P1 ingress image wired into the server path
APNs boundary using real provider credentials
external action boundary for a concrete service
issuer/device key rotation runbook
receipt export and incident reconciliation path
operational deadline/fault observability
external TLS terminator forwarding to loopback only
service manager entry for restart and logs
```

Production server readiness is not inferred from the bounded acceptor alone.
The acceptor only normalizes public bytes. A deployable server must also
provide the concrete APNs, billing, external action, key rotation, receipt
export, and observability facts listed above.

The server binary must fail closed in release/preflight mode unless those
facts are configured:

```bash
cargo run -p pico-nod-example --bin pico-nod-http-acceptor -- --preflight
cargo run -p pico-nod-example --bin pico-nod-http-acceptor -- --production <addr>
```

Local proof mode may listen on loopback without production credentials, but
production mode must not.

Production clear HTTP must not be public. Pico Nod's minimal acceptor listens
only behind an external TLS terminator:

```text
public TLS
  -> external TLS terminator
  -> loopback pico-nod-http-acceptor
  -> WASI P1 ingress
```

The production server gate requires:

```text
PICO_NOD_TLS_TERMINATION=external-loopback
production bind address is loopback
```

The current minimal acceptor is deliberately not authority.

```text
HTTP request bytes
  -> bounded acceptor
  -> WASI ingress normalization
  -> candidate facts
```

It does not approve, select routes, hold credentials, call APNs, or commit
external actions.

## 17. Final Definition

```text
Pico Nod by Hibana
  = human approval device + minimal Hibana server
    for choreography-governed external intents
```

Minimal invariant:

```text
External intent in.
Public request decoded by WASI P1 ingress.
Human evidence over APNs/local approval.
Nod / Reject / Fence as explicit Hibana route.
Approved branch emits one-shot commit permit.
CommitBoundary executes.
Everything else is evidence.
```
