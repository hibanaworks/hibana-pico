use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
};

const SHELL_PROMPT: &[u8] = b"$ ";
const SHELL_CATALOG: &[u8] = b"w /face/frame FaceFrame\n$ ";
const SHELL_INVALID_COMMAND: &[u8] = b"err /face/frame h,a,s,u,mw\n$ ";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShellCommand {
    Catalog,
    Face(u8),
    Invalid,
}

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

fn main() -> io::Result<()> {
    run()
}

fn run() -> io::Result<()> {
    let mut llm_stdin = OpenOptions::new().read(true).open("/llm/stdin")?;
    let mut llm_stdout = OpenOptions::new().write(true).open("/llm/stdout")?;
    let mut face = OpenOptions::new().write(true).open("/face/frame")?;
    let mut ordinal = 0u8;

    write_exact_flush(&mut llm_stdout, SHELL_PROMPT)?;
    loop {
        match read_command(&mut llm_stdin)? {
            ShellCommand::Catalog => write_exact_flush(&mut llm_stdout, SHELL_CATALOG)?,
            ShellCommand::Invalid => write_exact_flush(&mut llm_stdout, SHELL_INVALID_COMMAND)?,
            ShellCommand::Face(face_kind) => {
                let frame = [face_kind, ordinal];
                write_exact_flush(&mut face, &frame)?;
                ordinal = ordinal.wrapping_add(1);
                write_exact_flush(&mut llm_stdout, SHELL_PROMPT)?;
            }
        }
    }
}

fn read_command(stdin: &mut File) -> io::Result<ShellCommand> {
    let mut buffer = [0u8; 30];
    let len = stdin.read(&mut buffer)?;
    let mut end = len;
    if end != 0 && buffer[end - 1] == b'\n' {
        end -= 1;
    }
    let command = &buffer[..end];
    if is_catalog_discovery_command!(command) {
        return Ok(ShellCommand::Catalog);
    }
    Ok(decode_echo_face_command(command)
        .map(ShellCommand::Face)
        .unwrap_or(ShellCommand::Invalid))
}

fn write_exact_flush(file: &mut File, bytes: &[u8]) -> io::Result<()> {
    file.write_all(bytes)?;
    file.flush()
}

#[inline(always)]
fn decode_echo_face_command(command: &[u8]) -> Option<u8> {
    let prefix = b"echo ";
    let redirect = b" > /face/frame";
    if command.len() <= prefix.len() + redirect.len()
        || &command[..prefix.len()] != prefix
        || &command[command.len() - redirect.len()..] != redirect
    {
        return None;
    }
    decode_face_code(&command[prefix.len()..command.len() - redirect.len()])
}

#[inline(always)]
fn decode_face_code(face: &[u8]) -> Option<u8> {
    if face.len() == 1 {
        if face[0] == b'h' {
            return Some(1);
        }
        if face[0] == b's' {
            return Some(2);
        }
        if face[0] == b'a' {
            return Some(3);
        }
        if face[0] == b'u' {
            return Some(4);
        }
    }
    if face.len() == 2 && face[0] == b'm' {
        if face[1] == b'c' {
            return Some(16);
        }
        if face[1] == b's' {
            return Some(17);
        }
        if face[1] == b'w' {
            return Some(18);
        }
        if face[1] == b'r' {
            return Some(19);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::{SHELL_INVALID_COMMAND, ShellCommand, decode_echo_face_command, decode_face_code};

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
    fn surprised_accepts_only_declared_code() {
        assert_eq!(decode_face_code(b"u"), Some(4));
        assert_eq!(decode_face_code(b"v"), None);
    }

    #[test]
    fn invalid_llm_terminal_input_is_shell_error_not_process_exit() {
        assert_eq!(decode_echo_face_command(b"please laugh"), None);
        let command = decode_echo_face_command(b"please laugh")
            .map(ShellCommand::Face)
            .unwrap_or(ShellCommand::Invalid);
        assert_eq!(command, ShellCommand::Invalid);
    }

    #[test]
    fn invalid_face_codes_are_shell_errors_not_face_frames() {
        for command in [
            b"echo c > /face/frame" as &[u8],
            b"echo comfy > /face/frame",
            b"echo cold_dark > /face/frame",
        ] {
            let command = decode_echo_face_command(command)
                .map(ShellCommand::Face)
                .unwrap_or(ShellCommand::Invalid);
            assert_eq!(command, ShellCommand::Invalid);
        }
    }

    #[test]
    fn invalid_response_returns_available_commands_to_llm() {
        let response = core::str::from_utf8(SHELL_INVALID_COMMAND).unwrap();
        assert!(response.contains("err"));
        assert!(response.contains("/face/frame"));
        assert!(response.contains("h,a,s,u,mw"));
        assert!(response.ends_with("$ "));
        assert!(SHELL_INVALID_COMMAND.len() <= 30);
    }
}
