use unicode_width::UnicodeWidthChar;

pub(super) fn format_batch_return_message(message: &str, interactive: bool) -> String {
    let message = message.trim();
    if interactive {
        message.to_string()
    } else {
        format!(
            "{}当前是非交互模式；如需返回上一级菜单，请在交互终端中按 ESC。",
            message
        )
    }
}

pub(super) fn render_single_line_prompt(prompt: &str, width: usize) -> String {
    render_single_line_text(prompt.trim(), width)
}

pub(super) fn render_single_line_text(text: &str, width: usize) -> String {
    let width = width.max(1);
    if text.is_empty() {
        return " ".repeat(width);
    }

    let mut rendered = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + char_width > width {
            break;
        }
        rendered.push(ch);
        used += char_width;
    }
    if used < width {
        rendered.push_str(&" ".repeat(width - used));
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_single_line_prompt_trims_and_pads_ascii() {
        let rendered = render_single_line_prompt("  按 ESC 返回上一级菜单  ", 12);
        assert_eq!(rendered, "按 ESC 返回 ");
    }

    #[test]
    fn render_single_line_prompt_handles_wide_chars_without_wrapping() {
        let rendered = render_single_line_prompt("中文提示AB", 6);
        assert_eq!(rendered, "中文提");
    }

    #[test]
    fn render_single_line_text_keeps_leading_spaces() {
        let rendered = render_single_line_text("  日志", 8);
        assert_eq!(rendered, "  日志  ");
    }

    #[test]
    fn wait_for_batch_return_formats_headless_message() {
        let message = "自动羊了个羊处理完成。";
        let formatted = format_batch_return_message(message, false);
        assert_eq!(
            formatted,
            "自动羊了个羊处理完成。当前是非交互模式；如需返回上一级菜单，请在交互终端中按 ESC。"
        );
    }

    #[test]
    fn wait_for_batch_return_keeps_interactive_message_clean() {
        let message = "自动羊了个羊处理完成。";
        let formatted = format_batch_return_message(message, true);
        assert_eq!(formatted, "自动羊了个羊处理完成。");
    }
}
