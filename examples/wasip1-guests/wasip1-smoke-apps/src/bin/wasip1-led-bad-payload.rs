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
}

static BAD: [u8; 1] = *b"2";
static BAD_IOV: Ciovec = Ciovec {
    buf: BAD.as_ptr(),
    len: BAD.len(),
};

static mut WRITTEN: usize = 0;

#[unsafe(export_name = "__main_void")]
pub extern "C" fn main_void() {
    unsafe {
        let errno = fd_write(3, &BAD_IOV, 1, &raw mut WRITTEN);
        if errno != 0 {
            core::arch::wasm32::unreachable();
        }
    }
}
