use hibana::{
    g,
    g::{Msg, Role},
    substrate::{
        SessionKit,
        binding::NoBinding,
        ids::SessionId,
        program::{RoleProgram, project},
        runtime::{Config, CounterClock, LabelUniverse},
        tap::TapEvent,
    },
};
use hibana_pico::{port::host_queue::HostQueueBackend, port::transport::SioTransport};

const LABEL_PING: u8 = 1;
const LABEL_PONG: u8 = 2;
const PING_VALUE: u8 = 0x2a;
const PONG_VALUE: u8 = 0x55;

#[derive(Clone, Copy, Debug, Default)]
struct PingPongLabelUniverse;

impl LabelUniverse for PingPongLabelUniverse {
    const MAX_LABEL: u8 = LABEL_PONG;
}

type TestTransport<'a> = SioTransport<&'a HostQueueBackend>;
type TestKit<'a> = SessionKit<'a, TestTransport<'a>, PingPongLabelUniverse, CounterClock, 1>;

fn project_ping_pong_roles() -> (RoleProgram<0>, RoleProgram<1>) {
    let program = g::seq(
        g::send::<Role<1>, Role<0>, Msg<LABEL_PING, u8>, 0>(),
        g::send::<Role<0>, Role<1>, Msg<LABEL_PONG, u8>, 0>(),
    );
    let core0: RoleProgram<0> = project(&program);
    let core1: RoleProgram<1> = project(&program);
    (core0, core1)
}

#[test]
fn host_backend_roundtrips_hibana_localside_ping_pong() {
    hibana_pico::port::exec::run_current_task(async {
        let backend = HostQueueBackend::new();

        let clock0 = CounterClock::new();
        let mut tap0 = [TapEvent::zero(); 128];
        let mut slab0 = vec![0u8; 262_144];
        let cluster0 = TestKit::new(&clock0);
        let rv0 = cluster0
            .add_rendezvous_from_config(
                Config::new(&mut tap0, slab0.as_mut_slice()).with_universe(PingPongLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register core0 rendezvous");

        let clock1 = CounterClock::new();
        let mut tap1 = [TapEvent::zero(); 128];
        let mut slab1 = vec![0u8; 262_144];
        let cluster1 = TestKit::new(&clock1);
        let rv1 = cluster1
            .add_rendezvous_from_config(
                Config::new(&mut tap1, slab1.as_mut_slice()).with_universe(PingPongLabelUniverse),
                SioTransport::new(&backend),
            )
            .expect("register core1 rendezvous");

        let sid = SessionId::new(17);
        let (core0_program, core1_program) = project_ping_pong_roles();
        let mut core0 = cluster0
            .enter(rv0, sid, &core0_program, NoBinding)
            .expect("attach core0 endpoint");
        let mut core1 = cluster1
            .enter(rv1, sid, &core1_program, NoBinding)
            .expect("attach core1 endpoint");

        let _ = (core1
            .flow::<Msg<LABEL_PING, u8>>()
            .expect("core1 flow<ping>")
            .send(&PING_VALUE))
        .await
        .expect("core1 send ping");

        let ping = (core0.recv::<Msg<LABEL_PING, u8>>())
            .await
            .expect("core0 recv ping");
        assert_eq!(ping, PING_VALUE);

        let _ = (core0
            .flow::<Msg<LABEL_PONG, u8>>()
            .expect("core0 flow<pong>")
            .send(&PONG_VALUE))
        .await
        .expect("core0 send pong");

        let pong = (core1.recv::<Msg<LABEL_PONG, u8>>())
            .await
            .expect("core1 recv pong");
        assert_eq!(pong, PONG_VALUE);
    });
}
