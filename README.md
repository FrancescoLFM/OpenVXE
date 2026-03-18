# 🦀 OpenVXE

![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)
![Linux](https://img.shields.io/badge/OS-Linux-blue.svg)
![Status](https://img.shields.io/badge/Status-Early_Beta-red.svg)

An open-source, lightweight, and blazing-fast TUI (Terminal User Interface) driver for the **VXE R1 SE+** wireless gaming mouse, written entirely in Rust.

## ✨ Features

* **Native Linux Support:** Direct hardware communication via `hidapi` (no root required with udev rules).
* **Terminal UI:** Beautiful, minimalist interface built with `ratatui`.
* **Live Configuration:** Apply changes instantly without saving.
  * Adjust DPI (50 - 26,000)
  * Polling Rate (250Hz / 500Hz / 1000Hz)
  * Lift-off Distance (LOD)
  * Debounce Time (1ms - 20ms)
  * Motion Sync (On/Off)
  * Angle Snapping (On/Off)
* **Real-time Battery Monitor:** Accurate battery parsing straight from the mouse's internal sensors.
* **Hex Sniffer:** Built-in packet monitor for debugging hardware responses.

## 🚀 Installation & Setup

### 1. Install Dependencies (Arch Linux)
Ensure you have Rust and the required HID libraries installed:
```bash
sudo pacman -S rustup base-devel hidapi
rustup default stable
# OpenVXE
