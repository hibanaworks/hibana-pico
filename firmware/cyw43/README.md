# CYW43439 Firmware Artifacts

This directory is for local Raspberry Pi Pico W / Pico 2 W CYW43439 Wi-Fi
firmware artifacts used by the Hibana/Pico QEMU load-prefix proof.

Do not commit the generated firmware blobs, manifest, disassembly excerpt, or
local license copy. They are intentionally ignored by git. Users should fetch
them from Pico SDK / cyw43-driver themselves under the upstream Raspberry Pi
firmware license.

The only file intended to be committed in this directory is this README.

Source:

- Pico SDK: `raspberrypi/pico-sdk` commit
  `a1438dff1d38bd9c65dbd693f0e5db4b9ae91779`
- CYW43 driver submodule: `georgerobotics/cyw43-driver` commit
  `dd7568229f3bf7a37737b9e1ef250c26efe75b23`
- Header:
  `lib/cyw43-driver/firmware/w43439A0_7_95_49_00_combined.h`

Local generated files:

- `w43439A0_7_95_49_00_combined.bin`: padded Wi-Fi firmware plus CLM blob
- `w43439A0_7_95_49_00_firmware.bin`: unpadded Wi-Fi firmware bytes
- `w43439A0_7_95_49_00_clm.bin`: CLM blob bytes
- `w43439A0_7_95_49_00.manifest.json`: lengths, offsets, SHA-256, and
  FNV-1a-32 values used by the QEMU firmware-load protocol
- `w43439A0_7_95_49_00_firmware.thumb.disasm.head.txt`: local excerpt from
  the raw Thumb disassembly

Regenerate:

```sh
git clone --depth 1 https://github.com/raspberrypi/pico-sdk.git /tmp/pico-sdk
git -C /tmp/pico-sdk submodule update --init --depth 1 lib/cyw43-driver

./scripts/extract_cyw43_firmware.py \
  /tmp/pico-sdk/lib/cyw43-driver/firmware/w43439A0_7_95_49_00_combined.h \
  --out-dir firmware/cyw43 \
  --pico-sdk-commit a1438dff1d38bd9c65dbd693f0e5db4b9ae91779 \
  --cyw43-driver-commit dd7568229f3bf7a37737b9e1ef250c26efe75b23

cp /tmp/pico-sdk/lib/cyw43-driver/LICENSE.RP firmware/cyw43/LICENSE.RP
./scripts/disassemble_cyw43_firmware.sh
```

License:

The firmware artifacts are supplied upstream under `LICENSE.RP`. Hibana/Pico
does not redistribute those artifacts or the copied license file. Keep them
local and review the upstream license before use; it limits use and
redistribution to RP2040 or other Raspberry Pi semiconductor devices.
