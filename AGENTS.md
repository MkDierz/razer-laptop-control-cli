# AGENTS.md

Single-crate Rust CLI (`razer-cli`) that controls Razer laptops (fan/power/RGB keyboard/battery health optimizer) directly over USB HID feature reports — no kernel module, no daemon, no GUI, no installer, no Nix. Those were removed deliberately; don't reintroduce them without asking.

## Build & verify

Run all cargo commands from the repo root (the crate is at the root, not a subdirectory):

- `cargo build` / `cargo test` / `cargo clippy`
- System deps to compile: `pkg-config`, `libudev`, `libusb-1.0` dev packages (Fedora: `systemd-devel libusb1-devel`; Debian: `libudev-dev libusb-1.0-0-dev`).
- CI (`.github/workflows/ci.yml`, targets `main`) runs build + test only; GitHub Actions versions are managed by Dependabot.

## Structure

- `src/main.rs` — CLI layer: clap definitions, validation, profile/save/apply logic. Must not build HID packets.
- `src/device.rs` — hardware layer: discovery, 91-byte packet codec, protocol commands, unit tests.
- `src/config.rs` — persistence: two power profiles (index 0 = battery, 1 = AC), sync flag, last effect; JSON at `~/.local/share/razercontrol/config.json`.
- `data/devices/laptops.json` — supported models, **embedded via `include_str!`** in `src/device.rs`; changing it requires a rebuild, and there is no runtime path dependency anymore.

## Hardware behavior gotchas

- Protocol reference lives in `docs/razer-hid-protocol.md`; read it before touching `src/device.rs`.
- Preserve known quirks for behavior parity: transmitted packets always carry `crc = 0x00` (see `calc_crc`), manual fan RPM is ignored while power mode is custom (`power != 4` guard), CPU boost 3 clamps to 2 unless the model has the `boost` feature, BHO get responses bypass the normal request-match check. Unit tests pin these — run them after any change there.
- Profile-aware commands automatically target the active profile (bat/ac), based on `/sys/class/power_supply/AC/online`.

## Conventions

- Clippy lints configured in `[lints.clippy]` in `Cargo.toml` (e.g. `get_first = "allow"`) — respect them instead of sprinkling inline allows.
- Docs are modular: `docs/architecture.md` (design/config model) and `docs/razer-hid-protocol.md` (wire protocol). Update them together with code changes.
