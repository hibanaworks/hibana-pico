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

static ON: [u8; 1] = *b"1";
static ON_IOV: Ciovec = Ciovec {
    buf: ON.as_ptr(),
    len: ON.len(),
};

static mut WRITTEN: usize = 0;

#[unsafe(export_name = "__main_void")]
pub extern "C" fn main_void() {
    unsafe {
        let errno = fd_write(6, &ON_IOV, 1, &raw mut WRITTEN);
        if errno != 0 {
            core::arch::wasm32::unreachable();
        }
    }
}
