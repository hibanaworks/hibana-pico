use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolicySlot(u8);

impl PolicySlot {
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeTarget<Node> {
    node: Node,
}

impl<Node: Copy> NodeTarget<Node> {
    pub const fn new(node: Node) -> Self {
        Self { node }
    }

    pub const fn node(self) -> Node {
        self.node
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleTarget<Node, Role = u16> {
    node: Node,
    role: Role,
}

impl<Node: Copy, Role: Copy> RoleTarget<Node, Role> {
    pub const fn new(node: Node, role: Role) -> Self {
        Self { node, role }
    }

    pub const fn node(self) -> Node {
        self.node
    }

    pub const fn role(self) -> Role {
        self.role
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteKey<Target> {
    target: Target,
    lane: u8,
    label: u8,
    session_generation: u16,
    policy: PolicySlot,
}

impl<Target: Copy> RouteKey<Target> {
    pub const fn new(
        target: Target,
        lane: u8,
        label: u8,
        session_generation: u16,
        policy: PolicySlot,
    ) -> Self {
        Self {
            target,
            lane,
            label,
            session_generation,
            policy,
        }
    }

    pub const fn target(self) -> Target {
        self.target
    }

    pub const fn lane(self) -> u8 {
        self.lane
    }

    pub const fn route(self) -> u8 {
        self.label
    }

    pub const fn route_label(self) -> u8 {
        self.label
    }

    pub const fn session_generation(self) -> u16 {
        self.session_generation
    }

    pub const fn policy_slot(self) -> u8 {
        self.policy.raw()
    }

    pub const fn policy(self) -> PolicySlot {
        self.policy
    }
}

impl<Node: Copy> RouteKey<NodeTarget<Node>> {
    pub const fn target_node(self) -> Node {
        self.target.node()
    }
}

impl<Node: Copy, Role: Copy> RouteKey<RoleTarget<Node, Role>> {
    pub const fn target_node(self) -> Node {
        self.target.node()
    }

    pub const fn target_role(self) -> Role {
        self.target.role()
    }
}

pub type PublishNormalControl = Msg<LABEL_ROUTE_PUBLISH_NORMAL, (), RouteDecisionKind>;
pub type PublishAlertControl = Msg<LABEL_ROUTE_PUBLISH_ALERT, (), RouteDecisionKind>;
pub type RemoteSensorRouteControl = Msg<LABEL_ROUTE_REMOTE_SENSOR, (), RouteDecisionKind>;
pub type RemoteActuatorRouteControl = Msg<LABEL_ROUTE_REMOTE_ACTUATOR, (), RouteDecisionKind>;
pub type RemoteManagementRouteControl = Msg<LABEL_ROUTE_REMOTE_MANAGEMENT, (), RouteDecisionKind>;
pub type RemoteTelemetryRouteControl = Msg<LABEL_ROUTE_REMOTE_TELEMETRY, (), RouteDecisionKind>;
pub type RemoteRejectRouteControl = Msg<LABEL_ROUTE_REMOTE_REJECT, (), RouteDecisionKind>;
pub type NetworkDatagramSendRouteControl =
    Msg<LABEL_ROUTE_NETWORK_DATAGRAM_SEND, (), RouteDecisionKind>;
pub type NetworkDatagramRecvRouteControl =
    Msg<LABEL_ROUTE_NETWORK_DATAGRAM_RECV, (), RouteDecisionKind>;
pub type NetworkStreamWriteRouteControl =
    Msg<LABEL_ROUTE_NETWORK_STREAM_WRITE, (), RouteDecisionKind>;
pub type NetworkStreamReadRouteControl =
    Msg<LABEL_ROUTE_NETWORK_STREAM_READ, (), RouteDecisionKind>;
pub type NetworkRejectRouteControl = Msg<LABEL_ROUTE_NETWORK_REJECT, (), RouteDecisionKind>;
pub type NetworkAcceptRouteControl = Msg<LABEL_ROUTE_NETWORK_ACCEPT, (), RouteDecisionKind>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsOpenAdmitRoute;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoreoFsOpenRejectRoute;

impl WireEncode for ChoreoFsOpenAdmitRoute {
    fn encoded_len(&self) -> Option<usize> {
        Some(0)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        core::hint::black_box(out.len());
        Ok(0)
    }
}

impl WirePayload for ChoreoFsOpenAdmitRoute {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        if input.as_bytes().is_empty() {
            Ok(Self)
        } else {
            Err(CodecError::Invalid("choreofs admit route is empty"))
        }
    }
}

impl WireEncode for ChoreoFsOpenRejectRoute {
    fn encoded_len(&self) -> Option<usize> {
        Some(0)
    }

    fn encode_into(&self, out: &mut [u8]) -> Result<usize, CodecError> {
        core::hint::black_box(out.len());
        Ok(0)
    }
}

impl WirePayload for ChoreoFsOpenRejectRoute {
    type Decoded<'a> = Self;

    wire_payload_via_decode!();

    fn decode_payload<'a>(input: Payload<'a>) -> Result<Self::Decoded<'a>, CodecError> {
        if input.as_bytes().is_empty() {
            Ok(Self)
        } else {
            Err(CodecError::Invalid("choreofs reject route is empty"))
        }
    }
}

pub type ChoreoFsOpenAdmitRouteMsg = Msg<LABEL_ROUTE_CHOREOFS_OPEN_ADMIT, ChoreoFsOpenAdmitRoute>;
pub type ChoreoFsOpenRejectRouteMsg =
    Msg<LABEL_ROUTE_CHOREOFS_OPEN_REJECT, ChoreoFsOpenRejectRoute>;
