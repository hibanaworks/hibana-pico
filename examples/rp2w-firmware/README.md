# rp2w-firmware

Pico 2 W firmware examples for the hibana-pico choreographic runtime.

## rp2w-sensor-panel

This is the Pico 2 W sensor-panel demo used by `uno-q-heterogeneous`.

Runtime shape:

- core0 runs the hibana driver role, owns the physical sensors/LCD/CYW43 Wi-Fi
  path, and handles ChoreoFS requests from the guest.
- core1 runs the hibana engine role and drives a `wasm32-wasip1` guest.
- the guest opens `device/rp2w/sample`, `device/rp2w/display`, and
  `device/rp2w/udp/uno-q`.
- each cycle reads one atomic 9-byte `Pico2wSensorSample`, writes that same
  typed payload to the display fd and to the UDP fd.
- the display driver formats the typed sample for the LCD.
- the UDP driver sends the typed sample as a CYW43 UDP frame for Uno Q.

```text
status:u8 temp_c_x10:i16 humidity_pct_x10:u16 light_raw:u16 seq:u16
```

Start the Uno Q side with:

```sh
UNO_Q_PICO2W_SENSOR_MODE=udp \
UNO_Q_PICO2W_SENSOR_UDP_BIND=0.0.0.0:8787 \
cargo run -p uno-q-heterogeneous --features runtime-wasip1,embed-wasip1-artifacts --bin uno-q-hardware-proof -- --sensor-udp
```

## rp2w-epf-policy-timer

This is the RP2350/Pico 2 W EPF proof demo.

Runtime shape:

- core0 runs the hibana driver role, owns Debug Probe UART ingress, and handles
  ChoreoFS requests from the guest.
- core1 runs the hibana engine role and drives a `std` `wasm32-wasip1` guest.
- the WASI P1 guest uses ordinary Rust `std::fs` / `std::io`; those WASI
  imports are carried by hibana choreography over the RP2350 SIO transport and
  materialized by host-side ChoreoFS facts.
- Debug Probe UART carries an EPF policy image into the choreography.
- both cores load the same EPF image through the hibana-selected image delivery
  branch.
- the timer IRQ fact resolver chooses the timer-expired branch when the timer
  IRQ is observed.
- a loaded `Target::Policy(57)` EPF VM can select the response-ready branch
  from that resolver entry instead.
- the native runtime drains the hibana `TapPort`; loaded observe bytecode can
  report compact `Out` records to RAM markers and UART.

The guest is intentionally small:

```text
device/rp2w/epf-policy-timer
  path_open(sample)
  path_open(display)
  fd_read(sample) -> "rp2w sample t=23.4 h=45\n"
  fd_write(display, sample-bytes)
```

This keeps the proof about hibana wiring, WASI P1 coexistence, SIO transport,
TapEvent observation, EPF policy override, and real driver-side device commit.
It is not a standalone UART smoke test.

The ChoreoFS counters are only wiring guards: they prove that the core1 WASI P1
guest really entered hibana through `path_open`, `fd_read`, and `fd_write`.
They are not the purpose of the demo. The acceptance signal is the EPF path plus
the physical display commit:

```text
UART bytecode ingress
  -> hibana-selected image delivery branch
  -> Target::Policy(57) loaded on both cores
  -> timer TapEvent fact observed at resolver entry
  -> EPF policy VM consumes fuel and selects the response-ready branch
  -> compact Out reaches RAM markers / UART
  -> WASI P1 guest fd_write reaches core0
  -> core0 writes the sample value to the I2C LCD
```

## Wiring

- Debug Probe TX -> Pico GPIO1 / UART0 RX
- Debug Probe RX -> Pico GPIO0 / UART0 TX
- Debug Probe GND -> Pico GND
- HD44780/PCF8574 or ST7032 LCD SDA/SCL -> any probed Pico I2C pair except GPIO0/1
- HD44780/PCF8574 or ST7032 LCD VCC/GND -> board power/GND

The firmware probes common Pico I2C pairs on I2C0/I2C1, skipping GPIO0/1
because Debug Probe UART owns those pins. It then tries the common PCF8574 LCD
addresses `0x27` and `0x3f` plus the common ST7032 address `0x3e`. A successful
hardware run requires the LCD write marker to report an acknowledged address
and the SDA/SCL pair, not just the display payload hash marker.

## Build

```sh
RP2W_SENSOR_PANEL_WIFI_SSID='your-ssid' \
RP2W_SENSOR_PANEL_WIFI_KEY='your-passphrase' \
RP2W_SENSOR_PANEL_LOCAL_MAC='02:12:34:56:78:9a' \
RP2W_SENSOR_PANEL_LOCAL_IP='your-pico2w-ip' \
RP2W_SENSOR_PANEL_UNO_Q_MAC='your-uno-q-wlan0-mac' \
RP2W_SENSOR_PANEL_UNO_Q_IP='your-uno-q-wlan0-ip' \
bash scripts/check_wasip1_guest_builds.sh
bash examples/rp2w-firmware/scripts/build_sensor_panel.sh
cargo build --target thumbv8m.main-none-eabi --release \
  -p rp2w-firmware \
  --bin rp2w-epf-policy-timer \
  --features wasm-engine-core,embed-wasip1-artifacts
```
