use chrono as _;
use crossterm as _;
use mining as _;
use rand as _;
use reqwest as _;
use serde as _;
use serde_json as _;
#[cfg(test)]
use tempfile as _;
use unicode_width as _;
use url as _;

use hdd_autopilot::cli;
use hdd_autopilot::runtime;
use hdd_autopilot::ui;

fn main() {
    runtime::migrate_legacy_data_file("auth.json");
    runtime::migrate_legacy_data_file("invite-codes.txt");
    runtime::migrate_legacy_data_file("balance-codes.txt");
    ui::prepare_console();
    cli::run()
}
