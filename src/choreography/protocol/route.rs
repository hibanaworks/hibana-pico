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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteControl<const KIND_LABEL: u8, const ARM: u8>;

impl<const KIND_LABEL: u8, const ARM: u8> ResourceKind for RouteControl<KIND_LABEL, ARM> {
    type Handle = RouteWireHandle;
    const TAG: u8 = <RouteDecisionKind as ResourceKind>::TAG;
    const NAME: &'static str = "RouteControl";

    fn encode_handle(handle: &Self::Handle) -> [u8; CAP_HANDLE_LEN] {
        let mut buf = [0u8; CAP_HANDLE_LEN];
        buf[0] = handle.0;
        buf[1..9].copy_from_slice(&handle.1.to_le_bytes());
        buf
    }

    fn decode_handle(data: [u8; CAP_HANDLE_LEN]) -> Result<Self::Handle, CapError> {
        let mut scope_bytes = [0u8; 8];
        scope_bytes.copy_from_slice(&data[1..9]);
        Ok((data[0], u64::from_le_bytes(scope_bytes)))
    }

    fn zeroize(handle: &mut Self::Handle) {
        *handle = (0, 0);
    }
}

impl<const KIND_LABEL: u8, const ARM: u8> ControlResourceKind for RouteControl<KIND_LABEL, ARM> {
    const SCOPE: ControlScopeKind = ControlScopeKind::Route;
    const TAP_ID: u16 = <RouteDecisionKind as ControlResourceKind>::TAP_ID;
    const SHOT: CapShot = CapShot::One;
    const PATH: ControlPath = ControlPath::Local;
    const OP: ControlOp = ControlOp::RouteDecision;
    const AUTO_MINT_WIRE: bool = false;

    fn mint_handle(sid: SessionId, lane: Lane, scope: ScopeId) -> <Self as ResourceKind>::Handle {
        core::hint::black_box(sid.raw());
        core::hint::black_box(lane.raw());
        (ARM, scope.raw())
    }
}

pub type PublishNormalKind = RouteControl<LABEL_ROUTE_PUBLISH_NORMAL, 0>;
pub type PublishAlertKind = RouteControl<LABEL_ROUTE_PUBLISH_ALERT, 1>;
pub type PublishNormalControl =
    Msg<LABEL_ROUTE_PUBLISH_NORMAL, GenericCapToken<PublishNormalKind>, PublishNormalKind>;
pub type PublishAlertControl =
    Msg<LABEL_ROUTE_PUBLISH_ALERT, GenericCapToken<PublishAlertKind>, PublishAlertKind>;
pub type RemoteSensorRouteKind = RouteControl<LABEL_ROUTE_REMOTE_SENSOR, 0>;
pub type RemoteActuatorRouteKind = RouteControl<LABEL_ROUTE_REMOTE_ACTUATOR, 1>;
pub type RemoteManagementRouteKind = RouteControl<LABEL_ROUTE_REMOTE_MANAGEMENT, 2>;
pub type RemoteTelemetryRouteKind = RouteControl<LABEL_ROUTE_REMOTE_TELEMETRY, 3>;
pub type RemoteRejectRouteKind = RouteControl<LABEL_ROUTE_REMOTE_REJECT, 4>;
pub type RemoteSensorRouteControl =
    Msg<LABEL_ROUTE_REMOTE_SENSOR, GenericCapToken<RemoteSensorRouteKind>, RemoteSensorRouteKind>;
pub type RemoteActuatorRouteControl = Msg<
    LABEL_ROUTE_REMOTE_ACTUATOR,
    GenericCapToken<RemoteActuatorRouteKind>,
    RemoteActuatorRouteKind,
>;
pub type RemoteManagementRouteControl = Msg<
    LABEL_ROUTE_REMOTE_MANAGEMENT,
    GenericCapToken<RemoteManagementRouteKind>,
    RemoteManagementRouteKind,
>;
pub type RemoteTelemetryRouteControl = Msg<
    LABEL_ROUTE_REMOTE_TELEMETRY,
    GenericCapToken<RemoteTelemetryRouteKind>,
    RemoteTelemetryRouteKind,
>;
pub type RemoteRejectRouteControl =
    Msg<LABEL_ROUTE_REMOTE_REJECT, GenericCapToken<RemoteRejectRouteKind>, RemoteRejectRouteKind>;
pub type NetworkDatagramSendRouteKind = RouteControl<LABEL_ROUTE_NETWORK_DATAGRAM_SEND, 0>;
pub type NetworkDatagramRecvRouteKind = RouteControl<LABEL_ROUTE_NETWORK_DATAGRAM_RECV, 1>;
pub type NetworkStreamWriteRouteKind = RouteControl<LABEL_ROUTE_NETWORK_STREAM_WRITE, 2>;
pub type NetworkStreamReadRouteKind = RouteControl<LABEL_ROUTE_NETWORK_STREAM_READ, 3>;
pub type NetworkRejectRouteKind = RouteControl<LABEL_ROUTE_NETWORK_REJECT, 4>;
pub type NetworkAcceptRouteKind = RouteControl<LABEL_ROUTE_NETWORK_ACCEPT, 5>;
pub type NetworkDatagramSendRouteControl = Msg<
    LABEL_ROUTE_NETWORK_DATAGRAM_SEND,
    GenericCapToken<NetworkDatagramSendRouteKind>,
    NetworkDatagramSendRouteKind,
>;
pub type NetworkDatagramRecvRouteControl = Msg<
    LABEL_ROUTE_NETWORK_DATAGRAM_RECV,
    GenericCapToken<NetworkDatagramRecvRouteKind>,
    NetworkDatagramRecvRouteKind,
>;
pub type NetworkStreamWriteRouteControl = Msg<
    LABEL_ROUTE_NETWORK_STREAM_WRITE,
    GenericCapToken<NetworkStreamWriteRouteKind>,
    NetworkStreamWriteRouteKind,
>;
pub type NetworkStreamReadRouteControl = Msg<
    LABEL_ROUTE_NETWORK_STREAM_READ,
    GenericCapToken<NetworkStreamReadRouteKind>,
    NetworkStreamReadRouteKind,
>;
pub type NetworkRejectRouteControl = Msg<
    LABEL_ROUTE_NETWORK_REJECT,
    GenericCapToken<NetworkRejectRouteKind>,
    NetworkRejectRouteKind,
>;
pub type NetworkAcceptRouteControl = Msg<
    LABEL_ROUTE_NETWORK_ACCEPT,
    GenericCapToken<NetworkAcceptRouteKind>,
    NetworkAcceptRouteKind,
>;
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
