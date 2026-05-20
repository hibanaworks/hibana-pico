use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn main() -> Result<(), ProbeError> {
    match probe("/Applications/Codex.app/Contents/Resources/codex") {
        Ok(()) => {
            println!("codex app-server stdio initialize ok");
            return Ok(());
        }
        Err(ProbeError::UnexpectedResponse) | Err(ProbeError::CodexUnavailable) => {}
        Err(error) => return Err(error),
    }

    probe("codex")?;
    println!("codex app-server stdio initialize ok");
    Ok(())
}

fn probe(command: &'static str) -> Result<(), ProbeError> {
    let help = Command::new(command)
        .args(["app-server", "--help"])
        .output()?;
    if !help.status.success() {
        return Err(ProbeError::CodexUnavailable);
    }

    let mut child = Command::new(command)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().ok_or(ProbeError::MissingPipe)?;
    stdin.write_all(INITIALIZE_REQUEST.as_bytes())?;
    stdin.write_all(INITIALIZED_NOTIFICATION.as_bytes())?;
    stdin.flush()?;

    let stdout = child.stdout.take().ok_or(ProbeError::MissingPipe)?;
    let mut reader = BufReader::new(stdout);
    let mut saw_initialize_response = false;
    let mut attempts = 0usize;
    while attempts < 8 {
        let mut response = String::new();
        let bytes = reader.read_line(&mut response)?;
        if bytes == 0 {
            break;
        }
        if response.contains("\"codexHome\"") && response.contains("\"platformOs\"") {
            saw_initialize_response = true;
            break;
        }
        attempts += 1;
    }
    drop(stdin);

    match child.kill() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(ProbeError::Io(error)),
    }
    child.wait()?;

    if saw_initialize_response {
        Ok(())
    } else {
        Err(ProbeError::UnexpectedResponse)
    }
}

const INITIALIZE_REQUEST: &str = concat!(
    "{\"id\":0,\"method\":\"initialize\",\"params\":{",
    "\"clientInfo\":{\"name\":\"hibana-xbot-probe\",\"title\":\"Hibana X Bot Probe\",",
    "\"version\":\"0.1.0\"}}}\n",
);

const INITIALIZED_NOTIFICATION: &str = "{\"method\":\"initialized\",\"params\":{}}\n";

#[derive(Debug)]
enum ProbeError {
    Io(std::io::Error),
    CodexUnavailable,
    MissingPipe,
    UnexpectedResponse,
}

impl core::fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "io error: {error}"),
            Self::CodexUnavailable => write!(formatter, "codex app-server is unavailable"),
            Self::MissingPipe => write!(formatter, "codex app-server pipe was unavailable"),
            Self::UnexpectedResponse => write!(formatter, "unexpected codex app-server response"),
        }
    }
}

impl std::error::Error for ProbeError {}

impl From<std::io::Error> for ProbeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
