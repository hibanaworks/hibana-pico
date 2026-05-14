//! Generic helpers for ChoreoFS-backed guest files.
//!
//! This layer exposes opaque safe wrappers around `path_open` and `fd_write`.
//! It does not encode board-specific device paths. Proof-specific preopen
//! numbers stay in their modules; the default root is only for host/full
//! ChoreoFS selectors.

use crate::{Error, Result, sys};

const DEFAULT_ROOT_FD: u32 = 3;

pub struct WriteFile {
    fd: u32,
}

impl WriteFile {
    pub fn write_once_exact(&self, bytes: &[u8]) -> Result<()> {
        sys::write_once_exact(self.fd, bytes)
    }
}

pub fn open_write(preopen_fd: u32, path: &str) -> Result<WriteFile> {
    let path = normalize_choreofs_path(path)?;
    let fd = sys::open_path(preopen_fd, path.as_bytes(), sys::FD_WRITE_RIGHT)?;
    Ok(WriteFile { fd })
}

pub(crate) const fn default_root_fd() -> u32 {
    DEFAULT_ROOT_FD
}

fn normalize_choreofs_path(path: &str) -> Result<&str> {
    let path = path.strip_prefix('/').unwrap_or(path);
    if path.is_empty() || path.as_bytes().contains(&0) {
        return Err(Error::InvalidPath);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_choreofs_path_accepts_absolute_and_relative_paths() {
        assert_eq!(
            normalize_choreofs_path("/device/led/green"),
            Ok("device/led/green")
        );
        assert_eq!(
            normalize_choreofs_path("device/led/yellow"),
            Ok("device/led/yellow")
        );
    }

    #[test]
    fn normalize_choreofs_path_rejects_empty_and_nul_paths() {
        for path in ["", "/", "device/led/green\0"] {
            assert_eq!(normalize_choreofs_path(path), Err(Error::InvalidPath));
        }
    }
}
