# 🦀 OpenVXE

![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)
![Linux](https://img.shields.io/badge/OS-Linux-blue.svg)
![Status](https://img.shields.io/badge/Status-Early_Beta-red.svg)

An open-source, lightweight, and blazing-fast TUI (Terminal User Interface) driver for the **VXE R1 SE+** wireless gaming mouse, written entirely in Rust.

## ✨ Features

* **Native Linux Support:** Direct hardware communication via `hidapi` (no root required with udev rules).
* **Terminal UI:** Beautiful, minimalist interface built with `ratatui`.
* **Live Configuration:** Apply changes instantly without saving.
  * Adjust DPI (50 - 12,750)
    > ⚠️ Values above 12,750 DPI are not supported yet due to a known encoding limitation; full range (up to 26,000) is pending protocol reverse engineering.
  * Polling Rate (250Hz / 500Hz / 1000Hz)
  * Lift-off Distance (LOD)
  * Debounce Time (1ms - 20ms)
  * Motion Sync (On/Off)
  * Angle Snapping (On/Off)
* **Real-time Battery Monitor:** Accurate battery parsing straight from the mouse's internal sensors.
* **Hex Sniffer:** Built-in packet monitor for debugging hardware responses.

## 🚀 Installation & Setup

### 1. Install Dependencies

Ensure you have Rust and the required HID libraries installed.

**Arch Linux:**
```bash
sudo pacman -S rustup base-devel hidapi
rustup default stable
```

**Debian / Ubuntu:**
```bash
sudo apt install build-essential libhidapi-dev pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Fedora:**
```bash
sudo dnf install gcc hidapi-devel pkgconf-pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Install the udev Rules

The included rule gives the logged-in user access to the mouse's `hidraw` nodes, so the driver can run without `sudo`:
```bash
sudo cp udev/99-openvxe.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```
Then unplug and replug the USB dongle.

### 3. Build & Run

```bash
cargo build --release
./target/release/vxe_driver
```

## 🎮 Usage

| Key | Action |
| --- | --- |
| `↑` / `↓` or `k` / `j` | Select setting |
| `←` / `→` or `h` / `l` | Change value |
| `q` or `Esc` | Quit |

Settings are applied **live** to the mouse as soon as you change them.

> ℹ️ The UI does not yet read the current configuration from the mouse: it starts from default values, so the first change you make is applied relative to those defaults.

## 🛠️ Troubleshooting

**"Device not found"**
* Make sure the udev rules are installed and reloaded (see step 2), then replug the dongle.
* Check that the mouse/dongle is detected by the system:
  ```bash
  lsusb | grep 373b
  ```
  If nothing shows up, try a different USB port or cable.

## 🤝 Contributing

Contributions are welcome! See [ROADMAP.md](ROADMAP.md) for known bugs, missing features and planned work before opening an issue or pull request.

## 📄 License

OpenVXE is released under the **GNU General Public License v3.0**. See [LICENSE](LICENSE) for the full text.
