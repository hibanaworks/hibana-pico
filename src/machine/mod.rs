pub mod rp2040;
pub mod rp2350;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SafeGpioLevel {
    pin: u8,
    high: bool,
}

impl SafeGpioLevel {
    pub const fn new(pin: u8, high: bool) -> Self {
        Self { pin, high }
    }

    pub const fn pin(self) -> u8 {
        self.pin
    }

    pub const fn high(self) -> bool {
        self.high
    }
}

pub trait BoardSafeState {
    const GPIO_LEVELS: &'static [SafeGpioLevel];
}
