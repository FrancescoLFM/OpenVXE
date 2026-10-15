//! Rendering.

use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};
use vxe_driver::protocol::MAX_DPI;

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}

pub fn draw(f: &mut Frame, app: &mut App) {
    // Main container
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(8), // Logs
        ])
        .split(f.area());

    // Header
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " OpenVXE ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | VXE R1 SE+ | Early Beta | Press 'q' to exit"),
    ]))
    .block(panel(""))
    .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // Two columns: settings on the left, device status on the right
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    // Settings list
    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|i| ListItem::new(i.to_string()).style(Style::default().fg(Color::White)))
        .collect();
    let list = List::new(items)
        .block(panel(" Configuration (Use Arrow Keys) "))
        .highlight_style(
            Style::default()
                .bg(Color::White)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" > ");
    f.render_stateful_widget(list, main_chunks[0], &mut app.state);

    // Device status panel
    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[1]);

    // DPI gauge
    let dpi = app.dpi();
    let dpi_gauge = Gauge::default()
        .block(panel(" DPI Level "))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent((dpi as f64 / MAX_DPI as f64 * 100.0) as u16)
        .label(format!("{} DPI", dpi));
    f.render_widget(dpi_gauge, status_chunks[0]);

    // Battery gauge
    let bat_color = match app.battery {
        0..=20 => Color::Red,
        21..=50 => Color::Yellow,
        _ => Color::Green,
    };
    let bat_gauge = Gauge::default()
        .block(panel(" Battery Level "))
        .gauge_style(Style::default().fg(bat_color).bg(Color::DarkGray))
        .percent(app.battery.into())
        .label(format!("{}%", app.battery));
    f.render_widget(bat_gauge, status_chunks[1]);

    // Event log
    let log_lines: Vec<Line> = app.logs.iter().map(|l| Line::from(l.as_str())).collect();
    f.render_widget(Paragraph::new(log_lines).block(panel(" Event Log ")), chunks[2]);
}
