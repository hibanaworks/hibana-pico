use hibana_pico::{
    choreography::protocol::GpioSet,
    machine::rp2040::{baker_link::BAKER_LINK_SAFE_GPIO_LEVELS, gpio},
    projects::baker_link_led::manifest::apply_baker_link_led_bank_set,
};

pub fn rp2040_gpio_bank_init() {
    gpio::bank_init();
}

pub fn rp2040_gpio_init_output(pin: u8, initial_high: bool) {
    gpio::init_output(pin, initial_high);
}

fn rp2040_gpio_write(pin: u8, high: bool) {
    gpio::write(pin, high);
}

pub fn rp2040_gpio_apply_baker_led_set(set: GpioSet) {
    apply_baker_link_led_bank_set(rp2040_gpio_write, set);
}

pub fn baker_link_leds_off_direct() {
    for level in BAKER_LINK_SAFE_GPIO_LEVELS {
        rp2040_gpio_write(level.pin(), level.high());
    }
}
