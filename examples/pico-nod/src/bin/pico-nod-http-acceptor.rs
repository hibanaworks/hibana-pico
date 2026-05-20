use std::io::{Read, Write};
use std::net::TcpListener;

use hibana_pico::appkit;
use pico_nod_example::acceptor::{AcceptorError, HttpTlsAcceptor};
use pico_nod_example::ingress::WasiIngress;
use pico_nod_example::protocol::{ActionKind, Generation, IssuerId, TxId, WorkspaceId};
use pico_nod_example::release::{RELEASE_FILE_REQUIREMENTS, RELEASE_REQUIREMENTS, TLS_TERMINATION};

const MAX_REQUEST_BYTES: usize = 8192;
const MAX_HEADER_BYTES: usize = 4096;
const MAX_BODY_BYTES: usize = pico_nod_example::protocol::MAX_BODY_BYTES;

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    if args.preflight || args.production {
        release_preflight(&args)?;
        if args.preflight {
            return Ok(());
        }
    }
    let address = args.address;
    let listener = TcpListener::bind(&address)?;
    eprintln!("pico-nod-http-acceptor listening on {address}");
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut buffer = [0u8; MAX_REQUEST_BYTES];
                let read_len = stream.read(&mut buffer)?;
                let response = handle_request(buffer.split_at(read_len).0);
                stream.write_all(response.as_bytes())?;
            }
            Err(error) => {
                eprintln!("accept failed: {error}");
            }
        }
    }
    Ok(())
}

struct Args {
    address: String,
    preflight: bool,
    production: bool,
}

impl Args {
    fn parse() -> Self {
        let mut address = String::from("127.0.0.1:8787");
        let mut preflight = false;
        let mut production = false;
        for arg in std::env::args().skip(1) {
            match arg.as_str() {
                "--preflight" => preflight = true,
                "--production" => production = true,
                value => address = value.to_owned(),
            }
        }
        Self {
            address,
            preflight,
            production,
        }
    }
}

fn release_preflight(args: &Args) -> std::io::Result<()> {
    let mut missing = 0usize;
    for requirement in RELEASE_REQUIREMENTS {
        if std::env::var_os(requirement.name).is_none() {
            eprintln!("missing: {} ({})", requirement.name, requirement.purpose);
            missing += 1;
        }
    }
    for name in RELEASE_FILE_REQUIREMENTS {
        if let Some(path) = std::env::var_os(name) {
            if std::fs::File::open(&path).is_err() {
                eprintln!("missing: {name} readable file");
                missing += 1;
            }
        }
    }
    match std::env::var(TLS_TERMINATION) {
        Ok(value) if value == "external-loopback" => {}
        Ok(_) => {
            eprintln!("invalid: {TLS_TERMINATION} must be external-loopback");
            missing += 1;
        }
        Err(_) => {}
    }
    if args.production && !is_loopback_bind_address(&args.address) {
        eprintln!("invalid: production bind address must be loopback");
        missing += 1;
    }
    if missing == 0 {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "pico-nod production configuration is incomplete",
    ))
}

fn is_loopback_bind_address(address: &str) -> bool {
    address == "localhost"
        || address.starts_with("localhost:")
        || address.starts_with("127.")
        || address.starts_with("[::1]:")
        || address.starts_with("::1:")
}

fn handle_request(request: &[u8]) -> &'static str {
    let acceptor = HttpTlsAcceptor::new(MAX_HEADER_BYTES, MAX_BODY_BYTES);
    match acceptor.parse(request) {
        Ok(public_request) => {
            let normalized = WasiIngress::normalize_public_request(
                IssuerId(1),
                WorkspaceId(1),
                TxId(1),
                Generation(1),
                ActionKind::Post,
                appkit::ObjectId(1),
                public_request.body,
                b"pico nod public intent",
            );
            match normalized {
                Ok((body, intent)) if body.body_hash() == intent.body_hash => {
                    "HTTP/1.1 202 Accepted\r\nContent-Length: 8\r\n\r\naccepted"
                }
                Ok((body, intent)) => {
                    core::hint::black_box((body, intent));
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 6\r\n\r\nfault\n"
                }
                Err(error) => {
                    core::hint::black_box(error);
                    "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 10\r\n\r\ntoo-large\n"
                }
            }
        }
        Err(error) => response_for_error(error),
    }
}

fn response_for_error(error: AcceptorError) -> &'static str {
    match error {
        AcceptorError::MethodNotAllowed | AcceptorError::PathNotAllowed => {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 10\r\n\r\nnot-found\n"
        }
        AcceptorError::HeaderTooLarge
        | AcceptorError::BodyTooLarge
        | AcceptorError::BodyTruncated => {
            "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 10\r\n\r\ntoo-large\n"
        }
        AcceptorError::MissingHeaderEnd
        | AcceptorError::MissingContentLength
        | AcceptorError::InvalidContentLength
        | AcceptorError::ChunkedUnsupported => {
            "HTTP/1.1 400 Bad Request\r\nContent-Length: 12\r\n\r\nbad-request\n"
        }
    }
}
