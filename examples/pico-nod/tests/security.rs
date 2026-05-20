use hibana_pico::appkit::{self, ArtifactBundle, ArtifactEvidence, LogicalImage};
use hibana_pico::site;
use pico_nod_example::acceptor::{AcceptorError, HttpTlsAcceptor};
use pico_nod_example::apns::{ApnsBoundary, ApnsCredential, ApnsProvider, ApnsProviderError};
use pico_nod_example::approval::{ApprovalBoundary, ApprovalDecision};
use pico_nod_example::audit::AuditLog;
use pico_nod_example::billing::{BillingBoundary, EntitlementState, StoreEvidence};
use pico_nod_example::commit::{
    CommitBoundary, CommitFacts, CommitOutcome, CommitState, ExternalActionApi,
    ExternalActionCredential, ExternalActionError,
};
use pico_nod_example::ingress::WasiIngress;
use pico_nod_example::local_app::LocalApprovalApp;
use pico_nod_example::protocol::{
    ActionKind, ApprovalAction, CapabilityTicket, DeviceDeliveryCap, DeviceId, DeviceSigningKey,
    ExternalActionId, Generation, Hash, IssuerId, KeyId, Nonce, PicoNodError, TicketClock, TxId,
    WorkspaceId, displayed_hash,
};
use pico_nod_example::release::{
    RELEASE_ARTIFACTS, RELEASE_FILE_REQUIREMENTS, RELEASE_REQUIREMENTS,
};
use pico_nod_example::support::{SupportAction, SupportIntent};
use pico_nod_example::{PicoNodArtifacts, PicoNodCapsule, image};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApiMode {
    Success,
    UnknownWithoutEvidence,
    FailedClosed,
}

struct CountingApi {
    calls: u32,
    mode: ApiMode,
    next_external_id: ExternalActionId,
    last_credential: Hash,
}

impl CountingApi {
    const fn new(mode: ApiMode) -> Self {
        Self {
            calls: 0,
            mode,
            next_external_id: ExternalActionId(900),
            last_credential: Hash(0),
        }
    }
}

impl ExternalActionApi for CountingApi {
    fn commit(
        &mut self,
        credential: ExternalActionCredential,
        tx_id: TxId,
        body_hash: Hash,
    ) -> Result<ExternalActionId, ExternalActionError> {
        self.calls += 1;
        self.last_credential = credential.fingerprint();
        match self.mode {
            ApiMode::Success => Ok(ExternalActionId(
                self.next_external_id.0 + tx_id.0 + body_hash.0 % 7,
            )),
            ApiMode::UnknownWithoutEvidence => {
                Err(ExternalActionError::UnknownWithoutIdempotencyEvidence)
            }
            ApiMode::FailedClosed => Err(ExternalActionError::FailedClosed),
        }
    }
}

struct CountingApns {
    calls: u32,
    last_credential: Hash,
}

impl CountingApns {
    const fn new() -> Self {
        Self {
            calls: 0,
            last_credential: Hash(0),
        }
    }
}

impl ApnsProvider for CountingApns {
    fn notify(
        &mut self,
        credential: ApnsCredential,
        request: pico_nod_example::protocol::ApprovalRequest,
        delivery: DeviceDeliveryCap,
    ) -> Result<(), ApnsProviderError> {
        self.calls += 1;
        self.last_credential = credential.fingerprint();
        core::hint::black_box((request, delivery));
        Ok(())
    }
}

fn proof_intent() -> (
    pico_nod_example::protocol::IntentBodyObject,
    pico_nod_example::protocol::IntentRequest,
) {
    WasiIngress::normalize_public_request(
        IssuerId(7),
        WorkspaceId(3),
        TxId(11),
        Generation(1),
        ActionKind::Post,
        appkit::ObjectId(44),
        b"bounded public request body",
        b"bounded summary",
    )
    .expect("proof request is bounded")
}

fn approved_intent() -> (
    pico_nod_example::protocol::IntentBodyObject,
    pico_nod_example::protocol::ApprovedIntent,
) {
    let (body, intent) = proof_intent();
    let key = DeviceSigningKey::proof_only(DeviceId(5), Hash(0xAA55));
    let boundary = ApprovalBoundary::new(key.public_key());
    let request = boundary.request(intent, Nonce(99));
    let evidence = key.sign(request, ApprovalAction::Nod);
    let mut audit = AuditLog::<8>::empty();
    let decision = boundary
        .decide(request, evidence, &mut audit)
        .expect("valid approval evidence");
    let ApprovalDecision::Nod(approved) = decision else {
        panic!("expected nod decision");
    };
    (body, approved)
}

fn commit_boundary() -> CommitBoundary {
    CommitBoundary::new(
        ExternalActionCredential::proof_only(Hash(0xC0FFEE)),
        Hash(0x5151),
    )
}

#[test]
fn pico_nod_capsule_projects_role_split_and_approval_route() {
    let caps = appkit::derive_projection_caps::<PicoNodCapsule>();

    for role in 0..=6 {
        assert!(caps.roles.contains(role));
    }
    assert!(
        caps.labels[..caps.label_count as usize]
            .contains(&pico_nod_example::protocol::LABEL_APPROVED_INTENT)
    );
    assert!(
        caps.labels[..caps.label_count as usize]
            .contains(&pico_nod_example::protocol::LABEL_INTENT_FENCED)
    );
    assert!(
        caps.policies[..caps.policy_count as usize]
            .contains(&pico_nod_example::PICO_NOD_APPROVAL_POLICY)
    );
    assert!(appkit::validate_requested_roles::<
        PicoNodCapsule,
        site::Local<image::WasiIngressProcess>,
    >());
    assert!(appkit::validate_requested_roles::<
        PicoNodCapsule,
        site::Local<image::CommitBoundaryProcess>,
    >());
    assert!(appkit::validate_requested_roles::<
        PicoNodCapsule,
        site::Local<image::HostProofProcess>,
    >());
}

#[test]
fn pico_nod_artifacts_keep_wasi_ingress_separate_from_boundaries() {
    fn assert_wasi<I: LogicalImage<PicoNodCapsule, Artifact = appkit::WasiImage<'static>>>() {}
    fn assert_no_wasi<I: LogicalImage<PicoNodCapsule, Artifact = appkit::NoWasi>>() {}

    assert_wasi::<site::Local<image::WasiIngressProcess>>();
    assert_wasi::<site::Local<image::HostProofProcess>>();
    assert_no_wasi::<site::Local<image::RouterProcess>>();
    assert_no_wasi::<site::Local<image::ApprovalBoundaryProcess>>();
    assert_no_wasi::<site::Local<image::ApnsBoundaryProcess>>();
    assert_no_wasi::<site::Local<image::CommitBoundaryProcess>>();
    assert_no_wasi::<site::Local<image::AuditBoundaryProcess>>();

    let artifacts = PicoNodArtifacts {
        wasi_ingress: appkit::WasiImage::from_static(b"\0asm"),
    };
    let ingress = artifacts.for_image::<site::Local<image::WasiIngressProcess>>();
    assert_eq!(ingress.byte_len(), 4);
    let boundary = artifacts.for_image::<site::Local<image::CommitBoundaryProcess>>();
    assert_eq!(boundary.byte_len(), 0);
}

#[test]
fn http_tls_acceptor_forwards_bounded_body_to_wasi_ingress_only() {
    let acceptor = HttpTlsAcceptor::new(512, 128);
    let request =
        b"POST /intent HTTP/1.1\r\nHost: local\r\nContent-Length: 11\r\n\r\nhello worldextra";

    let public_request = acceptor.parse(request).expect("bounded public body");
    let (body, intent) = WasiIngress::normalize_public_request(
        IssuerId(7),
        WorkspaceId(3),
        TxId(17),
        Generation(1),
        ActionKind::Post,
        appkit::ObjectId(88),
        public_request.body,
        b"summary",
    )
    .expect("WASI ingress normalizes bounded public body");

    assert_eq!(public_request.body, b"hello world");
    assert_eq!(body.body_hash(), intent.body_hash);
    assert!(acceptor.cannot_hold_credentials());
    assert!(acceptor.cannot_select_routes());
    assert!(acceptor.cannot_commit_external_actions());
}

#[test]
fn http_tls_acceptor_rejects_unbounded_or_implicit_http_shapes() {
    let acceptor = HttpTlsAcceptor::new(128, 8);

    assert_eq!(
        acceptor.parse(b"GET /intent HTTP/1.1\r\nContent-Length: 0\r\n\r\n"),
        Err(AcceptorError::MethodNotAllowed)
    );
    assert_eq!(
        acceptor.parse(b"POST /other HTTP/1.1\r\nContent-Length: 0\r\n\r\n"),
        Err(AcceptorError::PathNotAllowed)
    );
    assert_eq!(
        acceptor.parse(
            b"POST /intent HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Length: 4\r\n\r\nbody"
        ),
        Err(AcceptorError::ChunkedUnsupported)
    );
    assert_eq!(
        acceptor.parse(b"POST /intent HTTP/1.1\r\nContent-Length: 9\r\n\r\n123456789"),
        Err(AcceptorError::BodyTooLarge)
    );
}

#[test]
fn wasi_ingress_output_is_candidate_evidence_not_authority() {
    let (body, intent) = proof_intent();
    let mut api = CountingApi::new(ApiMode::Success);
    let mut facts = CommitFacts::<4>::empty();
    let mut audit = AuditLog::<4>::empty();

    assert_eq!(body.body_hash(), intent.body_hash);
    assert!(WasiIngress::cannot_hold_credentials());
    assert!(WasiIngress::cannot_select_routes());
    assert_eq!(api.calls, 0);
    assert_eq!(facts.state(intent.tx_id), None);
    assert_eq!(audit.len(), 0);

    let boundary = commit_boundary();
    core::hint::black_box((&boundary, &mut facts, &mut audit, &mut api));
    assert_eq!(api.calls, 0);
}

#[test]
fn approval_display_hash_mismatch_rejected() {
    let (body, intent) = proof_intent();
    let key = DeviceSigningKey::proof_only(DeviceId(5), Hash(0xAA55));
    let boundary = ApprovalBoundary::new(key.public_key());
    let request = boundary.request(intent, Nonce(99));
    let mut evidence = key.sign(request, ApprovalAction::Nod);
    evidence.displayed_hash = Hash(displayed_hash(request).0 ^ 1);
    let mut audit = AuditLog::<4>::empty();

    assert_eq!(
        boundary.decide(request, evidence, &mut audit),
        Err(PicoNodError::DisplayMismatch)
    );
    assert_eq!(body.body_hash(), intent.body_hash);
}

#[test]
fn stale_or_wrong_generation_approval_is_rejected() {
    let (body, intent) = proof_intent();
    let key = DeviceSigningKey::proof_only(DeviceId(5), Hash(0xAA55));
    let boundary = ApprovalBoundary::new(key.public_key());
    let request = boundary.request(intent, Nonce(99));
    let mut stale = key.sign(request, ApprovalAction::Nod);
    stale.generation = Generation(2);
    let mut audit = AuditLog::<4>::empty();

    assert_eq!(
        boundary.decide(request, stale, &mut audit),
        Err(PicoNodError::ApprovalMismatch)
    );
    assert_eq!(body.object_id(), intent.object_id);
}

#[test]
fn reject_and_fence_create_no_commit_permit() {
    let (body, intent) = proof_intent();
    let key = DeviceSigningKey::proof_only(DeviceId(5), Hash(0xAA55));
    let boundary = ApprovalBoundary::new(key.public_key());
    let request = boundary.request(intent, Nonce(99));
    let mut audit = AuditLog::<8>::empty();

    let rejected = boundary
        .decide(
            request,
            key.sign(request, ApprovalAction::Reject),
            &mut audit,
        )
        .expect("reject evidence is valid");
    assert_eq!(rejected, ApprovalDecision::Reject);

    let fenced = boundary
        .decide(
            request,
            key.sign(request, ApprovalAction::Fence),
            &mut audit,
        )
        .expect("fence evidence is valid");
    assert_eq!(fenced, ApprovalDecision::Fence);
    assert_eq!(body.body_hash(), intent.body_hash);
}

#[test]
fn approved_branch_commits_once_and_duplicate_returns_receipt_without_second_call() {
    let (body, approved) = approved_intent();
    let boundary = commit_boundary();
    let mut facts = CommitFacts::<4>::empty();
    let mut audit = AuditLog::<8>::empty();
    let mut api = CountingApi::new(ApiMode::Success);

    let first = boundary
        .commit(approved, &body, &mut facts, &mut audit, &mut api)
        .expect("approved intent commits");
    let CommitOutcome::Committed(first_receipt) = first else {
        panic!("expected first commit");
    };
    let second = boundary
        .commit(approved, &body, &mut facts, &mut audit, &mut api)
        .expect("duplicate is resolved from commit facts");
    let CommitOutcome::DuplicateCommitted(second_receipt) = second else {
        panic!("expected duplicate committed");
    };

    assert_eq!(api.calls, 1);
    assert_eq!(first_receipt.external_id, second_receipt.external_id);
    assert_eq!(facts.state(approved.tx_id), Some(CommitState::Committed));
}

#[test]
fn duplicate_tx_id_different_body_rejected() {
    let (body, approved) = approved_intent();
    let other_body = pico_nod_example::protocol::IntentBodyObject::new(
        body.object_id(),
        b"different bounded body",
    )
    .expect("bounded body");
    let boundary = commit_boundary();
    let mut facts = CommitFacts::<4>::empty();
    let mut audit = AuditLog::<8>::empty();
    let mut api = CountingApi::new(ApiMode::Success);

    assert!(
        boundary
            .commit(approved, &body, &mut facts, &mut audit, &mut api)
            .is_ok()
    );
    assert_eq!(
        facts.reserve_pending(approved.tx_id, other_body.body_hash()),
        Err(PicoNodError::DuplicateTxDifferentBody)
    );
}

#[test]
fn lost_commit_ack_does_not_commit_twice() {
    let (body, approved) = approved_intent();
    let boundary = commit_boundary();
    let mut facts = CommitFacts::<4>::empty();
    let mut audit = AuditLog::<8>::empty();
    let mut api = CountingApi::new(ApiMode::Success);

    let first = boundary
        .commit_with_lost_local_ack(approved, &body, &mut facts, &mut audit, &mut api)
        .expect("external action succeeded before local ack loss");
    let second = boundary
        .commit(approved, &body, &mut facts, &mut audit, &mut api)
        .expect("retry uses committed evidence");
    let CommitOutcome::DuplicateCommitted(second_receipt) = second else {
        panic!("expected duplicate committed");
    };

    assert_eq!(api.calls, 1);
    assert_eq!(first.external_id, second_receipt.external_id);
}

#[test]
fn external_unknown_outcome_fences_without_idempotency_evidence() {
    let (body, approved) = approved_intent();
    let boundary = commit_boundary();
    let mut facts = CommitFacts::<4>::empty();
    let mut audit = AuditLog::<8>::empty();
    let mut api = CountingApi::new(ApiMode::UnknownWithoutEvidence);

    let outcome = boundary
        .commit(approved, &body, &mut facts, &mut audit, &mut api)
        .expect("unknown outcome fences safely");

    assert_eq!(outcome, CommitOutcome::Fenced);
    assert_eq!(facts.state(approved.tx_id), Some(CommitState::Fenced));
    assert_eq!(api.calls, 1);
}

#[test]
fn external_failed_closed_records_terminal_fault_without_commit() {
    let (body, approved) = approved_intent();
    let boundary = commit_boundary();
    let mut facts = CommitFacts::<4>::empty();
    let mut audit = AuditLog::<8>::empty();
    let mut api = CountingApi::new(ApiMode::FailedClosed);

    assert_eq!(
        boundary.commit(approved, &body, &mut facts, &mut audit, &mut api),
        Err(PicoNodError::ExternalFailed)
    );
    assert_eq!(facts.state(approved.tx_id), Some(CommitState::Fenced));
    assert_eq!(api.calls, 1);
    assert!(
        audit
            .events()
            .iter()
            .any(|event| event.code == pico_nod_example::audit::AuditCode::TerminalFault)
    );
}

#[test]
fn expired_ticket_rejected_without_global_revocation_database() {
    let signing_hash = Hash(0xDEAD);
    let ticket = CapabilityTicket::new(
        IssuerId(7),
        WorkspaceId(3),
        Generation(1),
        10,
        KeyId(1),
        signing_hash,
    );
    let clock = TicketClock { now: 20, skew: 1 };

    assert_eq!(
        ticket.verify(clock, signing_hash),
        Err(PicoNodError::ExpiredTicket)
    );
}

#[test]
fn old_key_verifies_only_unexpired_evidence() {
    let old_hash = Hash(0x1111);
    let ticket = CapabilityTicket::new(
        IssuerId(7),
        WorkspaceId(3),
        Generation(1),
        100,
        KeyId(1),
        old_hash,
    );

    assert!(
        ticket
            .verify(TicketClock { now: 99, skew: 1 }, old_hash)
            .is_ok()
    );
    assert_eq!(
        ticket.verify(TicketClock { now: 200, skew: 1 }, old_hash),
        Err(PicoNodError::ExpiredTicket)
    );
}

#[test]
fn raw_apns_token_is_delivery_evidence_not_device_identity() {
    let cap = DeviceDeliveryCap {
        user_id: 1,
        workspace_id: WorkspaceId(3),
        device_id: DeviceId(5),
        apns_token_hash: Hash(0xABCD),
        topic_hash: Hash(0x1234),
        expires_at: 100,
        key_id: KeyId(2),
        signature: pico_nod_example::protocol::Signature(0xFF),
    };

    assert_ne!(cap.apns_token_hash.0, cap.device_id.0);
}

#[test]
fn apns_delivery_success_does_not_approve_or_commit() {
    let (body, intent) = proof_intent();
    let key = DeviceSigningKey::proof_only(DeviceId(5), Hash(0xAA55));
    let approval = ApprovalBoundary::new(key.public_key());
    let request = approval.request(intent, Nonce(99));
    let delivery = DeviceDeliveryCap::new(
        1,
        WorkspaceId(3),
        DeviceId(5),
        Hash(0xABCD),
        Hash(0x1234),
        100,
        KeyId(2),
        Hash(0xD17E),
    );
    let boundary = ApnsBoundary::new(
        ApnsCredential::proof_only(Hash(0xA9A9)),
        Hash(0xD17E),
        Hash(0x5151),
    );
    let mut provider = CountingApns::new();
    let api = CountingApi::new(ApiMode::Success);

    let receipt = boundary
        .dispatch(
            request,
            delivery,
            TicketClock { now: 1, skew: 0 },
            &mut provider,
        )
        .expect("delivery capability is valid");

    assert_eq!(provider.calls, 1);
    assert_eq!(provider.last_credential, Hash(0xA9A9));
    assert_eq!(receipt.token_hash, delivery.apns_token_hash);
    assert!(boundary.cannot_approve());
    assert!(boundary.cannot_select_routes());
    assert!(boundary.cannot_commit_external_actions());
    assert_eq!(api.calls, 0);
    assert_eq!(body.body_hash(), intent.body_hash);
}

#[test]
fn local_approval_app_signs_displayed_intent_but_cannot_commit_or_route() {
    let (body, intent) = proof_intent();
    let key = DeviceSigningKey::proof_only(DeviceId(5), Hash(0xAA55));
    let boundary = ApprovalBoundary::new(key.public_key());
    let request = boundary.request(intent, Nonce(99));
    let app = LocalApprovalApp::new(key);
    let displayed = app.display(request);

    let evidence = app
        .decide(displayed, ApprovalAction::Nod)
        .expect("local app signs displayed intent");

    assert_eq!(displayed.displayed_hash(), evidence.displayed_hash);
    assert!(app.cannot_select_routes());
    assert!(app.cannot_commit_external_actions());
    assert!(app.cannot_hold_apns_provider_credentials());
    assert_eq!(body.body_hash(), intent.body_hash);
}

#[test]
fn token_redaction_keeps_credentials_out_of_audit() {
    let (body, approved) = approved_intent();
    let boundary = commit_boundary();
    let mut facts = CommitFacts::<4>::empty();
    let mut audit = AuditLog::<8>::empty();
    let mut api = CountingApi::new(ApiMode::Success);

    assert!(
        boundary
            .commit(approved, &body, &mut facts, &mut audit, &mut api)
            .is_ok()
    );
    for event in audit.events() {
        assert_ne!(event.hash, api.last_credential);
    }
}

#[test]
fn no_database_contract_keeps_global_revoke_outside_protocol() {
    let plan = include_str!("../plan.md");

    assert!(plan.contains("Pico Nod has no database."));
    assert!(plan.contains("no immediate global revocation promise"));
    assert!(plan.contains("Deployments requiring instant global revocation"));
    assert!(plan.contains("are outside the no-database Pico Nod contract."));
}

#[test]
fn billing_entitlement_is_fact_not_approval_or_commit() {
    let boundary = BillingBoundary::new(Hash(0xB177));
    let evidence = StoreEvidence::new(
        WorkspaceId(3),
        EntitlementState::Unknown,
        100,
        KeyId(7),
        Hash(0xB177),
    );
    let fact = boundary
        .verify(evidence, TicketClock { now: 1, skew: 0 })
        .expect("signed store evidence verifies");

    assert_eq!(
        fact.require_paid_feature(),
        Err(PicoNodError::EntitlementInactive)
    );
    assert!(fact.cannot_approve());
    assert!(fact.cannot_commit_external_actions());
}

#[test]
fn support_actions_are_intents_not_admin_direct_paths() {
    let support = SupportIntent::new(
        IssuerId(7),
        WorkspaceId(3),
        TxId(99),
        Generation(1),
        SupportAction::FenceWorkspace,
        appkit::ObjectId(77),
        b"fence workspace after incident",
    )
    .expect("support intent is bounded");

    assert_eq!(support.request.action_kind, ActionKind::LocalCommand);
    assert_eq!(support.request.body_hash, support.body.body_hash());
    assert!(support.cannot_select_routes());
    assert!(support.cannot_commit_without_approval());
}

#[test]
fn release_requirements_cover_app_store_and_server_operation() {
    let names = RELEASE_REQUIREMENTS
        .iter()
        .map(|requirement| requirement.name)
        .collect::<std::vec::Vec<_>>();

    assert!(names.contains(&"PICO_NOD_APPLE_TEAM_ID"));
    assert!(names.contains(&"PICO_NOD_BUNDLE_ID"));
    assert!(names.contains(&"PICO_NOD_APNS_KEY_ID"));
    assert!(names.contains(&"PICO_NOD_APNS_TEAM_ID"));
    assert!(names.contains(&"PICO_NOD_APNS_TOPIC"));
    assert!(names.contains(&"PICO_NOD_APNS_PRIVATE_KEY_PATH"));
    assert!(names.contains(&"PICO_NOD_STORE_ISSUER_ID"));
    assert!(names.contains(&"PICO_NOD_STORE_KEY_ID"));
    assert!(names.contains(&"PICO_NOD_STORE_PRIVATE_KEY_PATH"));
    assert!(names.contains(&"PICO_NOD_TLS_TERMINATION"));
    assert!(names.contains(&"PICO_NOD_EXTERNAL_ACTION_ENDPOINT"));
    assert!(names.contains(&"PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH"));

    assert_eq!(
        RELEASE_FILE_REQUIREMENTS,
        &[
            "PICO_NOD_APNS_PRIVATE_KEY_PATH",
            "PICO_NOD_STORE_PRIVATE_KEY_PATH",
            "PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH",
        ]
    );

    let artifact_paths = RELEASE_ARTIFACTS
        .iter()
        .map(|artifact| artifact.path)
        .collect::<std::vec::Vec<_>>();
    assert!(artifact_paths.contains(&"examples/pico-nod/release/app-store-review.md"));
    assert!(artifact_paths.contains(&"examples/pico-nod/release/privacy-labels.md"));
    assert!(artifact_paths.contains(&"examples/pico-nod/release/operations-runbook.md"));
}

#[test]
fn production_server_preflight_fails_closed_without_release_configuration() {
    let bin = env!("CARGO_BIN_EXE_pico-nod-http-acceptor");
    let mut command = std::process::Command::new(bin);
    command.arg("--preflight");
    for requirement in RELEASE_REQUIREMENTS {
        command.env_remove(requirement.name);
    }
    let output = command.output().expect("preflight process should execute");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("PICO_NOD_APPLE_TEAM_ID"));
    assert!(stderr.contains("PICO_NOD_TLS_TERMINATION"));
    assert!(stderr.contains("PICO_NOD_EXTERNAL_ACTION_ENDPOINT"));
    assert!(stderr.contains("production configuration is incomplete"));
}

#[test]
fn production_server_rejects_public_clear_http_bind_even_when_configured() {
    let bin = env!("CARGO_BIN_EXE_pico-nod-http-acceptor");
    let readable = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("Cargo.toml")
        .display()
        .to_string();
    let mut command = std::process::Command::new(bin);
    command.arg("--production").arg("0.0.0.0:0");
    command.env("PICO_NOD_APPLE_TEAM_ID", "TEAM");
    command.env("PICO_NOD_BUNDLE_ID", "com.hibana.piconod");
    command.env("PICO_NOD_APNS_KEY_ID", "APNSKEY");
    command.env("PICO_NOD_APNS_TEAM_ID", "TEAM");
    command.env("PICO_NOD_APNS_TOPIC", "com.hibana.piconod");
    command.env("PICO_NOD_APNS_PRIVATE_KEY_PATH", &readable);
    command.env("PICO_NOD_STORE_ISSUER_ID", "ISSUER");
    command.env("PICO_NOD_STORE_KEY_ID", "STOREKEY");
    command.env("PICO_NOD_STORE_PRIVATE_KEY_PATH", &readable);
    command.env("PICO_NOD_TLS_TERMINATION", "external-loopback");
    command.env(
        "PICO_NOD_EXTERNAL_ACTION_ENDPOINT",
        "https://example.invalid/action",
    );
    command.env("PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH", &readable);

    let output = command.output().expect("production process should execute");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("production bind address must be loopback"));
}

#[test]
fn production_server_preflight_rejects_unreadable_credential_paths() {
    let bin = env!("CARGO_BIN_EXE_pico-nod-http-acceptor");
    let missing = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("missing-credential.pem")
        .display()
        .to_string();
    let mut command = std::process::Command::new(bin);
    command.arg("--preflight");
    command.env("PICO_NOD_APPLE_TEAM_ID", "TEAM");
    command.env("PICO_NOD_BUNDLE_ID", "com.hibana.piconod");
    command.env("PICO_NOD_APNS_KEY_ID", "APNSKEY");
    command.env("PICO_NOD_APNS_TEAM_ID", "TEAM");
    command.env("PICO_NOD_APNS_TOPIC", "com.hibana.piconod");
    command.env("PICO_NOD_APNS_PRIVATE_KEY_PATH", &missing);
    command.env("PICO_NOD_STORE_ISSUER_ID", "ISSUER");
    command.env("PICO_NOD_STORE_KEY_ID", "STOREKEY");
    command.env("PICO_NOD_STORE_PRIVATE_KEY_PATH", &missing);
    command.env("PICO_NOD_TLS_TERMINATION", "external-loopback");
    command.env(
        "PICO_NOD_EXTERNAL_ACTION_ENDPOINT",
        "https://example.invalid/action",
    );
    command.env("PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH", &missing);

    let output = command.output().expect("preflight process should execute");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("PICO_NOD_APNS_PRIVATE_KEY_PATH readable file"));
    assert!(stderr.contains("PICO_NOD_STORE_PRIVATE_KEY_PATH readable file"));
    assert!(stderr.contains("PICO_NOD_EXTERNAL_ACTION_CREDENTIAL_PATH readable file"));
}
