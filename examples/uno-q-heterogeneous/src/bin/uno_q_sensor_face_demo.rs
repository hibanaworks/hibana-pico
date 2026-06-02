use std::{
    env,
    process::{Command, ExitCode, ExitStatus},
};

use uno_q_heterogeneous::DEFAULT_UNO_Q_SENSOR_UDP_BIND;

struct Args {
    bind: String,
    serial: Option<String>,
    hardware_bin: String,
    scripted_llm: bool,
    trace: bool,
}

fn main() -> ExitCode {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    let status = match run_hardware_proof(&args) {
        Ok(status) => status,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match run_hardware_proof_via_cargo(&args) {
                Ok(status) => status,
                Err(cargo_error) => {
                    eprintln!(
                        "failed to run uno-q-hardware-proof directly or via cargo: {cargo_error}"
                    );
                    return ExitCode::from(1);
                }
            }
        }
        Err(error) => {
            eprintln!("failed to run uno-q-hardware-proof: {error}");
            return ExitCode::from(1);
        }
    };

    ExitCode::from(status.code().unwrap_or(1) as u8)
}

fn run_hardware_proof(args: &Args) -> std::io::Result<ExitStatus> {
    let mut command = Command::new(&args.hardware_bin);
    append_hardware_args(&mut command, args);
    command.status()
}

fn run_hardware_proof_via_cargo(args: &Args) -> std::io::Result<ExitStatus> {
    let mut command = Command::new("cargo");
    command.args([
        "run",
        "-p",
        "uno-q-heterogeneous",
        "--bin",
        "uno-q-hardware-proof",
        "--",
    ]);
    append_hardware_args(&mut command, args);
    command.status()
}

fn append_hardware_args(command: &mut Command, args: &Args) {
    command.args(["--sensor-bind", &args.bind, "--face-loop-forever"]);
    if let Some(serial) = &args.serial {
        command.args(["--serial", serial]);
    }
    if args.scripted_llm {
        command.arg("--scripted-llm");
    }
    if args.trace {
        command.arg("--trace");
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut bind = DEFAULT_UNO_Q_SENSOR_UDP_BIND.to_owned();
    let mut serial = None;
    let mut hardware_bin =
        env::var("UNO_Q_HARDWARE_PROOF_BIN").unwrap_or_else(|_| "uno-q-hardware-proof".to_owned());
    let mut scripted_llm = false;
    let mut trace = false;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                bind = args
                    .next()
                    .ok_or_else(|| "--bind requires ADDRESS:PORT".to_owned())?;
            }
            "--serial" => {
                serial = Some(
                    args.next()
                        .ok_or_else(|| "--serial requires DEVICE".to_owned())?,
                );
            }
            "--hardware-bin" => {
                hardware_bin = args
                    .next()
                    .ok_or_else(|| "--hardware-bin requires PATH".to_owned())?;
            }
            "--scripted-llm" => scripted_llm = true,
            "--trace" => trace = true,
            "--help" | "-h" => {
                println!(
                    "usage: uno-q-sensor-face-demo [--bind ADDRESS:PORT] [--serial DEVICE] \
[--hardware-bin PATH] [--scripted-llm] [--trace]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}; pass --help")),
        }
    }

    Ok(Args {
        bind,
        serial,
        hardware_bin,
        scripted_llm,
        trace,
    })
}
