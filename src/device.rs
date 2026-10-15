//! HID discovery and I/O.

use crate::protocol::{Packet, VXE_PID, VXE_VID};
use hidapi::{HidApi, HidDevice};

/// Open every HID interface of the mouse in non-blocking mode.
pub fn open_devices(api: &HidApi) -> Vec<HidDevice> {
    let mut devices = Vec::new();
    for info in api.device_list() {
        if info.vendor_id() == VXE_VID && info.product_id() == VXE_PID {
            if let Ok(dev) = info.open_device(api) {
                // A blocking device would freeze the UI loop, so skip it.
                if dev.set_blocking_mode(false).is_ok() {
                    devices.push(dev);
                }
            }
        }
    }
    devices
}

/// Send a packet to every interface, both as output and as feature report.
pub fn send(devices: &[HidDevice], packet: &Packet) {
    for dev in devices {
        let _ = dev.write(packet);
        let _ = dev.send_feature_report(packet);
    }
}

/// Drain all pending input reports from every interface.
pub fn read_all(devices: &[HidDevice]) -> Vec<Vec<u8>> {
    let mut reports = Vec::new();
    for dev in devices {
        let mut buf = [0u8; 64];
        while let Ok(sz) = dev.read(&mut buf) {
            if sz == 0 {
                break;
            }
            reports.push(buf[..sz].to_vec());
        }
    }
    reports
}
