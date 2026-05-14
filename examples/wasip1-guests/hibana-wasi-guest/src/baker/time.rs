use crate::{Result, sys};

pub fn sleep_ms(ms: u32) -> Result<()> {
    sys::sleep_ms(ms)
}
