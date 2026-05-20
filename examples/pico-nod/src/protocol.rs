use core::marker::PhantomData;

use hibana_pico::appkit;

pub const LABEL_INTENT_REQUEST: u8 = 180;
pub const LABEL_APPROVAL_REQUEST: u8 = 181;
pub const LABEL_NOTIFY_APPROVAL_DEVICE: u8 = 182;
pub const LABEL_NOTIFICATION_DISPATCHED: u8 = 183;
pub const LABEL_APPROVAL_EVIDENCE: u8 = 184;
pub const LABEL_NOD_ROUTE: u8 = 185;
pub const LABEL_REJECT_ROUTE: u8 = 186;
pub const LABEL_FENCE_ROUTE: u8 = 187;
pub const LABEL_APPROVED_INTENT: u8 = 188;
pub const LABEL_INTENT_COMMITTED: u8 = 189;
pub const LABEL_INTENT_REJECTED: u8 = 190;
pub const LABEL_INTENT_FENCED: u8 = 191;
pub const LABEL_NOT_NOD_ROUTE: u8 = 192;

pub const ROLE_WASI_INGRESS: u8 = 0;
pub const ROLE_INTENT_ROUTER: u8 = 1;
pub const ROLE_APPROVAL_BOUNDARY: u8 = 2;
pub const ROLE_APNS_BOUNDARY: u8 = 3;
pub const ROLE_APPROVAL_INGRESS: u8 = 4;
pub const ROLE_COMMIT_BOUNDARY: u8 = 5;
pub const ROLE_AUDIT_BOUNDARY: u8 = 6;

pub const MAX_BODY_BYTES: usize = 512;
pub const MAX_SUMMARY_BYTES: usize = 160;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IssuerId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TxId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Generation(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyId(pub u16);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nonce(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hash(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Signature(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExternalActionId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Post,
    Reply,
    LocalCommand,
}

impl Default for ActionKind {
    fn default() -> Self {
        Self::Post
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalAction {
    Nod,
    Reject,
    Fence,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct One;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicoNodError {
    BodyTooLarge,
    SummaryTooLarge,
    SignatureMismatch,
    ApprovalMismatch,
    DisplayMismatch,
    ExpiredTicket,
    WrongWorkspace,
    WrongGeneration,
    DuplicateTxDifferentBody,
    MissingIdempotencyEvidence,
    CapacityFull,
    NotApproved,
    ExternalFailed,
    EntitlementInactive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundedBytes<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> BoundedBytes<N> {
    pub const EMPTY: Self = Self {
        bytes: [0; N],
        len: 0,
    };

    pub fn new(bytes: &[u8]) -> Result<Self, PicoNodError> {
        if bytes.len() > N {
            return Err(PicoNodError::BodyTooLarge);
        }
        let mut out = [0u8; N];
        let mut index = 0usize;
        while index < bytes.len() {
            out[index] = bytes[index];
            index += 1;
        }
        Ok(Self {
            bytes: out,
            len: bytes.len(),
        })
    }

    pub fn new_summary(bytes: &[u8]) -> Result<Self, PicoNodError> {
        if bytes.len() > N {
            return Err(PicoNodError::SummaryTooLarge);
        }
        Self::new(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.split_at(self.len).0
    }

    pub fn hash(&self) -> Hash {
        hash_bytes(self.as_bytes())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentBodyObject {
    object_id: appkit::ObjectId,
    body: BoundedBytes<MAX_BODY_BYTES>,
    body_hash: Hash,
}

impl IntentBodyObject {
    pub fn new(object_id: appkit::ObjectId, body: &[u8]) -> Result<Self, PicoNodError> {
        let body = BoundedBytes::new(body)?;
        Ok(Self {
            object_id,
            body_hash: body.hash(),
            body,
        })
    }

    pub const fn object_id(&self) -> appkit::ObjectId {
        self.object_id
    }

    pub const fn body_hash(&self) -> Hash {
        self.body_hash
    }

    pub const fn body(&self) -> &BoundedBytes<MAX_BODY_BYTES> {
        &self.body
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentRequest {
    pub issuer_id: IssuerId,
    pub workspace_id: WorkspaceId,
    pub tx_id: TxId,
    pub generation: Generation,
    pub action_kind: ActionKind,
    pub object_id: appkit::ObjectId,
    pub body_hash: Hash,
    pub summary_hash: Hash,
}

impl IntentRequest {
    pub fn new(
        issuer_id: IssuerId,
        workspace_id: WorkspaceId,
        tx_id: TxId,
        generation: Generation,
        action_kind: ActionKind,
        body: &IntentBodyObject,
        summary: &BoundedBytes<MAX_SUMMARY_BYTES>,
    ) -> Self {
        Self {
            issuer_id,
            workspace_id,
            tx_id,
            generation,
            action_kind,
            object_id: body.object_id(),
            body_hash: body.body_hash(),
            summary_hash: summary.hash(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub tx_id: TxId,
    pub generation: Generation,
    pub workspace_id: WorkspaceId,
    pub object_id: appkit::ObjectId,
    pub body_hash: Hash,
    pub summary_hash: Hash,
    pub nonce: Nonce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovalEvidence {
    pub tx_id: TxId,
    pub generation: Generation,
    pub workspace_id: WorkspaceId,
    pub object_id: appkit::ObjectId,
    pub body_hash: Hash,
    pub summary_hash: Hash,
    pub nonce: Nonce,
    pub device_id: DeviceId,
    pub action: ApprovalAction,
    pub displayed_version: u16,
    pub displayed_hash: Hash,
    pub signature: Signature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovedIntent {
    pub tx_id: TxId,
    pub generation: Generation,
    pub workspace_id: WorkspaceId,
    pub object_id: appkit::ObjectId,
    pub body_hash: Hash,
    pub permit: IntentCommitPermit<One>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentCommitPermit<State> {
    generation: Generation,
    tx_id: TxId,
    object_id: appkit::ObjectId,
    body_hash: Hash,
    state: PhantomData<State>,
}

impl IntentCommitPermit<One> {
    pub const fn new(
        generation: Generation,
        tx_id: TxId,
        object_id: appkit::ObjectId,
        body_hash: Hash,
    ) -> Self {
        Self {
            generation,
            tx_id,
            object_id,
            body_hash,
            state: PhantomData,
        }
    }

    pub const fn generation(&self) -> Generation {
        self.generation
    }

    pub const fn tx_id(&self) -> TxId {
        self.tx_id
    }

    pub const fn object_id(&self) -> appkit::ObjectId {
        self.object_id
    }

    pub const fn body_hash(&self) -> Hash {
        self.body_hash
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DevicePublicKey {
    pub device_id: DeviceId,
    pub verification_hash: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceSigningKey {
    device_id: DeviceId,
    signing_hash: Hash,
}

impl DeviceSigningKey {
    pub const fn proof_only(device_id: DeviceId, signing_hash: Hash) -> Self {
        Self {
            device_id,
            signing_hash,
        }
    }

    pub const fn public_key(&self) -> DevicePublicKey {
        DevicePublicKey {
            device_id: self.device_id,
            verification_hash: self.signing_hash,
        }
    }

    pub fn sign(&self, request: ApprovalRequest, action: ApprovalAction) -> ApprovalEvidence {
        let displayed_hash = displayed_hash(request);
        let signature = sign_approval(self.signing_hash, request, action, displayed_hash);
        ApprovalEvidence {
            tx_id: request.tx_id,
            generation: request.generation,
            workspace_id: request.workspace_id,
            object_id: request.object_id,
            body_hash: request.body_hash,
            summary_hash: request.summary_hash,
            nonce: request.nonce,
            device_id: self.device_id,
            action,
            displayed_version: 1,
            displayed_hash,
            signature,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceDeliveryCap {
    pub user_id: u64,
    pub workspace_id: WorkspaceId,
    pub device_id: DeviceId,
    pub apns_token_hash: Hash,
    pub topic_hash: Hash,
    pub expires_at: u64,
    pub key_id: KeyId,
    pub signature: Signature,
}

impl DeviceDeliveryCap {
    pub fn new(
        user_id: u64,
        workspace_id: WorkspaceId,
        device_id: DeviceId,
        apns_token_hash: Hash,
        topic_hash: Hash,
        expires_at: u64,
        key_id: KeyId,
        signing_hash: Hash,
    ) -> Self {
        let unsigned = Self {
            user_id,
            workspace_id,
            device_id,
            apns_token_hash,
            topic_hash,
            expires_at,
            key_id,
            signature: Signature(0),
        };
        Self {
            signature: sign_delivery_cap(signing_hash, unsigned),
            ..unsigned
        }
    }

    pub fn verify(
        self,
        clock: TicketClock,
        signing_hash: Hash,
        workspace_id: WorkspaceId,
    ) -> Result<Self, PicoNodError> {
        if self.workspace_id != workspace_id {
            return Err(PicoNodError::WrongWorkspace);
        }
        if clock.now > self.expires_at.saturating_add(clock.skew) {
            return Err(PicoNodError::ExpiredTicket);
        }
        if sign_delivery_cap(
            signing_hash,
            Self {
                signature: Signature(0),
                ..self
            },
        ) != self.signature
        {
            return Err(PicoNodError::SignatureMismatch);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityTicket {
    pub issuer_id: IssuerId,
    pub workspace_id: WorkspaceId,
    pub generation: Generation,
    pub expires_at: u64,
    pub key_id: KeyId,
    pub signature: Signature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TicketClock {
    pub now: u64,
    pub skew: u64,
}

impl CapabilityTicket {
    pub fn new(
        issuer_id: IssuerId,
        workspace_id: WorkspaceId,
        generation: Generation,
        expires_at: u64,
        key_id: KeyId,
        signing_hash: Hash,
    ) -> Self {
        let unsigned = Self {
            issuer_id,
            workspace_id,
            generation,
            expires_at,
            key_id,
            signature: Signature(0),
        };
        Self {
            signature: sign_ticket(signing_hash, unsigned),
            ..unsigned
        }
    }

    pub fn verify(self, clock: TicketClock, signing_hash: Hash) -> Result<Self, PicoNodError> {
        if clock.now > self.expires_at.saturating_add(clock.skew) {
            return Err(PicoNodError::ExpiredTicket);
        }
        if sign_ticket(
            signing_hash,
            Self {
                signature: Signature(0),
                ..self
            },
        ) != self.signature
        {
            return Err(PicoNodError::SignatureMismatch);
        }
        Ok(self)
    }
}

pub fn displayed_hash(request: ApprovalRequest) -> Hash {
    hash_fields(&[
        request.tx_id.0,
        request.generation.0 as u64,
        request.workspace_id.0,
        request.object_id.0 as u64,
        request.body_hash.0,
        request.summary_hash.0,
        request.nonce.0,
    ])
}

pub fn sign_approval(
    signing_hash: Hash,
    request: ApprovalRequest,
    action: ApprovalAction,
    displayed: Hash,
) -> Signature {
    let action_value = match action {
        ApprovalAction::Nod => 1,
        ApprovalAction::Reject => 2,
        ApprovalAction::Fence => 3,
    };
    Signature(
        hash_fields(&[
            signing_hash.0,
            request.tx_id.0,
            request.generation.0 as u64,
            request.workspace_id.0,
            request.object_id.0 as u64,
            request.body_hash.0,
            request.summary_hash.0,
            request.nonce.0,
            displayed.0,
            action_value,
        ])
        .0,
    )
}

pub fn sign_ticket(signing_hash: Hash, ticket: CapabilityTicket) -> Signature {
    Signature(
        hash_fields(&[
            signing_hash.0,
            ticket.issuer_id.0,
            ticket.workspace_id.0,
            ticket.generation.0 as u64,
            ticket.expires_at,
            ticket.key_id.0 as u64,
        ])
        .0,
    )
}

pub fn sign_delivery_cap(signing_hash: Hash, cap: DeviceDeliveryCap) -> Signature {
    Signature(
        hash_fields(&[
            signing_hash.0,
            cap.user_id,
            cap.workspace_id.0,
            cap.device_id.0,
            cap.apns_token_hash.0,
            cap.topic_hash.0,
            cap.expires_at,
            cap.key_id.0 as u64,
        ])
        .0,
    )
}

pub fn hash_bytes(bytes: &[u8]) -> Hash {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    Hash(hash)
}

pub fn hash_fields(fields: &[u64]) -> Hash {
    let mut hash = 0x9e37_79b9_7f4a_7c15u64;
    for field in fields {
        hash ^= *field;
        hash = hash.rotate_left(13).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    }
    Hash(hash)
}
