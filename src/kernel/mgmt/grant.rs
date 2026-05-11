use crate::kernel::swarm::{NodeId, SwarmCredential};

use super::ImageSlotError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MgmtInstallGrant {
    node_id: NodeId,
    session_generation: u16,
    slot: u8,
    image_generation: u32,
    tag: u32,
}

impl MgmtInstallGrant {
    pub fn new(
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        slot: u8,
        image_generation: u32,
    ) -> Self {
        Self {
            node_id,
            session_generation,
            slot,
            image_generation,
            tag: Self::compute_tag(
                node_id,
                credential,
                session_generation,
                slot,
                image_generation,
            ),
        }
    }

    pub const fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub const fn session_generation(&self) -> u16 {
        self.session_generation
    }

    pub const fn slot(&self) -> u8 {
        self.slot
    }

    pub const fn image_generation(&self) -> u32 {
        self.image_generation
    }

    pub const fn tag(&self) -> u32 {
        self.tag
    }

    pub(super) fn verify(
        self,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        slot: u8,
        image_generation: u32,
    ) -> Result<(), ImageSlotError> {
        if self.session_generation != session_generation {
            return Err(ImageSlotError::BadSessionGeneration);
        }
        if self.node_id != node_id || self.slot != slot || self.image_generation != image_generation
        {
            return Err(ImageSlotError::AuthFailed);
        }
        if self.tag
            != Self::compute_tag(
                node_id,
                credential,
                session_generation,
                slot,
                image_generation,
            )
        {
            return Err(ImageSlotError::AuthFailed);
        }
        Ok(())
    }

    fn compute_tag(
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        slot: u8,
        image_generation: u32,
    ) -> u32 {
        let mut acc = credential.key() ^ 0x4d47_4d54;
        acc = acc.rotate_left(5) ^ node_id.raw() as u32;
        acc = acc.rotate_left(5) ^ session_generation as u32;
        acc = acc.rotate_left(5) ^ slot as u32;
        acc = acc.rotate_left(5) ^ image_generation;
        acc
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MgmtControl {
    InstallGrant(MgmtInstallGrant),
}

impl MgmtControl {
    pub fn install_grant(
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        slot: u8,
        image_generation: u32,
    ) -> Self {
        Self::InstallGrant(MgmtInstallGrant::new(
            node_id,
            credential,
            session_generation,
            slot,
            image_generation,
        ))
    }
}
