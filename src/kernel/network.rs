use core::cell::Cell;

use hibana::{
    g::Msg,
    substrate::wire::{CodecError, Payload, WireEncode, WirePayload},
};

use crate::{
    choreography::protocol::{
        LABEL_NET_DATAGRAM_ACK, LABEL_NET_DATAGRAM_RECV, LABEL_NET_DATAGRAM_RECV_RET,
        LABEL_NET_DATAGRAM_SEND, LABEL_NET_STREAM_ACK, LABEL_NET_STREAM_READ,
        LABEL_NET_STREAM_READ_RET, LABEL_NET_STREAM_WRITE,
    },
    kernel::policy::PolicySlotTable,
    kernel::swarm::{NodeId, SwarmCredential},
};

pub const NET_DATAGRAM_PAYLOAD_CAPACITY: usize = 48;
pub const NET_STREAM_PAYLOAD_CAPACITY: usize = NET_DATAGRAM_PAYLOAD_CAPACITY;
pub const NET_STREAM_FLAG_FIN: u8 = 0b0000_0001;
const NET_STREAM_KNOWN_FLAGS: u8 = NET_STREAM_FLAG_FIN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkError {
    TableFull,
    BadFd,
    BadGeneration,
    BadSessionGeneration,
    AuthFailed,
    BadRoute,
    PolicyDenied,
    PermissionDenied,
    PayloadTooLarge,
    BadFlags,
    Revoked,
    WrongProtocol,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkObjectRejectionTelemetry {
    auth_failed: u16,
    bad_generation: u16,
    bad_route: u16,
    permission_denied: u16,
    revoked: u16,
    wrong_protocol: u16,
    other: u16,
}

impl NetworkObjectRejectionTelemetry {
    pub const fn new() -> Self {
        Self {
            auth_failed: 0,
            bad_generation: 0,
            bad_route: 0,
            permission_denied: 0,
            revoked: 0,
            wrong_protocol: 0,
            other: 0,
        }
    }

    pub const fn auth_failed(self) -> u16 {
        self.auth_failed
    }

    pub const fn bad_generation(self) -> u16 {
        self.bad_generation
    }

    pub const fn bad_route(self) -> u16 {
        self.bad_route
    }

    pub const fn permission_denied(self) -> u16 {
        self.permission_denied
    }

    pub const fn revoked(self) -> u16 {
        self.revoked
    }

    pub const fn wrong_protocol(self) -> u16 {
        self.wrong_protocol
    }

    pub const fn other(self) -> u16 {
        self.other
    }

    pub const fn total(self) -> u16 {
        self.auth_failed
            .saturating_add(self.bad_generation)
            .saturating_add(self.bad_route)
            .saturating_add(self.permission_denied)
            .saturating_add(self.revoked)
            .saturating_add(self.wrong_protocol)
            .saturating_add(self.other)
    }

    fn record(&mut self, error: NetworkError) {
        let slot = match error {
            NetworkError::AuthFailed => &mut self.auth_failed,
            NetworkError::BadGeneration | NetworkError::BadSessionGeneration => {
                &mut self.bad_generation
            }
            NetworkError::BadRoute => &mut self.bad_route,
            NetworkError::PolicyDenied | NetworkError::PermissionDenied => {
                &mut self.permission_denied
            }
            NetworkError::Revoked => &mut self.revoked,
            NetworkError::WrongProtocol => &mut self.wrong_protocol,
            _ => &mut self.other,
        };
        *slot = slot.saturating_add(1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkRights {
    Receive,
    Send,
    SendReceive,
}

impl NetworkRights {
    const fn bits(self) -> u8 {
        match self {
            Self::Receive => 0b01,
            Self::Send => 0b10,
            Self::SendReceive => 0b11,
        }
    }

    pub const fn allows(self, required: Self) -> bool {
        self.bits() & required.bits() == required.bits()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkRoleProtocol {
    Datagram,
    Stream,
}

impl NetworkRoleProtocol {
    const fn code(self) -> u8 {
        match self {
            Self::Datagram => 1,
            Self::Stream => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkObject {
    fd: u8,
    generation: u16,
    target_node: NodeId,
    lane: u8,
    route: u8,
    session_generation: u16,
    policy_slot: u8,
    rights: NetworkRights,
    protocol: NetworkRoleProtocol,
    revoked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkRoute {
    target_node: NodeId,
    lane: u8,
    route: u8,
    session_generation: u16,
    policy_slot: u8,
}

impl NetworkRoute {
    pub const fn new(target_node: NodeId, lane: u8, route: u8, session_generation: u16) -> Self {
        Self::with_policy(target_node, lane, route, session_generation, 0)
    }

    pub const fn with_policy(
        target_node: NodeId,
        lane: u8,
        route: u8,
        session_generation: u16,
        policy_slot: u8,
    ) -> Self {
        Self {
            target_node,
            lane,
            route,
            session_generation,
            policy_slot,
        }
    }

    pub const fn target_node(&self) -> NodeId {
        self.target_node
    }

    pub const fn lane(&self) -> u8 {
        self.lane
    }

    pub const fn route(&self) -> u8 {
        self.route
    }

    pub const fn session_generation(&self) -> u16 {
        self.session_generation
    }

    pub const fn policy_slot(&self) -> u8 {
        self.policy_slot
    }
}

impl NetworkObject {
    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn target_node(&self) -> NodeId {
        self.target_node
    }

    pub const fn lane(&self) -> u8 {
        self.lane
    }

    pub const fn route(&self) -> u8 {
        self.route
    }

    pub const fn session_generation(&self) -> u16 {
        self.session_generation
    }

    pub const fn rights(&self) -> NetworkRights {
        self.rights
    }

    pub const fn protocol(&self) -> NetworkRoleProtocol {
        self.protocol
    }

    pub const fn policy_slot(&self) -> u8 {
        self.policy_slot
    }

    pub const fn route_key(&self) -> NetworkRoute {
        NetworkRoute::with_policy(
            self.target_node,
            self.lane,
            self.route,
            self.session_generation,
            self.policy_slot,
        )
    }

    pub const fn is_revoked(&self) -> bool {
        self.revoked
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkGrant {
    node_id: NodeId,
    target_node: NodeId,
    lane: u8,
    route: u8,
    session_generation: u16,
    policy_slot: u8,
    rights: NetworkRights,
    protocol: NetworkRoleProtocol,
    tag: u32,
}

impl NetworkGrant {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node_id: NodeId,
        credential: SwarmCredential,
        target_node: NodeId,
        lane: u8,
        route: u8,
        session_generation: u16,
        policy_slot: u8,
        rights: NetworkRights,
        protocol: NetworkRoleProtocol,
    ) -> Self {
        Self {
            node_id,
            target_node,
            lane,
            route,
            session_generation,
            policy_slot,
            rights,
            protocol,
            tag: Self::compute_tag(
                node_id,
                credential,
                target_node,
                lane,
                route,
                session_generation,
                policy_slot,
                rights,
                protocol,
            ),
        }
    }

    fn verify(
        self,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
    ) -> Result<(), NetworkError> {
        if self.session_generation != session_generation {
            return Err(NetworkError::BadSessionGeneration);
        }
        if self.node_id != node_id {
            return Err(NetworkError::AuthFailed);
        }
        let expected = Self::compute_tag(
            self.node_id,
            credential,
            self.target_node,
            self.lane,
            self.route,
            self.session_generation,
            self.policy_slot,
            self.rights,
            self.protocol,
        );
        if self.tag != expected {
            return Err(NetworkError::AuthFailed);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_tag(
        node_id: NodeId,
        credential: SwarmCredential,
        target_node: NodeId,
        lane: u8,
        route: u8,
        session_generation: u16,
        policy_slot: u8,
        rights: NetworkRights,
        protocol: NetworkRoleProtocol,
    ) -> u32 {
        let mut acc = credential.key() ^ 0x4e46_4443;
        acc = acc.rotate_left(5) ^ node_id.raw() as u32;
        acc = acc.rotate_left(5) ^ target_node.raw() as u32;
        acc = acc.rotate_left(5) ^ lane as u32;
        acc = acc.rotate_left(5) ^ route as u32;
        acc = acc.rotate_left(5) ^ session_generation as u32;
        acc = acc.rotate_left(5) ^ policy_slot as u32;
        acc = acc.rotate_left(5) ^ rights.bits() as u32;
        acc = acc.rotate_left(5) ^ protocol.code() as u32;
        acc
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkControl {
    CapGrant(NetworkGrant),
    CapRevoke { fd: u8 },
}

impl NetworkControl {
    #[allow(clippy::too_many_arguments)]
    pub fn cap_grant(
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        policy_slot: u8,
        rights: NetworkRights,
        protocol: NetworkRoleProtocol,
    ) -> Self {
        Self::CapGrant(NetworkGrant::new(
            node_id,
            credential,
            target_node,
            lane,
            route,
            session_generation,
            policy_slot,
            rights,
            protocol,
        ))
    }

    pub fn cap_grant_datagram(
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        rights: NetworkRights,
    ) -> Self {
        Self::cap_grant_datagram_with_policy(
            node_id,
            credential,
            session_generation,
            target_node,
            lane,
            route,
            0,
            rights,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn cap_grant_datagram_with_policy(
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        policy_slot: u8,
        rights: NetworkRights,
    ) -> Self {
        Self::cap_grant(
            node_id,
            credential,
            session_generation,
            target_node,
            lane,
            route,
            policy_slot,
            rights,
            NetworkRoleProtocol::Datagram,
        )
    }

    pub fn cap_grant_stream(
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        rights: NetworkRights,
    ) -> Self {
        Self::cap_grant_stream_with_policy(
            node_id,
            credential,
            session_generation,
            target_node,
            lane,
            route,
            0,
            rights,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn cap_grant_stream_with_policy(
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        policy_slot: u8,
        rights: NetworkRights,
    ) -> Self {
        Self::cap_grant(
            node_id,
            credential,
            session_generation,
            target_node,
            lane,
            route,
            policy_slot,
            rights,
            NetworkRoleProtocol::Stream,
        )
    }
}

pub struct NetworkObjectTable<const N: usize> {
    slots: [Option<NetworkObject>; N],
    next_generation: u16,
    rejection_telemetry: Cell<NetworkObjectRejectionTelemetry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkObjectWriteRoute {
    Datagram(NetworkObject),
    Stream(NetworkObject),
    Rejected(NetworkError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkObjectReadRoute {
    Datagram(NetworkObject),
    Stream(NetworkObject),
    Rejected(NetworkError),
}

impl<const N: usize> NetworkObjectTable<N> {
    pub const fn new() -> Self {
        Self {
            slots: [None; N],
            next_generation: 1,
            rejection_telemetry: Cell::new(NetworkObjectRejectionTelemetry::new()),
        }
    }

    pub fn rejection_telemetry(&self) -> NetworkObjectRejectionTelemetry {
        self.rejection_telemetry.get()
    }

    pub fn apply_control(
        &mut self,
        control: NetworkControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
    ) -> Result<NetworkObject, NetworkError> {
        match control {
            NetworkControl::CapGrant(grant) => {
                if let Err(error) = grant.verify(node_id, credential, session_generation) {
                    return Err(self.record_rejection(error));
                }
                self.materialize_cap_grant(
                    grant.target_node,
                    grant.lane,
                    grant.route,
                    grant.session_generation,
                    grant.policy_slot,
                    grant.rights,
                    grant.protocol,
                )
            }
            NetworkControl::CapRevoke { fd } => self.revoke_fd_entry(fd),
        }
    }

    pub fn apply_cap_grant_datagram(
        &mut self,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        rights: NetworkRights,
    ) -> Result<NetworkObject, NetworkError> {
        self.apply_control(
            NetworkControl::cap_grant_datagram(
                node_id,
                credential,
                session_generation,
                target_node,
                lane,
                route,
                rights,
            ),
            node_id,
            credential,
            session_generation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_cap_grant_datagram_with_policy(
        &mut self,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        policy_slot: u8,
        rights: NetworkRights,
    ) -> Result<NetworkObject, NetworkError> {
        self.apply_control(
            NetworkControl::cap_grant_datagram_with_policy(
                node_id,
                credential,
                session_generation,
                target_node,
                lane,
                route,
                policy_slot,
                rights,
            ),
            node_id,
            credential,
            session_generation,
        )
    }

    pub fn apply_cap_grant_stream(
        &mut self,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        rights: NetworkRights,
    ) -> Result<NetworkObject, NetworkError> {
        self.apply_control(
            NetworkControl::cap_grant_stream(
                node_id,
                credential,
                session_generation,
                target_node,
                lane,
                route,
                rights,
            ),
            node_id,
            credential,
            session_generation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_cap_grant_stream_with_policy(
        &mut self,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        target_node: NodeId,
        lane: u8,
        route: u8,
        policy_slot: u8,
        rights: NetworkRights,
    ) -> Result<NetworkObject, NetworkError> {
        self.apply_control(
            NetworkControl::cap_grant_stream_with_policy(
                node_id,
                credential,
                session_generation,
                target_node,
                lane,
                route,
                policy_slot,
                rights,
            ),
            node_id,
            credential,
            session_generation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn materialize_cap_grant(
        &mut self,
        target_node: NodeId,
        lane: u8,
        route: u8,
        session_generation: u16,
        policy_slot: u8,
        rights: NetworkRights,
        protocol: NetworkRoleProtocol,
    ) -> Result<NetworkObject, NetworkError> {
        let fd = match self.slots.iter().position(Option::is_none) {
            Some(fd) => fd as u8,
            None => return Err(self.record_rejection(NetworkError::TableFull)),
        };
        let generation = self.bump_generation();
        let entry = NetworkObject {
            fd,
            generation,
            target_node,
            lane,
            route,
            session_generation,
            policy_slot,
            rights,
            protocol,
            revoked: false,
        };
        self.slots[fd as usize] = Some(entry);
        Ok(entry)
    }

    pub fn resolve(
        &self,
        fd: u8,
        generation: u16,
        required: NetworkRights,
        session_generation: u16,
    ) -> Result<NetworkObject, NetworkError> {
        let entry = match self.slots.get(fd as usize).and_then(|slot| *slot) {
            Some(entry) => entry,
            None => return Err(self.record_rejection(NetworkError::BadFd)),
        };
        if entry.revoked {
            return Err(self.record_rejection(NetworkError::Revoked));
        }
        if entry.generation != generation {
            return Err(self.record_rejection(NetworkError::BadGeneration));
        }
        if entry.session_generation != session_generation {
            return Err(self.record_rejection(NetworkError::BadSessionGeneration));
        }
        if !entry.rights.allows(required) {
            return Err(self.record_rejection(NetworkError::PermissionDenied));
        }
        Ok(entry)
    }

    pub fn resolve_routed(
        &self,
        fd: u8,
        generation: u16,
        required: NetworkRights,
        route: NetworkRoute,
    ) -> Result<NetworkObject, NetworkError> {
        let entry = self.resolve(fd, generation, required, route.session_generation)?;
        if let Err(error) = Self::validate_route(entry, route) {
            return Err(self.record_rejection(error));
        }
        Ok(entry)
    }

    pub fn resolve_route(
        &self,
        required: NetworkRights,
        route: NetworkRoute,
    ) -> Result<NetworkObject, NetworkError> {
        let mut saw_revoked = false;
        let mut saw_wrong_rights = false;

        for entry in self.slots.iter().flatten().copied() {
            if entry.session_generation != route.session_generation {
                continue;
            }
            if Self::validate_route(entry, route).is_err() {
                continue;
            }
            if entry.revoked {
                saw_revoked = true;
                continue;
            }
            if !entry.rights.allows(required) {
                saw_wrong_rights = true;
                continue;
            }
            return Ok(entry);
        }

        if saw_revoked {
            return Err(self.record_rejection(NetworkError::Revoked));
        }
        if saw_wrong_rights {
            return Err(self.record_rejection(NetworkError::PermissionDenied));
        }
        Err(self.record_rejection(NetworkError::BadRoute))
    }

    pub fn route_fd_write(
        &self,
        fd: u8,
        generation: u16,
        session_generation: u16,
    ) -> NetworkObjectWriteRoute {
        match self.resolve(fd, generation, NetworkRights::Send, session_generation) {
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Datagram => {
                NetworkObjectWriteRoute::Datagram(entry)
            }
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Stream => {
                NetworkObjectWriteRoute::Stream(entry)
            }
            Ok(_) => NetworkObjectWriteRoute::Rejected(
                self.record_rejection(NetworkError::WrongProtocol),
            ),
            Err(error) => NetworkObjectWriteRoute::Rejected(error),
        }
    }

    pub fn route_fd_write_routed(
        &self,
        fd: u8,
        generation: u16,
        route: NetworkRoute,
    ) -> NetworkObjectWriteRoute {
        match self.resolve_routed(fd, generation, NetworkRights::Send, route) {
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Datagram => {
                NetworkObjectWriteRoute::Datagram(entry)
            }
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Stream => {
                NetworkObjectWriteRoute::Stream(entry)
            }
            Ok(_) => NetworkObjectWriteRoute::Rejected(
                self.record_rejection(NetworkError::WrongProtocol),
            ),
            Err(error) => NetworkObjectWriteRoute::Rejected(error),
        }
    }

    pub fn route_fd_write_authorized<const P: usize>(
        &self,
        fd: u8,
        generation: u16,
        route: NetworkRoute,
        policy: &PolicySlotTable<P>,
    ) -> NetworkObjectWriteRoute {
        if !policy.is_allowed(route.policy_slot()) {
            return NetworkObjectWriteRoute::Rejected(
                self.record_rejection(NetworkError::PolicyDenied),
            );
        }
        self.route_fd_write_routed(fd, generation, route)
    }

    pub fn route_fd_read(
        &self,
        fd: u8,
        generation: u16,
        session_generation: u16,
    ) -> NetworkObjectReadRoute {
        match self.resolve(fd, generation, NetworkRights::Receive, session_generation) {
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Datagram => {
                NetworkObjectReadRoute::Datagram(entry)
            }
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Stream => {
                NetworkObjectReadRoute::Stream(entry)
            }
            Ok(_) => {
                NetworkObjectReadRoute::Rejected(self.record_rejection(NetworkError::WrongProtocol))
            }
            Err(error) => NetworkObjectReadRoute::Rejected(error),
        }
    }

    pub fn route_fd_read_routed(
        &self,
        fd: u8,
        generation: u16,
        route: NetworkRoute,
    ) -> NetworkObjectReadRoute {
        match self.resolve_routed(fd, generation, NetworkRights::Receive, route) {
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Datagram => {
                NetworkObjectReadRoute::Datagram(entry)
            }
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Stream => {
                NetworkObjectReadRoute::Stream(entry)
            }
            Ok(_) => {
                NetworkObjectReadRoute::Rejected(self.record_rejection(NetworkError::WrongProtocol))
            }
            Err(error) => NetworkObjectReadRoute::Rejected(error),
        }
    }

    pub fn route_receive_routed(&self, route: NetworkRoute) -> NetworkObjectReadRoute {
        match self.resolve_route(NetworkRights::Receive, route) {
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Datagram => {
                NetworkObjectReadRoute::Datagram(entry)
            }
            Ok(entry) if entry.protocol() == NetworkRoleProtocol::Stream => {
                NetworkObjectReadRoute::Stream(entry)
            }
            Ok(_) => {
                NetworkObjectReadRoute::Rejected(self.record_rejection(NetworkError::WrongProtocol))
            }
            Err(error) => NetworkObjectReadRoute::Rejected(error),
        }
    }

    pub fn route_fd_read_authorized<const P: usize>(
        &self,
        fd: u8,
        generation: u16,
        route: NetworkRoute,
        policy: &PolicySlotTable<P>,
    ) -> NetworkObjectReadRoute {
        if !policy.is_allowed(route.policy_slot()) {
            return NetworkObjectReadRoute::Rejected(
                self.record_rejection(NetworkError::PolicyDenied),
            );
        }
        self.route_fd_read_routed(fd, generation, route)
    }

    pub fn revoke_node(&mut self, node_id: NodeId) -> usize {
        let mut revoked = 0;
        for entry in self.slots.iter_mut().flatten() {
            if entry.target_node == node_id && !entry.revoked {
                entry.revoked = true;
                revoked += 1;
            }
        }
        revoked
    }

    pub fn revoke_node_generation(
        &mut self,
        node_id: NodeId,
        session_generation: u16,
    ) -> Result<usize, NetworkError> {
        if self.slots.iter().flatten().any(|entry| {
            entry.target_node == node_id
                && !entry.revoked
                && entry.session_generation != session_generation
        }) {
            return Err(self.record_rejection(NetworkError::BadSessionGeneration));
        }

        let mut revoked = 0;
        for entry in self.slots.iter_mut().flatten() {
            if entry.target_node == node_id && !entry.revoked {
                entry.revoked = true;
                revoked += 1;
            }
        }
        Ok(revoked)
    }

    pub fn revoke_fd(&mut self, fd: u8) -> Result<(), NetworkError> {
        self.revoke_fd_entry(fd).map(|_| ())
    }

    fn revoke_fd_entry(&mut self, fd: u8) -> Result<NetworkObject, NetworkError> {
        let entry = match self.slots.get_mut(fd as usize).and_then(Option::as_mut) {
            Some(entry) => entry,
            None => return Err(self.record_rejection(NetworkError::BadFd)),
        };
        entry.revoked = true;
        Ok(*entry)
    }

    pub fn quiesce_all(&mut self) -> usize {
        let mut revoked = 0;
        for entry in self.slots.iter_mut().flatten() {
            if !entry.revoked {
                entry.revoked = true;
                revoked += 1;
            }
        }
        revoked
    }

    pub fn has_active(&self) -> bool {
        self.slots.iter().flatten().any(|entry| !entry.revoked)
    }

    fn record_rejection(&self, error: NetworkError) -> NetworkError {
        let mut telemetry = self.rejection_telemetry.get();
        telemetry.record(error);
        self.rejection_telemetry.set(telemetry);
        error
    }

    fn validate_route(entry: NetworkObject, route: NetworkRoute) -> Result<(), NetworkError> {
        if entry.target_node != route.target_node
            || entry.lane != route.lane
            || entry.route != route.route
            || entry.policy_slot != route.policy_slot
        {
            return Err(NetworkError::BadRoute);
        }
        Ok(())
    }

    fn bump_generation(&mut self) -> u16 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        if self.next_generation == 0 {
            self.next_generation = 1;
        }
        generation
    }
}

impl<const N: usize> Default for NetworkObjectTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatagramSend {
    fd: u8,
    generation: u16,
    route: u8,
    len: u8,
    payload: [u8; NET_DATAGRAM_PAYLOAD_CAPACITY],
}

impl DatagramSend {
    pub fn new(fd: u8, generation: u16, route: u8, payload: &[u8]) -> Result<Self, NetworkError> {
        if payload.len() > NET_DATAGRAM_PAYLOAD_CAPACITY {
            return Err(NetworkError::PayloadTooLarge);
        }
        let mut out = Self {
            fd,
            generation,
            route,
            len: payload.len() as u8,
            payload: [0; NET_DATAGRAM_PAYLOAD_CAPACITY],
        };
        out.payload[..payload.len()].copy_from_slice(payload);
        Ok(out)
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn route(&self) -> u8 {
        self.route
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.len()]
    }
}

impl WireEncode for DatagramSend {
    fn encoded_len(&self) -> Option<usize> {
        Some(5 + self.len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let len = self.len();
        if out.len() < 5 + len {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3] = self.route;
        out[4] = self.len;
        out[5..5 + len].copy_from_slice(self.payload());
        Ok(5 + len)
    }
}

impl WirePayload for DatagramSend {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 5 {
            return Err(CodecError::Truncated);
        }
        let len = bytes[4] as usize;
        if len > NET_DATAGRAM_PAYLOAD_CAPACITY {
            return Err(CodecError::Invalid("datagram send payload too large"));
        }
        if bytes.len() != 5 + len {
            return Err(CodecError::Invalid("datagram send length mismatch"));
        }
        Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            bytes[3],
            &bytes[5..],
        )
        .map_err(|_| CodecError::Invalid("datagram send payload too large"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatagramAck {
    fd: u8,
    generation: u16,
    accepted: bool,
}

impl DatagramAck {
    pub const fn new(fd: u8, generation: u16, accepted: bool) -> Self {
        Self {
            fd,
            generation,
            accepted,
        }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn accepted(&self) -> bool {
        self.accepted
    }

    pub const fn accepted_for(&self, fd: u8, generation: u16) -> bool {
        self.accepted && self.fd == fd && self.generation == generation
    }
}

impl WireEncode for DatagramAck {
    fn encoded_len(&self) -> Option<usize> {
        Some(4)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 4 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3] = u8::from(self.accepted);
        Ok(4)
    }
}

impl WirePayload for DatagramAck {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 4 {
            return Err(CodecError::Invalid("datagram ack carries four bytes"));
        }
        let accepted = match bytes[3] {
            0 => false,
            1 => true,
            _ => return Err(CodecError::Invalid("datagram ack boolean")),
        };
        Ok(Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            accepted,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatagramRecv {
    fd: u8,
    generation: u16,
    max_len: u8,
}

impl DatagramRecv {
    pub const fn new(fd: u8, generation: u16, max_len: u8) -> Self {
        Self {
            fd,
            generation,
            max_len,
        }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn max_len(&self) -> u8 {
        self.max_len
    }
}

impl WireEncode for DatagramRecv {
    fn encoded_len(&self) -> Option<usize> {
        Some(4)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 4 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3] = self.max_len;
        Ok(4)
    }
}

impl WirePayload for DatagramRecv {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 4 {
            return Err(CodecError::Invalid(
                "datagram recv request carries four bytes",
            ));
        }
        Ok(Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            bytes[3],
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatagramRecvRet {
    fd: u8,
    generation: u16,
    len: u8,
    payload: [u8; NET_DATAGRAM_PAYLOAD_CAPACITY],
}

impl DatagramRecvRet {
    pub fn new(fd: u8, generation: u16, payload: &[u8]) -> Result<Self, NetworkError> {
        if payload.len() > NET_DATAGRAM_PAYLOAD_CAPACITY {
            return Err(NetworkError::PayloadTooLarge);
        }
        let mut out = Self {
            fd,
            generation,
            len: payload.len() as u8,
            payload: [0; NET_DATAGRAM_PAYLOAD_CAPACITY],
        };
        out.payload[..payload.len()].copy_from_slice(payload);
        Ok(out)
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.len()]
    }
}

impl WireEncode for DatagramRecvRet {
    fn encoded_len(&self) -> Option<usize> {
        Some(4 + self.len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let len = self.len();
        if out.len() < 4 + len {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3] = self.len;
        out[4..4 + len].copy_from_slice(self.payload());
        Ok(4 + len)
    }
}

impl WirePayload for DatagramRecvRet {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 4 {
            return Err(CodecError::Truncated);
        }
        let len = bytes[3] as usize;
        if len > NET_DATAGRAM_PAYLOAD_CAPACITY {
            return Err(CodecError::Invalid("datagram recv payload too large"));
        }
        if bytes.len() != 4 + len {
            return Err(CodecError::Invalid("datagram recv length mismatch"));
        }
        Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            &bytes[4..],
        )
        .map_err(|_| CodecError::Invalid("datagram recv payload too large"))
    }
}

pub type DatagramSendMsg = Msg<LABEL_NET_DATAGRAM_SEND, DatagramSend>;
pub type DatagramAckMsg = Msg<LABEL_NET_DATAGRAM_ACK, DatagramAck>;
pub type DatagramRecvMsg = Msg<LABEL_NET_DATAGRAM_RECV, DatagramRecv>;
pub type DatagramRecvRetMsg = Msg<LABEL_NET_DATAGRAM_RECV_RET, DatagramRecvRet>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamWrite {
    fd: u8,
    generation: u16,
    route: u8,
    sequence: u16,
    flags: u8,
    len: u8,
    payload: [u8; NET_STREAM_PAYLOAD_CAPACITY],
}

impl StreamWrite {
    pub fn new(
        fd: u8,
        generation: u16,
        route: u8,
        sequence: u16,
        flags: u8,
        payload: &[u8],
    ) -> Result<Self, NetworkError> {
        if payload.len() > NET_STREAM_PAYLOAD_CAPACITY {
            return Err(NetworkError::PayloadTooLarge);
        }
        if flags & !NET_STREAM_KNOWN_FLAGS != 0 {
            return Err(NetworkError::BadFlags);
        }
        let mut out = Self {
            fd,
            generation,
            route,
            sequence,
            flags,
            len: payload.len() as u8,
            payload: [0; NET_STREAM_PAYLOAD_CAPACITY],
        };
        out.payload[..payload.len()].copy_from_slice(payload);
        Ok(out)
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn route(&self) -> u8 {
        self.route
    }

    pub const fn sequence(&self) -> u16 {
        self.sequence
    }

    pub const fn flags(&self) -> u8 {
        self.flags
    }

    pub const fn is_fin(&self) -> bool {
        self.flags & NET_STREAM_FLAG_FIN != 0
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.len()]
    }
}

impl WireEncode for StreamWrite {
    fn encoded_len(&self) -> Option<usize> {
        Some(8 + self.len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let len = self.len();
        if out.len() < 8 + len {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3] = self.route;
        out[4..6].copy_from_slice(&self.sequence.to_be_bytes());
        out[6] = self.flags;
        out[7] = self.len;
        out[8..8 + len].copy_from_slice(self.payload());
        Ok(8 + len)
    }
}

impl WirePayload for StreamWrite {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 8 {
            return Err(CodecError::Truncated);
        }
        let len = bytes[7] as usize;
        if len > NET_STREAM_PAYLOAD_CAPACITY {
            return Err(CodecError::Invalid("stream write payload too large"));
        }
        if bytes.len() != 8 + len {
            return Err(CodecError::Invalid("stream write length mismatch"));
        }
        Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            bytes[3],
            u16::from_be_bytes([bytes[4], bytes[5]]),
            bytes[6],
            &bytes[8..],
        )
        .map_err(|error| match error {
            NetworkError::BadFlags => CodecError::Invalid("stream write flags"),
            _ => CodecError::Invalid("stream write payload too large"),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamAck {
    fd: u8,
    generation: u16,
    sequence: u16,
    accepted: bool,
}

impl StreamAck {
    pub const fn new(fd: u8, generation: u16, sequence: u16, accepted: bool) -> Self {
        Self {
            fd,
            generation,
            sequence,
            accepted,
        }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn sequence(&self) -> u16 {
        self.sequence
    }

    pub const fn accepted(&self) -> bool {
        self.accepted
    }

    pub const fn accepted_for(&self, fd: u8, generation: u16, sequence: u16) -> bool {
        self.accepted && self.fd == fd && self.generation == generation && self.sequence == sequence
    }
}

impl WireEncode for StreamAck {
    fn encoded_len(&self) -> Option<usize> {
        Some(6)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 6 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3..5].copy_from_slice(&self.sequence.to_be_bytes());
        out[5] = u8::from(self.accepted);
        Ok(6)
    }
}

impl WirePayload for StreamAck {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 6 {
            return Err(CodecError::Invalid("stream ack carries six bytes"));
        }
        let accepted = match bytes[5] {
            0 => false,
            1 => true,
            _ => return Err(CodecError::Invalid("stream ack boolean")),
        };
        Ok(Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            u16::from_be_bytes([bytes[3], bytes[4]]),
            accepted,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamRead {
    fd: u8,
    generation: u16,
    next_sequence: u16,
    max_len: u8,
}

impl StreamRead {
    pub const fn new(fd: u8, generation: u16, next_sequence: u16, max_len: u8) -> Self {
        Self {
            fd,
            generation,
            next_sequence,
            max_len,
        }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn next_sequence(&self) -> u16 {
        self.next_sequence
    }

    pub const fn max_len(&self) -> u8 {
        self.max_len
    }
}

impl WireEncode for StreamRead {
    fn encoded_len(&self) -> Option<usize> {
        Some(6)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 6 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3..5].copy_from_slice(&self.next_sequence.to_be_bytes());
        out[5] = self.max_len;
        Ok(6)
    }
}

impl WirePayload for StreamRead {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 6 {
            return Err(CodecError::Invalid("stream read request carries six bytes"));
        }
        Ok(Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            u16::from_be_bytes([bytes[3], bytes[4]]),
            bytes[5],
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamReadRet {
    fd: u8,
    generation: u16,
    sequence: u16,
    flags: u8,
    len: u8,
    payload: [u8; NET_STREAM_PAYLOAD_CAPACITY],
}

impl StreamReadRet {
    pub fn new(
        fd: u8,
        generation: u16,
        sequence: u16,
        flags: u8,
        payload: &[u8],
    ) -> Result<Self, NetworkError> {
        if payload.len() > NET_STREAM_PAYLOAD_CAPACITY {
            return Err(NetworkError::PayloadTooLarge);
        }
        if flags & !NET_STREAM_KNOWN_FLAGS != 0 {
            return Err(NetworkError::BadFlags);
        }
        let mut out = Self {
            fd,
            generation,
            sequence,
            flags,
            len: payload.len() as u8,
            payload: [0; NET_STREAM_PAYLOAD_CAPACITY],
        };
        out.payload[..payload.len()].copy_from_slice(payload);
        Ok(out)
    }

    pub const fn sequence(&self) -> u16 {
        self.sequence
    }

    pub const fn flags(&self) -> u8 {
        self.flags
    }

    pub const fn is_fin(&self) -> bool {
        self.flags & NET_STREAM_FLAG_FIN != 0
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.len()]
    }
}

impl WireEncode for StreamReadRet {
    fn encoded_len(&self) -> Option<usize> {
        Some(7 + self.len())
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        let len = self.len();
        if out.len() < 7 + len {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3..5].copy_from_slice(&self.sequence.to_be_bytes());
        out[5] = self.flags;
        out[6] = self.len;
        out[7..7 + len].copy_from_slice(self.payload());
        Ok(7 + len)
    }
}

impl WirePayload for StreamReadRet {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() < 7 {
            return Err(CodecError::Truncated);
        }
        let len = bytes[6] as usize;
        if len > NET_STREAM_PAYLOAD_CAPACITY {
            return Err(CodecError::Invalid("stream read payload too large"));
        }
        if bytes.len() != 7 + len {
            return Err(CodecError::Invalid("stream read length mismatch"));
        }
        Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            u16::from_be_bytes([bytes[3], bytes[4]]),
            bytes[5],
            &bytes[7..],
        )
        .map_err(|error| match error {
            NetworkError::BadFlags => CodecError::Invalid("stream read flags"),
            _ => CodecError::Invalid("stream read payload too large"),
        })
    }
}

pub type StreamWriteMsg = Msg<LABEL_NET_STREAM_WRITE, StreamWrite>;
pub type StreamAckMsg = Msg<LABEL_NET_STREAM_ACK, StreamAck>;
pub type StreamReadMsg = Msg<LABEL_NET_STREAM_READ, StreamRead>;
pub type StreamReadRetMsg = Msg<LABEL_NET_STREAM_READ_RET, StreamReadRet>;

#[cfg(test)]
mod tests {
    use super::*;

    const COORDINATOR: NodeId = NodeId::new(1);
    const SENSOR: NodeId = NodeId::new(2);
    const GATEWAY: NodeId = NodeId::new(4);
    const SESSION_GENERATION: u16 = 7;
    const SWARM_CREDENTIAL: SwarmCredential = SwarmCredential::new(0x4849_4241);

    #[test]
    fn stream_payloads_round_trip_and_reject_unknown_flags() {
        let write =
            StreamWrite::new(3, 11, 114, 7, NET_STREAM_FLAG_FIN, b"stream").expect("stream write");
        let mut wire = [0u8; 64];
        let len = write.encode_into(&mut wire).expect("encode stream write");
        assert_eq!(
            StreamWrite::decode_payload(Payload::new(&wire[..len])).expect("decode stream write"),
            write
        );

        let ack = StreamAck::new(3, 11, 7, true);
        let len = ack.encode_into(&mut wire).expect("encode stream ack");
        assert_eq!(
            StreamAck::decode_payload(Payload::new(&wire[..len])).expect("decode stream ack"),
            ack
        );

        let read = StreamRead::new(3, 11, 8, NET_STREAM_PAYLOAD_CAPACITY as u8);
        let len = read.encode_into(&mut wire).expect("encode stream read");
        assert_eq!(
            StreamRead::decode_payload(Payload::new(&wire[..len])).expect("decode stream read"),
            read
        );

        let ret =
            StreamReadRet::new(3, 11, 8, NET_STREAM_FLAG_FIN, b"pipe").expect("stream read ret");
        let len = ret.encode_into(&mut wire).expect("encode stream read ret");
        assert_eq!(
            StreamReadRet::decode_payload(Payload::new(&wire[..len]))
                .expect("decode stream read ret"),
            ret
        );

        assert_eq!(
            StreamWrite::new(3, 11, 114, 7, 0b1000_0000, b"bad"),
            Err(NetworkError::BadFlags)
        );
        assert_eq!(
            StreamReadRet::new(3, 11, 8, 0b1000_0000, b"bad"),
            Err(NetworkError::BadFlags)
        );
    }

    #[test]
    fn network_acks_are_bound_to_materialized_fd_generation_and_sequence() {
        let datagram_ack = DatagramAck::new(3, 11, true);
        assert_eq!(datagram_ack.fd(), 3);
        assert_eq!(datagram_ack.generation(), 11);
        assert!(datagram_ack.accepted_for(3, 11));
        assert!(!datagram_ack.accepted_for(4, 11));
        assert!(!datagram_ack.accepted_for(3, 12));
        assert!(!DatagramAck::new(3, 11, false).accepted_for(3, 11));

        let stream_ack = StreamAck::new(5, 13, 8, true);
        assert_eq!(stream_ack.fd(), 5);
        assert_eq!(stream_ack.generation(), 13);
        assert!(stream_ack.accepted_for(5, 13, 8));
        assert!(!stream_ack.accepted_for(6, 13, 8));
        assert!(!stream_ack.accepted_for(5, 14, 8));
        assert!(!stream_ack.accepted_for(5, 13, 9));
        assert!(!StreamAck::new(5, 13, 8, false).accepted_for(5, 13, 8));
    }

    #[test]
    fn inbound_routes_are_authorized_by_route_not_peer_fd_generation() {
        let mut fds: NetworkObjectTable<3> = NetworkObjectTable::new();
        let _dummy = fds
            .apply_cap_grant_datagram(
                GATEWAY,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                COORDINATOR,
                21,
                LABEL_NET_DATAGRAM_RECV,
                NetworkRights::Receive,
            )
            .expect("pre-seed table");
        let datagram = fds
            .apply_cap_grant_datagram(
                GATEWAY,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                COORDINATOR,
                22,
                LABEL_NET_DATAGRAM_SEND,
                NetworkRights::Receive,
            )
            .expect("install datagram receive route");
        let stream = fds
            .apply_cap_grant_stream(
                GATEWAY,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                COORDINATOR,
                23,
                LABEL_NET_STREAM_WRITE,
                NetworkRights::Receive,
            )
            .expect("install stream receive route");

        assert_ne!(datagram.fd(), 0);
        assert_ne!(datagram.generation(), 1);
        assert_eq!(
            fds.route_receive_routed(NetworkRoute::new(
                COORDINATOR,
                22,
                LABEL_NET_DATAGRAM_SEND,
                SESSION_GENERATION,
            )),
            NetworkObjectReadRoute::Datagram(datagram)
        );
        assert_eq!(
            fds.route_receive_routed(NetworkRoute::new(
                COORDINATOR,
                23,
                LABEL_NET_STREAM_WRITE,
                SESSION_GENERATION,
            )),
            NetworkObjectReadRoute::Stream(stream)
        );
        assert_eq!(
            fds.route_receive_routed(NetworkRoute::new(
                COORDINATOR,
                22,
                LABEL_NET_STREAM_WRITE,
                SESSION_GENERATION,
            )),
            NetworkObjectReadRoute::Rejected(NetworkError::BadRoute)
        );
    }

    #[test]
    fn network_object_table_revoke_and_quiesce_fail_closed() {
        let mut fds: NetworkObjectTable<4> = NetworkObjectTable::new();
        let datagram = fds
            .apply_cap_grant_datagram_with_policy(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                22,
                LABEL_NET_DATAGRAM_SEND,
                3,
                NetworkRights::SendReceive,
            )
            .expect("install authenticated datagram fd");
        assert_eq!(datagram.policy_slot(), 3);
        let stream = fds
            .apply_cap_grant_stream(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                GATEWAY,
                23,
                LABEL_NET_STREAM_WRITE,
                NetworkRights::SendReceive,
            )
            .expect("install authenticated stream fd");
        assert!(fds.has_active());
        assert_eq!(
            fds.route_fd_write(datagram.fd(), datagram.generation(), SESSION_GENERATION),
            NetworkObjectWriteRoute::Datagram(datagram)
        );
        assert_eq!(
            fds.route_fd_write_routed(datagram.fd(), datagram.generation(), datagram.route_key()),
            NetworkObjectWriteRoute::Datagram(datagram)
        );
        assert_eq!(
            fds.route_fd_write_routed(
                datagram.fd(),
                datagram.generation(),
                NetworkRoute::new(
                    SENSOR,
                    22,
                    LABEL_NET_DATAGRAM_SEND,
                    SESSION_GENERATION.wrapping_add(1),
                )
            ),
            NetworkObjectWriteRoute::Rejected(NetworkError::BadSessionGeneration)
        );
        assert_eq!(
            fds.route_fd_write_routed(
                datagram.fd(),
                datagram.generation(),
                NetworkRoute::new(SENSOR, 23, LABEL_NET_DATAGRAM_SEND, SESSION_GENERATION)
            ),
            NetworkObjectWriteRoute::Rejected(NetworkError::BadRoute)
        );
        assert_eq!(
            fds.route_fd_write_routed(
                datagram.fd(),
                datagram.generation(),
                NetworkRoute::with_policy(
                    SENSOR,
                    22,
                    LABEL_NET_DATAGRAM_SEND,
                    SESSION_GENERATION,
                    datagram.policy_slot().wrapping_add(1),
                )
            ),
            NetworkObjectWriteRoute::Rejected(NetworkError::BadRoute)
        );

        fds.revoke_fd(datagram.fd()).expect("revoke datagram fd");
        assert_eq!(
            fds.route_fd_write(datagram.fd(), datagram.generation(), SESSION_GENERATION),
            NetworkObjectWriteRoute::Rejected(NetworkError::Revoked)
        );
        assert!(fds.has_active());

        assert_eq!(
            fds.revoke_node_generation(GATEWAY, SESSION_GENERATION.wrapping_add(1)),
            Err(NetworkError::BadSessionGeneration)
        );
        assert_eq!(
            fds.route_fd_read(stream.fd(), stream.generation(), SESSION_GENERATION),
            NetworkObjectReadRoute::Stream(stream)
        );
        assert_eq!(
            fds.revoke_node_generation(GATEWAY, SESSION_GENERATION),
            Ok(1)
        );
        assert_eq!(
            fds.route_fd_read(stream.fd(), stream.generation(), SESSION_GENERATION),
            NetworkObjectReadRoute::Rejected(NetworkError::Revoked)
        );
        assert!(!fds.has_active());

        let reopened = fds
            .apply_cap_grant_datagram(
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
                SENSOR,
                22,
                LABEL_NET_DATAGRAM_RECV,
                NetworkRights::Receive,
            )
            .expect("install authenticated reopened datagram fd");
        assert!(fds.has_active());
        assert_eq!(fds.quiesce_all(), 1);
        assert!(!fds.has_active());
        assert_eq!(
            fds.route_fd_read(reopened.fd(), reopened.generation(), SESSION_GENERATION),
            NetworkObjectReadRoute::Rejected(NetworkError::Revoked)
        );
    }

    #[test]
    fn authenticated_network_object_control_rejects_forged_or_stale_grants() {
        let grant = NetworkGrant::new(
            COORDINATOR,
            SWARM_CREDENTIAL,
            SENSOR,
            22,
            LABEL_NET_DATAGRAM_SEND,
            SESSION_GENERATION,
            3,
            NetworkRights::SendReceive,
            NetworkRoleProtocol::Datagram,
        );
        let mut fds: NetworkObjectTable<4> = NetworkObjectTable::new();

        assert_eq!(
            fds.apply_control(
                NetworkControl::cap_grant_datagram_with_policy(
                    COORDINATOR,
                    SWARM_CREDENTIAL,
                    SESSION_GENERATION.wrapping_sub(1),
                    SENSOR,
                    22,
                    LABEL_NET_DATAGRAM_SEND,
                    3,
                    NetworkRights::SendReceive,
                ),
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
            ),
            Err(NetworkError::BadSessionGeneration)
        );

        assert_eq!(
            fds.apply_control(
                NetworkControl::cap_grant_datagram_with_policy(
                    COORDINATOR,
                    SwarmCredential::new(0x5752_4f4e),
                    SESSION_GENERATION,
                    SENSOR,
                    22,
                    LABEL_NET_DATAGRAM_SEND,
                    3,
                    NetworkRights::SendReceive,
                ),
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
            ),
            Err(NetworkError::AuthFailed)
        );

        assert_eq!(
            fds.apply_control(
                NetworkControl::cap_grant_datagram_with_policy(
                    GATEWAY,
                    SWARM_CREDENTIAL,
                    SESSION_GENERATION,
                    SENSOR,
                    22,
                    LABEL_NET_DATAGRAM_SEND,
                    3,
                    NetworkRights::SendReceive,
                ),
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
            ),
            Err(NetworkError::AuthFailed)
        );

        let mut forged = grant;
        forged.lane ^= 0x01;
        assert_eq!(
            fds.apply_control(
                NetworkControl::CapGrant(forged),
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
            ),
            Err(NetworkError::AuthFailed)
        );

        let fd = fds
            .apply_control(
                NetworkControl::CapGrant(grant),
                COORDINATOR,
                SWARM_CREDENTIAL,
                SESSION_GENERATION,
            )
            .expect("authorized network object grant installs");
        assert_eq!(fd.target_node(), SENSOR);
        assert_eq!(fd.policy_slot(), 3);
        let mut policy = PolicySlotTable::<4>::new();
        policy.allow(3).expect("allow network policy slot");
        assert_eq!(
            fds.route_fd_write_authorized(fd.fd(), fd.generation(), fd.route_key(), &policy),
            NetworkObjectWriteRoute::Datagram(fd)
        );

        let telemetry = fds.rejection_telemetry();
        assert_eq!(telemetry.auth_failed(), 3);
        assert_eq!(telemetry.bad_generation(), 1);
        assert_eq!(telemetry.total(), 4);
    }
}
