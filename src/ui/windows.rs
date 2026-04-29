use std::io::{self, IsTerminal};

pub fn prepare_console() {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() || !io::stderr().is_terminal() {
        return;
    }
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::ansi_support::supports_ansi();
}
