use std::io;
use std::path::Path;

use crate::model::AuthConfig;
use crate::storage::{cache_from_login, load_cache, save_cache, upsert_account};
use crate::ui;
use crate::workflows::free_play::execute_all_free_features;
use crate::workflows::{checkin, scratch, sheepmatch};

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
            "2" => {
                if show_batch_feature_hub(config, auth_path) {
                    return true;
                }
            }
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
            "1" => {
                if show_free_feature_menu(config, auth_path) {
                    return true;
                }
            }
            "2" => {
                if show_paid_feature_menu(config, auth_path) {
                    return true;
                }
            }
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
                "1. 全自动完成所有白嫖玩法",
                "2. 自动签到",
                "3. 自动羊了个羊",
                "4. 返回上一级菜单",
                "5. 退出脚本",
            ],
            "请输入选项 (1/2/3/4/5): ",
            &["1", "2", "3", "4", "5"],
            "1、2、3、4 或 5",
            Some("4"),
        ) else {
            return false;
        };
        match choice.as_str() {
            "1" => run_all_free_features(config, auth_path),
            "2" => run_checkin_feature(config, auth_path),
            "3" => run_sheepmatch_feature(config, auth_path),
            "4" => return false,
            "5" => return true,
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
            "开始全自动完成所有白嫖玩法，本次会处理 {} 个账号。",
            original_config.accounts.len()
        ),
        Some("全自动完成所有白嫖玩法。"),
        move |cancel_flag, log| {
            let checkin_log = log.clone();
            let sheepmatch_log = log.clone();
            execute_all_free_features(
                original_config,
                &cancel_flag,
                &log,
                move |config, account, cancel_flag| {
                    checkin::run_account_with_log(config, account, cancel_flag, &checkin_log)
                },
                move |config, account, cancel_flag| {
                    sheepmatch::run_account_for_free_play_with_log(
                        config,
                        account,
                        cancel_flag,
                        &sheepmatch_log,
                    )
                },
                move |merged_config| save_cache(&save_auth_path, merged_config),
            )
        },
    ) {
        Ok(Some((_results, updated_config))) => {
            *config = updated_config;
            print_account_summary(config, &auth_path);
        }
        Ok(None) => {}
        Err(error) => println!("全自动完成所有白嫖玩法运行失败：{}", error),
    }
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
            for (index, line) in lines.iter().enumerate() {
                println!(
                    "[账号 {}] {} | 余额 {} | 账号状态 {}",
                    index + 1,
                    line.email,
                    line.balance,
                    line.status,
                );
            }
        }
        Err(error) => {
            println!("[账号] 刷新余额失败：{}", error);
            print_account_list(config);
        }
    }
}
