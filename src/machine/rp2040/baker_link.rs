use crate::machine::{BoardSafeState, SafeGpioLevel};

pub const BAKER_LINK_USER_LED_PINS: [u8; 3] = [22, 21, 20];
pub const BAKER_LINK_USER_LED_ACTIVE_HIGH: bool = true;
pub const BAKER_LINK_SAFE_GPIO_LEVELS: [SafeGpioLevel; 3] = [
    SafeGpioLevel::new(
        BAKER_LINK_USER_LED_PINS[0],
        !BAKER_LINK_USER_LED_ACTIVE_HIGH,
    ),
    SafeGpioLevel::new(
        BAKER_LINK_USER_LED_PINS[1],
        !BAKER_LINK_USER_LED_ACTIVE_HIGH,
    ),
    SafeGpioLevel::new(
        BAKER_LINK_USER_LED_PINS[2],
        !BAKER_LINK_USER_LED_ACTIVE_HIGH,
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakerLinkBoard;

impl BoardSafeState for BakerLinkBoard {
    const GPIO_LEVELS: &'static [SafeGpioLevel] = &BAKER_LINK_SAFE_GPIO_LEVELS;
}

#[cfg(test)]
mod tests {
    use super::{
        BAKER_LINK_SAFE_GPIO_LEVELS, BAKER_LINK_USER_LED_ACTIVE_HIGH, BAKER_LINK_USER_LED_PINS,
        BakerLinkBoard,
    };
    use crate::machine::BoardSafeState;

    #[test]
    fn baker_board_safe_state_turns_all_user_leds_inactive() {
        let levels = BakerLinkBoard::GPIO_LEVELS;
        assert_eq!(levels.len(), BAKER_LINK_USER_LED_PINS.len());
        assert_eq!(levels, &BAKER_LINK_SAFE_GPIO_LEVELS);
        for (level, pin) in levels.iter().zip(BAKER_LINK_USER_LED_PINS) {
            assert_eq!(level.pin(), pin);
            assert_eq!(level.high(), !BAKER_LINK_USER_LED_ACTIVE_HIGH);
        }
    }
}
