use std::io;
use std::path::Path;
use std::sync::Arc;

use crate::model::AuthConfig;
use crate::storage::{cache_from_login, load_cache, save_cache, upsert_account};
use crate::ui;
use crate::workflows::common::format_amount;
use crate::workflows::free_play::{FreeFeatureRunners, execute_all_free_features};
use crate::workflows::{
    arrow_out, checkin, flowfree, lightsout, maze, memory, minesweeper, nonogram, puzzle_15,
    puzzle_2048, scratch, sheepmatch, sokoban, sudoku,
};
use unicode_width::UnicodeWidthStr;

use super::prompt::{prompt_choice, prompt_email, prompt_password};
use super::{ADD_ACCOUNT_RETRY_PROMPT, render_menu_page};

pub(super) fn show_batch_menu(config: &mut AuthConfig, auth_path: &Path) -> bool {
    loop {
        render_menu_page();
        print_account_list(config);
        println!();
        let Ok(choice) = prompt_choice(
            &[
                "1. 添加账号",
                "2. 账号添加完成，选择脚本功能",
                "3. 返回上一级菜单",
                "4. 退出脚本",
            ],
            "请输入选项 (1/2/3/4): ",
            &["1", "2", "3", "4"],
            "1、2、3 或 4",
            Some("3"),
        ) else {
            return false;
        };
        match choice.as_str() {
            "1" => add_one_account(config, auth_path),
            "2" if show_batch_feature_hub(config, auth_path) => return true,
            "2" => {}
            "3" => return false,
            "4" => return true,
            _ => {}
        }
    }
}

fn add_one_account(config: &mut AuthConfig, auth_path: &Path) {
    loop {
        let email = match prompt_email() {
            Ok(email) => email,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return,
            Err(error) => {
                println!("读取邮箱失败：{}", error);
                return;
            }
        };

        let password = match prompt_password() {
            Ok(password) => password,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return,
            Err(error) => {
                println!("读取密码失败：{}", error);
                return;
            }
        };

        let client = crate::api::ApiClient::new(&config.base_url);
        match client.do_login(&email, &password) {
            Ok((login_response, _auth_token)) => {
                let account = cache_from_login(
                    &login_response,
                    &email,
                    &password,
                    client.base_url(),
                    client.export_session_cookies(),
                );
                *config = upsert_account(config.clone(), account.clone());
                if let Err(error) = save_cache(auth_path, config.clone()) {
                    println!("保存账号失败：{}", error);
                    return;
                }
                println!("登录成功并已保存账号：{}", account.email);
                return;
            }
            Err(error) => {
                println!("添加账号失败：{}", error);
                println!("{}", ADD_ACCOUNT_RETRY_PROMPT);
            }
        }
    }
}

fn show_batch_feature_hub(config: &mut AuthConfig, auth_path: &Path) -> bool {
    loop {
        render_menu_page();
        print_account_summary(config, auth_path);
        println!();
        let Ok(choice) = prompt_choice(
            &[
                "1. 白嫖玩法",
                "2. 赌狗玩法",
                "3. 返回上一级菜单",
                "4. 退出脚本",
            ],
            "请输入选项 (1/2/3/4): ",
            &["1", "2", "3", "4"],
            "1、2、3 或 4",
            Some("3"),
        ) else {
            return false;
        };
        match choice.as_str() {
            "1" if show_free_feature_menu(config, auth_path) => return true,
            "1" => {}
            "2" if show_paid_feature_menu(config, auth_path) => return true,
            "2" => {}
            "3" => return false,
            "4" => return true,
            _ => {}
        }
    }
}

fn show_free_feature_menu(config: &mut AuthConfig, auth_path: &Path) -> bool {
    loop {
        render_menu_page();
        print_account_summary(config, auth_path);
        println!();
        let Ok(choice) = prompt_choice(
            &[
                "1. 有次数限制",
                "2. 无次数限制",
                "3. 返回上一级菜单",
                "4. 退出脚本",
            ],
            "请输入选项 (1/2/3/4): ",
            &["1", "2", "3", "4"],
            "1、2、3 或 4",
            Some("3"),
        ) else {
            return false;
        };
        match choice.as_str() {
            "1" if show_limited_free_feature_menu(config, auth_path) => return true,
            "1" => {}
            "2" if show_unlimited_free_feature_menu(config, auth_path) => return true,
            "2" => {}
            "3" => return false,
            "4" => return true,
            _ => {}
        }
    }
}

fn show_limited_free_feature_menu(config: &mut AuthConfig, auth_path: &Path) -> bool {
    loop {
        render_menu_page();
        print_account_summary(config, auth_path);
        println!();
        let Ok(choice) = prompt_choice(
            &[
                "1. 全自动运行所有有次数限制白嫖玩法",
                "2. 自动扫雷",
                "3. 自动羊了个羊",
                "4. 自动谜题2048",
                "5. 自动推箱子",
                "6. 自动点灯",
                "7. 自动迷宫",
                "8. 自动数织",
                "9. 自动连线",
                "10. 自动记忆翻牌",
                "11. 自动华容道",
                "12. 自动数独",
                "13. 自动签到",
                "14. 返回上一级菜单",
                "15. 退出脚本",
            ],
            "请输入选项 (1/2/3/4/5/6/7/8/9/10/11/12/13/14/15): ",
            &[
                "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
            ],
            "1 到 15",
            Some("14"),
        ) else {
            return false;
        };
        match choice.as_str() {
            "1" => run_all_free_features(config, auth_path),
            "2" => run_minesweeper_feature(config, auth_path),
            "3" => run_sheepmatch_feature(config, auth_path),
            "4" => run_puzzle_2048_feature(config, auth_path),
            "5" => run_sokoban_feature(config, auth_path),
            "6" => run_lightsout_feature(config, auth_path),
            "7" => run_maze_feature(config, auth_path),
            "8" => run_nonogram_feature(config, auth_path),
            "9" => run_flowfree_feature(config, auth_path),
            "10" => run_memory_feature(config, auth_path),
            "11" => run_puzzle_15_feature(config, auth_path),
            "12" => run_sudoku_feature(config, auth_path),
            "13" => run_checkin_feature(config, auth_path),
            "14" => return false,
            "15" => return true,
            _ => {}
        }
    }
}

fn show_unlimited_free_feature_menu(config: &mut AuthConfig, auth_path: &Path) -> bool {
    loop {
        render_menu_page();
        print_account_summary(config, auth_path);
        println!();
        let Ok(choice) = prompt_choice(
            &[
                "1. 全自动运行所有无次数限制白嫖玩法",
                "2. 自动箭头逃离",
                "3. 返回上一级菜单",
                "4. 退出脚本",
            ],
            "请输入选项 (1/2/3/4): ",
            &["1", "2", "3", "4"],
            "1、2、3 或 4",
            Some("3"),
        ) else {
            return false;
        };
        match choice.as_str() {
            "1" => run_all_unlimited_free_features(config, auth_path),
            "2" => run_arrow_out_feature(config, auth_path),
            "3" => return false,
            "4" => return true,
            _ => {}
        }
    }
}

fn show_paid_feature_menu(config: &mut AuthConfig, auth_path: &Path) -> bool {
    loop {
        render_menu_page();
        print_account_summary(config, auth_path);
        println!();
        let Ok(choice) = prompt_choice(
            &["1. 自动随机刮刮乐", "2. 返回上一级菜单", "3. 退出脚本"],
            "请输入选项 (1/2/3): ",
            &["1", "2", "3"],
            "1、2 或 3",
            Some("2"),
        ) else {
            return false;
        };
        match choice.as_str() {
            "1" => run_scratch_feature(config, auth_path),
            "2" => return false,
            "3" => return true,
            _ => {}
        }
    }
}

fn run_checkin_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动签到，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some("全部账号签到完成。"),
        move |cancel_flag, log| {
            checkin::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(_results)) => {
            if let Ok(latest) = load_cache(&auth_path) {
                *config = latest;
            }
        }
        Ok(None) => {}
        Err(error) => {
            println!("自动签到运行失败：{}", error);
        }
    }
}

fn run_scratch_feature(config: &mut AuthConfig, auth_path: &Path) {
    match scratch::run_batch(config.clone(), auth_path) {
        Ok(updated_config) => {
            *config = updated_config;
        }
        Err(error) => {
            println!("自动随机刮刮乐运行失败：{}", error);
        }
    }
}

fn run_puzzle_2048_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动谜题2048，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(puzzle_2048::DONE_MESSAGE),
        move |cancel_flag, log| {
            puzzle_2048::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动谜题2048运行失败：{}", error),
    }
}

fn run_memory_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动记忆翻牌，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(memory::DONE_MESSAGE),
        move |cancel_flag, log| {
            memory::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动记忆翻牌运行失败：{}", error),
    }
}

fn run_sokoban_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动推箱子，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(sokoban::DONE_MESSAGE),
        move |cancel_flag, log| {
            sokoban::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动推箱子运行失败：{}", error),
    }
}

fn run_lightsout_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动点灯，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(lightsout::DONE_MESSAGE),
        move |cancel_flag, log| {
            lightsout::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动点灯运行失败：{}", error),
    }
}

fn run_maze_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动迷宫，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(maze::DONE_MESSAGE),
        move |cancel_flag, log| {
            maze::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动迷宫运行失败：{}", error),
    }
}

fn run_nonogram_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动数织，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(nonogram::DONE_MESSAGE),
        move |cancel_flag, log| {
            nonogram::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动数织运行失败：{}", error),
    }
}

fn run_flowfree_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动连线，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(flowfree::DONE_MESSAGE),
        move |cancel_flag, log| {
            flowfree::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动连线运行失败：{}", error),
    }
}

fn run_minesweeper_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动扫雷，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(minesweeper::DONE_MESSAGE),
        move |cancel_flag, log| {
            minesweeper::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动扫雷运行失败：{}", error),
    }
}

fn run_puzzle_15_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动华容道，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(puzzle_15::DONE_MESSAGE),
        move |cancel_flag, log| {
            puzzle_15::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动华容道运行失败：{}", error),
    }
}

fn run_sudoku_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动数独，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(sudoku::DONE_MESSAGE),
        move |cancel_flag, log| {
            sudoku::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动数独运行失败：{}", error),
    }
}

fn run_sheepmatch_feature(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始自动羊了个羊，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some(sheepmatch::DONE_MESSAGE),
        move |cancel_flag, log| {
            sheepmatch::run_batch(original_config, &run_auth_path, &cancel_flag, &log)
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
        }
        Ok(None) => {}
        Err(error) => println!("自动羊了个羊运行失败：{}", error),
    }
}

fn run_arrow_out_feature(config: &mut AuthConfig, auth_path: &Path) {
    run_arrow_out_with_title(config, auth_path, "自动箭头逃离");
}

fn run_all_unlimited_free_features(config: &mut AuthConfig, auth_path: &Path) {
    run_arrow_out_with_title(config, auth_path, "全自动运行所有无次数限制白嫖玩法");
}

fn run_arrow_out_with_title(config: &mut AuthConfig, auth_path: &Path, title: &'static str) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let run_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始{}，本次会处理 {} 个账号；无次数限制玩法会持续运行，按 ESC 停止。",
            title,
            original_config.accounts.len()
        ),
        Some(arrow_out::DONE_MESSAGE),
        move |cancel_flag, log| {
            arrow_out::run_batch_with_title(
                original_config,
                &run_auth_path,
                &cancel_flag,
                &log,
                title,
            )
        },
    ) {
        Ok(Some(updated_config)) => {
            *config = updated_config;
            print_account_summary(config, &auth_path);
        }
        Ok(None) => {
            if let Ok(latest) = load_cache(&auth_path) {
                *config = latest;
            }
        }
        Err(error) => println!("{}运行失败：{}", title, error),
    }
}

fn run_all_free_features(config: &mut AuthConfig, auth_path: &Path) {
    if config.accounts.is_empty() {
        println!("当前还没有可用账号。");
        return;
    }
    let auth_path = auth_path.to_path_buf();
    let save_auth_path = auth_path.clone();
    let original_config = config.clone();
    match ui::run_with_escape_interrupt(
        &format!(
            "开始全自动运行所有有次数限制白嫖玩法，本次会处理 {} 个账号；每个账号会并发运行所有有次数限制白嫖项目。",
            original_config.accounts.len()
        ),
        Some("全自动运行所有有次数限制白嫖玩法已完成。"),
        move |cancel_flag, log| {
            let checkin_log = log.clone();
            let minesweeper_log = log.clone();
            let sheepmatch_log = log.clone();
            execute_all_free_features(
                original_config,
                &cancel_flag,
                &log,
                FreeFeatureRunners {
                    run_checkin: Arc::new(move |config, account, cancel_flag| {
                        let feature_log = feature_log(&checkin_log, "自动签到", &account.email);
                        checkin::run_account_with_log(config, account, cancel_flag, &feature_log)
                    }),
                    run_minesweeper: Arc::new(move |config, account, cancel_flag| {
                        let feature_log = feature_log(&minesweeper_log, "自动扫雷", &account.email);
                        minesweeper::run_account_for_free_play_with_log(
                            config,
                            account,
                            cancel_flag,
                            &feature_log,
                        )
                    }),
                    run_sheepmatch: Arc::new(move |config, account, cancel_flag| {
                        let feature_log =
                            feature_log(&sheepmatch_log, "自动羊了个羊", &account.email);
                        sheepmatch::run_account_for_free_play_with_log(
                            config,
                            account,
                            cancel_flag,
                            &feature_log,
                        )
                    }),
                    run_puzzle_2048: {
                        let puzzle_2048_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log =
                                feature_log(&puzzle_2048_log, "自动谜题2048", &account.email);
                            puzzle_2048::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    run_sokoban: {
                        let sokoban_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log =
                                feature_log(&sokoban_log, "自动推箱子", &account.email);
                            sokoban::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    run_lightsout: {
                        let lightsout_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log =
                                feature_log(&lightsout_log, "自动点灯", &account.email);
                            lightsout::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    run_maze: {
                        let maze_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log = feature_log(&maze_log, "自动迷宫", &account.email);
                            maze::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    run_nonogram: {
                        let nonogram_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log =
                                feature_log(&nonogram_log, "自动数织", &account.email);
                            nonogram::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    run_flowfree: {
                        let flowfree_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log =
                                feature_log(&flowfree_log, "自动连线", &account.email);
                            flowfree::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    run_memory: {
                        let memory_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log =
                                feature_log(&memory_log, "自动记忆翻牌", &account.email);
                            memory::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    run_puzzle_15: {
                        let puzzle_15_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log =
                                feature_log(&puzzle_15_log, "自动华容道", &account.email);
                            puzzle_15::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    run_sudoku: {
                        let sudoku_log = log.clone();
                        Arc::new(move |config, account, cancel_flag| {
                            let feature_log = feature_log(&sudoku_log, "自动数独", &account.email);
                            sudoku::run_account_for_free_play_with_log(
                                config,
                                account,
                                cancel_flag,
                                &feature_log,
                            )
                        })
                    },
                    save_merged_config: Box::new(move |merged_config| {
                        save_cache(&save_auth_path, merged_config)
                    }),
                },
            )
        },
    ) {
        Ok(Some((_results, updated_config))) => {
            *config = updated_config;
            print_account_summary(config, &auth_path);
        }
        Ok(None) => {}
        Err(error) => println!("全自动运行所有有次数限制白嫖玩法运行失败：{}", error),
    }
}

fn feature_log(log: &ui::TaskLog, feature: &str, email: &str) -> ui::TaskLog {
    let email = email.trim();
    let email = if email.is_empty() {
        "未知账号"
    } else {
        email
    };
    log.prefixed(format!("【{}｜{}】", feature, email))
}

fn print_account_list(config: &AuthConfig) {
    println!("当前已保存的账号：");
    if config.accounts.is_empty() {
        println!("[账号] （还没有）");
        return;
    }
    for (index, account) in config.accounts.iter().enumerate() {
        println!("[账号 {}] {}", index + 1, account.email);
    }
}

fn print_account_summary(config: &mut AuthConfig, auth_path: &Path) {
    println!("当前账号情况：");
    if config.accounts.is_empty() {
        println!("[账号] （还没有）");
        return;
    }
    match checkin::load_balance_lines(config.clone(), auth_path) {
        Ok((updated_config, lines)) => {
            *config = updated_config;
            if lines.is_empty() {
                println!("[账号] （还没有）");
                return;
            }
            let account_label_width = (1..=lines.len())
                .map(|index| display_width(&format!("[账号 {}]", index)))
                .max()
                .unwrap_or(0);
            let email_width = lines
                .iter()
                .map(|line| display_width(&line.email))
                .max()
                .unwrap_or(0);
            let balance_width = lines
                .iter()
                .map(|line| display_width(&line.balance))
                .max()
                .unwrap_or(0);
            for (index, line) in lines.iter().enumerate() {
                println!(
                    "{}",
                    format_account_summary_line(
                        index + 1,
                        line,
                        account_label_width,
                        email_width,
                        balance_width,
                    )
                );
            }
            let total_balance = lines
                .iter()
                .filter_map(balance_amount_from_line)
                .sum::<f64>();
            println!("所有账号余额汇总：{}", format_amount(total_balance));
        }
        Err(error) => {
            println!("[账号] 刷新余额失败：{}", error);
            print_account_list(config);
        }
    }
}

fn format_account_summary_line(
    index: usize,
    line: &checkin::BalanceLine,
    account_label_width: usize,
    email_width: usize,
    balance_width: usize,
) -> String {
    format!(
        "{} {} | 余额 {} | 账号状态 {}",
        pad_display_right(&format!("[账号 {}]", index), account_label_width),
        pad_display_right(&line.email, email_width),
        pad_display_right(&line.balance, balance_width),
        line.status,
    )
}

fn pad_display_right(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(display_width(text));
    format!("{}{}", text, " ".repeat(padding))
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn balance_amount_from_line(line: &checkin::BalanceLine) -> Option<f64> {
    line.balance
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflows::checkin::BalanceLine;

    #[test]
    fn account_summary_lines_align_separators() {
        let lines = [
            BalanceLine {
                email: "short@example.org".to_string(),
                balance: "817.37747521".to_string(),
                status: "正常".to_string(),
            },
            BalanceLine {
                email: "very-long-account@example.org".to_string(),
                balance: "9.5".to_string(),
                status: "正常".to_string(),
            },
        ];
        let account_label_width = (1..=lines.len())
            .map(|index| display_width(&format!("[账号 {}]", index)))
            .max()
            .unwrap();
        let email_width = lines
            .iter()
            .map(|line| display_width(&line.email))
            .max()
            .unwrap();
        let balance_width = lines
            .iter()
            .map(|line| display_width(&line.balance))
            .max()
            .unwrap();

        let rendered = lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                format_account_summary_line(
                    index + 1,
                    line,
                    account_label_width,
                    email_width,
                    balance_width,
                )
            })
            .collect::<Vec<_>>();
        let separator_columns = rendered
            .iter()
            .map(|line| {
                line.match_indices('|')
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        assert_eq!(separator_columns[0], separator_columns[1]);
    }
}
