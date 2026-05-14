#![no_main]

#[repr(C)]
struct Ciovec {
    buf: *const u8,
    len: usize,
}

unsafe impl Sync for Ciovec {}

#[link(wasm_import_module = "wasi_snapshot_preview1")]
unsafe extern "C" {
    fn fd_write(fd: u32, iovs: *const Ciovec, iovs_len: usize, nwritten: *mut usize) -> u16;
    fn poll_oneoff(in_: *const u8, out: *mut u8, nsubscriptions: usize, nevents: *mut usize)
    -> u16;
}

static ON: [u8; 1] = *b"1";
static ON_IOV: Ciovec = Ciovec {
    buf: ON.as_ptr(),
    len: ON.len(),
};
const EVENTTYPE_CLOCK: u8 = 0;
const SUBSCRIPTION_EVENTTYPE_OFFSET: usize = 8;
const SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET: usize = 24;

static mut WRITTEN: usize = 0;
static mut NEVENTS: usize = 0;

fn sleep_ms(ms: u64) {
    let mut subscription = [0u8; 48];
    let mut event = [0u8; 32];
    subscription[SUBSCRIPTION_EVENTTYPE_OFFSET] = EVENTTYPE_CLOCK;
    subscription[SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET..SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET + 8]
        .copy_from_slice(&(ms * 1_000_000).to_le_bytes());
    unsafe {
        let errno = poll_oneoff(
            subscription.as_ptr(),
            event.as_mut_ptr(),
            1,
            &raw mut NEVENTS,
        );
        core::hint::black_box(errno);
    }
}

#[unsafe(export_name = "__main_void")]
pub extern "C" fn main_void() {
    unsafe {
        sleep_ms(50);
        fd_write(3, &ON_IOV, 1, &raw mut WRITTEN);
    }
}
