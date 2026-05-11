#[cfg(any(
    all(
        feature = "baker-bad-order-demo",
        any(
            feature = "baker-chaser-demo",
            feature = "baker-ordinary-std-demo",
            feature = "baker-invalid-fd-demo",
            feature = "baker-bad-payload-demo",
            feature = "baker-choreofs-demo",
            feature = "baker-choreofs-bad-path-demo",
            feature = "baker-choreofs-bad-payload-demo",
            feature = "baker-choreofs-wrong-object-demo",
            feature = "baker-abort-safe-demo"
        )
    ),
    all(
        feature = "baker-chaser-demo",
        any(
            feature = "baker-ordinary-std-demo",
            feature = "baker-invalid-fd-demo",
            feature = "baker-bad-payload-demo",
            feature = "baker-choreofs-demo",
            feature = "baker-choreofs-bad-path-demo",
            feature = "baker-choreofs-bad-payload-demo",
            feature = "baker-choreofs-wrong-object-demo",
            feature = "baker-abort-safe-demo"
        )
    ),
    all(
        feature = "baker-ordinary-std-demo",
        any(
            feature = "baker-invalid-fd-demo",
            feature = "baker-bad-payload-demo",
            feature = "baker-choreofs-demo",
            feature = "baker-choreofs-bad-path-demo",
            feature = "baker-choreofs-bad-payload-demo",
            feature = "baker-choreofs-wrong-object-demo",
            feature = "baker-abort-safe-demo"
        )
    ),
    all(
        feature = "baker-invalid-fd-demo",
        any(
            feature = "baker-bad-payload-demo",
            feature = "baker-choreofs-demo",
            feature = "baker-choreofs-bad-path-demo",
            feature = "baker-choreofs-bad-payload-demo",
            feature = "baker-choreofs-wrong-object-demo",
            feature = "baker-abort-safe-demo"
        )
    ),
    all(
        feature = "baker-bad-payload-demo",
        any(
            feature = "baker-choreofs-demo",
            feature = "baker-choreofs-bad-path-demo",
            feature = "baker-choreofs-bad-payload-demo",
            feature = "baker-choreofs-wrong-object-demo",
            feature = "baker-abort-safe-demo"
        )
    ),
    all(
        feature = "baker-choreofs-demo",
        any(
            feature = "baker-choreofs-bad-path-demo",
            feature = "baker-choreofs-bad-payload-demo",
            feature = "baker-choreofs-wrong-object-demo",
            feature = "baker-abort-safe-demo"
        )
    ),
    all(
        feature = "baker-choreofs-bad-path-demo",
        any(
            feature = "baker-choreofs-bad-payload-demo",
            feature = "baker-choreofs-wrong-object-demo",
            feature = "baker-abort-safe-demo"
        )
    ),
    all(
        feature = "baker-choreofs-bad-payload-demo",
        any(
            feature = "baker-choreofs-wrong-object-demo",
            feature = "baker-abort-safe-demo"
        )
    ),
    all(
        feature = "baker-choreofs-wrong-object-demo",
        feature = "baker-abort-safe-demo"
    )
))]
compile_error!("select at most one Baker WASI guest pattern");

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(not(any(
    feature = "baker-bad-order-demo",
    feature = "baker-chaser-demo",
    feature = "baker-ordinary-std-demo",
    feature = "baker-choreofs-demo",
    feature = "baker-choreofs-bad-path-demo",
    feature = "baker-choreofs-bad-payload-demo",
    feature = "baker-choreofs-wrong-object-demo",
    feature = "baker-invalid-fd-demo",
    feature = "baker-bad-payload-demo",
    feature = "baker-abort-safe-demo"
)))]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-blink.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-bad-order-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-bad-order.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-chaser-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-chaser.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-invalid-fd-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-invalid-fd.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-bad-payload-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-bad-payload.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-ordinary-std-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-ordinary-std-chaser.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-choreofs-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-choreofs-open.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-choreofs-bad-path-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-choreofs-bad-path.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-choreofs-bad-payload-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-choreofs-bad-payload.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-choreofs-wrong-object-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-choreofs-wrong-object.wasm"
));
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[cfg(feature = "baker-abort-safe-demo")]
pub static WASIP1_LED_GUEST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasip1-apps/wasm32-wasip1/release/wasip1-led-blink.wasm"
));

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn write_selected_guest_in_place<'slot>(
    slot: &'slot mut core::mem::MaybeUninit<crate::kernel::engine::wasm::Guest<'static>>,
) -> Result<
    &'slot mut crate::kernel::engine::wasm::Guest<'static>,
    crate::kernel::engine::wasm::Error,
> {
    crate::kernel::engine::wasm::Guest::place_in_static_slot(slot, WASIP1_LED_GUEST)
}
