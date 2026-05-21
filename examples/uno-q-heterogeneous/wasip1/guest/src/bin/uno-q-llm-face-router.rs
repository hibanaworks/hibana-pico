use hibana_wasip1_guest::{Result, choreofs};

const PREOPEN_FD: u32 = 9;

fn main() {
    run().expect("UNO Q LLM face router failed");
}

fn run() -> Result<()> {
    let llm = choreofs::open_read(PREOPEN_FD, "/llm/frame")?;
    let face = choreofs::open_write(PREOPEN_FD, "/face/frame")?;

    loop {
        let mut frame = [0u8; 2];
        let len = llm.read_once(&mut frame)?;
        assert_eq!(len, frame.len(), "short LLM face frame");
        face.write_once_exact(&frame)?;
    }
}
