use std::{
    fs::File,
    io::Write,
    os::fd::{FromRawFd, IntoRawFd},
};

const LED_FD: i32 = 3;

fn main() {
    let mut led = unsafe { File::from_raw_fd(LED_FD) };
    led.write_all(b"1").expect("led on");
    led.write_all(b"0").expect("led off");
    let raw_fd = led.into_raw_fd();
    core::hint::black_box(raw_fd);
}
