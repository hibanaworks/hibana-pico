# rpw2-firmware

Pico 2 W sensor panel firmware for the hibana-pico choreographic runtime.

## Runtime shape

- Core 0 runs the role-0 driver image: RP2350 hardware setup, Grove sensor reads,
  UART0 output, LCD writes, and ChoreoFS request handling.
- Core 1 runs the role-1 engine image with the embedded `wasm32-wasip1` guest.
- The choreography admits the WASI import loop as `path_open`, `fd_read`,
  `fd_write`, and `poll_oneoff` exchanges between role 1 and role 0.
- ChoreoFS exposes normal WASI path-opened files:
  `device/rpw2/sample` as fd 3 for sensor reads,
  `device/rpw2/display` as fd 4 for LCD/UART writes, and
  `device/rpw2/udp/172.20.10.8/8787` as fd 5 for writes to the Uno Q sensor
  receiver.

## Wiring

- UART0 debug output: Pico GPIO0/UART0 TX to Debug Probe RX, Pico GPIO1/UART0 RX to Debug Probe TX, and GND to GND.
- I2C0 LCD: Grove 16x2 LCD v2.0 White on Blue, address `0x3e`, on GPIO8 SDA / GPIO9 SCL.
- I2C1 temperature/humidity: Grove DHT20 v2.1, address `0x38`, on GPIO6 SDA / GPIO7 SCL.
- A0 light: Grove Light Sensor v1.3 on GPIO26 / ADC0. Power this Grove sensor from 3.3 V.

## Build

```sh
examples/rpw2-firmware/scripts/build_sensor_panel.sh
```

The script builds the WASI P1 guest first, embeds it into the RP2350 firmware,
and writes the firmware ELF to:

```text
target/thumbv8m.main-none-eabi/release/rpw2-sensor-panel
```
