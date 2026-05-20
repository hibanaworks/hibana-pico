use hibana_wasip1_guest::choreofs;

const XBOT_PREOPEN_FD: u32 = 8;
const INPUT_PATH: &str = "xbot/codex-proposal.txt";
const OUTPUT_PATH: &str = "xbot/reply-draft.txt";
const MAX_REPLY_BYTES: usize = 280;
const MAX_READ_BYTES: usize = 30;

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> hibana_wasip1_guest::Result<()> {
    let input = choreofs::open_read(XBOT_PREOPEN_FD, INPUT_PATH)?;
    let mut input_buf = [0u8; MAX_READ_BYTES];
    let input_len = input.read_once(&mut input_buf)?;

    let mut output = [0u8; MAX_REPLY_BYTES];
    let output_len = normalize(&input_buf[..input_len], &mut output);
    let draft = choreofs::open_write(XBOT_PREOPEN_FD, OUTPUT_PATH)?;
    draft.write_once_exact(&output[..output_len])
}

fn normalize(input: &[u8], out: &mut [u8; MAX_REPLY_BYTES]) -> usize {
    let mut in_idx = 0usize;
    let mut out_idx = 0usize;
    while in_idx < input.len() && out_idx < out.len() {
        let byte = input[in_idx];
        let normalized = match byte {
            b'\r' | b'\n' | b'\t' => b' ',
            _ => byte,
        };
        if normalized != b' ' || out_idx == 0 || out[out_idx - 1] != b' ' {
            out[out_idx] = normalized;
            out_idx += 1;
        }
        in_idx += 1;
    }
    while out_idx > 0 && out[out_idx - 1] == b' ' {
        out_idx -= 1;
    }
    out_idx
}

#[cold]
fn abort() -> ! {
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    loop {
        core::hint::spin_loop();
    }
}
