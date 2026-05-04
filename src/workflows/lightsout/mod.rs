use std::io;
use std::path::Path;

use crate::model::{AuthCache, AuthConfig, LogicGameKind};
use crate::ui;
use crate::workflows::logic_game_common;

pub const DONE_MESSAGE: &str = "自动点灯已完成。";

#[derive(Debug, Clone)]
pub struct AccountRunOutput {
    pub account: AuthCache,
    pub total_reward: f64,
}

pub fn run_batch(
    config: AuthConfig,
    auth_cache_file: impl AsRef<Path>,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
) -> io::Result<AuthConfig> {
    logic_game_common::run_batch(
        config,
        auth_cache_file,
        cancel_flag,
        log,
        LogicGameKind::LightsOut,
    )
}

pub fn run_account_for_free_play_with_log(
    config: &AuthConfig,
    account: AuthCache,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
) -> io::Result<AccountRunOutput> {
    let output = logic_game_common::run_account_for_free_play_with_log(
        config,
        account,
        cancel_flag,
        log,
        LogicGameKind::LightsOut,
    )?;
    Ok(AccountRunOutput {
        account: output.account,
        total_reward: output.total_reward,
    })
}
