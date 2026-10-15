//! Send raw packets to the mouse and print its responses, to test protocol hypotheses.

use hidapi::HidApi;
use std::time::{Duration, Instant};
use vxe_driver::{device, protocol};

const USAGE: &str = "Usage:
  vxe-probe --list                       List the HID interfaces of the mouse
  vxe-probe --info                       Query battery, CID/MID, firmware version (read-only)
  vxe-probe --dump [--from HEX --to HEX] Read the configuration memory (read-only,
                                         default 0x0000-0x00C0)
  vxe-probe [OPTIONS] HEX...             Send a packet and print the responses
  vxe-probe --listen MS                  Only print incoming reports for MS milliseconds

HEX is the payload without report ID and checksum (up to 15 bytes, zero padded),
e.g. `vxe-probe 04` sends the battery level query. The 0x08 report ID and the
checksum are added automatically.

Options:
  --raw        Send HEX as-is (17 bytes, report ID and checksum included)
  --listen MS  Read responses for MS milliseconds after sending (default 500)
  --feature    Also read feature report 0x08 after sending
  --yes        Required for any command that is not a known read: writes (0x07),
               factory reset (0x09), firmware upgrade mode (0x0D), pairing...";

/// Commands known to only read state; everything else needs --yes.
const READ_ONLY_COMMANDS: [u8; 12] = [0x02, 0x03, 0x04, 0x06, 0x08, 0x0e, 0x10, 0x12, 0x15, 0x17, 0x19, 0x1d];

/// How long to wait for the answer to one request.
const REPLY_TIMEOUT: Duration = Duration::from_millis(300);

struct Options {
    list: bool,
    info: bool,
    dump: Option<(u16, u16)>,
    raw: bool,
    feature: bool,
    yes: bool,
    listen_ms: u64,
    hex: Vec<String>,
}

fn parse_args() -> Result<Options, String> {
    let mut opts = Options {
        list: false,
        info: false,
        dump: None,
        raw: false,
        feature: false,
        yes: false,
        listen_ms: 500,
        hex: Vec::new(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--list" => opts.list = true,
            "--info" => opts.info = true,
            "--dump" => opts.dump = Some(opts.dump.unwrap_or((0x0000, 0x00c0))),
            "--from" | "--to" => {
                let value = args.next().ok_or(format!("{} needs a value", arg))?;
                let addr = u16::from_str_radix(value.trim_start_matches("0x"), 16)
                    .map_err(|_| format!("invalid address: {}", value))?;
                let (from, to) = opts.dump.unwrap_or((0x0000, 0x00c0));
                opts.dump = Some(if arg == "--from" { (addr, to) } else { (from, addr) });
            }
            "--raw" => opts.raw = true,
            "--feature" => opts.feature = true,
            "--yes" => opts.yes = true,
            "--listen" => {
                let ms = args.next().ok_or("--listen needs a value")?;
                opts.listen_ms = ms.parse().map_err(|_| format!("invalid --listen value: {}", ms))?;
            }
            _ => opts.hex.push(arg),
        }
    }
    Ok(opts)
}

fn build(opts: &Options) -> Result<Option<protocol::Packet>, String> {
    if opts.hex.is_empty() {
        return Ok(None);
    }
    let bytes = protocol::parse_hex(&opts.hex.join(" ")).ok_or("invalid hex payload")?;
    let packet = if opts.raw {
        bytes
            .as_slice()
            .try_into()
            .map_err(|_| format!("--raw needs exactly 17 bytes, got {}", bytes.len()))?
    } else {
        protocol::build_packet(&bytes).ok_or(format!("payload too long: {} bytes (max 15)", bytes.len()))?
    };
    if packet[0] != protocol::REPORT_ID {
        return Err(format!("report ID must be 0x{:02X}", protocol::REPORT_ID));
    }
    if !READ_ONLY_COMMANDS.contains(&packet[1]) && !opts.yes {
        return Err(format!(
            "command 0x{:02X} is not a known read and may change the mouse state; add --yes to send it",
            packet[1]
        ));
    }
    Ok(Some(packet))
}

fn print_report(tag: &str, report: &[u8]) {
    println!("{} {}", tag, protocol::to_hex(report));
    println!("    -> {}", protocol::describe(report));
}

/// Payload of a vendor report, without the report ID.
fn payload(report: &[u8]) -> &[u8] {
    if report.len() >= 17 && report[0] == protocol::REPORT_ID {
        &report[1..]
    } else {
        report
    }
}

/// Send one request and print the matching reply (same command and address).
/// Some firmware answers GetEEPROM with the SetEEPROM command code.
fn request(devices: &[hidapi::HidDevice], packet: &protocol::Packet) -> Option<Vec<u8>> {
    device::send(devices, packet);
    let start = Instant::now();
    while start.elapsed() < REPLY_TIMEOUT {
        for report in device::read_all(devices) {
            let p = payload(&report);
            let same_cmd = p.first() == Some(&packet[1])
                || (packet[1] == protocol::CMD_GET_EEPROM && p.first() == Some(&protocol::CMD_WRITE));
            if same_cmd && p.get(2..4) == Some(&packet[3..5]) {
                return Some(report);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

fn print_reply(tag: &str, reply: Option<Vec<u8>>) {
    match reply {
        Some(report) => print_report(tag, &report),
        None => println!("{} no reply", tag),
    }
}

fn info(devices: &[hidapi::HidDevice]) {
    for cmd in [
        protocol::CMD_BATTERY_LEVEL,
        protocol::CMD_GET_CID_MID,
        protocol::CMD_GET_VERSION,
        0x17, // GetFarDistanceMode, used as battery query by the TUI
    ] {
        let packet = protocol::build_packet(&[cmd]).expect("one byte payload");
        print_reply(&format!("[0x{:02X}]", cmd), request(devices, &packet));
    }
}

fn dump(devices: &[hidapi::HidDevice], from: u16, to: u16) {
    let mut addr = from;
    while addr < to {
        let len = (to - addr).min(protocol::EEPROM_CHUNK as u16) as u8;
        let reply = request(devices, &protocol::eeprom_read(addr, len));
        print_reply(&format!("[0x{:04X}]", addr), reply);
        addr += len as u16;
    }
}

fn run(opts: Options) -> Result<(), String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    if opts.list {
        for line in device::describe_interfaces(&api) {
            println!("{}", line);
        }
        return Ok(());
    }

    let packet = build(&opts)?;
    let devices = device::open_devices(&api);
    if devices.is_empty() {
        return Err(if device::describe_interfaces(&api).is_empty() {
            "device not found: is the dongle plugged in?".into()
        } else {
            "device found but it cannot be opened: install the udev rules (see README) and replug the dongle".into()
        });
    }

    if opts.info {
        info(&devices);
        return Ok(());
    }
    if let Some((from, to)) = opts.dump {
        dump(&devices, from, to);
        return Ok(());
    }

    if let Some(packet) = packet {
        print_report("TX", &packet);
        device::send(&devices, &packet);
    }

    let start = Instant::now();
    let listen = Duration::from_millis(opts.listen_ms);
    while start.elapsed() < listen {
        for report in device::read_all(&devices) {
            // Skip standard mouse input reports.
            if report.first().is_some_and(|&id| id == 0x01 || id == 0x02) {
                continue;
            }
            print_report(&format!("RX +{}ms", start.elapsed().as_millis()), &report);
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    if opts.feature {
        for report in device::get_feature_reports(&devices, protocol::REPORT_ID) {
            print_report("FEATURE", &report);
        }
    }
    Ok(())
}

fn main() {
    let result = parse_args().and_then(|opts| {
        if !opts.list
            && !opts.info
            && opts.dump.is_none()
            && opts.hex.is_empty()
            && !std::env::args().any(|a| a == "--listen")
        {
            return Err(String::new());
        }
        run(opts)
    });
    if let Err(msg) = result {
        if msg.is_empty() {
            println!("{}", USAGE);
        } else {
            eprintln!("Error: {}", msg);
            std::process::exit(1);
        }
    }
}
