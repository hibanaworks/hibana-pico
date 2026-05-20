use hibana_pico::{appkit, appkit::ArtifactBundle, site};
use uno_q_heterogeneous::{UnoQCapsule, image};

fn main() {
    type Proof = site::Local<image::HostLoopbackProof>;

    let report =
        appkit::run::<Proof, UnoQCapsule>(uno_q_heterogeneous::ARTIFACTS.for_image::<Proof>());

    assert_eq!(report.image_id(), appkit::ImageId(710));
    assert_eq!(report.site_id(), appkit::SiteId(7100));
    assert_eq!(report.requested_roles(), appkit::RoleSet::from_bits(0x3f));
    assert_eq!(report.attached_endpoint_count(), 6);
    assert_eq!(report.attached_role_kinds().engine, 1);
    assert_eq!(report.attached_role_kinds().driver, 1);
    assert_eq!(report.attached_role_kinds().boundary, 4);
    assert!(report.artifact_len() > 0);

    println!(
        "uno-q proof ok: choreography authority routed iOS prompt, WASI P1 guest, LLM sidecar, M33 face kernel, and Challenger fd"
    );
}
