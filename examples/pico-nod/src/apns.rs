use crate::protocol::{
    ApprovalRequest, DeviceDeliveryCap, DeviceId, Hash, PicoNodError, Signature, TicketClock, TxId,
    hash_fields,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApnsCredential {
    fingerprint: Hash,
}

impl ApnsCredential {
    pub const fn proof_only(fingerprint: Hash) -> Self {
        Self { fingerprint }
    }

    pub const fn fingerprint(&self) -> Hash {
        self.fingerprint
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotificationReceipt {
    pub tx_id: TxId,
    pub device_id: DeviceId,
    pub request_hash: Hash,
    pub token_hash: Hash,
    pub signature: Signature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApnsProviderError {
    FailedClosed,
}

pub trait ApnsProvider {
    fn notify(
        &mut self,
        credential: ApnsCredential,
        request: ApprovalRequest,
        delivery: DeviceDeliveryCap,
    ) -> Result<(), ApnsProviderError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApnsBoundary {
    credential: ApnsCredential,
    delivery_signing_hash: Hash,
    receipt_key: Hash,
}

impl ApnsBoundary {
    pub const fn new(
        credential: ApnsCredential,
        delivery_signing_hash: Hash,
        receipt_key: Hash,
    ) -> Self {
        Self {
            credential,
            delivery_signing_hash,
            receipt_key,
        }
    }

    pub fn dispatch(
        &self,
        request: ApprovalRequest,
        delivery: DeviceDeliveryCap,
        clock: TicketClock,
        provider: &mut impl ApnsProvider,
    ) -> Result<NotificationReceipt, PicoNodError> {
        let delivery = delivery.verify(clock, self.delivery_signing_hash, request.workspace_id)?;
        match provider.notify(self.credential, request, delivery) {
            Ok(()) => {}
            Err(ApnsProviderError::FailedClosed) => return Err(PicoNodError::ExternalFailed),
        }
        Ok(self.receipt(request, delivery))
    }

    pub const fn cannot_approve(&self) -> bool {
        true
    }

    pub const fn cannot_select_routes(&self) -> bool {
        true
    }

    pub const fn cannot_commit_external_actions(&self) -> bool {
        true
    }

    fn receipt(
        &self,
        request: ApprovalRequest,
        delivery: DeviceDeliveryCap,
    ) -> NotificationReceipt {
        let request_hash = hash_fields(&[
            request.tx_id.0,
            request.generation.0 as u64,
            request.workspace_id.0,
            request.object_id.0 as u64,
            request.body_hash.0,
            request.summary_hash.0,
            request.nonce.0,
        ]);
        let signature = Signature(
            hash_fields(&[
                self.receipt_key.0,
                request_hash.0,
                delivery.device_id.0,
                delivery.apns_token_hash.0,
            ])
            .0,
        );
        NotificationReceipt {
            tx_id: request.tx_id,
            device_id: delivery.device_id,
            request_hash,
            token_hash: delivery.apns_token_hash,
            signature,
        }
    }
}
