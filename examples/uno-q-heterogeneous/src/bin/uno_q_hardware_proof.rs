use std::{
    env,
    fs::OpenOptions,
    io::Read,
    process::Command,
    time::{Duration, Instant},
};

use hibana_pico::appkit;
use uno_q_heterogeneous::{UnoQCapsule, image};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProofMode {
    Once,
    FaceLoop,
}

fn main() {
    let proof_mode = apply_cli_args();
    set_env_default("UNO_Q_HIBANA_UART_TURNAROUND_US", "10000");
    set_env_default("UNO_Q_HIBANA_UART_BYTE_US", "1000");
    if matches!(proof_mode, ProofMode::FaceLoop) {
        eprintln!(
            "uno-q face loop mode: local LLM drives the WASI ChoreoFS shell into /face/frame forever"
        );
    }

    let serial = env::var("UNO_Q_HIBANA_SERIAL")
        .or_else(|_| env::var("UNO_Q_FACE_SERIAL"))
        .unwrap_or_else(|_| "/dev/ttyHS1".to_owned());

    let _serial_services = SerialServiceGuards::stop_for(&serial);
    prepare_micro_sideband();
    configure_serial(&serial);
    let mut ready_serial = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&serial)
        .unwrap_or_else(|error| panic!("failed to open {serial}: {error}"));
    uno_q_heterogeneous::configure_uno_q_uart_modem_ready(&ready_serial)
        .unwrap_or_else(|error| panic!("failed to assert DTR/RTS for {serial}: {error}"));
    reset_m33_appkit_image();
    wait_m33_appkit_ready(&mut ready_serial, &serial, Duration::from_secs(8));
    drop(ready_serial);
    unsafe {
        env::set_var("UNO_Q_HIBANA_SERIAL", &serial);
    }
    run_hardware_split_proof(&serial, proof_mode);

    println!(
        "uno-q hardware proof ok: split appkit images exchanged projected Endpoint/carrier frames over {serial}"
    );
}

fn run_hardware_split_proof(serial: &str, mode: ProofMode) {
    match mode {
        ProofMode::Once => run_hardware_split_proof_once(serial),
        ProofMode::FaceLoop => run_hardware_split_proof_loop(serial),
    }
}

fn assert_hardware_split_image<I>(serial: &str)
where
    I: appkit::LogicalImage<Capsule = UnoQCapsule>,
{
    assert_eq!(I::REQUESTED_ROLES, appkit::RoleSet::from_bits(0x1e));
    assert_eq!(
        env::var("UNO_Q_HIBANA_SERIAL").as_deref(),
        Ok(serial),
        "hardware proof must use the configured projected UART carrier"
    );
}

fn run_hardware_split_proof_once(serial: &str) {
    type Proof = image::HardwarePeerProof;

    appkit::run::<Proof>(image::HardwarePeerProof::wasi_image());
    assert_hardware_split_image::<Proof>(serial);
}

fn run_hardware_split_proof_loop(serial: &str) {
    type Proof = image::HardwarePeerLoopProof;

    appkit::run::<Proof>(image::HardwarePeerLoopProof::wasi_image());
    assert_hardware_split_image::<Proof>(serial);
}

fn apply_cli_args() -> ProofMode {
    let mut proof_mode = if env::var_os("UNO_Q_FACE_LOOP_FOREVER").is_some() {
        ProofMode::FaceLoop
    } else {
        ProofMode::Once
    };
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--prompt-shell" => {
                set_env("UNO_Q_HUMAN_INPUT_MODE", "prompt");
                set_env("UNO_Q_FACE_LOOP_FOREVER", "1");
                proof_mode = ProofMode::FaceLoop;
            }
            "--voice-shell" => {
                set_env("UNO_Q_HUMAN_INPUT_MODE", "voice");
                set_env("UNO_Q_FACE_LOOP_FOREVER", "1");
                proof_mode = ProofMode::FaceLoop;
            }
            "--voice-cmd" => {
                let Some(command) = args.next() else {
                    panic!("--voice-cmd requires a command string");
                };
                set_env("UNO_Q_HUMAN_INPUT_VOICE_CMD", &command);
                set_env("UNO_Q_HUMAN_INPUT_MODE", "voice");
                set_env("UNO_Q_FACE_LOOP_FOREVER", "1");
                proof_mode = ProofMode::FaceLoop;
            }
            "--sensor-udp" => {
                set_env("UNO_Q_PICO2W_SENSOR_MODE", "udp");
                set_env("UNO_Q_FACE_LOOP_FOREVER", "1");
                proof_mode = ProofMode::FaceLoop;
            }
            "--sensor-bind" => {
                let Some(bind) = args.next() else {
                    panic!("--sensor-bind requires ADDRESS:PORT");
                };
                set_env("UNO_Q_PICO2W_SENSOR_UDP_BIND", &bind);
                set_env("UNO_Q_PICO2W_SENSOR_MODE", "udp");
                set_env("UNO_Q_FACE_LOOP_FOREVER", "1");
                proof_mode = ProofMode::FaceLoop;
            }
            "--serial" => {
                let Some(serial) = args.next() else {
                    panic!("--serial requires a device path");
                };
                set_env("UNO_Q_HIBANA_SERIAL", &serial);
            }
            "--trace" => set_env("UNO_Q_HIBANA_TRACE", "1"),
            "--face-loop-forever" => {
                set_env("UNO_Q_FACE_LOOP_FOREVER", "1");
                proof_mode = ProofMode::FaceLoop;
            }
            "--scripted-llm" => set_env("UNO_Q_LOCAL_LLM_SCRIPTED", "1"),
            "--help" | "-h" => {
                println!(
                    "usage: uno-q-hardware-proof [--prompt-shell | --voice-shell] \
[--voice-cmd CMD] [--sensor-udp] [--sensor-bind ADDRESS:PORT] [--serial PATH] \
[--trace] [--face-loop-forever] [--scripted-llm]"
                );
                std::process::exit(0);
            }
            other => panic!("unknown argument {other}; pass --help for usage"),
        }
    }
    proof_mode
}

fn set_env(key: &str, value: &str) {
    unsafe {
        env::set_var(key, value);
    }
}

fn set_env_default(key: &str, value: &str) {
    if env::var_os(key).is_none() {
        set_env(key, value);
    }
}

struct SerialServiceGuards {
    _router: ServiceGuard,
    _getty: Option<ServiceGuard>,
}

impl SerialServiceGuards {
    fn stop_for(path: &str) -> Self {
        let router = ServiceGuard::stop_if_active("arduino-router.service");
        let getty = if path.ends_with("ttyMSM0") {
            Some(ServiceGuard::stop_if_active("serial-getty@ttyMSM0.service"))
        } else {
            None
        };
        Self {
            _router: router,
            _getty: getty,
        }
    }
}

struct ServiceGuard {
    name: &'static str,
    restart: bool,
}

impl ServiceGuard {
    fn stop_if_active(name: &'static str) -> Self {
        if env::var_os("UNO_Q_PRESERVE_SERIAL_SERVICES").is_some() {
            return Self {
                name,
                restart: false,
            };
        }
        let active = Command::new("systemctl")
            .args(["is-active", "--quiet", name])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if active {
            let status = Command::new("systemctl")
                .args(["stop", name])
                .status()
                .unwrap_or_else(|error| panic!("failed to stop {name}: {error}"));
            assert!(status.success(), "failed to stop {name}: {status}");
        }
        Self {
            name,
            restart: active,
        }
    }
}

impl Drop for ServiceGuard {
    fn drop(&mut self) {
        if self.restart {
            let _ = Command::new("systemctl")
                .args(["start", self.name])
                .status();
        }
    }
}

fn prepare_micro_sideband() {
    if env::var_os("UNO_Q_SKIP_MICRO_SIDEBAND").is_some() {
        return;
    }
    pulse_gpio_line("37=0");
    pulse_gpio_line("70=1");
}

fn pulse_gpio_line(line: &str) {
    let status = Command::new("gpioset")
        .args(["-c", "/dev/gpiochip1", "-t0", line])
        .status()
        .unwrap_or_else(|error| panic!("failed to pulse UNO Q sideband GPIO {line}: {error}"));
    assert!(
        status.success(),
        "failed to pulse UNO Q sideband GPIO {line}: {status}"
    );
}

fn configure_serial(path: &str) {
    let device_flag = if cfg!(target_os = "macos") {
        "-f"
    } else {
        "-F"
    };
    let status = Command::new("stty")
        .args([
            device_flag,
            path,
            "115200",
            "raw",
            "-echo",
            "-crtscts",
            "clocal",
            "-hupcl",
            "min",
            "0",
            "time",
            "0",
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
            "reset_config srst_only srst_push_pull; init; reset run; shutdown",
        ])
        .status()
        .unwrap_or_else(|error| {
            panic!("failed to reset STM32U585 appkit image before proof: {error}")
        });
    assert!(status.success(), "STM32U585 reset-run failed: {status}");
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
