use std::io::{self, Read};

fn main() {
    let mut buf = [0u8; 24];
    let bytes_read = io::stdin()
        .read(&mut buf)
        .expect("read stdin program input");
    core::hint::black_box(bytes_read);
}
