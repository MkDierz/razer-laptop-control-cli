# Razer HID Protocol

How this project talks to Razer laptop hardware. Everything below is read directly from `src/device.rs` — treat it as the source of truth if code and this doc diverge.

## Architecture overview

There is **no kernel module and no daemon**. Each `razer-cli` invocation opens the keyboard's USB HID interface directly, sends feature reports, applies or reads state, saves the profile to `~/.local/share/razercontrol/config.json` when writing, and exits.

```
razer-cli --[91-byte feature reports]--> /dev/hidraw* --> EC / keyboard controller
```

- Discovery (`open_device`, src/device.rs): enumerate HID devices with vendor ID `0x1532` on interface `0`, match product ID against the supported list, open first hit.
- The supported-device list is **embedded at compile time** from `data/devices/laptops.json` (`include_str!`) — there is no runtime file dependency.
- Access to `/dev/hidraw*` requires either the udev rule in `data/udev/99-hidraw-permissions.rules` or root.
- Power source detection (AC vs battery) reads `/sys/class/power_supply/AC/online`; it does not come from the HID device. Profile-aware CLI commands use this source automatically, so they do not require an `ac` or `bat` argument.

## Control packet format

All commands use a fixed 91-byte **feature report** (`RazerPacket`, serialized with bincode):

| Offset | Size | Field              | Notes                                              |
|--------|------|--------------------|----------------------------------------------------|
| 0x00   | 1    | `report`           | HID report ID, always `0x00`                       |
| 0x01   | 1    | `status`           | Request: `0x00` (new). Response: `0x02` = success, `0x05` = not supported |
| 0x02   | 1    | `id`               | Transaction ID, always `0x1F`                      |
| 0x03   | 2    | `remaining_packets`| Always `0x0000` (u16, little-endian)                |
| 0x05   | 1    | `protocol_type`    | Always `0x00`                                      |
| 0x06   | 1    | `data_size`        | Number of meaningful arg bytes for this command     |
| 0x07   | 1    | `command_class`    | Function group (see below)                         |
| 0x08   | 1    | `command_id`       | Read (`0x8x`) or write command within the class     |
| 0x09   | 80   | `args`             | Command payload                                    |
| 0x59   | 1    | `crc`              | XOR of bytes 0x02..=0x57                           |
| 0x5A   | 1    | `reserved`         | `0x00`                                             |

### Transaction flow (`RazerLaptop::send_report`)

1. `hid_send_feature_report(91 bytes)`
2. sleep ~1 ms
3. `hid_get_feature_report` into a 91-byte buffer; must return exactly 91 bytes to be accepted
4. Validate response: `command_class`, `command_id`, and `remaining_packets` must match the request, then require `status == 0x02`
5. Retry up to 3 times; after all failures, sleep ~8 ms and return `None`

Known quirk: `RazerPacket::calc_crc` serializes the struct *before* writing the computed CRC into it, so the transmitted packet effectively carries `crc = 0x00`. The firmware accepts this. Preserve this behavior when touching `calc_crc`/`send_report`; a unit test pins it (`transmitted_crc_is_zero_but_field_holds_checksum`).

## Command classes

### LED control — class `0x03`

| cmd   | Direction | Args                                          | Purpose |
|-------|-----------|-----------------------------------------------|---------|
| 0x00  | set       | `[VARSTORE(0x01), led_id, value]`             | LED state on/off |
| 0x02  | set       | `[VARSTORE, LOGO_LED(0x04), 0x00\|0x02]`      | Logo effect: off / breathing (only sent when mode > 0) |
| 0x03  | set       | `[VARSTORE, BACKLIGHT_LED(0x05), brightness]` | Backlight brightness, 0–255 |
| 0x82  | get       | `[VARSTORE, led_id]`                          | Read LED state (response in `args[2]`) |
| 0x83  | get       | `[VARSTORE, BACKLIGHT_LED]`                   | Read brightness (response in `args[2]`) |
| 0x0A  | set       | `[effect_id, params...]`                      | Activate firmware effect |

LED storage flag: `VARSTORE = 0x01`.

Firmware effect IDs (names accepted by `razer-cli effect`):

| ID    | Name        | Params |
|-------|-------------|--------|
| 0x00  | `off`       | none |
| 0x01  | `wave`      | direction |
| 0x02  | `reactive`  | speed, R, G, B |
| 0x03  | `breathing` | type [, colours] |
| 0x04  | `spectrum`  | none |
| 0x06  | `static`    | R, G, B |
| 0x19  | `starlight` | type [, colours] |

For completeness, the hardware also supports per-key custom frames (`0x0B` row upload with RGB data at `args[7..52]`, then effect id `0x05`); this project previously used them for software animations and no longer does.

### Power/fan control — class `0x0D`

Zones are `0x01` and `0x02`; most operations are applied to both zones in sequence.

| cmd   | Direction | Args                                        | Purpose |
|-------|-----------|---------------------------------------------|---------|
| 0x82  | get       | `[0x00, zone, 0x00, 0x00]`                  | Read power mode of zone (response `args[2]`) |
| 0x02  | set       | `[0x00, zone, mode, fan_flag]`              | Set power level; `fan_flag` = `0x01` when manual RPM active, else `0x00` |
| 0x87  | get       | `[0x00, 0x01(CPU)\|0x02(GPU), 0x00]`        | Read boost level (response `args[2]`) |
| 0x07  | set       | `[0x00, 0x01\|0x02, level]`                 | Set CPU/GPU boost, 0–3 (3 clamped to 2 unless model has `boost` feature) |
| 0x01  | set       | `[0x00, zone, rpm_hundreds]`                | Set fan RPM; RPM is sent divided by 100 |

Power modes: `0` balanced, `1` gaming, `2` creator, `4` custom. Writing mode ≤ 3 just sets both zones. Mode 4 (custom) additionally performs get+set of CPU boost and GPU boost before re-setting both zones, and resets fan control to automatic.

Manual fan RPM is only applied while **not** in custom mode (`power != 4`); `rpm == 0` always means automatic and skips the `0x01` set-rpm command.

### Battery health optimizer — class `0x07` (models with `bho` feature)

| cmd   | Direction | Args        | Purpose |
|-------|-----------|-------------|---------|
| 0x92  | get       | `[0x00]`    | Read status into `args[0]` |
| 0x12  | set       | `[encoded]` | Write status |

Encoding of the status byte: bit 7 = enabled, bits 0–6 = charge threshold (%). See `bho_to_byte`/`byte_to_bho`.

Note: for the BHO get, the response is accepted based on `command_id == 0x92` alone, bypassing the normal request-match check.
