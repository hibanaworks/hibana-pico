use hibana_pico::appkit;
use uno_q_heterogeneous::{UnoQCapsule, image};

fn main() {
    type Proof = image::HostLoopbackProof;

    appkit::run::<Proof>(image::HostLoopbackProof::wasi_image());

    assert_eq!(
        <Proof as appkit::LogicalImage>::REQUESTED_ROLES,
        appkit::RoleSet::from_bits(0x1f)
    );

    println!(
        "uno-q proof ok: WASI ChoreoFS read frames from local LLM and wrote M33 face frames through projected choreography"
    );
}
