use hibana::{
    g,
    g::{Msg, Role},
    substrate::{
        SessionKit,
        binding::NoBinding,
        ids::SessionId,
        program::{RoleProgram, project},
        runtime::{Config, CounterClock},
        tap::TapEvent,
    },
};
use hibana_pico::{
    choreography::protocol::{
        EngineLabelUniverse, GpioWait, LABEL_MEM_FENCE, LABEL_MGMT_IMAGE_ACTIVATE,
        LABEL_MGMT_IMAGE_BEGIN, LABEL_MGMT_IMAGE_CHUNK, LABEL_MGMT_IMAGE_END,
        LABEL_MGMT_IMAGE_STATUS, MemBorrow, MemFence, MemFenceReason, MgmtImageActivate,
        MgmtImageBegin, MgmtImageChunk, MgmtImageEnd, MgmtStatus, MgmtStatusCode,
    },
    kernel::mgmt::{ActivationBoundary, ImageSlotError, ImageSlotTable, MgmtControl},
    kernel::resolver::PicoInterruptResolver,
    kernel::swarm::{NodeId, SwarmCredential},
    kernel::wasi::MemoryLeaseTable,
    port::host_queue::HostQueueBackend,
    port::transport::SioTransport,
};

type TestTransport<'a> = SioTransport<&'a HostQueueBackend>;
type TestKit<'a> = SessionKit<'a, TestTransport<'a>, EngineLabelUniverse, CounterClock, 1>;

const VALID_MODULE: &[u8] = b"\0asm\x01\0\0\0wasi_snapshot_preview1";

fn project_management_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = g::seq(
        g::send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>, 1>(),
        g::seq(
            g::send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(),
            g::seq(
                g::send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>, 1>(),
                g::seq(
                    g::send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(),
                    g::seq(
                        g::send::<Role<1>, Role<0>, Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>, 1>(),
                        g::seq(
                            g::send::<Role<0>, Role<1>, Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>, 1>(
                            ),
                            g::seq(
                                g::send::<
                                    Role<1>,
                                    Role<0>,
                                    Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>,
                                    1,
                                >(),
                                g::seq(
                                    g::send::<
                                        Role<0>,
                                        Role<1>,
                                        Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>,
                                        1,
                                    >(),
                                    g::seq(
                                        g::send::<
                                            Role<1>,
                                            Role<0>,
                                            Msg<LABEL_MEM_FENCE, MemFence>,
                                            1,
                                        >(),
                                        g::seq(
                                            g::send::<
                                                Role<1>,
                                                Role<0>,
                                                Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>,
                                                1,
                                            >(),
                                            g::send::<
                                                Role<0>,
                                                Role<1>,
                                                Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>,
                                                1,
                                            >(),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    );
    let supervisor: RoleProgram<0> = project(&program);
    let management: RoleProgram<1> = project(&program);
    (supervisor, management)
}

#[test]
fn host_backend_management_install_requires_mem_fence_before_activate() {
    hibana_pico::port::exec::run_current_task(async {
        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register supervisor rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(EngineLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register management rendezvous");

        let sid = SessionId::new(61);
        let (supervisor_program, management_program) = project_management_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut management = cluster1
            .enter(rv1, sid, &management_program, NoBinding)
            .expect("attach management endpoint");

        let mut images: ImageSlotTable<2, 128> = ImageSlotTable::new();
        let mgmt_node = NodeId::new(1);
        let mgmt_credential = SwarmCredential::new(0x4849_4241);
        let mgmt_session = 2;
        let mgmt_grant = MgmtControl::install_grant(mgmt_node, mgmt_credential, mgmt_session, 0, 1);
        let mut leases: MemoryLeaseTable<2> = MemoryLeaseTable::new(4096, 1);
        leases
            .grant_read(MemBorrow::new(1024, 8, 1))
            .expect("seed outstanding lease");
        assert!(leases.has_outstanding_leases());
        let mut resolver: PicoInterruptResolver<1, 2, 2> = PicoInterruptResolver::new();
        resolver
            .request_gpio_wait(GpioWait::new(60, 7, 4, 2))
            .expect("seed outstanding interrupt subscription");
        assert!(resolver.has_active_gpio_waits());

        let begin = MgmtImageBegin::new(0, VALID_MODULE.len() as u32, 1);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>()
            .expect("management flow<image begin>")
            .send(&begin))
        .await
        .expect("send begin");
        let received_begin = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_BEGIN, MgmtImageBegin>>())
            .await
            .expect("recv begin");
        assert_eq!(received_begin, begin);
        let status = images
            .begin_with_control(
                mgmt_grant,
                mgmt_node,
                mgmt_credential,
                mgmt_session,
                received_begin,
            )
            .expect("begin image");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<begin status>")
            .send(&status))
        .await
        .expect("send begin status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("recv begin status")
                .code(),
            MgmtStatusCode::Ok
        );

        let chunk = MgmtImageChunk::new(0, 0, VALID_MODULE).expect("image chunk");
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>()
            .expect("management flow<image chunk>")
            .send(&chunk))
        .await
        .expect("send chunk");
        let received_chunk = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_CHUNK, MgmtImageChunk>>())
            .await
            .expect("recv chunk");
        assert_eq!(received_chunk, chunk);
        let status = images
            .chunk_with_control(
                mgmt_grant,
                mgmt_node,
                mgmt_credential,
                mgmt_session,
                received_chunk,
            )
            .expect("append chunk");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<chunk status>")
            .send(&status))
        .await
        .expect("send chunk status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("recv chunk status")
                .code(),
            MgmtStatusCode::Ok
        );

        let end = MgmtImageEnd::new(0, VALID_MODULE.len() as u32);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>()
            .expect("management flow<image end>")
            .send(&end))
        .await
        .expect("send end");
        let received_end = (supervisor.recv::<Msg<LABEL_MGMT_IMAGE_END, MgmtImageEnd>>())
            .await
            .expect("recv end");
        assert_eq!(received_end, end);
        let status = images
            .end_with_control(
                mgmt_grant,
                mgmt_node,
                mgmt_credential,
                mgmt_session,
                received_end,
            )
            .expect("end image");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<end status>")
            .send(&status))
        .await
        .expect("send end status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("recv end status")
                .code(),
            MgmtStatusCode::Ok
        );

        let activate = MgmtImageActivate::new(0, 2);
        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
            .expect("management flow<activate before fence>")
            .send(&activate))
        .await
        .expect("send activate before fence");
        let received_activate = (supervisor
            .recv::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>())
        .await
        .expect("recv activate before fence");
        assert_eq!(received_activate, activate);
        let activation_error = images
            .activate_with_control(
                mgmt_grant,
                mgmt_node,
                mgmt_credential,
                mgmt_session,
                received_activate,
                ActivationBoundary::single_node(
                    !leases.has_outstanding_leases(),
                    !resolver.has_active_gpio_waits(),
                    leases.epoch(),
                ),
            )
            .expect_err("activation before fence must fail");
        assert_eq!(activation_error, ImageSlotError::NeedFence);
        let status = activation_error.status(received_activate.slot());
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<activate need fence status>")
            .send(&status))
        .await
        .expect("send need fence status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("recv need fence status")
                .code(),
            MgmtStatusCode::NeedFence
        );

        let fence = MemFence::new(MemFenceReason::HotSwap, 2);
        (management
            .flow::<Msg<LABEL_MEM_FENCE, MemFence>>()
            .expect("management flow<mem fence>")
            .send(&fence))
        .await
        .expect("send fence");
        let received_fence = (supervisor.recv::<Msg<LABEL_MEM_FENCE, MemFence>>())
            .await
            .expect("recv fence");
        assert_eq!(received_fence, fence);
        leases.fence(received_fence);
        assert!(!leases.has_outstanding_leases());
        assert_eq!(
            images.activate_with_control(
                mgmt_grant,
                mgmt_node,
                mgmt_credential,
                mgmt_session,
                activate,
                ActivationBoundary::single_node(
                    !leases.has_outstanding_leases(),
                    !resolver.has_active_gpio_waits(),
                    leases.epoch(),
                ),
            ),
            Err(ImageSlotError::NeedFence)
        );
        assert_eq!(resolver.fence_gpio_waits(), 1);
        assert!(!resolver.has_active_gpio_waits());
        assert_eq!(
            images.activate_with_control(
                mgmt_grant,
                mgmt_node,
                mgmt_credential,
                mgmt_session,
                MgmtImageActivate::new(0, 1),
                ActivationBoundary::single_node(
                    !leases.has_outstanding_leases(),
                    !resolver.has_active_gpio_waits(),
                    leases.epoch(),
                ),
            ),
            Err(ImageSlotError::BadFenceEpoch)
        );

        (management
            .flow::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>()
            .expect("management flow<activate after fence>")
            .send(&activate))
        .await
        .expect("send activate after fence");
        let received_activate = (supervisor
            .recv::<Msg<LABEL_MGMT_IMAGE_ACTIVATE, MgmtImageActivate>>())
        .await
        .expect("recv activate after fence");
        let status = images
            .activate_with_control(
                mgmt_grant,
                mgmt_node,
                mgmt_credential,
                mgmt_session,
                received_activate,
                ActivationBoundary::single_node(
                    !leases.has_outstanding_leases(),
                    !resolver.has_active_gpio_waits(),
                    leases.epoch(),
                ),
            )
            .expect("activate after fence");
        (supervisor
            .flow::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>()
            .expect("supervisor flow<activate ok status>")
            .send(&status))
        .await
        .expect("send activate ok status");
        assert_eq!(
            (management.recv::<Msg<LABEL_MGMT_IMAGE_STATUS, MgmtStatus>>())
                .await
                .expect("recv activate ok status")
                .code(),
            MgmtStatusCode::Ok
        );
        assert_eq!(images.active_slot(), Some(0));
    });
}
