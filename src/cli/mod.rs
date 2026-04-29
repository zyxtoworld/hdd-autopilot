mod batch;
mod mining;
mod prompt;

use std::io;

use crate::runtime::resolve_data_file_path;
use crate::storage::load_cache;
use crate::ui;

use self::batch::show_batch_menu;
use self::mining::show_mining_menu;
use self::prompt::prompt_choice;

const ADD_ACCOUNT_EMAIL_PROMPT: &str = "请输入邮箱（按 ESC 取消添加）: ";
const ADD_ACCOUNT_PASSWORD_PROMPT: &str = "请输入密码（按 ESC 取消添加）: ";
const ADD_ACCOUNT_RETRY_PROMPT: &str = "请重新输入账号密码，按 ESC 取消添加。";

pub fn run() {
    let auth_path = resolve_data_file_path("auth.json");
    let mut config = match load_cache(&auth_path) {
        Ok(config) => config,
        Err(error) => {
            println!("加载账号信息失败：{}", error);
            return;
        }
    };

    loop {
        render_menu_page();

        match prompt_main_menu_choice() {
            Ok(choice) => match choice.as_str() {
                "1" => {
                    if show_mining_menu() {
                        println!("已退出脚本。");
                        return;
                    }
                }
                "2" => {
                    if show_batch_menu(&mut config, &auth_path) {
                        println!("已退出脚本。");
                        return;
                    }
                }
                "3" => {
                    println!("已退出脚本。");
                    return;
                }
                _ => {}
            },
            Err(error) => {
                println!("读取选项失败：{}", error);
                return;
            }
        }
    }
}

fn render_menu_page() {
    ui::hide_pinned_prompt();
    ui::clear_screen();
    println!("{}", ui::APP_BANNER);
    println!();
}

fn prompt_main_menu_choice() -> io::Result<String> {
    prompt_choice(
        &["1. 挖矿", "2. 需要登录的多账号批量操作功能", "3. 退出脚本"],
        "请输入选项 (1/2/3): ",
        &["1", "2", "3"],
        "1、2 或 3",
        None,
    )
}
