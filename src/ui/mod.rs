use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
};
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, queue};

#[cfg(not(windows))]
mod other;
mod render;
#[cfg(windows)]
mod windows;

use render::{
    format_batch_return_message, render_single_line_prompt, render_single_line_text,
    wrap_text_to_width,
};

pub type CancelFlag = Arc<AtomicBool>;
pub const ERR_INTERRUPTED: &str = "interrupted";
pub const APP_BANNER: &str = "欢迎使用号多多脚本整合工具。";

const CLEAR_SCREEN_SEQUENCE: &str = "\x1b[2J\x1b[H";
const RESET_SCROLL_SEQUENCE: &str = "\x1b[r";
const PINNED_SCROLL_START_ROW: u16 = 5;
const TASK_HEADER_ROWS: u16 = 4;
const TASK_FOOTER_ROWS: u16 = 1;
const MIN_TASK_VIEW_HEIGHT: u16 = TASK_HEADER_ROWS + TASK_FOOTER_ROWS + 1;

#[derive(Clone)]
pub struct TaskLog {
    write_line: Arc<dyn Fn(String) + Send + Sync>,
}

impl TaskLog {
    pub fn stdout() -> Self {
        Self::new(|line| {
            println!("{}", line);
            let _ = io::stdout().flush();
        })
    }

    pub fn line(&self, line: impl AsRef<str>) {
        let text = line.as_ref();
        if text.is_empty() {
            self.write(String::new());
            return;
        }
        for line in text.lines() {
            self.write(line.to_string());
        }
    }

    pub fn line_fmt(&self, args: fmt::Arguments<'_>) {
        self.line(args.to_string());
    }

    pub fn prefixed(&self, prefix: impl Into<String>) -> Self {
        let parent = self.clone();
        let prefix = prefix.into();
        Self::new(move |line| {
            if line.is_empty() {
                parent.line("");
            } else {
                parent.line(format!("{} {}", prefix, line));
            }
        })
    }

    fn sender(sender: mpsc::Sender<String>) -> Self {
        Self::new(move |line| {
            let _ = sender.send(line);
        })
    }

    fn new<F>(write_line: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        Self {
            write_line: Arc::new(write_line),
        }
    }

    fn write(&self, line: String) {
        (self.write_line)(line);
    }
}

impl fmt::Debug for TaskLog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TaskLog")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskRunStatus {
    Running,
    Cancelling,
    Succeeded,
    Failed,
}

struct TaskLogScreen;

impl TaskLogScreen {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        ) {
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TaskLogScreen {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            cursor::Show,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

pub fn prepare_console() {
    #[cfg(windows)]
    windows::prepare_console();
    #[cfg(not(windows))]
    other::prepare_console();
}

fn supports_interactive_terminal() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

pub fn clear_screen() {
    if !supports_interactive_terminal() {
        return;
    }
    print!("{}{}", CLEAR_SCREEN_SEQUENCE, RESET_SCROLL_SEQUENCE);
    let _ = io::stdout().flush();
}

fn terminal_width() -> usize {
    terminal::size()
        .map(|(width, _)| width.max(1) as usize)
        .unwrap_or(80)
}

fn terminal_height() -> u16 {
    terminal::size()
        .map(|(_, height)| height.max(PINNED_SCROLL_START_ROW))
        .unwrap_or(24)
}

fn task_view_size() -> (u16, u16) {
    terminal::size()
        .map(|(width, height)| (width.max(1), height.max(MIN_TASK_VIEW_HEIGHT)))
        .unwrap_or((80, 24))
}

fn task_log_height(height: u16) -> usize {
    height
        .saturating_sub(TASK_HEADER_ROWS + TASK_FOOTER_ROWS)
        .max(1) as usize
}

fn pinned_scroll_region_sequence(height: u16) -> String {
    format!(
        "\x1b[{};{}r",
        PINNED_SCROLL_START_ROW,
        height.max(PINNED_SCROLL_START_ROW)
    )
}

fn render_prompt(prompt: &str) -> String {
    render_single_line_prompt(prompt, terminal_width())
}

pub fn show_pinned_prompt(prompt: &str) {
    let prompt = prompt.trim();
    if prompt.is_empty() || !supports_interactive_terminal() {
        return;
    }
    let banner = render_prompt(APP_BANNER);
    let rendered = render_prompt(prompt);
    let scroll_region = pinned_scroll_region_sequence(terminal_height());
    print!(
        "{}{}\x1b[1;1H\x1b[2K{}\x1b[2;1H\x1b[2K\x1b[3;1H\x1b[2K{}\x1b[4;1H\x1b[2K{}\x1b[5;1H",
        CLEAR_SCREEN_SEQUENCE, RESET_SCROLL_SEQUENCE, banner, rendered, scroll_region,
    );
    let _ = io::stdout().flush();
}

pub fn hide_pinned_prompt() {
    if !supports_interactive_terminal() {
        return;
    }
    print!("{}", RESET_SCROLL_SEQUENCE);
    let _ = io::stdout().flush();
}

pub fn show_footer_prompt(prompt: &str) {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return;
    }
    println!();
    println!("{}", prompt);
    let _ = io::stdout().flush();
}

pub fn wait_for_escape() -> io::Result<()> {
    loop {
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        if let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
            && key.code == KeyCode::Esc
        {
            return Ok(());
        }
    }
}

pub fn wait_for_batch_return(message: &str) -> io::Result<()> {
    let interactive = supports_interactive_terminal();
    show_footer_prompt(&format_batch_return_message(message, interactive));
    if std::env::var("HDD_SMOKE_AUTO_RETURN").is_ok() {
        println!("已返回上一级菜单。");
        let _ = io::stdout().flush();
        hide_pinned_prompt();
        return Ok(());
    }
    if !interactive {
        return Ok(());
    }
    wait_for_escape()?;
    println!("已返回上一级菜单。");
    let _ = io::stdout().flush();
    hide_pinned_prompt();
    Ok(())
}

pub fn run_with_escape_interrupt<T, F>(
    prompt: &str,
    finish_message: Option<&str>,
    action: F,
) -> io::Result<Option<T>>
where
    T: Send + 'static,
    F: FnOnce(CancelFlag, TaskLog) -> io::Result<T> + Send + 'static,
{
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let prompt = prompt.trim().to_string();
    let finish_message = finish_message
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(ToOwned::to_owned);

    if !supports_interactive_terminal() {
        let result = action(Arc::clone(&cancel_flag), TaskLog::stdout())?;
        if let Some(message) = finish_message.as_deref() {
            wait_for_batch_return(message)?;
        }
        return Ok(Some(result));
    }

    run_interactive_task(prompt, finish_message, cancel_flag, action)
}

fn run_interactive_task<T, F>(
    prompt: String,
    finish_message: Option<String>,
    cancel_flag: CancelFlag,
    action: F,
) -> io::Result<Option<T>>
where
    T: Send + 'static,
    F: FnOnce(CancelFlag, TaskLog) -> io::Result<T> + Send + 'static,
{
    let mut screen = Some(TaskLogScreen::enter()?);
    let worker_cancel_flag = Arc::clone(&cancel_flag);
    let (result_tx, result_rx) = mpsc::channel::<io::Result<T>>();
    let (log_tx, log_rx) = mpsc::channel::<String>();
    let task_log = TaskLog::sender(log_tx);
    let mut worker = Some(thread::spawn(move || {
        let result = action(worker_cancel_flag, task_log);
        let _ = result_tx.send(result);
    }));

    let mut logs = Vec::new();
    let mut scroll_top = 0usize;
    let mut follow_tail = true;
    let mut cancel_requested = false;
    let mut outcome = None;
    let mut status = TaskRunStatus::Running;
    let mut previous_size = task_view_size();
    let mut dirty = true;
    let mut clear_next_render = true;

    loop {
        let current_size = task_view_size();
        let size_changed = current_size != previous_size;
        if size_changed {
            previous_size = current_size;
            dirty = true;
            clear_next_render = true;
        }
        let (width, height) = current_size;
        let view_height = task_log_height(height);
        let mut visual_line_count = wrapped_task_log_line_count(&logs, width as usize);
        if size_changed {
            update_scroll_after_line_count_change(
                visual_line_count,
                view_height,
                &mut scroll_top,
                &mut follow_tail,
            );
        }
        if drain_task_logs(&log_rx, &mut logs) {
            visual_line_count = wrapped_task_log_line_count(&logs, width as usize);
            update_scroll_after_line_count_change(
                visual_line_count,
                view_height,
                &mut scroll_top,
                &mut follow_tail,
            );
            dirty = true;
        }

        if outcome.is_none() {
            match result_rx.try_recv() {
                Ok(result) => {
                    join_task_worker(&mut worker);
                    status = if result.is_ok() {
                        if let Some(message) = &finish_message {
                            logs.push(message.clone());
                        }
                        TaskRunStatus::Succeeded
                    } else {
                        if let Err(error) = &result {
                            logs.push(format!("任务运行失败：{}", error));
                        }
                        TaskRunStatus::Failed
                    };
                    outcome = Some(result);
                    visual_line_count = wrapped_task_log_line_count(&logs, width as usize);
                    update_scroll_after_line_count_change(
                        visual_line_count,
                        view_height,
                        &mut scroll_top,
                        &mut follow_tail,
                    );
                    dirty = true;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    join_task_worker(&mut worker);
                    let error = io::Error::other("后台任务意外结束");
                    logs.push(format!("任务运行失败：{}", error));
                    status = TaskRunStatus::Failed;
                    outcome = Some(Err(error));
                    visual_line_count = wrapped_task_log_line_count(&logs, width as usize);
                    update_scroll_after_line_count_change(
                        visual_line_count,
                        view_height,
                        &mut scroll_top,
                        &mut follow_tail,
                    );
                    dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if cancel_requested && outcome.is_some() {
            drop(screen.take());
            return Ok(None);
        }
        if std::env::var("HDD_SMOKE_AUTO_RETURN").is_ok()
            && let Some(result) = outcome.take()
        {
            drop(screen.take());
            return result.map(Some);
        }

        if dirty {
            render_task_log_view(TaskLogView {
                prompt: &prompt,
                logs: &logs,
                scroll_top,
                view_height,
                follow_tail,
                status,
                finish_message: finish_message.as_deref(),
                clear_screen: clear_next_render,
            })?;
            dirty = false;
            clear_next_render = false;
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Esc => {
                        if let Some(result) = outcome.take() {
                            drop(screen.take());
                            return result.map(Some);
                        }
                        if !cancel_requested {
                            cancel_flag.store(true, Ordering::SeqCst);
                            cancel_requested = true;
                            status = TaskRunStatus::Cancelling;
                            logs.push("正在停止后台任务，请稍候。".to_string());
                            follow_tail = true;
                            dirty = true;
                        }
                    }
                    KeyCode::Up => {
                        scroll_log_up(&mut scroll_top, &mut follow_tail, 1);
                        dirty = true;
                    }
                    KeyCode::Down => {
                        scroll_log_down(
                            visual_line_count,
                            view_height,
                            &mut scroll_top,
                            &mut follow_tail,
                            1,
                        );
                        dirty = true;
                    }
                    KeyCode::PageUp => {
                        scroll_log_up(
                            &mut scroll_top,
                            &mut follow_tail,
                            view_height.saturating_sub(1).max(1),
                        );
                        dirty = true;
                    }
                    KeyCode::PageDown => {
                        scroll_log_down(
                            visual_line_count,
                            view_height,
                            &mut scroll_top,
                            &mut follow_tail,
                            view_height.saturating_sub(1).max(1),
                        );
                        dirty = true;
                    }
                    KeyCode::Home => {
                        scroll_top = 0;
                        follow_tail = false;
                        dirty = true;
                    }
                    KeyCode::End => {
                        follow_tail = true;
                        scroll_top = bottom_scroll_top(visual_line_count, view_height);
                        dirty = true;
                    }
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        scroll_log_up(&mut scroll_top, &mut follow_tail, 3);
                        dirty = true;
                    }
                    MouseEventKind::ScrollDown => {
                        scroll_log_down(
                            visual_line_count,
                            view_height,
                            &mut scroll_top,
                            &mut follow_tail,
                            3,
                        );
                        dirty = true;
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {
                    dirty = true;
                    clear_next_render = true;
                }
                _ => {}
            }
        }
    }
}

fn drain_task_logs(receiver: &mpsc::Receiver<String>, logs: &mut Vec<String>) -> bool {
    let mut changed = false;
    while let Ok(line) = receiver.try_recv() {
        logs.push(line);
        changed = true;
    }
    changed
}

fn join_task_worker<T>(worker: &mut Option<thread::JoinHandle<T>>) {
    if let Some(worker) = worker.take() {
        let _ = worker.join();
    }
}

fn update_scroll_after_line_count_change(
    total_lines: usize,
    view_height: usize,
    scroll_top: &mut usize,
    follow_tail: &mut bool,
) {
    let bottom = bottom_scroll_top(total_lines, view_height);
    if *follow_tail {
        *scroll_top = bottom;
    } else {
        *scroll_top = (*scroll_top).min(bottom);
        *follow_tail = *scroll_top >= bottom;
    }
}

fn bottom_scroll_top(total_lines: usize, view_height: usize) -> usize {
    total_lines.saturating_sub(view_height)
}

fn scroll_log_up(scroll_top: &mut usize, follow_tail: &mut bool, amount: usize) {
    *scroll_top = scroll_top.saturating_sub(amount);
    *follow_tail = false;
}

fn scroll_log_down(
    total_lines: usize,
    view_height: usize,
    scroll_top: &mut usize,
    follow_tail: &mut bool,
    amount: usize,
) {
    let bottom = bottom_scroll_top(total_lines, view_height);
    *scroll_top = (*scroll_top + amount).min(bottom);
    *follow_tail = *scroll_top >= bottom;
}

struct TaskLogView<'a> {
    prompt: &'a str,
    logs: &'a [String],
    scroll_top: usize,
    view_height: usize,
    follow_tail: bool,
    status: TaskRunStatus,
    finish_message: Option<&'a str>,
    clear_screen: bool,
}

fn render_task_log_view(view: TaskLogView<'_>) -> io::Result<()> {
    let (width, height) = task_view_size();
    let width = width as usize;
    let mut stdout = io::stdout();
    if view.clear_screen {
        queue!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;
    }
    let visual_logs = wrap_task_logs(view.logs, width);
    queue_task_line(&mut stdout, 0, width, APP_BANNER)?;
    queue_task_line(&mut stdout, 1, width, view.prompt)?;
    queue_task_line(
        &mut stdout,
        2,
        width,
        &task_help_line(view.status, view.finish_message),
    )?;
    queue_task_line(&mut stdout, 3, width, &"─".repeat(width))?;

    for row in 0..view.view_height {
        let terminal_row = TASK_HEADER_ROWS + row as u16;
        if terminal_row >= height.saturating_sub(TASK_FOOTER_ROWS) {
            break;
        }
        let line = visual_logs
            .get(view.scroll_top + row)
            .map(String::as_str)
            .unwrap_or_default();
        queue_task_line(&mut stdout, terminal_row, width, line)?;
    }

    queue_task_line(
        &mut stdout,
        height.saturating_sub(1),
        width,
        &task_footer_line(
            visual_logs.len(),
            view.scroll_top,
            view.view_height,
            view.follow_tail,
        ),
    )?;
    stdout.flush()
}

fn wrapped_task_log_line_count(logs: &[String], width: usize) -> usize {
    wrap_task_logs(logs, width).len()
}

fn wrap_task_logs(logs: &[String], width: usize) -> Vec<String> {
    logs.iter()
        .flat_map(|line| wrap_text_to_width(line, width))
        .collect()
}

fn queue_task_line(stdout: &mut io::Stdout, row: u16, width: usize, text: &str) -> io::Result<()> {
    queue!(
        stdout,
        cursor::MoveTo(0, row),
        Clear(ClearType::CurrentLine),
        Print(render_single_line_text(text, width))
    )
}

fn task_help_line(status: TaskRunStatus, finish_message: Option<&str>) -> String {
    let action = match status {
        TaskRunStatus::Running => "运行中：按 ESC 停止并返回",
        TaskRunStatus::Cancelling => "正在停止后台任务，请稍候",
        TaskRunStatus::Succeeded => finish_message.unwrap_or("任务已结束，按 ESC 返回上一级菜单"),
        TaskRunStatus::Failed => "任务运行失败，按 ESC 返回上一级菜单",
    };
    format!("{}；↑/↓ PgUp/PgDn Home/End 或鼠标滚轮查看日志。", action)
}

fn task_footer_line(
    total_lines: usize,
    scroll_top: usize,
    view_height: usize,
    follow_tail: bool,
) -> String {
    if total_lines == 0 {
        return "日志 0/0 | 跟随最新".to_string();
    }
    let start = scroll_top.min(total_lines) + 1;
    let end = (scroll_top + view_height).min(total_lines);
    let mode = if follow_tail {
        "跟随最新"
    } else {
        "查看历史"
    };
    format!("日志 {}-{}/{} | {}", start, end, total_lines, mode)
}

pub fn check_cancel(cancel_flag: &CancelFlag) -> io::Result<()> {
    if cancel_flag.load(Ordering::SeqCst) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, ERR_INTERRUPTED));
    }
    Ok(())
}

pub fn sleep_with_cancel(cancel_flag: &CancelFlag, wait: Duration) -> io::Result<()> {
    if wait <= Duration::ZERO {
        return Ok(());
    }
    let started = std::time::Instant::now();
    loop {
        check_cancel(cancel_flag)?;
        let elapsed = started.elapsed();
        if elapsed >= wait {
            return Ok(());
        }
        let remaining = wait - elapsed;
        thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_scroll_region_uses_terminal_bottom() {
        assert_eq!(pinned_scroll_region_sequence(40), "\x1b[5;40r");
    }

    #[test]
    fn pinned_scroll_region_clamps_small_height() {
        assert_eq!(pinned_scroll_region_sequence(3), "\x1b[5;5r");
    }

    #[test]
    fn bottom_scroll_clamps_to_zero_when_log_fits() {
        assert_eq!(bottom_scroll_top(3, 10), 0);
    }

    #[test]
    fn bottom_scroll_tracks_last_visible_page() {
        assert_eq!(bottom_scroll_top(30, 10), 20);
    }

    #[test]
    fn task_logs_wrap_to_terminal_width() {
        let logs = vec!["abcdef".to_string(), "中文ab".to_string()];

        assert_eq!(wrap_task_logs(&logs, 4), vec!["abcd", "ef", "中文", "ab"]);
    }

    #[test]
    fn wrapped_task_log_line_count_counts_visual_rows() {
        let logs = vec!["abcdef".to_string(), String::new()];

        assert_eq!(wrapped_task_log_line_count(&logs, 3), 3);
    }
}
