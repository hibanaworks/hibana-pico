use std::{
    fmt::Write as FmtWrite,
    fs::File,
    io::Write,
    os::fd::{FromRawFd, IntoRawFd},
};

#[cfg(target_arch = "wasm32")]
use core::arch::wasm32::{memory_grow, memory_size};

const STDOUT_FD: i32 = 1;
const MEMORY_GROW_MARKER: &str = "memory.grow";

fn add_one(value: u32) -> u32 {
    value.wrapping_add(1)
}

fn triple(value: u32) -> u32 {
    value.wrapping_mul(3)
}

#[cfg(target_arch = "wasm32")]
fn grow_default_memory() -> (usize, usize) {
    let before = memory_size::<0>();
    let previous = memory_grow::<0>(1);
    assert_ne!(previous, usize::MAX, "hibana std core coverage grow failed");
    let after = memory_size::<0>();
    assert_eq!(previous, before);
    assert_eq!(after, before + 1);
    (before, after)
}

#[cfg(not(target_arch = "wasm32"))]
fn grow_default_memory() -> (usize, usize) {
    (0, 0)
}

fn main() {
    let mut values = Vec::with_capacity(12);
    for index in 0..12u32 {
        values.push(match index % 4 {
            0 => index + 1,
            1 => index * 2,
            2 => index ^ 0x55,
            _ => index.rotate_left(1),
        });
    }

    let callbacks: [fn(u32) -> u32; 2] = [add_one, triple];
    let mut acc = 0u32;
    for (index, value) in values.iter().copied().enumerate() {
        let callback = callbacks[index & 1];
        acc = acc.wrapping_add(callback(value));
    }

    let branch = if acc & 1 == 0 { "even" } else { "odd" };
    let class = match acc % 3 {
        0 => "zero",
        1 => "one",
        _ => "two",
    };
    let float = ((acc as f64) + 0.25).sqrt() + (acc as f32 * 1.5).sqrt() as f64;
    let (before, after) = grow_default_memory();

    let mut message = String::new();
    write!(
        &mut message,
        "hibana std core coverage\n{MEMORY_GROW_MARKER}\n"
    )
    .expect("format coverage marker");
    core::hint::black_box((branch, class, acc, float, before, after, MEMORY_GROW_MARKER));

    let mut stdout = unsafe { File::from_raw_fd(STDOUT_FD) };
    stdout
        .write_all(message.as_bytes())
        .expect("write coverage marker");
    let raw_fd = stdout.into_raw_fd();
    core::hint::black_box(raw_fd);
}
