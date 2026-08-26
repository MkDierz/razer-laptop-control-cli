# Razer laptop control CLI

CLI to control fans, power modes and the RGB keyboard of Razer laptops on Linux — directly over USB HID feature reports, with no kernel module and no background daemon.

## Building

Requires a Rust toolchain plus these system packages:

- Fedora: `sudo dnf install pkg-config systemd-devel libusb1-devel`
- Debian/Ubuntu: `sudo apt install pkg-config libudev-dev libusb-1.0-0-dev`

```sh
cargo build --release
```

The binary lands at `target/release/razer-cli`.

## Device permissions

To use the CLI without root, install the udev rule once:

```sh
sudo cp data/udev/99-razer.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=hidraw
```

(Replug the laptop or reboot if permissions still don't apply.) Otherwise run the CLI with `sudo`.

## Usage

```sh
razer-cli write fan 5000            # manual fan RPM on the active profile (0 = auto)
razer-cli write power 4 2 2         # custom power mode: CPU boost 2, GPU boost 2
razer-cli write brightness 80       # keyboard brightness percent
razer-cli write bho on 70          # battery health optimizer threshold
razer-cli effect static 0 255 0    # firmware lighting effects
razer-cli read power               # inspect the active profile
razer-cli restore                  # apply the saved profile for the current power source
```

Settings are stored per power state (battery/AC) in `~/.local/share/razercontrol/config.json`. Profile-aware commands automatically use the current power source from `/sys/class/power_supply/AC/online` and apply changes immediately. Run `razer-cli restore` after switching, or hook it into your desktop autostart / a systemd user unit for automatic restoration at boot.

## Supported laptops

See `data/devices/laptops.json` for known models (matched by USB vendor/product ID). To add one, append an entry and rebuild.

## Documentation

- [Architecture](docs/architecture.md) — module map, config model, design decisions
- [Razer HID protocol](docs/razer-hid-protocol.md) — packet format and command reference
