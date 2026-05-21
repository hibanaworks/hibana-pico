use hibana_wasip1_guest::{Error, Result, choreofs};

const PREOPEN_FD: u32 = 9;
const SHELL_CATALOG: &[u8] = b"w /face/frame FaceFrame\n";
const CMD_LS: u8 = 0xff;

macro_rules! is_catalog_discovery_command {
    ($command:expr) => {{
        let command = $command;
        (command.len() == 2 && command[0] == b'l' && command[1] == b's')
            || (command.len() == 21
                && command[0] == b'f'
                && command[1] == b'i'
                && command[2] == b'n'
                && command[3] == b'd'
                && command[4] == b' '
                && (command[5] == b'C' || command[5] == b'c')
                && command[6] == b'h'
                && command[7] == b'o'
                && command[8] == b'r'
                && command[9] == b'e'
                && command[10] == b'o'
                && (command[11] == b'F' || command[11] == b'f')
                && (command[12] == b'S' || command[12] == b's')
                && command[13] == b' '
                && command[14] == b'-'
                && command[15] == b't'
                && command[16] == b'y'
                && command[17] == b'p'
                && command[18] == b'e'
                && command[19] == b' '
                && command[20] == b'f')
            || (command.len() == 14
                && command[0] == b'f'
                && command[1] == b'i'
                && command[2] == b'n'
                && command[3] == b'd'
                && command[4] == b' '
                && (command[5] == b'.' || command[5] == b'/')
                && command[6] == b' '
                && command[7] == b'-'
                && command[8] == b't'
                && command[9] == b'y'
                && command[10] == b'p'
                && command[11] == b'e'
                && command[12] == b' '
                && command[13] == b'f')
    }};
}

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
    if is_catalog_discovery_command!(command) {
        return Ok(CMD_LS);
    }
    decode_echo_face_command(command)
}

#[inline(always)]
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

#[inline(always)]
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
        if face[0] == b'v' {
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

#[cfg(test)]
mod tests {
    use crate::decode_face_code;

    #[test]
    fn catalog_discovery_accepts_ls_and_shell_find() {
        for command in [
            b"ls" as &[u8],
            b"find ChoreoFS -type f",
            b"find choreofs -type f",
            b"find . -type f",
            b"find / -type f",
        ] {
            assert!(is_catalog_discovery_command!(command));
        }
    }

    #[test]
    fn catalog_discovery_rejects_non_file_find_and_effect_commands() {
        for command in [
            b"find ChoreoFS -type d" as &[u8],
            b"find ChoreoFS",
            b"echo h > /face/frame",
        ] {
            assert!(!is_catalog_discovery_command!(command));
        }
    }

    #[test]
    fn surprised_accepts_model_alias_v() {
        assert_eq!(decode_face_code(b"u"), Ok(4));
        assert_eq!(decode_face_code(b"v"), Ok(4));
    }
}
