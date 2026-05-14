#![no_main]

const PREOPEN_FD: u32 = 9;
const FD_WRITE_RIGHT: u64 = 1 << 6;

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
}

static BAD_PATH: &[u8] = b"not/allowed";
static mut FD: u32 = 0;

#[unsafe(export_name = "__main_void")]
pub extern "C" fn main_void() {
    unsafe {
        let errno = path_open(
            PREOPEN_FD,
            0,
            BAD_PATH.as_ptr(),
            BAD_PATH.len(),
            0,
            FD_WRITE_RIGHT,
            0,
            0,
            &raw mut FD,
        );
        core::hint::black_box(errno);
    }
}
