use crate::{Error, Result, Syscall};

const ERRNO_SUCCESS: u16 = 0;
const EVENTTYPE_CLOCK: u8 = 0;
const EVENT_ERROR_OFFSET: usize = 8;
const EVENT_TYPE_OFFSET: usize = 10;
const SUBSCRIPTION_EVENTTYPE_OFFSET: usize = 8;
const SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET: usize = 24;

pub(crate) const FD_WRITE_RIGHT: u64 = 1 << 6;
pub(crate) const FD_READ_RIGHT: u64 = 1 << 1;

#[repr(C)]
struct Ciovec {
    buf: *const u8,
    buf_len: usize,
}

#[repr(C)]
struct Iovec {
    buf: *mut u8,
    buf_len: usize,
}

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
    fn fd_read(fd: u32, iovs: *mut Iovec, iovs_len: usize, nread: *mut usize) -> u16;
    fn fd_write(fd: u32, iovs: *const Ciovec, iovs_len: usize, nwritten: *mut usize) -> u16;
    fn poll_oneoff(
        input: *const u8,
        output: *mut u8,
        nsubscriptions: usize,
        nevents: *mut usize,
    ) -> u16;
}

pub(crate) fn open_path(fd: u32, path: &[u8], rights_base: u64) -> Result<u32> {
    let mut opened_fd = 0u32;
    let errno = unsafe {
        path_open(
            fd,
            0,
            path.as_ptr(),
            path.len(),
            0,
            rights_base,
            0,
            0,
            &mut opened_fd,
        )
    };
    errno_result(Syscall::PathOpen, errno)?;
    Ok(opened_fd)
}

pub(crate) fn read_once(fd: u32, out: &mut [u8]) -> Result<usize> {
    let mut iov = [Iovec {
        buf: out.as_mut_ptr(),
        buf_len: out.len(),
    }];
    let mut read = 0usize;
    let errno = unsafe { fd_read(fd, iov.as_mut_ptr(), iov.len(), &mut read) };
    errno_result(Syscall::FdRead, errno)?;
    Ok(read)
}

pub(crate) fn write_once_exact(fd: u32, bytes: &[u8]) -> Result<()> {
    let iov = [Ciovec {
        buf: bytes.as_ptr(),
        buf_len: bytes.len(),
    }];
    let mut written = 0usize;
    let errno = unsafe { fd_write(fd, iov.as_ptr(), iov.len(), &mut written) };
    errno_result(Syscall::FdWrite, errno)?;
    if written != bytes.len() {
        return Err(Error::ShortWrite {
            expected: bytes.len(),
            actual: written,
        });
    }
    Ok(())
}

pub(crate) fn sleep_ms(ms: u32) -> Result<()> {
    let mut subscription = [0u8; 48];
    let mut event = [0u8; 32];
    let mut ready = 0usize;

    subscription[SUBSCRIPTION_EVENTTYPE_OFFSET] = EVENTTYPE_CLOCK;
    subscription[SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET..SUBSCRIPTION_CLOCK_TIMEOUT_OFFSET + 8]
        .copy_from_slice(&(ms as u64 * 1_000_000).to_le_bytes());

    let errno = unsafe { poll_oneoff(subscription.as_ptr(), event.as_mut_ptr(), 1, &mut ready) };
    errno_result(Syscall::PollOneoff, errno)?;
    poll_oneoff_event_result(&event, ready)?;
    core::hint::black_box(event);
    Ok(())
}

fn poll_oneoff_event_result(event: &[u8; 32], ready: usize) -> Result<()> {
    if ready != 1 {
        return Err(Error::PollNotReady { ready });
    }
    let event_error =
        u16::from_le_bytes([event[EVENT_ERROR_OFFSET], event[EVENT_ERROR_OFFSET + 1]]);
    if event_error != ERRNO_SUCCESS {
        return Err(Error::Wasi {
            syscall: Syscall::PollOneoff,
            errno: event_error,
        });
    }
    let event_type = event[EVENT_TYPE_OFFSET];
    if event_type != EVENTTYPE_CLOCK {
        return Err(Error::UnexpectedEvent { event_type });
    }
    Ok(())
}

fn errno_result(syscall: Syscall, errno: u16) -> Result<()> {
    if errno == ERRNO_SUCCESS {
        Ok(())
    } else {
        Err(Error::Wasi { syscall, errno })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_oneoff_event_result_returns_event_error() {
        let mut event = [0u8; 32];
        event[8..10].copy_from_slice(&28u16.to_le_bytes());
        event[10] = EVENTTYPE_CLOCK;

        assert_eq!(
            poll_oneoff_event_result(&event, 1),
            Err(Error::Wasi {
                syscall: Syscall::PollOneoff,
                errno: 28,
            })
        );
    }

    #[test]
    fn poll_oneoff_event_result_rejects_non_clock_event() {
        let mut event = [0u8; 32];
        event[10] = 1;

        assert_eq!(
            poll_oneoff_event_result(&event, 1),
            Err(Error::UnexpectedEvent { event_type: 1 })
        );
    }
}
