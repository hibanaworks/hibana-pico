use core::{convert::Infallible, task::Poll};

use hibana::{
    g::{self, Msg},
    runtime::{
        program::Projectable,
        transport::{Outgoing, PortOpen, ReceivedFrame, Transport, TransportError},
    },
};
use hibana_pico::appkit;

const TEST_LABEL: u8 = 17;

struct HostCapsule;
struct HostPlacement;
struct HostLocal;
struct HostImage;
struct HostCarrier;
struct HostTx;
struct HostRx;

impl appkit::Capsule for HostCapsule {
    type Placement = HostPlacement;
    type Localside = HostLocal;

    fn choreography() -> impl Projectable {
        g::send::<0, 1, Msg<TEST_LABEL, ()>>()
    }
}

impl appkit::Placement<HostCapsule> for HostPlacement {
    fn role_kind<const ROLE: u8>() -> appkit::RoleKind {
        match ROLE {
            0 => appkit::RoleKind::Driver,
            1 => appkit::RoleKind::Boundary,
            _ => panic!("host placement has no role {ROLE}"),
        }
    }
}

impl appkit::Localside<HostCapsule> for HostLocal {
    type Error = Infallible;

    fn engine<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }

    fn driver<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }

    fn boundary<'endpoint, const ROLE: u8>(
        ctx: hibana::Endpoint<'endpoint, ROLE>,
    ) -> impl core::future::Future<Output = appkit::RoleResult<Self::Error>> {
        appkit::pending(ctx)
    }
}

impl appkit::LogicalImage for HostImage {
    type Capsule = HostCapsule;

    type Carrier<'a> = HostCarrier;
    const REQUESTED_ROLES: appkit::RoleSet = appkit::RoleSet::single(0);

    fn init() -> Self {
        Self
    }

    fn safe_state(&mut self) {}

    fn carrier<'a>() -> Self::Carrier<'a>
    where
        HostCapsule: 'a,
    {
        HostCarrier
    }
}

impl Transport for HostCarrier {
    type Tx<'a>
        = HostTx
    where
        Self: 'a;
    type Rx<'a>
        = HostRx
    where
        Self: 'a;

    fn open<'a>(&'a self, _: PortOpen) -> (Self::Tx<'a>, Self::Rx<'a>) {
        (HostTx, HostRx)
    }

    fn poll_send<'a, 'f>(
        &self,
        _: &'a mut Self::Tx<'a>,
        _: Outgoing<'f>,
        _: &mut core::task::Context<'_>,
    ) -> Poll<Result<(), TransportError>>
    where
        'a: 'f,
    {
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&self, _: &'a mut Self::Tx<'a>) {}

    fn poll_recv<'a>(
        &'a self,
        _: &'a mut Self::Rx<'a>,
        _: &mut core::task::Context<'_>,
    ) -> Poll<Result<ReceivedFrame<'a>, TransportError>> {
        Poll::Pending
    }

    fn requeue<'a>(&self, _: &mut Self::Rx<'a>) -> Result<(), TransportError> {
        Ok(())
    }
}

#[test]
fn host_capsule_uses_current_hibana_surface() {
    appkit::run::<HostImage>(appkit::NoWasi);

    assert_eq!(
        <HostImage as appkit::LogicalImage>::REQUESTED_ROLES,
        appkit::RoleSet::single(0)
    );
}
