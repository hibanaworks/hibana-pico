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

static GREEN_PATH: &[u8] = b"device/led/green";
static YELLOW_PATH: &[u8] = b"device/led/yellow";
static RED_PATH: &[u8] = b"device/led/red";
static BAD: [u8; 2] = *b"on";
static BAD_IOV: Ciovec = Ciovec {
    buf: BAD.as_ptr(),
    buf_len: BAD.len(),
};
static mut GREEN_FD: u32 = 0;
static mut YELLOW_FD: u32 = 0;
static mut RED_FD: u32 = 0;
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
    open_path(GREEN_PATH, &raw mut GREEN_FD);
    open_path(YELLOW_PATH, &raw mut YELLOW_FD);
    open_path(RED_PATH, &raw mut RED_FD);
    unsafe {
        let errno = fd_write(GREEN_FD, &BAD_IOV, 1, &raw mut WRITTEN);
        core::hint::black_box(errno);
    }
}
