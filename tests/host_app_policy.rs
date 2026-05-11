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
        EngineLabelUniverse, LABEL_PUBLISH_ALERT, LABEL_PUBLISH_NORMAL, MemRights,
        PublishAlertControl, PublishNormalControl,
    },
    kernel::app::{AppId, AppScopeError, AppStreamTable},
    port::host_queue::HostQueueBackend,
    port::transport::SioTransport,
};

type TestTransport<'a> = SioTransport<&'a HostQueueBackend>;
type TestKit<'a> = SessionKit<'a, TestTransport<'a>, EngineLabelUniverse, CounterClock, 2>;

const APP_NORMAL: u32 = 0;
const APP_ALERT: u32 = 1;

fn project_policy_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let app0_arm = g::seq(
        g::send::<Role<0>, Role<0>, PublishNormalControl, 0>(),
        g::send::<Role<0>, Role<1>, Msg<LABEL_PUBLISH_NORMAL, u32>, 0>(),
    );
    let app1_arm = g::seq(
        g::send::<Role<0>, Role<0>, PublishAlertControl, 0>(),
        g::send::<Role<0>, Role<1>, Msg<LABEL_PUBLISH_ALERT, u32>, 0>(),
    );
    let program = g::route(app0_arm, app1_arm);
    let supervisor: RoleProgram<0> = project(&program);
    let engine: RoleProgram<1> = project(&program);
    (supervisor, engine)
}

#[test]
fn host_policy_route_selects_one_app_scope_explicitly() {
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
            .expect("register engine rendezvous");

        let sid = SessionId::new(71);
        let (supervisor_program, engine_program) = project_policy_roles();
        let mut supervisor = cluster0
            .enter(rv0, sid, &supervisor_program, NoBinding)
            .expect("attach supervisor endpoint");
        let mut engine = cluster1
            .enter(rv1, sid, &engine_program, NoBinding)
            .expect("attach engine endpoint");

        let app0 = AppId::new(APP_NORMAL as u8);
        let app1 = AppId::new(APP_ALERT as u8);
        let mut streams: AppStreamTable<2> = AppStreamTable::new();
        let app0_stream = streams
            .open(app0, MemRights::Read)
            .expect("open app0 stream");
        let app1_stream = streams
            .open(app1, MemRights::Read)
            .expect("open app1 stream");

        (supervisor
            .flow::<PublishAlertControl>()
            .expect("supervisor flow<app1 route control>")
            .send(()))
        .await
        .expect("select app1 route");
        (supervisor
            .flow::<Msg<LABEL_PUBLISH_ALERT, u32>>()
            .expect("supervisor flow<app1 policy message>")
            .send(&APP_ALERT))
        .await
        .expect("send selected app");

        let branch = (engine.offer()).await.expect("engine offer policy route");
        assert_eq!(branch.label(), LABEL_PUBLISH_ALERT);
        let selected = (branch.decode::<Msg<LABEL_PUBLISH_ALERT, u32>>())
            .await
            .expect("decode selected app");
        assert_eq!(selected, APP_ALERT);

        streams
            .validate(AppId::new(selected as u8), app1_stream, MemRights::Read)
            .expect("selected app1 stream");
        assert_eq!(
            streams.validate(AppId::new(selected as u8), app0_stream, MemRights::Read),
            Err(AppScopeError::BadApp)
        );
        assert!(
            supervisor.flow::<Msg<LABEL_PUBLISH_NORMAL, u32>>().is_err(),
            "unselected app0 branch must not be reachable after app1 route"
        );
    });
}
