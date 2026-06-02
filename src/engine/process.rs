use std::path::PathBuf;
use std::process::Command;

use super::error::ForgeError;

pub(crate) fn run_shell(command: &str) -> Result<(), ForgeError> {
    let output = shell_command(command)
        .output()
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from(command),
            source,
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ForgeError::Command(format!(
            "{}\n{}",
            command,
            String::from_utf8_lossy(&output.stderr)
        )))
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
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

pub(crate) fn command_status_text(command: &str) -> &'static str {
    if run_shell_capture(&format!("{command} --version")).is_ok() {
        "已找到"
    } else {
        "未找到"
    }
}
