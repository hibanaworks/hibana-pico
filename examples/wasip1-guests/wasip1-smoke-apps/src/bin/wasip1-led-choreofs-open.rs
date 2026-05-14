use hibana_wasi_guest::baker::{Led, sleep_ms};

fn main() {
    if run().is_err() {
        abort();
    }
}

fn run() -> hibana_wasi_guest::Result<()> {
    let green = must(Led::open("/device/led/green"));
    let yellow = must(Led::open("/device/led/yellow"));
    let red = must(Led::open("/device/led/red"));

    loop {
        must(green.set(true));
        must(yellow.set(false));
        must(red.set(false));
        must(sleep_ms(180));

        must(green.set(false));
        must(yellow.set(true));
        must(red.set(false));
        must(sleep_ms(40));
        must(yellow.set(false));
        must(sleep_ms(40));
        must(yellow.set(true));
        must(sleep_ms(40));
        must(yellow.set(false));
        must(sleep_ms(40));
        must(yellow.set(true));
        must(sleep_ms(40));

        must(green.set(false));
        must(yellow.set(false));
        must(red.set(true));
        must(sleep_ms(180));
    }
}

fn must<T>(result: hibana_wasi_guest::Result<T>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => abort(),
    }
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
