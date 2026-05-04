use std::io::{self, IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use super::{ADD_ACCOUNT_EMAIL_PROMPT, ADD_ACCOUNT_PASSWORD_PROMPT};

pub(super) fn prompt_email() -> io::Result<String> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        return read_prompt_line_interactive(ADD_ACCOUNT_EMAIL_PROMPT, false);
    }

    loop {
        print!("请输入邮箱: ");
        io::stdout().flush()?;
        let mut line = String::new();
        let bytes = io::stdin().read_line(&mut line)?;
        let email = line.trim();
        if !email.is_empty() {
            return Ok(email.to_string());
        }
        if bytes == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "邮箱不能为空"));
        }
        println!("邮箱不能为空，请重新输入。");
    }
}

pub(super) fn prompt_password() -> io::Result<String> {
    read_prompt_line_interactive(ADD_ACCOUNT_PASSWORD_PROMPT, true)
}

fn read_prompt_line_interactive(prompt: &str, mask_input: bool) -> io::Result<String> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    enable_raw_mode()?;
    let result = (|| {
        loop {
            print!("{}", prompt);
            io::stdout().flush()?;
            let mut input = String::new();
            loop {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }
                    match key.code {
                        KeyCode::Enter => {
                            println!();
                            let value = input.trim();
                            if value.is_empty() {
                                println!("{}不能为空，请重新输入。", prompt_field_name(prompt));
                                break;
                            }
                            return Ok(value.to_string());
                        }
                        KeyCode::Char(ch) => {
                            input.push(ch);
                            if mask_input {
                                print!("*");
                            } else {
                                print!("{}", ch);
                            }
                            io::stdout().flush()?;
                        }
                        KeyCode::Backspace if !input.is_empty() => {
                            input.pop();
                            print!("\x08 \x08");
                            io::stdout().flush()?;
                        }
                        KeyCode::Esc => {
                            println!();
                            return Err(io::Error::new(io::ErrorKind::Interrupted, "输入已取消"));
                        }
                        _ => {}
                    }
                }
            }
        }
    })();
    disable_raw_mode()?;
    result
}

fn prompt_field_name(prompt: &str) -> &str {
    if prompt.contains("邮箱") {
        "邮箱"
    } else if prompt.contains("密码") {
        "密码"
    } else {
        "输入"
    }
}

pub(super) fn prompt_choice(
    lines: &[&str],
    prompt: &str,
    allowed: &[&str],
    allowed_label: &str,
    escape_choice: Option<&str>,
) -> io::Result<String> {
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    loop {
        for line in lines {
            if let Some(escape_choice) = escape_choice
                && let Some(rendered) = with_escape_hint(line, escape_choice)
            {
                println!("{}", rendered);
                continue;
            }
            println!("{}", line);
        }
        print!("{}", prompt);
        io::stdout().flush()?;

        let choice = if interactive {
            read_choice_interactive(allowed, escape_choice)?
        } else {
            read_choice_line(allowed, escape_choice)?
        };

        if !interactive {
            println!();
        }

        if allowed.contains(&choice.as_str()) {
            return Ok(choice);
        }
        if choice.is_empty() {
            println!("你还没有输入选项，请输入 {}。", allowed_label);
        } else {
            println!("无法识别的选项 {:?}，请输入 {}。", choice, allowed_label);
        }
    }
}

fn with_escape_hint(line: &str, escape_choice: &str) -> Option<String> {
    let prefix = format!("{}. ", escape_choice);
    let rest = line.strip_prefix(&prefix)?;
    if rest == "返回上一级菜单" {
        return Some(format!("{}返回上一级菜单（ESC）", prefix));
    }
    None
}

fn read_choice_interactive(allowed: &[&str], escape_choice: Option<&str>) -> io::Result<String> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    enable_raw_mode()?;
    let result = (|| {
        let mut input = String::new();
        loop {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                match key.code {
                    KeyCode::Enter => {
                        println!();
                        return Ok(input.trim().to_string());
                    }
                    KeyCode::Esc => {
                        if let Some(choice) = escape_choice {
                            println!();
                            return Ok(choice.to_string());
                        }
                    }
                    KeyCode::Backspace if !input.is_empty() => {
                        input.pop();
                        print!("\x08 \x08");
                        io::stdout().flush()?;
                    }
                    KeyCode::Char(ch) => {
                        let candidate = format!("{input}{ch}");
                        if is_allowed_choice_prefix(&candidate, allowed) {
                            input.push(ch);
                            print!("{}", ch);
                            io::stdout().flush()?;
                        } else {
                            let replacement = ch.to_string();
                            if is_allowed_choice_prefix(&replacement, allowed) {
                                for _ in input.chars() {
                                    print!("\x08 \x08");
                                }
                                input.clear();
                                input.push(ch);
                                print!("{}", ch);
                                io::stdout().flush()?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    })();
    disable_raw_mode()?;
    result
}

fn is_allowed_choice_prefix(candidate: &str, allowed: &[&str]) -> bool {
    !candidate.is_empty() && allowed.iter().any(|choice| choice.starts_with(candidate))
}

fn read_choice_line(_allowed: &[&str], _escape_choice: Option<&str>) -> io::Result<String> {
    let mut line = String::new();
    let bytes = io::stdin().read_line(&mut line)?;
    if line.trim().is_empty() && bytes == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "未输入选项"));
    }
    Ok(line.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_field_name_maps_supported_prompts() {
        assert_eq!(prompt_field_name("请输入邮箱: "), "邮箱");
        assert_eq!(prompt_field_name(ADD_ACCOUNT_EMAIL_PROMPT), "邮箱");
        assert_eq!(prompt_field_name("请输入密码: "), "密码");
        assert_eq!(prompt_field_name(ADD_ACCOUNT_PASSWORD_PROMPT), "密码");
        assert_eq!(prompt_field_name("请输入内容: "), "输入");
    }

    #[test]
    fn choice_prefix_accepts_multi_digit_menu_options() {
        let allowed = ["1", "2", "10", "11", "15"];

        assert!(is_allowed_choice_prefix("1", &allowed));
        assert!(is_allowed_choice_prefix("10", &allowed));
        assert!(is_allowed_choice_prefix("15", &allowed));
        assert!(!is_allowed_choice_prefix("16", &allowed));
        assert!(!is_allowed_choice_prefix("", &allowed));
    }
}
