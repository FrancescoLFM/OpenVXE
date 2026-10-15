mod app;
mod device;
mod protocol;
mod ui;

use app::App;
use crossterm::{
    cursor,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use hidapi::HidApi;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{io, time::Duration};

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, cursor::Show);
}

fn main() -> Result<(), io::Error> {
    // Initialize HID before touching the terminal, so errors are printed normally.
    let api = HidApi::new().map_err(io::Error::other)?;
    let mut app = App::new(device::open_devices(&api));

    // Restore the terminal on panic, otherwise it stays in raw mode / alternate screen.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(err) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
        restore_terminal();
        return Err(err);
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(err) => {
            restore_terminal();
            return Err(err);
        }
    };

    let res = run_app(&mut terminal, &mut app);

    restore_terminal();

    if let Err(err) = res {
        println!("{:?}", err)
    }
    Ok(())
}

fn run_app<B: Backend<Error = io::Error>>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        app.poll_device();

        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                // Ignore release/repeat events (sent on Windows and with the kitty protocol).
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                    KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
                    KeyCode::Left | KeyCode::Char('h') => app.modify_setting(-1),
                    KeyCode::Right | KeyCode::Char('l') => app.modify_setting(1),
                    _ => {}
                }
            }
        }
    }
}
