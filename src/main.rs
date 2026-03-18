use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use hidapi::{HidApi, HidDevice};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::{io, time::{Duration, Instant}};

const VXE_VID: u16 = 0x373b; 
const VXE_PID: u16 = 0x1085; 

struct App {
    devices: Vec<HidDevice>,
    battery: u8,
    last_battery_req: Instant,
    state: ListState,
    items: Vec<Setting>,
    logs: Vec<String>,
}

impl App {
    fn log(&mut self, msg: String) {
        self.logs.push(msg);
        if self.logs.len() > 6 {
            self.logs.remove(0);
        }
    }
}

#[derive(Clone)]
enum Setting {
    DPI(u16),
    PollingRate(u16),
    LOD(u8),
    Debounce(u8),
    MotionSync(bool),
    AngleSnap(bool),
}

impl Setting {
    fn to_string(&self) -> String {
        match self {
            Setting::DPI(v) => format!("DPI: {}", v),
            Setting::PollingRate(v) => format!("Polling Rate: {} Hz", v),
            Setting::LOD(v) => format!("Lift-off Distance: {}", if *v == 1 { "Low (1mm)" } else { "High (2mm)" }),
            Setting::Debounce(v) => format!("Debounce Time: {} ms", v),
            Setting::MotionSync(v) => format!("Motion Sync: {}", if *v { "Enabled" } else { "Disabled" }),
            Setting::AngleSnap(v) => format!("Angle Snapping: {}", if *v { "Enabled" } else { "Disabled" }),
        }
    }
}

fn calc_checksum(buf: &mut[u8; 16]) {
    let sum = buf[0..15].iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    buf[15] = 0x4D_u8.wrapping_sub(sum);
}

fn send_packet(devices: &[HidDevice], mut payload:[u8; 16]) {
    calc_checksum(&mut payload);
    let mut buf =[0u8; 17];
    buf[0] = 0x08;
    buf[1..17].copy_from_slice(&payload);
    
    for dev in devices {
        let _ = dev.write(&buf);
        let _ = dev.send_feature_report(&buf);
    }
}

fn apply_setting(devices: &[HidDevice], setting: &Setting) {
    match setting {
        Setting::DPI(dpi) => {
            let mut p =[0x07, 0x00, 0x00, 0x0c, 0x08, 0x04, 0x04, 0x00, 0x4d, 0, 0, 0, 0, 0, 0, 0];
            let val = (dpi / 50) as u8;
            p[9] = val; p[10] = val;
            p[12] = 0x55_u8.wrapping_sub(val.wrapping_add(val));
            send_packet(devices, p);
        }
        Setting::PollingRate(hz) => {
            let mut p =[0x07, 0x00, 0x00, 0x70, 0x04, 0x0b, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            let val = match hz { 1000 => 0x01, 500 => 0x02, _ => 0x03 };
            p[6] = val; p[8] = 0x55_u8.wrapping_sub(val);
            send_packet(devices, p);
        }
        Setting::LOD(lvl) => {
            let mut p =[0x07, 0x00, 0x00, 0x70, 0x04, 0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            p[6] = *lvl; p[8] = 0x55_u8.wrapping_sub(*lvl);
            send_packet(devices, p);
        }
        Setting::Debounce(ms) => {
            let mut p =[0x07, 0x00, 0x00, 0x70, 0x04, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            p[6] = *ms; p[8] = 0x55_u8.wrapping_sub(*ms);
            send_packet(devices, p);
        }
        Setting::AngleSnap(on) => {
            let mut p =[0x07, 0x00, 0x00, 0x70, 0x04, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            let val = if *on { 0x10 } else { 0x00 };
            p[6] = val; p[8] = 0x55_u8.wrapping_sub(val);
            send_packet(devices, p);
        }
        Setting::MotionSync(on) => {
            let mut p =[0x07, 0x00, 0x00, 0xa9, 0x0a, 0x08, 0x4d, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            let val = if *on { 0x01 } else { 0x00 };
            let sleep_min = 0x05; 
            p[7] = val; p[8] = 0x55_u8.wrapping_sub(val);
            p[9] = sleep_min; p[10] = 0x55_u8.wrapping_sub(sleep_min);
            send_packet(devices, p);
        }
    }
}

fn request_battery(devices: &[HidDevice]) {
    let mut buf =[0u8; 17];
    buf[0] = 0x08;
    buf[1..17].copy_from_slice(&[0x17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x36]);
    for dev in devices {
        let _ = dev.write(&buf);
        let _ = dev.send_feature_report(&buf);
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let api = HidApi::new().unwrap();
    let mut open_devices = Vec::new();

    for device_info in api.device_list() {
        if device_info.vendor_id() == VXE_VID && device_info.product_id() == VXE_PID {
            if let Ok(dev) = device_info.open_device(&api) {
                dev.set_blocking_mode(false).unwrap();
                open_devices.push(dev);
            }
        }
    }

    let mut app = App {
        devices: open_devices,
        battery: 0,
        last_battery_req: Instant::now() - Duration::from_secs(60),
        state: ListState::default(),
        items: vec![
            Setting::DPI(800),
            Setting::PollingRate(1000),
            Setting::LOD(1),
            Setting::Debounce(4),
            Setting::MotionSync(true),
            Setting::AngleSnap(false),
        ],
        logs: vec!["System: Initializing OpenVXE...".to_string()],
    };

    if app.devices.is_empty() {
        app.log("Error: Device not found. Please check udev rules.".to_string());
    } else {
        app.log(format!("System: Successfully connected to VXE R1 SE+ ({} interfaces).", app.devices.len()));
    }
    
    app.state.select(Some(0));

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res { println!("{:?}", err) }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        if !app.devices.is_empty() {
            if app.last_battery_req.elapsed() >= Duration::from_secs(10) {
                request_battery(&app.devices);
                app.last_battery_req = Instant::now();
            }

            let mut temp_logs = Vec::new();
            let mut new_battery = None;

            for dev in &app.devices {
                let mut buf =[0u8; 64];
                while let Ok(sz) = dev.read(&mut buf) {
                    if sz == 0 { break; } 
                    if buf[0] == 0x01 || buf[0] == 0x02 || sz <= 8 { continue; }
                    
                    // Парсинг АКБ: Берем значение и умножаем на 2
                    if sz >= 6 && buf[0] == 0x08 && buf[1] == 0x17 {
                        new_battery = Some(buf[5].saturating_mul(2).min(100));
                    } else if sz >= 5 && buf[0] == 0x17 { 
                        new_battery = Some(buf[4].saturating_mul(2).min(100));
                    } else {
                        // Логируем только другие пакеты (чтобы не спамить батареей)
                        let hex = buf[0..sz.min(8)].iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
                        temp_logs.push(format!("RX: [{}]", hex));
                    }
                }
            }

            if let Some(b) = new_battery {
                if app.battery != b {
                    app.battery = b;
                    temp_logs.push(format!("System: Battery updated to {}%", b));
                }
            }
            for msg in temp_logs {
                app.log(msg);
            }
        }

        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(16))? { 
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = match app.state.selected() {
                            Some(i) => if i >= app.items.len() - 1 { 0 } else { i + 1 },
                            None => 0,
                        };
                        app.state.select(Some(i));
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = match app.state.selected() {
                            Some(i) => if i == 0 { app.items.len() - 1 } else { i - 1 },
                            None => 0,
                        };
                        app.state.select(Some(i));
                    }
                    KeyCode::Left | KeyCode::Char('h') => modify_setting(app, -1),
                    KeyCode::Right | KeyCode::Char('l') => modify_setting(app, 1),
                    _ => {}
                }
            }
        }
    }
}

fn modify_setting(app: &mut App, dir: i32) {
    if let Some(idx) = app.state.selected() {
        let new_setting = {
            let setting = &mut app.items[idx];
            match setting {
                Setting::DPI(v) => *v = (*v as i32 + dir * 50).clamp(50, 26000) as u16,
                Setting::PollingRate(v) => *v = if dir > 0 { if *v == 1000 { 1000 } else { *v * 2 } } else { if *v == 250 { 250 } else { *v / 2 } },
                Setting::LOD(v) => *v = if dir > 0 { 2 } else { 1 },
                Setting::Debounce(v) => *v = (*v as i32 + dir).clamp(1, 20) as u8,
                Setting::MotionSync(v) => *v = !*v,
                Setting::AngleSnap(v) => *v = !*v,
            }
            setting.clone()
        };

        if !app.devices.is_empty() {
            apply_setting(&app.devices, &new_setting);
            app.log(format!("TX: Applied {}", new_setting.to_string()));
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let size = f.size();
    
    // Главный контейнер
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Main Content
            Constraint::Length(8),  // Logs
        ].as_ref())
        .split(size);

    // Заголовок
    let title = Paragraph::new(Line::from(vec![
        Span::styled(" OpenVXE ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" | VXE R1 SE+ | Early Beta | Press 'q' to exit"),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
    .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // Две колонки для контента
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
        .split(chunks[1]);

    // Список настроек
    let items: Vec<ListItem> = app.items.iter().map(|i| {
        ListItem::new(i.to_string()).style(Style::default().fg(Color::White))
    }).collect();

    let list = List::new(items)
        .block(Block::default().title(" Configuration (Use Arrow Keys) ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .highlight_style(Style::default().bg(Color::White).fg(Color::Black).add_modifier(Modifier::BOLD))
        .highlight_symbol(" > ");
    f.render_stateful_widget(list, main_chunks[0], &mut app.state);

    // Панель статуса устройства
    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(main_chunks[1]);

    // Виджет DPI
    let current_dpi = match app.items[0] { Setting::DPI(v) => v, _ => 800 };
    let dpi_percent = (current_dpi as f64 / 26000.0 * 100.0) as u16;
    let dpi_gauge = Gauge::default()
        .block(Block::default().title(" DPI Level ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent(dpi_percent)
        .label(format!("{} DPI", current_dpi));
    f.render_widget(dpi_gauge, status_chunks[0]);

    // Виджет Батареи
    let bat_color = match app.battery {
        0..=20 => Color::Red,
        21..=50 => Color::Yellow,
        _ => Color::Green,
    };
    let bat_gauge = Gauge::default()
        .block(Block::default().title(" Battery Level ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .gauge_style(Style::default().fg(bat_color).bg(Color::DarkGray))
        .percent(app.battery.into())
        .label(format!("{}%", app.battery));
    f.render_widget(bat_gauge, status_chunks[1]);

    // Журнал событий
    let log_lines: Vec<Line> = app.logs.iter().map(|l| Line::from(l.as_str())).collect();
    let log_para = Paragraph::new(log_lines)
        .block(Block::default().title(" Event Log ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(log_para, chunks[2]);
}
