//! Send raw packets to the mouse and print its responses, to test protocol hypotheses.

use hidapi::HidApi;
use std::time::{Duration, Instant};
use vxe_driver::{device, protocol};

const USAGE: &str = "Usage:
  vxe-probe --list                       List the HID interfaces of the mouse
  vxe-probe [OPTIONS] HEX...             Send a packet and print the responses
  vxe-probe --listen MS                  Only print incoming reports for MS milliseconds

HEX is the payload without report ID and checksum (up to 15 bytes, zero padded),
e.g. `vxe-probe 17` sends the battery query. The 0x08 report ID and the checksum
are added automatically.

Options:
  --raw        Send HEX as-is (17 bytes, report ID and checksum included)
  --listen MS  Read responses for MS milliseconds after sending (default 500)
  --feature    Also read feature report 0x08 after sending
  --yes        Required to send write commands (0x07): they change the mouse configuration";

struct Options {
    list: bool,
    raw: bool,
    feature: bool,
    yes: bool,
    listen_ms: u64,
    hex: Vec<String>,
}

fn parse_args() -> Result<Options, String> {
    let mut opts = Options {
        list: false,
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
    if packet[1] == protocol::CMD_WRITE && !opts.yes {
        return Err("this is a write command and changes the mouse configuration; add --yes to send it".into());
    }
    Ok(Some(packet))
}

fn print_report(tag: &str, report: &[u8]) {
    println!("{} {}", tag, protocol::to_hex(report));
    println!("    -> {}", protocol::describe(report));
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
        return Err("device not found, check the udev rules".into());
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
        if !opts.list && opts.hex.is_empty() && !std::env::args().any(|a| a == "--listen") {
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
