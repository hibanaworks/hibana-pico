use std::{
    env,
    fs::OpenOptions,
    io::Read,
    process::Command,
    time::{Duration, Instant},
};

use hibana_pico::{appkit, appkit::ArtifactBundle, site};
use uno_q_heterogeneous::{UnoQCapsule, image};

fn main() {
    run_choreography_proof();

    let serial = env::var("UNO_Q_HIBANA_SERIAL")
        .or_else(|_| env::var("UNO_Q_FACE_SERIAL"))
        .unwrap_or_else(|_| "/dev/ttyHS1".to_owned());

    configure_serial(&serial);
    let mut ready_serial = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&serial)
        .unwrap_or_else(|error| panic!("failed to open {serial}: {error}"));
    reset_m33_appkit_image();
    wait_m33_appkit_ready(&mut ready_serial, &serial, Duration::from_secs(8));
    drop(ready_serial);
    unsafe {
        env::set_var("UNO_Q_HIBANA_SERIAL", &serial);
    }
    run_hardware_split_proof(&serial);

    println!(
        "uno-q hardware proof ok: split appkit images exchanged projected Endpoint/carrier frames over {serial}"
    );
}

fn run_choreography_proof() {
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
}

fn run_hardware_split_proof(serial: &str) {
    type Proof = site::Local<image::HardwarePeerProof>;

    let report =
        appkit::run::<Proof, UnoQCapsule>(uno_q_heterogeneous::ARTIFACTS.for_image::<Proof>());

    assert_eq!(report.image_id(), appkit::ImageId(717));
    assert_eq!(report.site_id(), appkit::SiteId(7107));
    assert_eq!(report.requested_roles(), appkit::RoleSet::from_bits(0x3e));
    assert_eq!(report.attached_endpoint_count(), 5);
    assert_eq!(report.attached_role_kinds().engine, 1);
    assert_eq!(report.attached_role_kinds().driver, 0);
    assert_eq!(report.attached_role_kinds().boundary, 4);
    assert!(report.artifact_len() > 0);
    assert_eq!(
        env::var("UNO_Q_HIBANA_SERIAL").as_deref(),
        Ok(serial),
        "hardware proof must use the configured projected UART carrier"
    );
}

fn configure_serial(path: &str) {
    let status = Command::new("stty")
        .args([
            "-F", path, "115200", "raw", "-echo", "-crtscts", "min", "0", "time", "0",
        ])
        .status()
        .unwrap_or_else(|error| panic!("failed to run stty for {path}: {error}"));
    assert!(status.success(), "stty failed for {path}: {status}");
}

fn reset_m33_appkit_image() {
    if env::var_os("UNO_Q_SKIP_M33_RESET").is_some() {
        return;
    }
    let status = Command::new("/opt/openocd/bin/openocd")
        .args([
            "-d0",
            "-s",
            "/opt/openocd",
            "-f",
            "openocd_gpiod.cfg",
            "-c",
            "init; reset run; shutdown",
        ])
        .status()
        .unwrap_or_else(|error| {
            panic!("failed to reset STM32U585 appkit image before proof: {error}")
        });
    assert!(status.success(), "STM32U585 reset-run failed: {status}");
    std::thread::sleep(Duration::from_millis(300));
}

fn drain_serial(path: &str) {
    let mut serial = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap_or_else(|error| panic!("failed to open {path}: {error}"));

    let mut drain = [0u8; 256];
    let _ = serial.read(&mut drain);
}

fn wait_m33_appkit_ready(serial: &mut std::fs::File, path: &str, timeout: Duration) {
    let marker = b"HIBANA_M33:APPKIT_READY";
    let deadline = Instant::now() + timeout;
    let mut observed = Vec::new();
    while Instant::now() < deadline {
        let mut bytes = [0u8; 128];
        match serial.read(&mut bytes) {
            Ok(0) => {}
            Ok(len) => {
                observed.extend_from_slice(&bytes[..len]);
                if observed
                    .windows(marker.len())
                    .any(|window| window == marker)
                {
                    drain_serial(path);
                    return;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => panic!("failed to read M33 appkit ready marker from {path}: {error}"),
        }
    }
    panic!(
        "M33 appkit image did not emit ready marker on {path}; observed {:?}",
        String::from_utf8_lossy(&observed)
    );
}
