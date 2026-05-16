use baker_wasip1_guest::{Led, sleep_ms};

const STEP_MS: u32 = 80;

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> hibana_wasip1_guest::Result<()> {
    let green = Led::open("/device/led/green")?;
    let yellow = Led::open("/device/led/yellow")?;
    let red = Led::open("/device/led/red")?;

    loop {
        set_and_wait(&green, true)?;
        set_and_wait(&yellow, false)?;
        set_and_wait(&red, false)?;

        set_and_wait(&green, false)?;
        set_and_wait(&yellow, true)?;
        set_and_wait(&red, false)?;

        set_and_wait(&yellow, false)?;
        set_and_wait(&yellow, true)?;
        set_and_wait(&yellow, false)?;
        set_and_wait(&yellow, true)?;

        set_and_wait(&green, false)?;
        set_and_wait(&yellow, false)?;
        set_and_wait(&red, true)?;
    }
}

fn set_and_wait(led: &Led, on: bool) -> hibana_wasip1_guest::Result<()> {
    led.set(on)?;
    sleep_ms(STEP_MS)
}

#[cold]
fn abort() -> ! {
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    loop {
        core::hint::spin_loop();
    }
}
