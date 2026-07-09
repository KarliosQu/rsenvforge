use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::error::ForgeError;
use super::input::try_read_skip_request;

const FIRST_PROGRESS_NOTICE_SECONDS: u64 = 120;
const PROGRESS_OUTPUT_LIMIT: usize = 8 * 1024;
const PROGRESS_NOTICE_MAX_LINES: usize = 5;
const STATUS_BAR_LINES: u16 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellDisplayMode {
    Plain,
    StatusBar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellRunStatus {
    Completed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellStep {
    pub(crate) current: usize,
    pub(crate) total: usize,
}

pub(crate) fn run_shell_labeled(label: &str, command: &str) -> Result<ShellRunStatus, ForgeError> {
    run_shell_labeled_with_options(label, command, true, true, ShellDisplayMode::Plain, None)
}

pub(crate) fn run_shell_labeled_display_step(
    label: &str,
    command: &str,
    mode: ShellDisplayMode,
    step: Option<ShellStep>,
) -> Result<ShellRunStatus, ForgeError> {
    run_shell_labeled_with_options(label, command, true, true, mode, step)
}

pub(crate) fn run_shell_labeled_quiet_display_step(
    label: &str,
    command: &str,
    mode: ShellDisplayMode,
    step: Option<ShellStep>,
) -> Result<ShellRunStatus, ForgeError> {
    run_shell_labeled_with_options(label, command, false, false, mode, step)
}

fn run_shell_labeled_with_options(
    label: &str,
    command: &str,
    show_skip_hint: bool,
    include_command_on_error: bool,
    display_mode: ShellDisplayMode,
    step: Option<ShellStep>,
) -> Result<ShellRunStatus, ForgeError> {
    let command = command_for_current_user(command);
    let mut status_bar = StatusBar::new(label, step, show_skip_hint, display_mode);
    if show_skip_hint && !status_bar.is_active() {
        print_skip_hint(label);
    }
    let mut child = shell_command(&command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from(&command),
            source,
        })?;

    let output = Arc::new(Mutex::new(String::new()));
    let stdout_handle = child
        .stdout
        .take()
        .map(|stdout| collect_command_output(stdout, Arc::clone(&output)));
    let stderr_handle = child
        .stderr
        .take()
        .map(|stderr| collect_command_output(stderr, Arc::clone(&output)));

    let started = Instant::now();
    let mut next_notice = FIRST_PROGRESS_NOTICE_SECONDS;
    let status = loop {
        if user_requested_skip()? {
            terminate_child_process(&mut child);
            join_output_thread(stdout_handle, &command)?;
            join_output_thread(stderr_handle, &command)?;
            status_bar.finish();
            println!("已收到跳过指令，正在跳过当前安装：{label}");
            return Ok(ShellRunStatus::Skipped);
        }

        if let Some(status) = child.try_wait().map_err(|source| ForgeError::Io {
            path: PathBuf::from(&command),
            source,
        })? {
            break status;
        }

        let elapsed = started.elapsed().as_secs();
        status_bar.render(elapsed, &output);
        if elapsed >= next_notice {
            print_progress_notice(label, elapsed, &output);
            if show_skip_hint {
                print_skip_hint(label);
            }
            status_bar.render(elapsed, &output);
            next_notice = next_notice.saturating_mul(2);
        }

        thread::sleep(Duration::from_secs(1));
    };

    join_output_thread(stdout_handle, &command)?;
    join_output_thread(stderr_handle, &command)?;
    status_bar.finish();

    if status.success() {
        Ok(ShellRunStatus::Completed)
    } else {
        let output = output_snapshot(&output);
        let detail = if output.trim().is_empty() {
            "命令未输出错误详情".to_string()
        } else {
            output
        };
        if include_command_on_error {
            Err(ForgeError::Command(format!("{}\n{}", command, detail)))
        } else {
            Err(ForgeError::Command(detail))
        }
    }
}

pub(crate) fn run_shell_capture(command: &str) -> Result<String, ForgeError> {
    let output = shell_command(command)
        .output()
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from(command),
            source,
        })?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(if stdout.trim().is_empty() {
            stderr.to_string()
        } else {
            stdout.to_string()
        })
    } else {
        Err(ForgeError::Command(format!(
            "{}\n{}",
            command,
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

fn shell_command(command: &str) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else if command_exists("bash") {
        let mut cmd = Command::new("bash");
        cmd.arg("-lc").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn command_for_current_user(command: &str) -> String {
    if cfg!(target_os = "linux") && is_linux_root() {
        strip_sudo_from_apt_commands(command)
    } else {
        command.to_string()
    }
}

fn is_linux_root() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim() == "0")
        .unwrap_or(false)
}

fn strip_sudo_from_apt_commands(command: &str) -> String {
    command
        .replace("sudo apt-get ", "apt-get ")
        .replace("sudo apt ", "apt ")
        .replace("sudo -E apt-get ", "apt-get ")
        .replace("sudo -E apt ", "apt ")
}

fn collect_command_output<R>(mut reader: R, output: Arc<Mutex<String>>) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0; 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(size) => push_output(&output, &String::from_utf8_lossy(&buffer[..size])),
            }
        }
    })
}

fn push_output(output: &Arc<Mutex<String>>, text: &str) {
    let Ok(mut output) = output.lock() else {
        return;
    };
    output.push_str(text);
    if output.len() <= PROGRESS_OUTPUT_LIMIT {
        return;
    }
    let mut keep_from = output.len() - PROGRESS_OUTPUT_LIMIT;
    while !output.is_char_boundary(keep_from) {
        keep_from += 1;
    }
    output.drain(..keep_from);
}

fn print_progress_notice(label: &str, elapsed: u64, output: &Arc<Mutex<String>>) {
    let snapshot = output_snapshot(output);
    let progress = progress_notice_output(&snapshot);
    println!(
        "目前{label}的安装已经持续了{elapsed}秒，请注意，目前进度为（最多显示最近 {PROGRESS_NOTICE_MAX_LINES} 行）：\n{progress}"
    );
}

fn progress_notice_output(output: &str) -> String {
    let output = output.replace('\r', "\n");
    if output.trim().is_empty() {
        return "暂无命令输出".to_string();
    }

    let mut lines = output
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.len() > PROGRESS_NOTICE_MAX_LINES {
        lines = lines.split_off(lines.len() - PROGRESS_NOTICE_MAX_LINES);
    }
    lines.join("\n")
}

struct StatusBar {
    label: String,
    step: Option<ShellStep>,
    show_skip_hint: bool,
    active: bool,
}

impl StatusBar {
    fn new(
        label: &str,
        step: Option<ShellStep>,
        show_skip_hint: bool,
        mode: ShellDisplayMode,
    ) -> Self {
        let active = matches!(mode, ShellDisplayMode::StatusBar) && io::stdout().is_terminal();
        Self {
            label: label.to_string(),
            step,
            show_skip_hint,
            active,
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn render(&mut self, elapsed: u64, output: &Arc<Mutex<String>>) {
        if !self.active {
            return;
        }
        let snapshot = output_snapshot(output);
        let recent = progress_notice_output(&snapshot);
        let lines = status_bar_lines(
            &self.label,
            self.step,
            elapsed,
            self.show_skip_hint,
            &recent,
        );
        if draw_status_bar(&lines).is_err() {
            self.active = false;
        }
    }

    fn finish(&mut self) {
        if !self.active {
            return;
        }
        let _ = clear_status_bar();
        self.active = false;
    }
}

fn status_bar_lines(
    label: &str,
    step: Option<ShellStep>,
    elapsed: u64,
    show_skip_hint: bool,
    recent_output: &str,
) -> Vec<String> {
    let mut lines = vec!["──────────────── rsenvforge 安装状态 ────────────────".to_string()];
    if let Some(step) = step {
        lines.push(format!("Step {}/{}", step.current, step.total));
    }
    lines.extend([
        format!("当前组件：{label}"),
        format!("已运行：{elapsed} 秒"),
        "最近输出：".to_string(),
    ]);
    let reserved_lines = lines.len() + usize::from(show_skip_hint);
    let recent_line_limit = usize::from(STATUS_BAR_LINES).saturating_sub(reserved_lines);
    lines.extend(
        recent_output
            .lines()
            .take(recent_line_limit.min(PROGRESS_NOTICE_MAX_LINES))
            .map(|line| format!("  {line}")),
    );
    if show_skip_hint {
        lines.push("操作：输入 T 后回车跳过当前组件".to_string());
    }
    lines
}

fn draw_status_bar(lines: &[String]) -> io::Result<()> {
    let (_, height) = crossterm::terminal::size()?;
    let start_row = height.saturating_sub(STATUS_BAR_LINES).saturating_add(1);
    let mut out = io::stdout();
    write!(out, "\x1b7")?;
    for offset in 0..STATUS_BAR_LINES {
        let row = start_row.saturating_add(offset);
        write!(out, "\x1b[{row};1H\x1b[2K")?;
        if let Some(line) = lines.get(offset as usize) {
            write!(out, "{line}")?;
        }
    }
    write!(out, "\x1b8")?;
    out.flush()
}

fn clear_status_bar() -> io::Result<()> {
    let (_, height) = crossterm::terminal::size()?;
    let start_row = height.saturating_sub(STATUS_BAR_LINES).saturating_add(1);
    let mut out = io::stdout();
    write!(out, "\x1b7")?;
    for offset in 0..STATUS_BAR_LINES {
        let row = start_row.saturating_add(offset);
        write!(out, "\x1b[{row};1H\x1b[2K")?;
    }
    write!(out, "\x1b8")?;
    out.flush()
}

fn print_skip_hint(label: &str) {
    println!("安装过程中可输入 T 后回车，强制跳过当前工具：{label}");
}

fn user_requested_skip() -> Result<bool, ForgeError> {
    try_read_skip_request()
}

fn terminate_child_process(child: &mut std::process::Child) {
    let pid = child.id();
    terminate_process_tree(pid);
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn terminate_process_tree(pid: u32) {
    let _ = Command::new("taskkill")
        .arg("/PID")
        .arg(pid.to_string())
        .arg("/T")
        .arg("/F")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn terminate_process_tree(pid: u32) {
    let pid = pid.to_string();
    let _ = Command::new("pkill")
        .arg("-TERM")
        .arg("-P")
        .arg(&pid)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn output_snapshot(output: &Arc<Mutex<String>>) -> String {
    output
        .lock()
        .map(|output| output.clone())
        .unwrap_or_default()
}

fn join_output_thread(
    handle: Option<thread::JoinHandle<()>>,
    command: &str,
) -> Result<(), ForgeError> {
    if let Some(handle) = handle {
        handle
            .join()
            .map_err(|_| ForgeError::Command(format!("{command}\n读取命令输出时发生内部错误")))?;
    }
    Ok(())
}

pub(crate) fn command_status_text(command: &str) -> &'static str {
    if run_shell_capture(&format!("{command} --version")).is_ok() {
        "已找到"
    } else {
        "未找到"
    }
}

#[cfg(test)]
mod tests {
    use super::{
        progress_notice_output, status_bar_lines, strip_sudo_from_apt_commands, ShellStep,
        STATUS_BAR_LINES,
    };

    #[test]
    fn strips_sudo_only_from_apt_commands() {
        assert_eq!(
            strip_sudo_from_apt_commands("sudo apt-get update"),
            "apt-get update"
        );
        assert_eq!(
            strip_sudo_from_apt_commands("cd /tmp && sudo apt install -y cmake"),
            "cd /tmp && apt install -y cmake"
        );
        assert_eq!(
            strip_sudo_from_apt_commands("sudo systemctl restart demo"),
            "sudo systemctl restart demo"
        );
    }

    #[test]
    fn progress_notice_output_keeps_recent_five_lines() {
        let progress = progress_notice_output("line1\nline2\nline3\nline4\nline5\nline6\nline7\n");

        assert_eq!(progress, "line3\nline4\nline5\nline6\nline7");
        assert_eq!(progress.lines().count(), 5);
    }

    #[test]
    fn progress_notice_output_reports_empty_output() {
        assert_eq!(progress_notice_output("\n\r\n"), "暂无命令输出");
    }

    #[test]
    fn status_bar_lines_include_recent_output_and_skip_hint() {
        let lines = status_bar_lines(
            "demo-tool",
            Some(ShellStep {
                current: 2,
                total: 8,
            }),
            123,
            true,
            "a\nb\nc\nd\ne",
        );

        assert!(lines.iter().any(|line| line.contains("demo-tool")));
        assert!(lines.iter().any(|line| line.contains("Step 2/8")));
        assert!(lines.iter().any(|line| line.contains("123 秒")));
        assert!(lines.iter().any(|line| line.contains("输入 T 后回车")));
        assert!(lines.len() <= usize::from(STATUS_BAR_LINES));
        assert_eq!(
            lines.last().map(String::as_str),
            Some("操作：输入 T 后回车跳过当前组件")
        );
        assert_eq!(
            lines.iter().filter(|line| line.starts_with("  ")).count(),
            5
        );
    }
}
