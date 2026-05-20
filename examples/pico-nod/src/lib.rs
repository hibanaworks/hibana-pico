#![no_std]

pub mod acceptor;
pub mod apns;
pub mod approval;
pub mod audit;
pub mod billing;
pub mod commit;
pub mod ingress;
pub mod local_app;
pub mod protocol;
pub mod release;
pub mod support;

use core::convert::Infallible;
use core::task::Poll;

use hibana::g;
use hibana::integration::cap::{GenericCapToken, ResourceKind};
use hibana::integration::policy::{ResolverContext, ResolverError, ResolverRef, RouteResolution};
use hibana::integration::program::Projectable;
use hibana::integration::runtime::LabelUniverse;
use hibana_pico::choreography::protocol::RouteControl;
use hibana_pico::{appkit, site};

use protocol::{
    LABEL_APPROVAL_EVIDENCE, LABEL_APPROVAL_REQUEST, LABEL_APPROVED_INTENT, LABEL_FENCE_ROUTE,
    LABEL_INTENT_COMMITTED, LABEL_INTENT_FENCED, LABEL_INTENT_REJECTED, LABEL_INTENT_REQUEST,
    LABEL_NOD_ROUTE, LABEL_NOT_NOD_ROUTE, LABEL_NOTIFICATION_DISPATCHED,
    LABEL_NOTIFY_APPROVAL_DEVICE, LABEL_REJECT_ROUTE, ROLE_APNS_BOUNDARY, ROLE_APPROVAL_BOUNDARY,
    ROLE_APPROVAL_INGRESS, ROLE_AUDIT_BOUNDARY, ROLE_COMMIT_BOUNDARY, ROLE_INTENT_ROUTER,
    ROLE_WASI_INGRESS,
};

pub struct PicoNodCapsule;
pub struct PicoNodPlacement;
pub struct PicoNodLocal;
pub struct PicoNodArtifacts {
    pub wasi_ingress: appkit::WasiImage<'static>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PicoNodUniverse;

impl LabelUniverse for PicoNodUniverse {
    const MAX_LABEL: u8 = LABEL_NOT_NOD_ROUTE;
}

pub const PICO_NOD_CARRIER: appkit::CarrierKind = appkit::CarrierKind::new(80_01);
pub const PICO_NOD_APPROVAL_POLICY: u16 = 80;

type NodKind = RouteControl<LABEL_NOD_ROUTE, 0>;
type NotNodKind = RouteControl<LABEL_NOT_NOD_ROUTE, 1>;
type RejectKind = RouteControl<LABEL_REJECT_ROUTE, 0>;
type FenceKind = RouteControl<LABEL_FENCE_ROUTE, 1>;
type NodRouteMsg = g::Msg<LABEL_NOD_ROUTE, GenericCapToken<NodKind>, NodKind>;
type NotNodRouteMsg = g::Msg<LABEL_NOT_NOD_ROUTE, GenericCapToken<NotNodKind>, NotNodKind>;
type RejectRouteMsg = g::Msg<LABEL_REJECT_ROUTE, GenericCapToken<RejectKind>, RejectKind>;
type FenceRouteMsg = g::Msg<LABEL_FENCE_ROUTE, GenericCapToken<FenceKind>, FenceKind>;

pub mod image {
    pub struct WasiIngressProcess;
    pub struct RouterProcess;
    pub struct ApprovalBoundaryProcess;
    pub struct ApnsBoundaryProcess;
    pub struct ApprovalIngressProcess;
    pub struct CommitBoundaryProcess;
    pub struct AuditBoundaryProcess;
    pub struct HostProofProcess;
}

fn pico_nod_approval_resolver(ctx: ResolverContext) -> Result<RouteResolution, ResolverError> {
    let Some(route_tag) = ctx
        .attr(hibana::integration::policy::signals::core::TAG)
        .map(|value| value.as_u8())
    else {
        return Err(ResolverError::reject());
    };
    if route_tag == <NodKind as ResourceKind>::TAG {
        return Ok(RouteResolution::Arm(0));
    }
    if route_tag == <NotNodKind as ResourceKind>::TAG {
        return Ok(RouteResolution::Arm(1));
    }
    if route_tag == <RejectKind as ResourceKind>::TAG {
        return Ok(RouteResolution::Arm(0));
    }
    if route_tag == <FenceKind as ResourceKind>::TAG {
        return Ok(RouteResolution::Arm(1));
    }
    Err(ResolverError::reject())
}

impl appkit::Capsule for PicoNodCapsule {
    type Universe = PicoNodUniverse;
    type Placement = PicoNodPlacement;
    type Local = PicoNodLocal;
    type Report = Infallible;

    fn choreography() -> impl Projectable<Self::Universe> {
        let nod_path = g::seq(
            g::send::<
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                NodRouteMsg,
                0,
            >()
            .policy::<PICO_NOD_APPROVAL_POLICY>(),
            g::seq(
                g::send::<
                    g::Role<ROLE_APPROVAL_BOUNDARY>,
                    g::Role<ROLE_COMMIT_BOUNDARY>,
                    g::Msg<LABEL_APPROVED_INTENT, u8>,
                    0,
                >(),
                g::send::<
                    g::Role<ROLE_COMMIT_BOUNDARY>,
                    g::Role<ROLE_AUDIT_BOUNDARY>,
                    g::Msg<LABEL_INTENT_COMMITTED, u8>,
                    0,
                >(),
            ),
        );
        let reject_leaf = g::seq(
            g::send::<
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                RejectRouteMsg,
                0,
            >()
            .policy::<PICO_NOD_APPROVAL_POLICY>(),
            g::send::<
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                g::Role<ROLE_AUDIT_BOUNDARY>,
                g::Msg<LABEL_INTENT_REJECTED, u8>,
                0,
            >(),
        );
        let fence_leaf = g::seq(
            g::send::<
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                FenceRouteMsg,
                0,
            >()
            .policy::<PICO_NOD_APPROVAL_POLICY>(),
            g::send::<
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                g::Role<ROLE_AUDIT_BOUNDARY>,
                g::Msg<LABEL_INTENT_FENCED, u8>,
                0,
            >(),
        );
        let not_nod_path = g::seq(
            g::send::<
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                g::Role<ROLE_APPROVAL_BOUNDARY>,
                NotNodRouteMsg,
                0,
            >()
            .policy::<PICO_NOD_APPROVAL_POLICY>(),
            g::route(reject_leaf, fence_leaf),
        );
        g::seq(
            g::send::<
                g::Role<ROLE_WASI_INGRESS>,
                g::Role<ROLE_INTENT_ROUTER>,
                g::Msg<LABEL_INTENT_REQUEST, u8>,
                0,
            >(),
            g::seq(
                g::send::<
                    g::Role<ROLE_INTENT_ROUTER>,
                    g::Role<ROLE_APPROVAL_BOUNDARY>,
                    g::Msg<LABEL_APPROVAL_REQUEST, u8>,
                    0,
                >(),
                g::seq(
                    g::send::<
                        g::Role<ROLE_APPROVAL_BOUNDARY>,
                        g::Role<ROLE_APNS_BOUNDARY>,
                        g::Msg<LABEL_NOTIFY_APPROVAL_DEVICE, u8>,
                        0,
                    >(),
                    g::seq(
                        g::send::<
                            g::Role<ROLE_APNS_BOUNDARY>,
                            g::Role<ROLE_AUDIT_BOUNDARY>,
                            g::Msg<LABEL_NOTIFICATION_DISPATCHED, u8>,
                            0,
                        >(),
                        g::seq(
                            g::send::<
                                g::Role<ROLE_APPROVAL_INGRESS>,
                                g::Role<ROLE_APPROVAL_BOUNDARY>,
                                g::Msg<LABEL_APPROVAL_EVIDENCE, u8>,
                                0,
                            >(),
                            g::route(nod_path, not_nod_path),
                        ),
                    ),
                ),
            ),
        )
    }

    fn register_resolvers<'cfg, R>(registry: &mut R)
    where
        R: appkit::ResolverRegistry<'cfg, Self>,
    {
        registry.policy::<PICO_NOD_APPROVAL_POLICY, ROLE_APPROVAL_BOUNDARY>(ResolverRef::route_fn(
            pico_nod_approval_resolver,
        ));
    }
}

impl appkit::Placement<PicoNodCapsule> for PicoNodPlacement {
    fn role_kind(role: u8) -> appkit::RoleKind {
        match role {
            ROLE_WASI_INGRESS => appkit::RoleKind::Engine,
            ROLE_INTENT_ROUTER => appkit::RoleKind::Driver,
            ROLE_APPROVAL_BOUNDARY
            | ROLE_APNS_BOUNDARY
            | ROLE_APPROVAL_INGRESS
            | ROLE_COMMIT_BOUNDARY => appkit::RoleKind::Boundary,
            ROLE_AUDIT_BOUNDARY => appkit::RoleKind::Supervisor,
            _ => appkit::RoleKind::Link,
        }
    }
}

impl appkit::Localside<PicoNodCapsule> for PicoNodLocal {
    type Error = Infallible;

    fn engine<'endpoint, 'guest, const ROLE: u8>(
        ctx: appkit::EngineCtx<'endpoint, 'guest, PicoNodCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn driver<'a, const ROLE: u8>(
        ctx: appkit::DriverCtx<'a, PicoNodCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn boundary<'a, const ROLE: u8>(
        ctx: appkit::BoundaryCtx<'a, PicoNodCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn link<'a, const ROLE: u8>(
        ctx: appkit::LinkCtx<'a, PicoNodCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }

    fn supervisor<'a, const ROLE: u8>(
        ctx: appkit::SupervisorCtx<'a, PicoNodCapsule, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        ctx.pending()
    }
}

impl appkit::ArtifactForImage<PicoNodCapsule, site::Local<image::WasiIngressProcess>>
    for PicoNodArtifacts
{
    fn artifact_for_image(&self) -> appkit::WasiImage<'static> {
        self.wasi_ingress
    }
}

impl<I> appkit::ArtifactForImage<PicoNodCapsule, I> for PicoNodArtifacts
where
    I: appkit::LogicalImage<PicoNodCapsule, Artifact = appkit::NoWasi>,
{
    fn artifact_for_image(&self) -> I::Artifact {
        appkit::NoWasi
    }
}

pub struct ProofCarrier;
pub struct ProofTx;
pub struct ProofRx;

impl hibana::integration::Transport for ProofCarrier {
    type Error = hibana::integration::transport::TransportError;
    type Tx<'a>
        = ProofTx
    where
        Self: 'a;
    type Rx<'a>
        = ProofRx
    where
        Self: 'a;
    type Metrics = ();

    fn open<'a>(
        &'a self,
        local_role: u8,
        session_id: u32,
        lane: u8,
    ) -> (Self::Tx<'a>, Self::Rx<'a>) {
        core::hint::black_box((local_role, session_id, lane));
        (ProofTx, ProofRx)
    }

    fn poll_send<'a, 'f>(
        &'a self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: hibana::integration::transport::Outgoing<'f>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>>
    where
        'a: 'f,
    {
        core::hint::black_box((self, tx, outgoing, task_context));
        Poll::Ready(Err(hibana::integration::transport::TransportError::Failed))
    }

    fn cancel_send<'a>(&'a self, tx: &'a mut Self::Tx<'a>) {
        core::hint::black_box((self, tx));
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        task_context: &mut core::task::Context<'_>,
    ) -> Poll<Result<hibana::integration::wire::Payload<'a>, Self::Error>> {
        core::hint::black_box((self, rx, task_context));
        Poll::Ready(Err(hibana::integration::transport::TransportError::Failed))
    }

    fn requeue<'a>(&'a self, rx: &'a mut Self::Rx<'a>) {
        core::hint::black_box((self, rx));
    }

    fn drain_events(
        &self,
        emit: &mut dyn FnMut(hibana::integration::transport::advanced::TransportEvent),
    ) {
        core::hint::black_box((self, emit));
    }

    fn recv_frame_hint<'a>(
        &'a self,
        rx: &'a Self::Rx<'a>,
    ) -> Option<hibana::integration::transport::FrameLabel> {
        core::hint::black_box((self, rx));
        None
    }

    fn metrics(&self) -> Self::Metrics {
        core::hint::black_box(self);
    }

    fn apply_pacing_update(&self, interval_us: u32, burst_bytes: u16) {
        core::hint::black_box((self, interval_us, burst_bytes));
    }
}

impl appkit::LogicalImage<PicoNodCapsule> for site::Local<image::WasiIngressProcess> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ProofCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(8001);
    const SITE_ID: appkit::SiteId = appkit::SiteId(8001);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_WASI_INGRESS);
    const CARRIER: appkit::CarrierKind = PICO_NOD_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ProofCarrier
    }
}

impl appkit::LogicalImage<PicoNodCapsule> for site::Local<image::RouterProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ProofCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(8002);
    const SITE_ID: appkit::SiteId = appkit::SiteId(8002);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_INTENT_ROUTER);
    const CARRIER: appkit::CarrierKind = PICO_NOD_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ProofCarrier
    }
}

impl appkit::LogicalImage<PicoNodCapsule> for site::Local<image::ApprovalBoundaryProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ProofCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(8003);
    const SITE_ID: appkit::SiteId = appkit::SiteId(8003);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_APPROVAL_BOUNDARY);
    const CARRIER: appkit::CarrierKind = PICO_NOD_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ProofCarrier
    }
}

impl appkit::LogicalImage<PicoNodCapsule> for site::Local<image::ApnsBoundaryProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ProofCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(8004);
    const SITE_ID: appkit::SiteId = appkit::SiteId(8004);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_APNS_BOUNDARY);
    const CARRIER: appkit::CarrierKind = PICO_NOD_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ProofCarrier
    }
}

impl appkit::LogicalImage<PicoNodCapsule> for site::Local<image::ApprovalIngressProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ProofCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(8005);
    const SITE_ID: appkit::SiteId = appkit::SiteId(8005);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_APPROVAL_INGRESS);
    const CARRIER: appkit::CarrierKind = PICO_NOD_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ProofCarrier
    }
}

impl appkit::LogicalImage<PicoNodCapsule> for site::Local<image::CommitBoundaryProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ProofCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(8006);
    const SITE_ID: appkit::SiteId = appkit::SiteId(8006);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_COMMIT_BOUNDARY);
    const CARRIER: appkit::CarrierKind = PICO_NOD_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ProofCarrier
    }
}

impl appkit::LogicalImage<PicoNodCapsule> for site::Local<image::AuditBoundaryProcess> {
    type Artifact = appkit::NoWasi;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ProofCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(8007);
    const SITE_ID: appkit::SiteId = appkit::SiteId(8007);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(ROLE_AUDIT_BOUNDARY);
    const CARRIER: appkit::CarrierKind = PICO_NOD_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ProofCarrier
    }
}

impl appkit::LogicalImage<PicoNodCapsule> for site::Local<image::HostProofProcess> {
    type Artifact = appkit::WasiImage<'static>;
    type Exit<R> = appkit::RunReport<R, Self>;
    type Carrier<'a> = ProofCarrier;

    const IMAGE_ID: appkit::ImageId = appkit::ImageId(8008);
    const SITE_ID: appkit::SiteId = appkit::SiteId(8008);
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::from_bits(0b0111_1111);
    const CARRIER: appkit::CarrierKind = PICO_NOD_CARRIER;

    fn init() -> Self {
        Self::new()
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a> {
        ProofCarrier
    }
}
