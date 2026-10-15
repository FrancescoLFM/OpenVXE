//! VXE R1 SE+ HID protocol: packet encoding and report parsing.
//!
//! Pure functions with no HID dependency, so they can be unit tested.

use std::fmt;

pub const VXE_VID: u16 = 0x373b;
pub const VXE_PID: u16 = 0x1085;

/// DPI is encoded as a single byte in steps of 50 (255 * 50). Higher values need
/// the (still unknown) high byte of the protocol.
pub const MAX_DPI: u16 = 12750;
pub const MIN_DPI: u16 = 50;
pub const DPI_STEP: u16 = 50;

pub const REPORT_ID: u8 = 0x08;
pub const CMD_WRITE: u8 = 0x07;
const CMD_BATTERY: u8 = 0x17;

/// Every value byte is followed by its complement `0x55 - value`.
const VALUE_KEY: u8 = 0x55;
/// The last payload byte makes the payload sum equal to `0x4D`.
const CHECKSUM_KEY: u8 = 0x4D;

/// A complete output report: report ID followed by the 16-byte payload.
pub type Packet = [u8; 17];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setting {
    Dpi(u16),
    PollingRate(u16),
    Lod(u8),
    Debounce(u8),
    MotionSync(bool),
    AngleSnap(bool),
}

impl fmt::Display for Setting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Setting::Dpi(v) => write!(f, "DPI: {}", v),
            Setting::PollingRate(v) => write!(f, "Polling Rate: {} Hz", v),
            Setting::Lod(v) => write!(
                f,
                "Lift-off Distance: {}",
                if *v == 1 { "Low (1mm)" } else { "High (2mm)" }
            ),
            Setting::Debounce(v) => write!(f, "Debounce Time: {} ms", v),
            Setting::MotionSync(v) => write!(f, "Motion Sync: {}", if *v { "Enabled" } else { "Disabled" }),
            Setting::AngleSnap(v) => write!(f, "Angle Snapping: {}", if *v { "Enabled" } else { "Disabled" }),
        }
    }
}

impl Setting {
    /// Step the setting by one unit in direction `dir` (negative = decrease).
    pub fn adjust(&mut self, dir: i32) {
        match self {
            Setting::Dpi(v) => *v = (*v as i32 + dir * DPI_STEP as i32).clamp(MIN_DPI as i32, MAX_DPI as i32) as u16,
            Setting::PollingRate(v) => *v = if dir > 0 { (*v * 2).min(1000) } else { (*v / 2).max(250) },
            Setting::Lod(v) => *v = if dir > 0 { 2 } else { 1 },
            Setting::Debounce(v) => *v = (*v as i32 + dir).clamp(1, 20) as u8,
            Setting::MotionSync(v) => *v = !*v,
            Setting::AngleSnap(v) => *v = !*v,
        }
    }
}

fn complement(value: u8) -> u8 {
    VALUE_KEY.wrapping_sub(value)
}

fn checksum(payload: &[u8; 16]) -> u8 {
    let sum = payload[0..15].iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    CHECKSUM_KEY.wrapping_sub(sum)
}

fn finish(mut payload: [u8; 16]) -> Packet {
    payload[15] = checksum(&payload);
    let mut buf = [0u8; 17];
    buf[0] = REPORT_ID;
    buf[1..17].copy_from_slice(&payload);
    buf
}

/// Write one value to a sub-register of the sensor settings block (address `0x70`).
fn sensor_register(register: u8, value: u8) -> Packet {
    let mut p = [
        CMD_WRITE, 0x00, 0x00, 0x70, 0x04, register, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    p[6] = value;
    p[8] = complement(value);
    finish(p)
}

pub fn encode_setting(setting: &Setting) -> Packet {
    match setting {
        Setting::Dpi(dpi) => {
            let mut p = [
                CMD_WRITE, 0x00, 0x00, 0x0c, 0x08, 0x04, 0x04, 0x00, 0x4d, 0, 0, 0, 0, 0, 0, 0,
            ];
            let val = (dpi.min(&MAX_DPI) / DPI_STEP) as u8;
            // X and Y DPI; their complement is stored as a single combined byte.
            p[9] = val;
            p[10] = val;
            p[12] = VALUE_KEY.wrapping_sub(val.wrapping_add(val));
            finish(p)
        }
        Setting::PollingRate(hz) => {
            let val = match hz {
                1000 => 0x01,
                500 => 0x02,
                _ => 0x03,
            };
            sensor_register(0x0b, val)
        }
        Setting::Lod(lvl) => sensor_register(0x03, *lvl),
        Setting::Debounce(ms) => sensor_register(0x02, *ms),
        Setting::AngleSnap(on) => sensor_register(0x01, if *on { 0x10 } else { 0x00 }),
        Setting::MotionSync(on) => {
            let mut p = [CMD_WRITE, 0x00, 0x00, 0xa9, 0x0a, 0x08, 0x4d, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            let val = u8::from(*on);
            // The same packet also carries the sleep timer, not configurable yet.
            let sleep_min = 0x05;
            p[7] = val;
            p[8] = complement(val);
            p[9] = sleep_min;
            p[10] = complement(sleep_min);
            finish(p)
        }
    }
}

pub fn battery_request() -> Packet {
    finish([CMD_BATTERY, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
}

#[derive(Debug, PartialEq, Eq)]
pub enum Report {
    /// Standard mouse/keyboard input or too short to be a vendor response.
    Ignored,
    /// Battery level in percent.
    Battery(u8),
    /// Any other vendor response, kept for the hex sniffer.
    Unknown,
}

pub fn parse_report(buf: &[u8]) -> Report {
    if buf.is_empty() || buf[0] == 0x01 || buf[0] == 0x02 || buf.len() <= 8 {
        return Report::Ignored;
    }
    // The raw battery value is half the percentage.
    if buf[0] == REPORT_ID && buf[1] == CMD_BATTERY {
        Report::Battery(buf[5].saturating_mul(2).min(100))
    } else if buf[0] == CMD_BATTERY {
        Report::Battery(buf[4].saturating_mul(2).min(100))
    } else {
        Report::Unknown
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")
}

/// Parse hex bytes such as `07 00 0c`, `07:00:0c`, `0x07,0x00` or `07000c`.
pub fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let digits: String = s
        .split([' ', '\t', ':', ',', '-'])
        .map(|t| t.trim_start_matches("0x").trim_start_matches("0X"))
        .collect();
    if digits.is_empty() || !digits.len().is_multiple_of(2) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).ok())
        .collect()
}

/// Build a packet from up to 15 payload bytes, padding with zeros and appending the checksum.
pub fn build_packet(payload: &[u8]) -> Option<Packet> {
    if payload.len() > 15 {
        return None;
    }
    let mut p = [0u8; 16];
    p[..payload.len()].copy_from_slice(payload);
    Some(finish(p))
}

/// Extract the 16-byte payload from a report, with or without the leading report ID.
fn payload_of(bytes: &[u8]) -> Option<[u8; 16]> {
    let start = if bytes.len() >= 17 && bytes[0] == REPORT_ID {
        1
    } else {
        0
    };
    bytes.get(start..start + 16)?.try_into().ok()
}

/// Value bytes in `data` that are followed by their `0x55 - v` complement.
fn complemented_values(data: &[u8]) -> Vec<u8> {
    let mut values = Vec::new();
    let mut i = 0;
    while i + 1 < data.len() {
        if data[i + 1] == complement(data[i]) {
            values.push(data[i]);
            i += 2;
        } else {
            i += 1;
        }
    }
    values
}

fn on_off(v: u8) -> &'static str {
    if v != 0 {
        "on"
    } else {
        "off"
    }
}

/// Human-readable description of a vendor packet, for the sniffer and the capture decoder.
///
/// Known commands are decoded; unknown ones are split into command/address/length/data,
/// and value/complement pairs are highlighted to help reverse engineering.
pub fn describe(bytes: &[u8]) -> String {
    let Some(p) = payload_of(bytes) else {
        return format!("short report ({} bytes)", bytes.len());
    };
    let bad_checksum = if checksum(&p) == p[15] { "" } else { " [bad checksum]" };
    let addr = u16::from_be_bytes([p[2], p[3]]);
    let len = p[4] as usize;
    let data = &p[5..(5 + len).min(15)];

    let values = complemented_values(data);
    let values = if values.is_empty() {
        String::new()
    } else {
        format!(" values=[{}]", to_hex(&values))
    };

    let body = match p[0] {
        CMD_BATTERY if p[1..15].iter().all(|&b| b == 0) => "BATTERY query".to_string(),
        CMD_BATTERY => format!(
            "BATTERY response raw={} ({}%) [{}]",
            p[4],
            p[4].saturating_mul(2).min(100),
            to_hex(&p[1..15])
        ),
        CMD_WRITE => {
            let known = match (addr, data) {
                (0x0c, _) => format!(
                    "DPI X={} Y={} (p[11]=0x{:02X}, suspected high byte)",
                    p[9] as u32 * DPI_STEP as u32,
                    p[10] as u32 * DPI_STEP as u32,
                    p[11]
                ),
                (0x70, [0x01, v, ..]) => format!("angle snapping {}", on_off(*v)),
                (0x70, [0x02, v, ..]) => format!("debounce {} ms", v),
                (0x70, [0x03, v, ..]) => format!("LOD {}", v),
                (0x70, [0x0b, v, ..]) => format!(
                    "polling rate {}",
                    match v {
                        0x01 => "1000 Hz",
                        0x02 => "500 Hz",
                        0x03 => "250 Hz",
                        _ => "unknown",
                    }
                ),
                (0x70, [reg, v, ..]) => format!("unknown sensor register 0x{:02X} = 0x{:02X}", reg, v),
                (0xa9, _) => format!("motion sync {}, sleep {} min", on_off(p[7]), p[9]),
                _ => "unknown address".to_string(),
            };
            format!(
                "WRITE addr=0x{:04X} len={} data=[{}] -> {}{}",
                addr,
                len,
                to_hex(data),
                known,
                values
            )
        }
        cmd => format!(
            "CMD 0x{:02X} p1=0x{:02X} addr=0x{:04X} len={} data=[{}]{}",
            cmd,
            p[1],
            addr,
            len,
            to_hex(data),
            values
        ),
    };
    format!("{}{}", body, bad_checksum)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload_sum(p: &Packet) -> u8 {
        p[1..].iter().fold(0u8, |acc, &x| acc.wrapping_add(x))
    }

    #[test]
    fn checksum_makes_payload_sum_0x4d() {
        for s in [
            Setting::Dpi(1600),
            Setting::PollingRate(500),
            Setting::Lod(1),
            Setting::Debounce(7),
            Setting::MotionSync(false),
            Setting::AngleSnap(true),
        ] {
            assert_eq!(payload_sum(&encode_setting(&s)), CHECKSUM_KEY, "{s}");
        }
        assert_eq!(payload_sum(&battery_request()), CHECKSUM_KEY);
    }

    #[test]
    fn battery_request_bytes() {
        let mut expected = [0u8; 17];
        expected[0] = 0x08;
        expected[1] = 0x17;
        expected[16] = 0x36;
        assert_eq!(battery_request(), expected);
    }

    #[test]
    fn dpi_packets() {
        assert_eq!(
            encode_setting(&Setting::Dpi(800)),
            [0x08, 0x07, 0x00, 0x00, 0x0c, 0x08, 0x04, 0x04, 0x00, 0x4d, 0x10, 0x10, 0x00, 0x35, 0x00, 0x00, 0x88]
        );
        assert_eq!(
            encode_setting(&Setting::Dpi(12750)),
            [0x08, 0x07, 0x00, 0x00, 0x0c, 0x08, 0x04, 0x04, 0x00, 0x4d, 0xff, 0xff, 0x00, 0x57, 0x00, 0x00, 0x88]
        );
    }

    #[test]
    fn dpi_above_max_is_clamped_not_wrapped() {
        assert_eq!(
            encode_setting(&Setting::Dpi(26000)),
            encode_setting(&Setting::Dpi(MAX_DPI))
        );
    }

    #[test]
    fn sensor_register_packets() {
        assert_eq!(
            encode_setting(&Setting::PollingRate(1000)),
            [0x08, 0x07, 0x00, 0x00, 0x70, 0x04, 0x0b, 0x01, 0x00, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72]
        );
        assert_eq!(
            encode_setting(&Setting::PollingRate(250)),
            [0x08, 0x07, 0x00, 0x00, 0x70, 0x04, 0x0b, 0x03, 0x00, 0x52, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72]
        );
        assert_eq!(
            encode_setting(&Setting::Lod(2)),
            [0x08, 0x07, 0x00, 0x00, 0x70, 0x04, 0x03, 0x02, 0x00, 0x53, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7a]
        );
        assert_eq!(
            encode_setting(&Setting::Debounce(4)),
            [0x08, 0x07, 0x00, 0x00, 0x70, 0x04, 0x02, 0x04, 0x00, 0x51, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7b]
        );
        assert_eq!(
            encode_setting(&Setting::AngleSnap(true)),
            [0x08, 0x07, 0x00, 0x00, 0x70, 0x04, 0x01, 0x10, 0x00, 0x45, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7c]
        );
    }

    #[test]
    fn motion_sync_packet() {
        assert_eq!(
            encode_setting(&Setting::MotionSync(true)),
            [0x08, 0x07, 0x00, 0x00, 0xa9, 0x0a, 0x08, 0x4d, 0x01, 0x54, 0x05, 0x50, 0x00, 0x00, 0x00, 0x00, 0x94]
        );
    }

    #[test]
    fn adjust_clamps_ranges() {
        let mut dpi = Setting::Dpi(MAX_DPI);
        dpi.adjust(1);
        assert_eq!(dpi, Setting::Dpi(MAX_DPI));
        let mut dpi = Setting::Dpi(MIN_DPI);
        dpi.adjust(-1);
        assert_eq!(dpi, Setting::Dpi(MIN_DPI));

        let mut rate = Setting::PollingRate(250);
        rate.adjust(1);
        assert_eq!(rate, Setting::PollingRate(500));
        rate.adjust(1);
        rate.adjust(1);
        assert_eq!(rate, Setting::PollingRate(1000));
        rate.adjust(-1);
        rate.adjust(-1);
        rate.adjust(-1);
        assert_eq!(rate, Setting::PollingRate(250));

        let mut deb = Setting::Debounce(20);
        deb.adjust(1);
        assert_eq!(deb, Setting::Debounce(20));

        let mut sync = Setting::MotionSync(true);
        sync.adjust(-1);
        assert_eq!(sync, Setting::MotionSync(false));
    }

    #[test]
    fn parse_hex_formats() {
        let expected = Some(vec![0x07, 0x00, 0x0c]);
        assert_eq!(parse_hex("07 00 0c"), expected);
        assert_eq!(parse_hex("07:00:0C"), expected);
        assert_eq!(parse_hex("0x07,0x00,0x0c"), expected);
        assert_eq!(parse_hex("07000c"), expected);
        assert_eq!(parse_hex("070"), None);
        assert_eq!(parse_hex("zz"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn build_packet_pads_and_checksums() {
        assert_eq!(build_packet(&[0x17]), Some(battery_request()));
        assert_eq!(build_packet(&[0; 16]), None);
    }

    #[test]
    fn describe_known_packets() {
        let dpi = describe(&encode_setting(&Setting::Dpi(1600)));
        assert!(dpi.contains("DPI X=1600 Y=1600"), "{dpi}");
        assert!(!dpi.contains("bad checksum"));

        assert!(describe(&encode_setting(&Setting::PollingRate(500))).contains("polling rate 500 Hz"));
        assert!(describe(&encode_setting(&Setting::Debounce(7))).contains("debounce 7 ms"));
        assert!(describe(&encode_setting(&Setting::MotionSync(true))).contains("motion sync on, sleep 5 min"));
        assert_eq!(describe(&battery_request()), "BATTERY query");
        // Without the report ID, as some capture tools export it.
        assert!(describe(&encode_setting(&Setting::Lod(2))[1..]).contains("LOD 2"));
    }

    #[test]
    fn describe_unknown_packets() {
        let unknown = describe(&build_packet(&[0x08, 0x00, 0x00, 0x0c, 0x02, 0x10, 0x45]).unwrap());
        assert_eq!(unknown, "CMD 0x08 p1=0x00 addr=0x000C len=2 data=[10 45] values=[10]");

        let mut corrupted = encode_setting(&Setting::Dpi(800));
        corrupted[16] ^= 0xff;
        assert!(describe(&corrupted).ends_with("[bad checksum]"));
        assert_eq!(describe(&[0x08, 0x07]), "short report (2 bytes)");
    }

    #[test]
    fn parse_battery_reports() {
        let mut with_id = [0u8; 16];
        with_id[0] = 0x08;
        with_id[1] = 0x17;
        with_id[5] = 43;
        assert_eq!(parse_report(&with_id), Report::Battery(86));

        let mut without_id = [0u8; 16];
        without_id[0] = 0x17;
        without_id[4] = 200;
        assert_eq!(parse_report(&without_id), Report::Battery(100));
    }

    #[test]
    fn parse_ignores_input_and_short_reports() {
        assert_eq!(parse_report(&[]), Report::Ignored);
        assert_eq!(parse_report(&[0x01; 16]), Report::Ignored);
        assert_eq!(parse_report(&[0x02; 16]), Report::Ignored);
        assert_eq!(parse_report(&[0x17, 0, 0, 0, 50]), Report::Ignored);
        assert_eq!(parse_report(&[0x09; 16]), Report::Unknown);
    }
}
