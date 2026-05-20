use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Map, Value};

fn main() -> Result<(), StdioTurnError> {
    let input = match std::env::args().nth(1) {
        Some(input) => input,
        None => String::from("Thanks for sharing this. Keep it concise and friendly."),
    };
    let proposal = run_with_available_codex(&input)?;
    println!("{proposal}");
    Ok(())
}

fn run_with_available_codex(input: &str) -> Result<String, StdioTurnError> {
    match run_with_codex("/Applications/Codex.app/Contents/Resources/codex", input) {
        Ok(proposal) => return Ok(proposal),
        Err(StdioTurnError::CodexUnavailable) | Err(StdioTurnError::UnexpectedResponse) => {}
        Err(error) => return Err(error),
    }
    run_with_codex("codex", input)
}

fn run_with_codex(command: &'static str, input: &str) -> Result<String, StdioTurnError> {
    let help = Command::new(command)
        .args(["app-server", "--help"])
        .output()?;
    if !help.status.success() {
        return Err(StdioTurnError::CodexUnavailable);
    }

    let mut rpc = CodexRpc::spawn(command)?;
    rpc.initialize()?;
    let thread_id = rpc.start_thread()?;
    rpc.start_turn(&thread_id, input)?;
    rpc.wait_for_proposal()
}

struct CodexRpc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    agent_text: String,
}

impl CodexRpc {
    fn spawn(command: &'static str) -> Result<Self, StdioTurnError> {
        let mut child = Command::new(command)
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().ok_or(StdioTurnError::MissingPipe)?;
        let stdout = child.stdout.take().ok_or(StdioTurnError::MissingPipe)?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 0,
            agent_text: String::new(),
        })
    }

    fn initialize(&mut self) -> Result<(), StdioTurnError> {
        let id = self.next_request_id();
        let mut client_info = Map::new();
        client_info.insert(
            String::from("name"),
            Value::String(String::from("hibana-xbot-stdio-turn")),
        );
        client_info.insert(
            String::from("title"),
            Value::String(String::from("Hibana X Bot Stdio Turn")),
        );
        client_info.insert(
            String::from("version"),
            Value::String(String::from("0.1.0")),
        );

        let mut params = Map::new();
        params.insert(String::from("clientInfo"), Value::Object(client_info));
        self.request(id, "initialize", Value::Object(params))?;
        self.read_response(id)?;

        let mut notification = Map::new();
        notification.insert(
            String::from("method"),
            Value::String(String::from("initialized")),
        );
        notification.insert(String::from("params"), Value::Object(Map::new()));
        self.write_value(&Value::Object(notification))
    }

    fn start_thread(&mut self) -> Result<String, StdioTurnError> {
        let id = self.next_request_id();
        let mut params = Map::new();
        params.insert(String::from("ephemeral"), Value::Bool(true));
        params.insert(
            String::from("approvalPolicy"),
            Value::String(String::from("never")),
        );
        params.insert(
            String::from("sandbox"),
            Value::String(String::from("read-only")),
        );
        params.insert(String::from("cwd"), Value::String(current_dir_string()?));
        params.insert(
            String::from("baseInstructions"),
            Value::String(String::from(
                "You are the proposal engine for a Hibana xbot proof. \
                 You may only propose bounded reply text. Do not approve, \
                 do not claim authority, do not call tools, and do not post.",
            )),
        );
        params.insert(
            String::from("developerInstructions"),
            Value::String(String::from(
                "Return a short reply proposal only. The host will treat it as \
                 untrusted proposal evidence and will reject anything outside \
                 the output schema.",
            )),
        );

        self.request(id, "thread/start", Value::Object(params))?;
        let response = self.read_response(id)?;
        response
            .get("result")
            .and_then(|result| result.get("thread"))
            .and_then(|thread| thread.get("id"))
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or(StdioTurnError::UnexpectedResponse)
    }

    fn start_turn(&mut self, thread_id: &str, input: &str) -> Result<(), StdioTurnError> {
        let id = self.next_request_id();
        let mut params = Map::new();
        params.insert(
            String::from("threadId"),
            Value::String(String::from(thread_id)),
        );
        params.insert(String::from("input"), Value::Array(vec![text_input(input)]));
        params.insert(
            String::from("approvalPolicy"),
            Value::String(String::from("never")),
        );
        params.insert(String::from("sandboxPolicy"), read_only_sandbox_policy());
        params.insert(String::from("outputSchema"), proposal_schema());

        self.request(id, "turn/start", Value::Object(params))?;
        self.read_response(id)?;
        Ok(())
    }

    fn wait_for_proposal(&mut self) -> Result<String, StdioTurnError> {
        let mut remaining = 240usize;
        while remaining > 0 {
            let message = self.read_message()?;
            if self.capture_agent_delta(&message) {
                remaining -= 1;
                continue;
            }
            if let Some(proposal) = proposal_from_turn_completed(&message) {
                self.shutdown()?;
                return Ok(proposal);
            }
            if message.get("method").and_then(Value::as_str) == Some("turn/completed") {
                let proposal = proposal_from_text(&self.agent_text)?;
                self.shutdown()?;
                return Ok(proposal);
            }
            remaining -= 1;
        }
        Err(StdioTurnError::UnexpectedResponse)
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> Result<(), StdioTurnError> {
        let mut request = Map::new();
        request.insert(String::from("id"), Value::from(id));
        request.insert(String::from("method"), Value::String(String::from(method)));
        request.insert(String::from("params"), params);
        self.write_value(&Value::Object(request))
    }

    fn write_value(&mut self, value: &Value) -> Result<(), StdioTurnError> {
        serde_json::to_writer(&mut self.stdin, value)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn read_response(&mut self, id: u64) -> Result<Value, StdioTurnError> {
        let mut remaining = 80usize;
        while remaining > 0 {
            let message = self.read_message()?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if message.get("error").is_some() {
                    return Err(StdioTurnError::AppServerError(message.to_string()));
                }
                return Ok(message);
            }
            self.capture_agent_delta(&message);
            remaining -= 1;
        }
        Err(StdioTurnError::UnexpectedResponse)
    }

    fn read_message(&mut self) -> Result<Value, StdioTurnError> {
        let mut line = String::new();
        let bytes = self.stdout.read_line(&mut line)?;
        if bytes == 0 {
            return Err(StdioTurnError::UnexpectedResponse);
        }
        Ok(serde_json::from_str(&line)?)
    }

    fn capture_agent_delta(&mut self, message: &Value) -> bool {
        if message.get("method").and_then(Value::as_str) != Some("item/agentMessage/delta") {
            return false;
        }
        if let Some(delta) = message
            .get("params")
            .and_then(|params| params.get("delta"))
            .and_then(Value::as_str)
        {
            self.agent_text.push_str(delta);
        }
        true
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn shutdown(&mut self) -> Result<(), StdioTurnError> {
        match self.stdin.flush() {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(error) => return Err(StdioTurnError::Io(error)),
        }
        match self.child.kill() {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
            Err(error) => return Err(StdioTurnError::Io(error)),
        }
        self.child.wait()?;
        Ok(())
    }
}

fn current_dir_string() -> Result<String, StdioTurnError> {
    let path: PathBuf = std::env::current_dir()?;
    Ok(path.to_string_lossy().into_owned())
}

fn text_input(input: &str) -> Value {
    let mut text = Map::new();
    text.insert(String::from("type"), Value::String(String::from("text")));
    text.insert(String::from("text"), Value::String(String::from(input)));
    text.insert(String::from("text_elements"), Value::Array(Vec::new()));
    Value::Object(text)
}

fn read_only_sandbox_policy() -> Value {
    let mut policy = Map::new();
    policy.insert(
        String::from("type"),
        Value::String(String::from("readOnly")),
    );
    policy.insert(String::from("networkAccess"), Value::Bool(false));
    Value::Object(policy)
}

fn proposal_schema() -> Value {
    let mut proposal = Map::new();
    proposal.insert(String::from("type"), Value::String(String::from("string")));
    proposal.insert(String::from("maxLength"), Value::from(280u64));

    let mut properties = Map::new();
    properties.insert(String::from("proposal"), Value::Object(proposal));

    let mut schema = Map::new();
    schema.insert(String::from("type"), Value::String(String::from("object")));
    schema.insert(
        String::from("required"),
        Value::Array(vec![Value::String(String::from("proposal"))]),
    );
    schema.insert(String::from("additionalProperties"), Value::Bool(false));
    schema.insert(String::from("properties"), Value::Object(properties));
    Value::Object(schema)
}

fn proposal_from_turn_completed(message: &Value) -> Option<String> {
    let items = message
        .get("params")?
        .get("turn")?
        .get("items")?
        .as_array()?;
    let mut index = items.len();
    while index > 0 {
        index -= 1;
        let item = &items[index];
        if item.get("type").and_then(Value::as_str) != Some("agentMessage") {
            continue;
        }
        let text = item.get("text").and_then(Value::as_str)?;
        if let Ok(proposal) = proposal_from_text(text) {
            return Some(proposal);
        }
    }
    None
}

fn proposal_from_text(text: &str) -> Result<String, StdioTurnError> {
    let value: Value = serde_json::from_str(text.trim())?;
    let proposal = value
        .get("proposal")
        .and_then(Value::as_str)
        .ok_or(StdioTurnError::UnexpectedResponse)?;
    if proposal.len() > 280 {
        return Err(StdioTurnError::ProposalTooLong);
    }
    Ok(String::from(proposal))
}

#[derive(Debug)]
enum StdioTurnError {
    Io(std::io::Error),
    Json(serde_json::Error),
    CodexUnavailable,
    MissingPipe,
    UnexpectedResponse,
    AppServerError(String),
    ProposalTooLong,
}

impl core::fmt::Display for StdioTurnError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "io error: {error}"),
            Self::Json(error) => write!(formatter, "json error: {error}"),
            Self::CodexUnavailable => write!(formatter, "codex app-server is unavailable"),
            Self::MissingPipe => write!(formatter, "codex app-server pipe was unavailable"),
            Self::UnexpectedResponse => write!(formatter, "unexpected codex app-server response"),
            Self::AppServerError(error) => write!(formatter, "codex app-server error: {error}"),
            Self::ProposalTooLong => write!(formatter, "codex proposal exceeded bounded length"),
        }
    }
}

impl std::error::Error for StdioTurnError {}

impl From<std::io::Error> for StdioTurnError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StdioTurnError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
