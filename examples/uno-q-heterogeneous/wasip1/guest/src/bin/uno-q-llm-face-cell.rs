use hibana_wasip1_guest::{Result, choreofs, time};

const PREOPEN_FD: u32 = 9;

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> Result<()> {
    let inbox = choreofs::open_read(PREOPEN_FD, "/ios/prompt/inbox")?;
    let mut prompt_buf = [0u8; 30];
    let prompt_len = inbox.read_once(&mut prompt_buf)?;

    let prompt = choreofs::open_write(PREOPEN_FD, "/llm/prompt")?;
    prompt.write_once_exact(&prompt_buf[..prompt_len])?;

    time::sleep_ms(10)?;

    let tx = choreofs::open_write(PREOPEN_FD, "/net/challenger/tx")?;
    tx.write_once_exact(b"challenger ping")?;

    let rx = choreofs::open_read(PREOPEN_FD, "/net/challenger/rx")?;
    let mut reply = [0u8; 30];
    let reply_len = rx.read_once(&mut reply)?;
    if &reply[..reply_len] != b"challenger ok happy" {
        abort();
    }

    let ack = choreofs::open_write(PREOPEN_FD, "/face/ack")?;
    ack.write_once_exact(b"face committed")
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
