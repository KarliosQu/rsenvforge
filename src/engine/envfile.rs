use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::error::ForgeError;
use super::models::InstallConfig;
use super::util::home_dir;

pub(crate) fn apply_install_start_environment(config: &InstallConfig) -> Result<(), ForgeError> {
    write_cargo_config_if_empty(&cargo_config_path(), &config.environment.cargo_config)?;
    if cfg!(target_os = "linux") {
        append_lines_if_missing(&home_dir().join(".bashrc"), &config.environment.bashrc)?;
    }
    Ok(())
}

pub(crate) fn apply_after_rust_install_environment(
    config: &InstallConfig,
) -> Result<(), ForgeError> {
    write_cargo_config_if_empty(&cargo_config_path(), &config.environment.cargo_config)?;
    if cfg!(target_os = "linux") {
        append_lines_if_missing(&home_dir().join(".bashrc"), &config.environment.bashrc)?;
    }
    Ok(())
}

fn cargo_config_path() -> PathBuf {
    env::var("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".cargo"))
        .join("config.toml")
}

fn write_cargo_config_if_empty(path: &Path, lines: &[String]) -> Result<(), ForgeError> {
    if lines.is_empty() {
        return Ok(());
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    if !existing.trim().is_empty() {
        println!(
            "Cargo config 已存在且非空，跳过自动写入：{}",
            path.display()
        );
        return Ok(());
    }
    write_lines(path, lines)?;
    println!("已写入 Cargo config：{}", path.display());
    Ok(())
}

fn append_lines_if_missing(path: &Path, lines: &[String]) -> Result<(), ForgeError> {
    if lines.is_empty() {
        return Ok(());
    }
    let mut existing = fs::read_to_string(path).unwrap_or_default();
    let missing = lines
        .iter()
        .filter(|line| !line.trim().is_empty() && !existing.contains(line.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        println!("bashrc 已包含 rsenvforge 环境配置：{}", path.display());
        return Ok(());
    }
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str("\n# rsenvforge environment\n");
    existing.push_str(&missing.join("\n"));
    existing.push('\n');
    write_text(path, &existing)?;
    println!("已更新 bashrc：{}", path.display());
    Ok(())
}

fn write_lines(path: &Path, lines: &[String]) -> Result<(), ForgeError> {
    let mut contents = lines.join("\n");
    contents.push('\n');
    write_text(path, &contents)
}

fn write_text(path: &Path, contents: &str) -> Result<(), ForgeError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ForgeError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, contents).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{append_lines_if_missing, write_cargo_config_if_empty};
    use std::fs;

    #[test]
    fn writes_cargo_config_when_missing_or_empty() {
        let temp =
            std::env::temp_dir().join(format!("rsenvforge-envfile-cargo-{}", std::process::id()));
        let path = temp.join("config.toml");
        let lines = vec!["[net]".to_string(), "git-fetch-with-cli = true".to_string()];

        write_cargo_config_if_empty(&path, &lines).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[net]\ngit-fetch-with-cli = true\n"
        );

        fs::write(&path, "[http]\nproxy = \"http://127.0.0.1:7890\"\n").unwrap();
        write_cargo_config_if_empty(&path, &lines).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[http]\nproxy = \"http://127.0.0.1:7890\"\n"
        );

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn appends_missing_bashrc_lines() {
        let temp =
            std::env::temp_dir().join(format!("rsenvforge-envfile-bashrc-{}", std::process::id()));
        let path = temp.join(".bashrc");
        fs::create_dir_all(&temp).unwrap();
        fs::write(&path, "export DEMO=1\n").unwrap();

        append_lines_if_missing(&path, &[". \"$HOME/.cargo/env\"".to_string()]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("export DEMO=1"));
        assert!(contents.contains(". \"$HOME/.cargo/env\""));

        append_lines_if_missing(&path, &[". \"$HOME/.cargo/env\"".to_string()]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents.matches(". \"$HOME/.cargo/env\"").count(), 1);

        fs::remove_dir_all(temp).unwrap();
    }
}
