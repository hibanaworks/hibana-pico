use std::hint::black_box;

const STEP_BYTES: usize = 6;
const STEP_COUNT: usize = 7;
const ERRNO_SUCCESS: u16 = 0;
const EVENTTYPE_CLOCK: u8 = 0;
const SUBSCRIPTION_EVENTTYPE_OFFSET: usize = 8;
const SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET: usize = 24;

const TRAFFIC_PLAN: &[u8] = b"\x03\x31\xfa\x00\x00\x00\
\x04\x31\x32\x00\x00\x00\
\x05\x31\x32\x00\x00\x00\
\x04\x31\x32\x00\x00\x00\
\x03\x31\x32\x00\x00\x00\
\x04\x31\x32\x00\x00\x00\
\x05\x31\xfa\x00\x00\x00";

#[repr(C)]
struct Ciovec {
    buf: *const u8,
    buf_len: usize,
}

#[link(wasm_import_module = "wasi_snapshot_preview1")]
unsafe extern "C" {
    fn fd_write(fd: u32, iovs: *const Ciovec, iovs_len: usize, nwritten: *mut usize) -> u16;
    fn poll_oneoff(
        input: *const u8,
        output: *mut u8,
        nsubscriptions: usize,
        nevents: *mut usize,
    ) -> u16;
}

fn main() {
    black_box(TRAFFIC_PLAN);

    for index in 0..STEP_COUNT {
        let offset = index * STEP_BYTES;
        let fd = TRAFFIC_PLAN[offset] as u32;
        let payload = TRAFFIC_PLAN[offset + 1];
        let delay_ms = u32::from_le_bytes([
            TRAFFIC_PLAN[offset + 2],
            TRAFFIC_PLAN[offset + 3],
            TRAFFIC_PLAN[offset + 4],
            TRAFFIC_PLAN[offset + 5],
        ]);
        write_led(fd, payload);
        sleep_ms(delay_ms);
    }
}

fn write_led(fd: u32, payload: u8) {
    let byte = [payload];
    let iov = [Ciovec {
        buf: byte.as_ptr(),
        buf_len: byte.len(),
    }];
    let mut written = 0usize;
    let errno = unsafe { fd_write(fd, iov.as_ptr(), iov.len(), &mut written) };
    assert_eq!(errno, ERRNO_SUCCESS);
    assert_eq!(written, byte.len());
}

fn sleep_ms(ms: u32) {
    let mut subscription = [0u8; 48];
    let mut event = [0u8; 32];
    let mut ready = 0usize;
    subscription[SUBSCRIPTION_EVENTTYPE_OFFSET] = EVENTTYPE_CLOCK;
    subscription[SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET..SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET + 8]
        .copy_from_slice(&(ms as u64 * 1_000_000).to_le_bytes());

    let errno = unsafe { poll_oneoff(subscription.as_ptr(), event.as_mut_ptr(), 1, &mut ready) };
    assert_eq!(errno, ERRNO_SUCCESS);
    assert_eq!(ready, 1);
    black_box(event);
}
