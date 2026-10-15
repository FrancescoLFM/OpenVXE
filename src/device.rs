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

/// One line per HID interface of the mouse: interface number, usage page/usage and path.
pub fn describe_interfaces(api: &HidApi) -> Vec<String> {
    api.device_list()
        .filter(|info| info.vendor_id() == VXE_VID && info.product_id() == VXE_PID)
        .map(|info| {
            format!(
                "interface {} usage_page=0x{:04X} usage=0x{:04X} path={}",
                info.interface_number(),
                info.usage_page(),
                info.usage(),
                info.path().to_string_lossy()
            )
        })
        .collect()
}

/// Read the feature report `report_id` from every interface that answers.
pub fn get_feature_reports(devices: &[HidDevice], report_id: u8) -> Vec<Vec<u8>> {
    let mut reports = Vec::new();
    for dev in devices {
        let mut buf = [0u8; 64];
        buf[0] = report_id;
        if let Ok(sz) = dev.get_feature_report(&mut buf) {
            if sz > 0 {
                reports.push(buf[..sz].to_vec());
            }
        }
    }
    reports
}
