use core::cell::Cell;

use hibana::{
    g::Msg,
    substrate::wire::{CodecError, Payload, WireEncode, WirePayload},
};

use crate::{
    choreography::protocol::{
        LABEL_REMOTE_ACTUATE_REQ, LABEL_REMOTE_ACTUATE_RET, LABEL_REMOTE_SAMPLE_REQ,
        LABEL_REMOTE_SAMPLE_RET,
    },
    kernel::policy::PolicySlotTable,
    kernel::swarm::{NodeId, SwarmCredential},
    kernel::transaction::ObjectTransaction,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteError {
    TableFull,
    BadFd,
    BadGeneration,
    BadSessionGeneration,
    AuthFailed,
    BadRoute,
    PolicyDenied,
    PermissionDenied,
    Revoked,
    WrongResource,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RemoteRejectionTelemetry {
    auth_failed: u16,
    bad_generation: u16,
    bad_route: u16,
    permission_denied: u16,
    revoked: u16,
    wrong_resource: u16,
    other: u16,
}

impl RemoteRejectionTelemetry {
    pub const fn new() -> Self {
        Self {
            auth_failed: 0,
            bad_generation: 0,
            bad_route: 0,
            permission_denied: 0,
            revoked: 0,
            wrong_resource: 0,
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

    pub const fn wrong_resource(self) -> u16 {
        self.wrong_resource
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
            .saturating_add(self.wrong_resource)
            .saturating_add(self.other)
    }

    fn record(&mut self, error: RemoteError) {
        let slot = match error {
            RemoteError::AuthFailed => &mut self.auth_failed,
            RemoteError::BadGeneration | RemoteError::BadSessionGeneration => {
                &mut self.bad_generation
            }
            RemoteError::BadRoute => &mut self.bad_route,
            RemoteError::PolicyDenied | RemoteError::PermissionDenied => {
                &mut self.permission_denied
            }
            RemoteError::Revoked => &mut self.revoked,
            RemoteError::WrongResource => &mut self.wrong_resource,
            _ => &mut self.other,
        };
        *slot = slot.saturating_add(1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteRights {
    Read,
    Write,
    ReadWrite,
}

impl RemoteRights {
    const fn bits(self) -> u8 {
        match self {
            Self::Read => 0b01,
            Self::Write => 0b10,
            Self::ReadWrite => 0b11,
        }
    }

    pub const fn allows(self, required: Self) -> bool {
        self.bits() & required.bits() == required.bits()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteResource {
    Sensor,
    Actuator,
    Management,
    Telemetry,
}

impl RemoteResource {
    const fn code(self) -> u8 {
        match self {
            Self::Sensor => 1,
            Self::Actuator => 2,
            Self::Management => 3,
            Self::Telemetry => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteObject {
    fd: u8,
    generation: u16,
    target_node: NodeId,
    target_role: u8,
    lane: u8,
    route: u8,
    session_generation: u16,
    policy_slot: u8,
    rights: RemoteRights,
    resource: RemoteResource,
    revoked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteRoute {
    target_node: NodeId,
    target_role: u8,
    lane: u8,
    route: u8,
    session_generation: u16,
    policy_slot: u8,
}

impl RemoteRoute {
    pub const fn new(
        target_node: NodeId,
        target_role: u8,
        lane: u8,
        route: u8,
        session_generation: u16,
    ) -> Self {
        Self::with_policy(target_node, target_role, lane, route, session_generation, 0)
    }

    pub const fn with_policy(
        target_node: NodeId,
        target_role: u8,
        lane: u8,
        route: u8,
        session_generation: u16,
        policy_slot: u8,
    ) -> Self {
        Self {
            target_node,
            target_role,
            lane,
            route,
            session_generation,
            policy_slot,
        }
    }

    pub const fn target_node(&self) -> NodeId {
        self.target_node
    }

    pub const fn target_role(&self) -> u8 {
        self.target_role
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

impl RemoteObject {
    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn target_node(&self) -> NodeId {
        self.target_node
    }

    pub const fn target_role(&self) -> u8 {
        self.target_role
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

    pub const fn rights(&self) -> RemoteRights {
        self.rights
    }

    pub const fn resource(&self) -> RemoteResource {
        self.resource
    }

    pub const fn policy_slot(&self) -> u8 {
        self.policy_slot
    }

    pub const fn route_key(&self) -> RemoteRoute {
        RemoteRoute::with_policy(
            self.target_node,
            self.target_role,
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
pub struct RemoteGrant {
    node_id: NodeId,
    target_node: NodeId,
    target_role: u8,
    lane: u8,
    route: u8,
    session_generation: u16,
    policy_slot: u8,
    rights: RemoteRights,
    resource: RemoteResource,
    tag: u32,
}

impl RemoteGrant {
    pub fn new(
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
        resource: RemoteResource,
    ) -> Self {
        Self {
            node_id,
            target_node: route.target_node,
            target_role: route.target_role,
            lane: route.lane,
            route: route.route,
            session_generation: route.session_generation,
            policy_slot: route.policy_slot,
            rights,
            resource,
            tag: Self::compute_tag(node_id, credential, route, rights, resource),
        }
    }

    pub const fn route_key(&self) -> RemoteRoute {
        RemoteRoute::with_policy(
            self.target_node,
            self.target_role,
            self.lane,
            self.route,
            self.session_generation,
            self.policy_slot,
        )
    }

    fn verify(
        self,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
    ) -> Result<(), RemoteError> {
        if self.session_generation != session_generation {
            return Err(RemoteError::BadSessionGeneration);
        }
        if self.node_id != node_id {
            return Err(RemoteError::AuthFailed);
        }
        let expected = Self::compute_tag(
            self.node_id,
            credential,
            self.route_key(),
            self.rights,
            self.resource,
        );
        if self.tag != expected {
            return Err(RemoteError::AuthFailed);
        }
        Ok(())
    }

    fn compute_tag(
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
        resource: RemoteResource,
    ) -> u32 {
        let mut acc = credential.key() ^ 0x5246_4443;
        acc = acc.rotate_left(5) ^ node_id.raw() as u32;
        acc = acc.rotate_left(5) ^ route.target_node.raw() as u32;
        acc = acc.rotate_left(5) ^ route.target_role as u32;
        acc = acc.rotate_left(5) ^ route.lane as u32;
        acc = acc.rotate_left(5) ^ route.route as u32;
        acc = acc.rotate_left(5) ^ route.session_generation as u32;
        acc = acc.rotate_left(5) ^ route.policy_slot as u32;
        acc = acc.rotate_left(5) ^ rights.bits() as u32;
        acc = acc.rotate_left(5) ^ resource.code() as u32;
        acc
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteControl {
    CapGrant(RemoteGrant),
    CapRevoke { fd: u8 },
}

impl RemoteControl {
    pub fn cap_grant(
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
        resource: RemoteResource,
    ) -> Self {
        Self::CapGrant(RemoteGrant::new(
            node_id, credential, route, rights, resource,
        ))
    }

    pub fn cap_grant_remote(
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
        resource: RemoteResource,
    ) -> Self {
        Self::cap_grant(node_id, credential, route, rights, resource)
    }

    pub fn cap_grant_management(
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
    ) -> Self {
        Self::cap_grant_remote(
            node_id,
            credential,
            route,
            rights,
            RemoteResource::Management,
        )
    }

    pub fn cap_grant_telemetry(
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
    ) -> Self {
        Self::cap_grant_remote(
            node_id,
            credential,
            route,
            rights,
            RemoteResource::Telemetry,
        )
    }
}

pub struct RemoteObjectTable<const N: usize> {
    slots: [Option<RemoteObject>; N],
    next_generation: u16,
    rejection_telemetry: Cell<RemoteRejectionTelemetry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteFdReadRoute {
    RemoteSensor(RemoteObject),
    Rejected(RemoteError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteFdWriteRoute {
    RemoteActuator(RemoteObject),
    RemoteManagement(RemoteObject),
    RemoteTelemetry(RemoteObject),
    Rejected(RemoteError),
}

impl<const N: usize> RemoteObjectTable<N> {
    pub const fn new() -> Self {
        Self {
            slots: [None; N],
            next_generation: 1,
            rejection_telemetry: Cell::new(RemoteRejectionTelemetry::new()),
        }
    }

    pub fn rejection_telemetry(&self) -> RemoteRejectionTelemetry {
        self.rejection_telemetry.get()
    }

    pub fn apply_control(
        &mut self,
        control: RemoteControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
    ) -> Result<RemoteObject, RemoteError> {
        match control {
            RemoteControl::CapGrant(grant) => {
                if let Err(error) = grant.verify(node_id, credential, session_generation) {
                    return Err(self.record_rejection(error));
                }
                self.materialize_cap_grant(grant.route_key(), grant.rights, grant.resource)
            }
            RemoteControl::CapRevoke { fd } => self.revoke_fd_entry(fd),
        }
    }

    pub fn apply_control_in_tx(
        &mut self,
        control: RemoteControl,
        node_id: NodeId,
        credential: SwarmCredential,
        session_generation: u16,
        tx: ObjectTransaction,
    ) -> Result<RemoteObject, RemoteError> {
        if tx.generation() != session_generation {
            return Err(self.record_rejection(RemoteError::BadSessionGeneration));
        }
        if !tx.is_commit() {
            return Err(self.record_rejection(RemoteError::PolicyDenied));
        }
        self.apply_control(control, node_id, credential, session_generation)
    }

    pub fn apply_cap_grant(
        &mut self,
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
        resource: RemoteResource,
    ) -> Result<RemoteObject, RemoteError> {
        self.apply_cap_grant_with_policy(node_id, credential, route, rights, resource)
    }

    pub fn apply_cap_grant_with_policy(
        &mut self,
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
        resource: RemoteResource,
    ) -> Result<RemoteObject, RemoteError> {
        let session_generation = route.session_generation();
        self.apply_control(
            RemoteControl::cap_grant_remote(node_id, credential, route, rights, resource),
            node_id,
            credential,
            session_generation,
        )
    }

    pub fn apply_cap_grant_management(
        &mut self,
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
    ) -> Result<RemoteObject, RemoteError> {
        let session_generation = route.session_generation();
        self.apply_control(
            RemoteControl::cap_grant_management(node_id, credential, route, rights),
            node_id,
            credential,
            session_generation,
        )
    }

    pub fn apply_cap_grant_telemetry(
        &mut self,
        node_id: NodeId,
        credential: SwarmCredential,
        route: RemoteRoute,
        rights: RemoteRights,
    ) -> Result<RemoteObject, RemoteError> {
        let session_generation = route.session_generation();
        self.apply_control(
            RemoteControl::cap_grant_telemetry(node_id, credential, route, rights),
            node_id,
            credential,
            session_generation,
        )
    }

    fn materialize_cap_grant(
        &mut self,
        route_key: RemoteRoute,
        rights: RemoteRights,
        resource: RemoteResource,
    ) -> Result<RemoteObject, RemoteError> {
        let fd = match self.slots.iter().position(Option::is_none) {
            Some(fd) => fd as u8,
            None => return Err(self.record_rejection(RemoteError::TableFull)),
        };
        let generation = self.bump_generation();
        let cap = RemoteObject {
            fd,
            generation,
            target_node: route_key.target_node(),
            target_role: route_key.target_role(),
            lane: route_key.lane(),
            route: route_key.route(),
            session_generation: route_key.session_generation(),
            policy_slot: route_key.policy_slot(),
            rights,
            resource,
            revoked: false,
        };
        self.slots[fd as usize] = Some(cap);
        Ok(cap)
    }

    pub fn resolve(
        &self,
        fd: u8,
        generation: u16,
        required: RemoteRights,
        session_generation: u16,
    ) -> Result<RemoteObject, RemoteError> {
        let cap = match self.slots.get(fd as usize).and_then(|slot| *slot) {
            Some(cap) => cap,
            None => return Err(self.record_rejection(RemoteError::BadFd)),
        };
        if cap.revoked {
            return Err(self.record_rejection(RemoteError::Revoked));
        }
        if cap.generation != generation {
            return Err(self.record_rejection(RemoteError::BadGeneration));
        }
        if cap.session_generation != session_generation {
            return Err(self.record_rejection(RemoteError::BadSessionGeneration));
        }
        if !cap.rights.allows(required) {
            return Err(self.record_rejection(RemoteError::PermissionDenied));
        }
        Ok(cap)
    }

    pub fn resolve_routed(
        &self,
        fd: u8,
        generation: u16,
        required: RemoteRights,
        route: RemoteRoute,
    ) -> Result<RemoteObject, RemoteError> {
        let cap = self.resolve(fd, generation, required, route.session_generation)?;
        if let Err(error) = Self::validate_route(cap, route) {
            return Err(self.record_rejection(error));
        }
        Ok(cap)
    }

    pub fn route_fd_read(
        &self,
        fd: u8,
        generation: u16,
        session_generation: u16,
    ) -> RemoteFdReadRoute {
        match self.resolve(fd, generation, RemoteRights::Read, session_generation) {
            Ok(cap) if cap.resource() == RemoteResource::Sensor => {
                RemoteFdReadRoute::RemoteSensor(cap)
            }
            Ok(_) => RemoteFdReadRoute::Rejected(self.record_rejection(RemoteError::WrongResource)),
            Err(error) => RemoteFdReadRoute::Rejected(error),
        }
    }

    pub fn route_fd_read_routed(
        &self,
        fd: u8,
        generation: u16,
        route: RemoteRoute,
    ) -> RemoteFdReadRoute {
        match self.resolve_routed(fd, generation, RemoteRights::Read, route) {
            Ok(cap) if cap.resource() == RemoteResource::Sensor => {
                RemoteFdReadRoute::RemoteSensor(cap)
            }
            Ok(_) => RemoteFdReadRoute::Rejected(self.record_rejection(RemoteError::WrongResource)),
            Err(error) => RemoteFdReadRoute::Rejected(error),
        }
    }

    pub fn route_fd_read_authorized<const P: usize>(
        &self,
        fd: u8,
        generation: u16,
        route: RemoteRoute,
        policy: &PolicySlotTable<P>,
    ) -> RemoteFdReadRoute {
        if !policy.is_allowed(route.policy_slot()) {
            return RemoteFdReadRoute::Rejected(self.record_rejection(RemoteError::PolicyDenied));
        }
        self.route_fd_read_routed(fd, generation, route)
    }

    pub fn route_fd_write(
        &self,
        fd: u8,
        generation: u16,
        session_generation: u16,
    ) -> RemoteFdWriteRoute {
        match self.resolve(fd, generation, RemoteRights::Write, session_generation) {
            Ok(cap) if cap.resource() == RemoteResource::Actuator => {
                RemoteFdWriteRoute::RemoteActuator(cap)
            }
            Ok(cap) if cap.resource() == RemoteResource::Management => {
                RemoteFdWriteRoute::RemoteManagement(cap)
            }
            Ok(cap) if cap.resource() == RemoteResource::Telemetry => {
                RemoteFdWriteRoute::RemoteTelemetry(cap)
            }
            Ok(_) => {
                RemoteFdWriteRoute::Rejected(self.record_rejection(RemoteError::WrongResource))
            }
            Err(error) => RemoteFdWriteRoute::Rejected(error),
        }
    }

    pub fn route_fd_write_routed(
        &self,
        fd: u8,
        generation: u16,
        route: RemoteRoute,
    ) -> RemoteFdWriteRoute {
        match self.resolve_routed(fd, generation, RemoteRights::Write, route) {
            Ok(cap) if cap.resource() == RemoteResource::Actuator => {
                RemoteFdWriteRoute::RemoteActuator(cap)
            }
            Ok(cap) if cap.resource() == RemoteResource::Management => {
                RemoteFdWriteRoute::RemoteManagement(cap)
            }
            Ok(cap) if cap.resource() == RemoteResource::Telemetry => {
                RemoteFdWriteRoute::RemoteTelemetry(cap)
            }
            Ok(_) => {
                RemoteFdWriteRoute::Rejected(self.record_rejection(RemoteError::WrongResource))
            }
            Err(error) => RemoteFdWriteRoute::Rejected(error),
        }
    }

    pub fn route_fd_write_authorized<const P: usize>(
        &self,
        fd: u8,
        generation: u16,
        route: RemoteRoute,
        policy: &PolicySlotTable<P>,
    ) -> RemoteFdWriteRoute {
        if !policy.is_allowed(route.policy_slot()) {
            return RemoteFdWriteRoute::Rejected(self.record_rejection(RemoteError::PolicyDenied));
        }
        self.route_fd_write_routed(fd, generation, route)
    }

    pub fn revoke_fd(&mut self, fd: u8) -> Result<(), RemoteError> {
        self.revoke_fd_entry(fd).map(|_| ())
    }

    fn revoke_fd_entry(&mut self, fd: u8) -> Result<RemoteObject, RemoteError> {
        let cap = match self.slots.get_mut(fd as usize).and_then(Option::as_mut) {
            Some(cap) => cap,
            None => return Err(self.record_rejection(RemoteError::BadFd)),
        };
        cap.revoked = true;
        Ok(*cap)
    }

    pub fn revoke_node(&mut self, node_id: NodeId) -> usize {
        let mut revoked = 0;
        for cap in self.slots.iter_mut().flatten() {
            if cap.target_node == node_id && !cap.revoked {
                cap.revoked = true;
                revoked += 1;
            }
        }
        revoked
    }

    pub fn revoke_node_generation(
        &mut self,
        node_id: NodeId,
        session_generation: u16,
    ) -> Result<usize, RemoteError> {
        if self.slots.iter().flatten().any(|cap| {
            cap.target_node == node_id
                && !cap.revoked
                && cap.session_generation != session_generation
        }) {
            return Err(self.record_rejection(RemoteError::BadSessionGeneration));
        }

        let mut revoked = 0;
        for cap in self.slots.iter_mut().flatten() {
            if cap.target_node == node_id && !cap.revoked {
                cap.revoked = true;
                revoked += 1;
            }
        }
        Ok(revoked)
    }

    fn record_rejection(&self, error: RemoteError) -> RemoteError {
        let mut telemetry = self.rejection_telemetry.get();
        telemetry.record(error);
        self.rejection_telemetry.set(telemetry);
        error
    }

    pub fn quiesce_all(&mut self) -> usize {
        let mut revoked = 0;
        for cap in self.slots.iter_mut().flatten() {
            if !cap.revoked {
                cap.revoked = true;
                revoked += 1;
            }
        }
        revoked
    }

    pub fn has_active(&self) -> bool {
        self.slots.iter().flatten().any(|cap| !cap.revoked)
    }

    fn validate_route(cap: RemoteObject, route: RemoteRoute) -> Result<(), RemoteError> {
        if cap.target_node != route.target_node
            || cap.target_role != route.target_role
            || cap.lane != route.lane
            || cap.route != route.route
            || cap.policy_slot != route.policy_slot
        {
            return Err(RemoteError::BadRoute);
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

impl<const N: usize> Default for RemoteObjectTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteSampleRequest {
    fd: u8,
    generation: u16,
    sensor_id: u8,
}

impl RemoteSampleRequest {
    pub const fn new(fd: u8, generation: u16, sensor_id: u8) -> Self {
        Self {
            fd,
            generation,
            sensor_id,
        }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn sensor_id(&self) -> u8 {
        self.sensor_id
    }
}

impl WireEncode for RemoteSampleRequest {
    fn encoded_len(&self) -> Option<usize> {
        Some(4)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 4 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3] = self.sensor_id;
        Ok(4)
    }
}

impl WirePayload for RemoteSampleRequest {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 4 {
            return Err(CodecError::Invalid(
                "remote sample request carries four bytes",
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
pub struct RemoteSample {
    sensor_id: u8,
    result: u8,
    value: u32,
    timestamp: u64,
}

impl RemoteSample {
    pub const fn new(sensor_id: u8, result: u8, value: u32, timestamp: u64) -> Self {
        Self {
            sensor_id,
            result,
            value,
            timestamp,
        }
    }

    pub const fn sensor_id(&self) -> u8 {
        self.sensor_id
    }

    pub const fn result(&self) -> u8 {
        self.result
    }

    pub const fn value(&self) -> u32 {
        self.value
    }

    pub const fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl WireEncode for RemoteSample {
    fn encoded_len(&self) -> Option<usize> {
        Some(14)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 14 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.sensor_id;
        out[1] = self.result;
        out[2..6].copy_from_slice(&self.value.to_be_bytes());
        out[6..14].copy_from_slice(&self.timestamp.to_be_bytes());
        Ok(14)
    }
}

impl WirePayload for RemoteSample {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 14 {
            return Err(CodecError::Invalid("remote sample carries fourteen bytes"));
        }
        let mut value = [0u8; 4];
        let mut timestamp = [0u8; 8];
        value.copy_from_slice(&bytes[2..6]);
        timestamp.copy_from_slice(&bytes[6..14]);
        Ok(Self::new(
            bytes[0],
            bytes[1],
            u32::from_be_bytes(value),
            u64::from_be_bytes(timestamp),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteActuateRequest {
    fd: u8,
    generation: u16,
    channel: u8,
    value: u32,
}

impl RemoteActuateRequest {
    pub const fn new(fd: u8, generation: u16, channel: u8, value: u32) -> Self {
        Self {
            fd,
            generation,
            channel,
            value,
        }
    }

    pub const fn fd(&self) -> u8 {
        self.fd
    }

    pub const fn generation(&self) -> u16 {
        self.generation
    }

    pub const fn channel(&self) -> u8 {
        self.channel
    }

    pub const fn value(&self) -> u32 {
        self.value
    }
}

impl WireEncode for RemoteActuateRequest {
    fn encoded_len(&self) -> Option<usize> {
        Some(8)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 8 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.fd;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3] = self.channel;
        out[4..8].copy_from_slice(&self.value.to_be_bytes());
        Ok(8)
    }
}

impl WirePayload for RemoteActuateRequest {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 8 {
            return Err(CodecError::Invalid(
                "remote actuate request carries eight bytes",
            ));
        }
        let mut value = [0u8; 4];
        value.copy_from_slice(&bytes[4..8]);
        Ok(Self::new(
            bytes[0],
            u16::from_be_bytes([bytes[1], bytes[2]]),
            bytes[3],
            u32::from_be_bytes(value),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteActuateAck {
    channel: u8,
    result: u8,
}

impl RemoteActuateAck {
    pub const fn new(channel: u8, result: u8) -> Self {
        Self { channel, result }
    }

    pub const fn channel(&self) -> u8 {
        self.channel
    }

    pub const fn result(&self) -> u8 {
        self.result
    }
}

impl WireEncode for RemoteActuateAck {
    fn encoded_len(&self) -> Option<usize> {
        Some(2)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        if out.len() < 2 {
            return Err(CodecError::Truncated);
        }
        out[0] = self.channel;
        out[1] = self.result;
        Ok(2)
    }
}

impl WirePayload for RemoteActuateAck {
    type Decoded<'a> = Self;

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        let bytes = input.as_bytes();
        if bytes.len() != 2 {
            return Err(CodecError::Invalid("remote actuate ack carries two bytes"));
        }
        Ok(Self::new(bytes[0], bytes[1]))
    }
}

pub type RemoteSampleReqMsg = Msg<LABEL_REMOTE_SAMPLE_REQ, RemoteSampleRequest>;
pub type RemoteSampleRetMsg = Msg<LABEL_REMOTE_SAMPLE_RET, RemoteSample>;
pub type RemoteActuateReqMsg = Msg<LABEL_REMOTE_ACTUATE_REQ, RemoteActuateRequest>;
pub type RemoteActuateRetMsg = Msg<LABEL_REMOTE_ACTUATE_RET, RemoteActuateAck>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_remote_object_control_rejects_forged_or_stale_grants() {
        let node = NodeId::new(1);
        let target = NodeId::new(2);
        let credential = SwarmCredential::new(0x4849_4241);
        let grant = RemoteGrant::new(
            node,
            credential,
            RemoteRoute::with_policy(target, 0x02, 17, LABEL_REMOTE_SAMPLE_REQ, 7, 3),
            RemoteRights::Read,
            RemoteResource::Sensor,
        );

        let mut table: RemoteObjectTable<4> = RemoteObjectTable::new();
        assert_eq!(
            table.apply_control(
                RemoteControl::cap_grant_remote(
                    node,
                    credential,
                    RemoteRoute::with_policy(target, 0x02, 17, LABEL_REMOTE_SAMPLE_REQ, 6, 3),
                    RemoteRights::Read,
                    RemoteResource::Sensor,
                ),
                node,
                credential,
                7,
            ),
            Err(RemoteError::BadSessionGeneration)
        );

        assert_eq!(
            table.apply_control(
                RemoteControl::cap_grant_remote(
                    node,
                    SwarmCredential::new(0x5752_4f4e),
                    RemoteRoute::with_policy(target, 0x02, 17, LABEL_REMOTE_SAMPLE_REQ, 7, 3),
                    RemoteRights::Read,
                    RemoteResource::Sensor,
                ),
                node,
                credential,
                7,
            ),
            Err(RemoteError::AuthFailed)
        );

        assert_eq!(
            table.apply_control(
                RemoteControl::cap_grant_remote(
                    NodeId::new(9),
                    credential,
                    RemoteRoute::with_policy(target, 0x02, 17, LABEL_REMOTE_SAMPLE_REQ, 7, 3),
                    RemoteRights::Read,
                    RemoteResource::Sensor,
                ),
                node,
                credential,
                7,
            ),
            Err(RemoteError::AuthFailed)
        );

        let mut forged = grant;
        forged.target_role ^= 0x01;
        assert_eq!(
            table.apply_control(RemoteControl::CapGrant(forged), node, credential, 7),
            Err(RemoteError::AuthFailed)
        );

        let cap = table
            .apply_control(RemoteControl::CapGrant(grant), node, credential, 7)
            .expect("authorized remote fd grant installs");
        assert_eq!(cap.target_node(), target);
        assert_eq!(cap.policy_slot(), 3);
        let mut policy = PolicySlotTable::<4>::new();
        policy.allow(3).expect("allow authenticated remote fd");
        assert_eq!(
            table.route_fd_read_authorized(cap.fd(), cap.generation(), cap.route_key(), &policy),
            RemoteFdReadRoute::RemoteSensor(cap)
        );

        let telemetry = table.rejection_telemetry();
        assert_eq!(telemetry.auth_failed(), 3);
        assert_eq!(telemetry.bad_generation(), 1);
        assert_eq!(telemetry.total(), 4);
    }
}
