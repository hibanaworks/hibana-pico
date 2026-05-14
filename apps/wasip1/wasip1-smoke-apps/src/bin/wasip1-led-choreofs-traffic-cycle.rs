#![no_main]
#![no_std]

use core::panic::PanicInfo;

const PREOPEN_FD: u32 = 9;
const FD_WRITE_RIGHT: u64 = 1 << 6;

#[repr(C)]
struct Ciovec {
    buf: *const u8,
    buf_len: usize,
}

unsafe impl Sync for Ciovec {}

#[link(wasm_import_module = "wasi_snapshot_preview1")]
unsafe extern "C" {
    fn path_open(
        fd: u32,
        dirflags: u32,
        path: *const u8,
        path_len: usize,
        oflags: u32,
        fs_rights_base: u64,
        fs_rights_inheriting: u64,
        fdflags: u32,
        opened_fd: *mut u32,
    ) -> u16;
    fn fd_write(fd: u32, iovs: *const Ciovec, iovs_len: usize, nwritten: *mut usize) -> u16;
    fn poll_oneoff(
        input: *const u8,
        output: *mut u8,
        nsubscriptions: usize,
        nevents: *mut usize,
    ) -> u16;
}

static TRAFFIC_PATH: &[u8] = b"device/traffic";
static GREEN: [u8; 1] = *b"1";
static ORANGE: [u8; 1] = *b"2";
static RED: [u8; 1] = *b"4";
static GREEN_IOV: Ciovec = Ciovec {
    buf: GREEN.as_ptr(),
    buf_len: GREEN.len(),
};
static ORANGE_IOV: Ciovec = Ciovec {
    buf: ORANGE.as_ptr(),
    buf_len: ORANGE.len(),
};
static RED_IOV: Ciovec = Ciovec {
    buf: RED.as_ptr(),
    buf_len: RED.len(),
};
static mut TRAFFIC_FD: u32 = 0;
static mut WRITTEN: usize = 0;
static mut READY: usize = 0;
static mut SUBSCRIPTION: [u8; 48] = [0; 48];
static mut EVENT: [u8; 32] = [0; 32];

#[unsafe(export_name = "__main_void")]
pub extern "C" fn main_void() {
    unsafe {
        let errno = path_open(
            PREOPEN_FD,
            0,
            TRAFFIC_PATH.as_ptr(),
            TRAFFIC_PATH.len(),
            0,
            FD_WRITE_RIGHT,
            0,
            0,
            &raw mut TRAFFIC_FD,
        );
        core::hint::black_box(errno);
    }
    loop {
        write_and_wait(&GREEN_IOV);
        write_and_wait(&ORANGE_IOV);
        write_and_wait(&RED_IOV);
    }
}

fn write_and_wait(iov: &'static Ciovec) {
    unsafe {
        let errno = fd_write(TRAFFIC_FD, iov, 1, &raw mut WRITTEN);
        core::hint::black_box(errno);
        let timeout = 80_000_000u64.to_le_bytes();
        core::ptr::copy_nonoverlapping(
            timeout.as_ptr(),
            (&raw mut SUBSCRIPTION).cast::<u8>().add(24),
            timeout.len(),
        );
        let errno = poll_oneoff(
            (&raw const SUBSCRIPTION).cast::<u8>(),
            (&raw mut EVENT).cast::<u8>(),
            1,
            &raw mut READY,
        );
        core::hint::black_box(errno);
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    core::arch::wasm32::unreachable();
}

#[unsafe(export_name = "__wasi_init_tp")]
pub extern "C" fn wasi_init_tp() {}

#[unsafe(export_name = "__wasm_call_dtors")]
pub extern "C" fn wasm_call_dtors() {}

#[unsafe(export_name = "__wasi_proc_exit")]
pub extern "C" fn wasi_proc_exit(_: i32) -> ! {
    core::arch::wasm32::unreachable();
}
