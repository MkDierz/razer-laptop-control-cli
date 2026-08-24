# Architecture

Single-crate Rust CLI that talks to the hardware directly.

## Module map

```
Cargo.toml              one package, one binary: razer-cli
data/
  devices/laptops.json  supported models (vid/pid/features/fan range), embedded via include_str!
  udev/                 hidraw permission rule for non-root access
src/
  main.rs               CLI layer only: clap definitions, validation, profile logic, printing
  device.rs             hardware layer: HID discovery, packet codec, all protocol commands, unit tests
  config.rs             persistence layer: two power profiles + sync + last effect, JSON on disk
docs/
  architecture.md       this file
  razer-hid-protocol.md wire-level protocol reference
```

Rule of thumb: `main.rs` never builds packets and `device.rs` never touches config files.

## Data flow for a write command

Example `razer-cli write fan 5000`:

1. `open_device()` finds and opens the laptop (exits with a clear error if none).
2. `Config::load()` reads `~/.local/share/razercontrol/config.json` (defaults if missing).
3. The current power source is detected from `/sys/class/power_supply/AC/online`.
4. The value is written into the active battery or AC profile and saved.
5. The change is applied over HID immediately.
5. The result is printed.

There is no positional `ac`/`bat` argument. The inactive profile can be edited by changing the power source before running the command; there is no daemon watching for power-source changes anymore.

## Config model (`src/config.rs`)

`~/.local/share/razercontrol/config.json`:

```json
{
  "power": [ { "battery profile..." }, { "AC profile..." } ],
  "sync": false,
  "effect": { "name": "static", "params": [255, 0, 0] }
}
```

- Each profile holds `power_mode`, `cpu_boost`, `gpu_boost`, `fan_rpm`, `brightness` (percent), `logo_state`.
- Index `0` = battery, index `1` = AC.
- When `sync` is true, lighting writes (brightness, logo) are mirrored to the other profile by `Config::mirror_lighting`.
- `effect` stores the last firmware effect so `razer-cli restore` can reapply it.
- All fields are `#[serde(default)]`: missing fields fall back to sane defaults instead of failing.

## Restore flow

`razer-cli restore` applies the saved profile for the current power source: power mode (+ boosts), fan RPM, brightness, logo (only if the model has the feature), then the stored effect. Users who want boot-time restoration can run it from their own systemd user unit or desktop autostart entry; nothing is installed automatically.

## Supported devices (`data/devices/laptops.json`)

```json
{
    "name": "Blade 15 2016",
    "vid": "1532",
    "pid": "0224",
    "features": ["logo"],
    "fan": [3500, 5000]
}
```

- `features`: capability gates known to the code: `logo`, `boost` (CPU boost level 3), `bho`.
- `fan`: `[min, max]` RPM used to clamp manual fan values (stored/sent in units of 100).
- The file is compiled into the binary (`include_str!`); changing it requires a rebuild but removes all runtime path dependencies.

## Design decisions worth knowing

- **Firmware effects only.** The old software per-key RGB animator (custom-frame uploads, layered blending) was removed. Effects run on the embedded controller, so nothing needs to keep running after the CLI exits.
- **No daemon means no idle dimming/screensaver integration** (the old D-Bus/Mutter machinery is gone). Reintroducing any background behavior should be weighed against keeping the tool stateless.
- **Behavior parity with the daemon era was preserved** in the protocol layer, including quirks — see "Known quirk" notes in `docs/razer-hid-protocol.md`. Unit tests in `src/device.rs` pin the packet layout, CRC behavior, clamping and encodings.
