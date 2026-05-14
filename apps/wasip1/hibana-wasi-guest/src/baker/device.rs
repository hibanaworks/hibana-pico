use crate::{Error, Result, choreofs};

const DEVICE_PREOPEN_FD: u32 = 9;
const LED_PATH_PREFIX: &str = "device/led/";

pub struct Led {
    file: choreofs::WriteFile,
}

impl Led {
    pub fn open(path: &str) -> Result<Self> {
        let path = normalize_device_path(path)?;
        let file = choreofs::open_write(DEVICE_PREOPEN_FD, path)?;
        Ok(Self { file })
    }

    pub fn set(&self, on: bool) -> Result<()> {
        self.file.write_once_exact(if on { b"1" } else { b"0" })
    }
}

fn normalize_device_path(path: &str) -> Result<&str> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_led_path_accepts_absolute_and_relative_paths() {
        assert_eq!(
            normalize_device_path("/device/led/green"),
            Ok("device/led/green")
        );
        assert_eq!(
            normalize_device_path("device/led/yellow"),
            Ok("device/led/yellow")
        );
    }

    #[test]
    fn normalize_led_path_rejects_paths_outside_led_namespace() {
        for path in ["", "/", "device", "device/led", "device/gpio/green"] {
            assert_eq!(normalize_device_path(path), Err(Error::InvalidPath));
        }
    }

    #[test]
    fn normalize_led_path_rejects_ambiguous_led_names() {
        for path in [
            "device/led/",
            "device/led//green",
            "device/led/.",
            "device/led/..",
            "device/led/../state",
            "device/led/green/extra",
            "device/led/green\0",
        ] {
            assert_eq!(normalize_device_path(path), Err(Error::InvalidPath));
        }
    }

}
