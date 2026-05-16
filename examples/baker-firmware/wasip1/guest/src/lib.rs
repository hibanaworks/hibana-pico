#![no_std]

use hibana_wasip1_guest::{Error, Result, choreofs, time};

const DEVICE_PREOPEN_FD: u32 = 9;
const LED_PATH_PREFIX: &str = "device/led/";

pub struct Led {
    file: choreofs::WriteFile,
}

impl Led {
    pub fn open(path: &str) -> Result<Self> {
        let path = normalize_led_path(path)?;
        let file = choreofs::open_write(DEVICE_PREOPEN_FD, path)?;
        Ok(Self { file })
    }

    pub fn set(&self, on: bool) -> Result<()> {
        self.file.write_once_exact(if on { b"1" } else { b"0" })
    }
}

pub fn sleep_ms(ms: u32) -> Result<()> {
    time::sleep_ms(ms)
}

fn normalize_led_path(path: &str) -> Result<&str> {
    let path = path.strip_prefix('/').unwrap_or(path);
    let Some(led_name) = path.strip_prefix(LED_PATH_PREFIX) else {
        return Err(Error::InvalidPath);
    };
    if led_name.is_empty()
        || led_name == "."
        || led_name == ".."
        || led_name.as_bytes().contains(&0)
        || led_name.contains('/')
    {
        return Err(Error::InvalidPath);
    }
    Ok(path)
}
