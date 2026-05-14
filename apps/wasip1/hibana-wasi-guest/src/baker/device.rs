use crate::{Error, Result, choreofs};

const DEVICE_PREOPEN_FD: u32 = 9;
const LED_PATH_PREFIX: &str = "device/led/";
const TRAFFIC_PATH: &str = "device/traffic";

pub struct Led {
    file: choreofs::WriteFile,
}

pub struct TrafficLight {
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

impl TrafficLight {
    pub fn open(path: &str) -> Result<Self> {
        let path = normalize_traffic_path(path)?;
        let file = choreofs::open_write(DEVICE_PREOPEN_FD, path)?;
        Ok(Self { file })
    }

    pub fn set_mask(&self, mask: u8) -> Result<()> {
        let payload = match mask {
            1 => b"1".as_slice(),
            2 => b"2".as_slice(),
            4 => b"4".as_slice(),
            _ => return Err(Error::InvalidPayload),
        };
        self.file.write_once_exact(payload)
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

fn normalize_traffic_path(path: &str) -> Result<&str> {
    let path = path.strip_prefix('/').unwrap_or(path);
    if path == TRAFFIC_PATH {
        Ok(path)
    } else {
        Err(Error::InvalidPath)
    }
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
            normalize_device_path("device/led/orange"),
            Ok("device/led/orange")
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

    #[test]
    fn normalize_traffic_path_accepts_only_traffic_object() {
        assert_eq!(
            normalize_traffic_path("/device/traffic"),
            Ok("device/traffic")
        );
        assert_eq!(normalize_traffic_path("device/traffic"), Ok("device/traffic"));
        assert_eq!(
            normalize_traffic_path("device/led/green"),
            Err(Error::InvalidPath)
        );
    }
}
