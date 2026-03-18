# OpenVXE
Open-source configuration tool and TUI driver for VXE R1 SE+ written in Rust. (Early Beta)

## Features
* Native Linux support (via `hidapi`)
* Beautiful Terminal UI
* Live Configuration (DPI, Polling Rate, LOD, Debounce, Motion Sync, Angle Snapping)
* Accurate Battery Parsing

## Installation
1. Setup Udev rules for your mouse:
   `SUBSYSTEM=="hidraw", ATTRS{idVendor}=="373b", ATTRS{idProduct}=="1085", MODE="0666"`
2. Reload udev: `sudo udevadm control --reload-rules && sudo udevadm trigger`
3. Build and run: `cargo run --release`
