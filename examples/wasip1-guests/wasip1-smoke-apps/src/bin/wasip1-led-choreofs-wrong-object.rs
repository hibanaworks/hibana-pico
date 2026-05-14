#![no_main]

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
}

static YELLOW_PATH: &[u8] = b"device/led/yellow";
static RED_PATH: &[u8] = b"device/led/red";
static NOT_GPIO_PATH: &[u8] = b"device/not-gpio";
static ONE: [u8; 1] = *b"1";
static ONE_IOV: Ciovec = Ciovec {
    buf: ONE.as_ptr(),
    buf_len: ONE.len(),
};
static mut YELLOW_FD: u32 = 0;
static mut RED_FD: u32 = 0;
static mut NOT_GPIO_FD: u32 = 0;
static mut WRITTEN: usize = 0;

fn open_path(path: &[u8], fd: *mut u32) {
    unsafe {
        let errno = path_open(
            PREOPEN_FD,
            0,
            path.as_ptr(),
            path.len(),
            0,
            FD_WRITE_RIGHT,
            0,
            0,
            fd,
        );
        core::hint::black_box(errno);
    }
}

#[unsafe(export_name = "__main_void")]
pub extern "C" fn main_void() {
    open_path(YELLOW_PATH, &raw mut YELLOW_FD);
    open_path(RED_PATH, &raw mut RED_FD);
    open_path(NOT_GPIO_PATH, &raw mut NOT_GPIO_FD);
    unsafe {
        let errno = fd_write(NOT_GPIO_FD, &ONE_IOV, 1, &raw mut WRITTEN);
        core::hint::black_box(errno);
    }
}
