#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::ptr::{read_volatile, write_volatile};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use crate::{machine::rp2350::sio::core_id, port::exec::signal};

#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART0_BASE: usize = 0x4007_0000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTDR: *mut u32 = UART0_BASE as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTFR: *const u32 = (UART0_BASE + 0x18) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTIBRD: *mut u32 = (UART0_BASE + 0x24) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTFBRD: *mut u32 = (UART0_BASE + 0x28) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTLCR_H: *mut u32 = (UART0_BASE + 0x2c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UARTCR: *mut u32 = (UART0_BASE + 0x30) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const UART_TXFF: u32 = 1 << 5;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut UART_READY: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut UART_LOCK_WANT: [u32; 2] = [0; 2];
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut UART_LOCK_TURN: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn lock() {
    let me = core_id() as usize;
    let other = 1usize.saturating_sub(me);
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(UART_LOCK_WANT[me]), 1);
        write_volatile(core::ptr::addr_of_mut!(UART_LOCK_TURN), other as u32);
    }
    while unsafe { read_volatile(core::ptr::addr_of!(UART_LOCK_WANT[other])) } != 0
        && unsafe { read_volatile(core::ptr::addr_of!(UART_LOCK_TURN)) } == other as u32
    {
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn unlock() {
    let me = core_id() as usize;
    unsafe {
        write_volatile(core::ptr::addr_of_mut!(UART_LOCK_WANT[me]), 0);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn init() {
    unsafe {
        write_volatile(UARTCR, 0);
        write_volatile(UARTIBRD, 81);
        write_volatile(UARTFBRD, 24);
        write_volatile(UARTLCR_H, 0x60);
        write_volatile(UARTCR, 0x101);
        write_volatile(core::ptr::addr_of_mut!(UART_READY), 1);
    }
    signal();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn ready() -> bool {
    unsafe { read_volatile(core::ptr::addr_of!(UART_READY)) != 0 }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn putc(byte: u8) {
    while unsafe { read_volatile(UARTFR) } & UART_TXFF != 0 {}
    unsafe { write_volatile(UARTDR, byte as u32) };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn write_bytes_unlocked(bytes: &[u8]) {
    for &byte in bytes {
        if byte == b'\n' {
            putc(b'\r');
        }
        putc(byte);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn write_hex_unlocked(value: u32) {
    for shift in (0..8).rev() {
        let nibble = ((value >> (shift * 4)) & 0xf) as u8;
        let ch = match nibble {
            0..=9 => b'0' + nibble,
            _ => b'a' + (nibble - 10),
        };
        putc(ch);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn write_bytes(bytes: &[u8]) {
    lock();
    write_bytes_unlocked(bytes);
    unlock();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn line(text: &str) {
    lock();
    write_bytes_unlocked(text.as_bytes());
    putc(b'\r');
    putc(b'\n');
    unlock();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn hex_line(prefix: &str, value: u32) {
    lock();
    write_bytes_unlocked(prefix.as_bytes());
    write_hex_unlocked(value);
    putc(b'\r');
    putc(b'\n');
    unlock();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn hex_prefixed_bytes(prefix: &str, value: u32, infix: &str, bytes: &[u8]) {
    lock();
    write_bytes_unlocked(prefix.as_bytes());
    write_hex_unlocked(value);
    write_bytes_unlocked(infix.as_bytes());
    write_bytes_unlocked(bytes);
    unlock();
}
