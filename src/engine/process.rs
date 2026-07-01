use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::error::ForgeError;
use super::input::try_read_skip_request;

const FIRST_PROGRESS_NOTICE_SECONDS: u64 = 120;
const PROGRESS_OUTPUT_LIMIT: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellRunStatus {
    Completed,
    Skipped,
}

pub(crate) fn run_shell_labeled(label: &str, command: &str) -> Result<ShellRunStatus, ForgeError> {
    run_shell_labeled_with_options(label, command, true, true)
}

pub(crate) fn run_shell_labeled_quiet(
    label: &str,
    command: &str,
) -> Result<ShellRunStatus, ForgeError> {
    run_shell_labeled_with_options(label, command, false, false)
}

fn run_shell_labeled_with_options(
    label: &str,
    command: &str,
    show_skip_hint: bool,
    include_command_on_error: bool,
) -> Result<ShellRunStatus, ForgeError> {
    let command = command_for_current_user(command);
    if show_skip_hint {
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
        if elapsed >= next_notice {
            print_progress_notice(label, elapsed, &output);
            if show_skip_hint {
                print_skip_hint(label);
            }
            next_notice = next_notice.saturating_mul(2);
        }

        thread::sleep(Duration::from_secs(1));
    };

    join_output_thread(stdout_handle, &command)?;
    join_output_thread(stderr_handle, &command)?;

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
    let progress = if snapshot.trim().is_empty() {
        "暂无命令输出".to_string()
    } else {
        snapshot
    };
    println!("目前{label}的安装已经持续了{elapsed}秒，请注意，目前进度为：\n{progress}");
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
    use super::strip_sudo_from_apt_commands;

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
}
