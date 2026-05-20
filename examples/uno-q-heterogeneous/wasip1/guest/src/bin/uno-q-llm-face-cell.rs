#![no_std]
#![no_main]

use hibana_wasip1_guest::{Result, choreofs, time};

const PREOPEN_FD: u32 = 9;
const FACE_HAPPY: u8 = 1;
const FACE_SAD: u8 = 2;
const FACE_ANGRY: u8 = 3;
const FACE_SURPRISED: u8 = 4;
const FACE_MOUTH_CLOSED: u8 = 16;
const FACE_MOUTH_SMALL: u8 = 17;
const FACE_MOUTH_WIDE: u8 = 18;
const FACE_MOUTH_ROUND: u8 = 19;
const EMOTION_HOLD_MS: u32 = 1_000;
const MOUTH_HOLD_MS: u32 = 500;

const EMOTION_FRAMES: [u8; 12] = [
    FACE_HAPPY,
    FACE_ANGRY,
    FACE_SAD,
    FACE_SURPRISED,
    FACE_HAPPY,
    FACE_ANGRY,
    FACE_SAD,
    FACE_SURPRISED,
    FACE_HAPPY,
    FACE_ANGRY,
    FACE_SAD,
    FACE_SURPRISED,
];
const MOUTH_FRAMES: [u8; 8] = [
    FACE_MOUTH_CLOSED,
    FACE_MOUTH_SMALL,
    FACE_MOUTH_WIDE,
    FACE_MOUTH_ROUND,
    FACE_MOUTH_CLOSED,
    FACE_MOUTH_SMALL,
    FACE_MOUTH_WIDE,
    FACE_MOUTH_ROUND,
];

#[unsafe(export_name = "__main_void")]
pub extern "C" fn main_void() {
    if run().is_err() {
        abort();
    }
}

fn run() -> Result<()> {
    let llm = choreofs::open_read(PREOPEN_FD, "/llm/frame")?;
    let face = choreofs::open_write(PREOPEN_FD, "/face/frame")?;

    let mut index = 0usize;
    while index < EMOTION_FRAMES.len() + MOUTH_FRAMES.len() {
        let mut frame = [0u8; 2];
        let len = llm.read_once(&mut frame)?;
        if len != frame.len() {
            abort();
        }
        face.write_once_exact(&frame)?;
        time::sleep_ms(face_hold_ms(frame[0]))?;
        index += 1;
    }
    Ok(())
}

fn face_hold_ms(face: u8) -> u32 {
    match face {
        FACE_MOUTH_CLOSED | FACE_MOUTH_SMALL | FACE_MOUTH_WIDE | FACE_MOUTH_ROUND => MOUTH_HOLD_MS,
        _ => EMOTION_HOLD_MS,
    }
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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    abort()
}
