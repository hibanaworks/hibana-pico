use hibana_wasip1_guest::{Error, Result, choreofs};

const PREOPEN_FD: u32 = 9;
const SHELL_CATALOG: &[u8] = b"w /face/frame FaceFrame\n";
const CMD_LS: u8 = 0xff;

fn main() -> Result<()> {
    run()
}

fn run() -> Result<()> {
    let llm_stdin = choreofs::open_read(PREOPEN_FD, "/llm/stdin")?;
    let llm_stdout = choreofs::open_write(PREOPEN_FD, "/llm/stdout")?;
    let face = choreofs::open_write(PREOPEN_FD, "/face/frame")?;
    let mut ordinal = 0u8;

    loop {
        llm_stdout.write_once_exact(b"choreofs$ ")?;
        if read_command(&llm_stdin)? != CMD_LS {
            return Err(Error::InvalidPath);
        }

        llm_stdout.write_once_exact(SHELL_CATALOG)?;
        let face_kind = read_command(&llm_stdin)?;
        if face_kind == CMD_LS {
            return Err(Error::InvalidPath);
        }
        let frame = [face_kind, ordinal];
        face.write_once_exact(&frame)?;
        ordinal = ordinal.wrapping_add(1);
    }
}

fn read_command(stdin: &choreofs::ReadFile) -> Result<u8> {
    let mut buffer = [0u8; 30];
    let len = stdin.read_once(&mut buffer)?;
    let mut end = len;
    if end != 0 && buffer[end - 1] == b'\n' {
        end -= 1;
    }
    let command = &buffer[..end];
    if command == b"ls" {
        return Ok(CMD_LS);
    }
    decode_echo_face_command(command)
}

fn decode_echo_face_command(command: &[u8]) -> Result<u8> {
    let prefix = b"echo ";
    let redirect = b" > /face/frame";
    if command.len() <= prefix.len() + redirect.len()
        || &command[..prefix.len()] != prefix
        || &command[command.len() - redirect.len()..] != redirect
    {
        return Err(Error::InvalidPath);
    }
    decode_face_code(&command[prefix.len()..command.len() - redirect.len()])
}

#[inline(never)]
fn decode_face_code(face: &[u8]) -> Result<u8> {
    if face.len() == 1 {
        if face[0] == b'h' {
            return Ok(1);
        }
        if face[0] == b's' {
            return Ok(2);
        }
        if face[0] == b'a' {
            return Ok(3);
        }
        if face[0] == b'u' {
            return Ok(4);
        }
    }
    if face.len() == 2 && face[0] == b'm' {
        if face[1] == b'c' {
            return Ok(16);
        }
        if face[1] == b's' {
            return Ok(17);
        }
        if face[1] == b'w' {
            return Ok(18);
        }
        if face[1] == b'r' {
            return Ok(19);
        }
    }
    Err(Error::InvalidPath)
}
