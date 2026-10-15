//! Application state.

use hidapi::HidDevice;
use ratatui::widgets::ListState;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::{Duration, Instant};
use vxe_driver::device;
use vxe_driver::protocol::{self, Report, Setting};

const MAX_LOG_LINES: usize = 6;
const BATTERY_INTERVAL: Duration = Duration::from_secs(10);
/// Environment variable naming a file where every TX/RX packet is appended.
const SNIFF_ENV: &str = "OPENVXE_SNIFF";

pub struct App {
    pub devices: Vec<HidDevice>,
    /// Battery percentage and charging state, `None` until the first reply.
    pub battery: Option<(u8, bool)>,
    last_battery_req: Option<Instant>,
    pub state: ListState,
    pub items: Vec<Setting>,
    pub logs: Vec<String>,
    sniff: Option<File>,
    started: Instant,
}

impl App {
    pub fn new(devices: Vec<HidDevice>) -> Self {
        let mut app = App {
            devices,
            battery: None,
            last_battery_req: None,
            state: ListState::default(),
            items: vec![
                Setting::Dpi(800),
                Setting::PollingRate(1000),
                Setting::Lod(1),
                Setting::Debounce(4),
                Setting::MotionSync(true),
                Setting::AngleSnap(false),
            ],
            logs: vec!["System: Initializing OpenVXE...".to_string()],
            sniff: None,
            started: Instant::now(),
        };

        if let Ok(path) = std::env::var(SNIFF_ENV) {
            match OpenOptions::new().create(true).append(true).open(&path) {
                Ok(file) => {
                    app.sniff = Some(file);
                    app.log(format!("System: Sniffing packets to {}", path));
                }
                Err(err) => app.log(format!("Error: cannot open sniff file {}: {}", path, err)),
            }
        }

        if app.devices.is_empty() {
            app.log("Error: Device not found. Please check udev rules.".to_string());
        } else {
            app.log(format!(
                "System: Successfully connected to VXE R1 SE+ ({} interfaces).",
                app.devices.len()
            ));
        }
        app.state.select(Some(0));
        app
    }

    pub fn log(&mut self, msg: String) {
        self.logs.push(msg);
        if self.logs.len() > MAX_LOG_LINES {
            self.logs.remove(0);
        }
    }

    /// Append a packet to the sniff file, in a format `vxe-decode` understands.
    fn sniff(&mut self, direction: &str, bytes: &[u8]) {
        if let Some(file) = &mut self.sniff {
            let elapsed = self.started.elapsed().as_secs_f64();
            let _ = writeln!(file, "{:.3} {} {}", elapsed, direction, protocol::to_hex(bytes));
        }
    }

    fn send(&mut self, packet: &protocol::Packet) {
        device::send(&self.devices, packet);
        self.sniff("TX", packet);
    }

    pub fn dpi(&self) -> u16 {
        self.items
            .iter()
            .find_map(|s| match s {
                Setting::Dpi(v) => Some(*v),
                _ => None,
            })
            .unwrap_or(800)
    }

    /// Request the battery level periodically and process incoming reports.
    pub fn poll_device(&mut self) {
        if self.devices.is_empty() {
            return;
        }

        if self.last_battery_req.is_none_or(|t| t.elapsed() >= BATTERY_INTERVAL) {
            self.send(&protocol::battery_request());
            self.last_battery_req = Some(Instant::now());
        }

        let mut new_battery = None;
        let mut messages = Vec::new();
        for report in device::read_all(&self.devices) {
            if !matches!(report.first(), Some(0x01 | 0x02)) {
                self.sniff("RX", &report);
            }
            match protocol::parse_report(&report) {
                Report::Ignored => {}
                Report::Battery { percent, charging } => new_battery = Some((percent, charging)),
                // Log only unknown packets, so battery responses don't spam the log.
                Report::Unknown => {
                    let hex = report[..report.len().min(8)]
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    messages.push(format!("RX: [{}]", hex));
                }
            }
        }

        if let Some((percent, charging)) = new_battery {
            if self.battery != Some((percent, charging)) {
                self.battery = Some((percent, charging));
                let state = if charging { " (charging)" } else { "" };
                messages.push(format!("System: Battery updated to {}%{}", percent, state));
            }
        }
        for msg in messages {
            self.log(msg);
        }
    }

    pub fn select_next(&mut self) {
        let i = match self.state.selected() {
            Some(i) if i + 1 < self.items.len() => i + 1,
            _ => 0,
        };
        self.state.select(Some(i));
    }

    pub fn select_prev(&mut self) {
        let i = match self.state.selected() {
            Some(0) | None => self.items.len() - 1,
            Some(i) => i - 1,
        };
        self.state.select(Some(i));
    }

    /// Change the selected setting and send it to the mouse.
    pub fn modify_setting(&mut self, dir: i32) {
        let Some(idx) = self.state.selected() else { return };
        let setting = &mut self.items[idx];
        setting.adjust(dir);
        let setting = setting.clone();

        if !self.devices.is_empty() {
            self.send(&protocol::encode_setting(&setting));
            self.log(format!("TX: Applied {}", setting));
        }
    }
}
