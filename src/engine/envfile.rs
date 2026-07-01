use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::error::ForgeError;
use super::models::InstallConfig;
use super::util::home_dir;

pub(crate) fn apply_install_start_environment(config: &InstallConfig) -> Result<(), ForgeError> {
    write_cargo_config_if_empty(&cargo_config_path(), &config.environment.cargo_config)?;
    append_lines_if_missing("npmrc", &npmrc_path(), &config.environment.npmrc)?;
    if cfg!(target_os = "linux") {
        append_lines_if_missing(
            "bashrc",
            &home_dir().join(".bashrc"),
            &config.environment.bashrc,
        )?;
    }
    Ok(())
}

pub(crate) fn apply_after_rust_install_environment(
    config: &InstallConfig,
) -> Result<(), ForgeError> {
    write_cargo_config_if_empty(&cargo_config_path(), &config.environment.cargo_config)?;
    append_lines_if_missing("npmrc", &npmrc_path(), &config.environment.npmrc)?;
    if cfg!(target_os = "linux") {
        append_lines_if_missing(
            "bashrc",
            &home_dir().join(".bashrc"),
            &config.environment.bashrc,
        )?;
    }
    refresh_rust_process_environment();
    Ok(())
}

pub(crate) fn refresh_rust_process_environment() {
    let cargo_home = env::var_os("CARGO_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".cargo"));
    let rustup_home = env::var_os("RUSTUP_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".rustup"));

    env::set_var("CARGO_HOME", &cargo_home);
    env::set_var("RUSTUP_HOME", &rustup_home);

    let added = prepend_process_path([cargo_home.join("bin")], true);
    if !added.is_empty() {
        println!("已刷新当前安装进程 Rust 环境：{}", display_paths(&added));
    }
}

pub(crate) fn refresh_node_process_environment() {
    let mut candidates = Vec::new();
    if cfg!(windows) {
        push_env_path(&mut candidates, "NVM_HOME");
        push_env_path(&mut candidates, "NVM_SYMLINK");
        if let Some(path) = env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(path).join("nvm"));
        }
        if let Some(path) = env::var_os("APPDATA") {
            candidates.push(PathBuf::from(path).join("nvm"));
        }
        if let Some(path) = env::var_os("ProgramFiles") {
            candidates.push(PathBuf::from(path).join("nodejs"));
        }
        if let Some(path) = env::var_os("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(path).join("nodejs"));
        }
    } else {
        let nvm_dir = env::var_os("NVM_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".nvm"));
        candidates.push(nvm_dir.join("current").join("bin"));
        candidates.extend(discover_nvm_node_bins(&nvm_dir));
    }

    let added = prepend_process_path(candidates, false);
    if !added.is_empty() {
        println!("已刷新当前安装进程 Node.js 环境：{}", display_paths(&added));
    }
}

fn push_env_path(target: &mut Vec<PathBuf>, name: &str) {
    if let Some(value) = env::var_os(name).filter(|value| !value.is_empty()) {
        target.push(PathBuf::from(value));
    }
}

fn discover_nvm_node_bins(nvm_dir: &Path) -> Vec<PathBuf> {
    let versions_dir = nvm_dir.join("versions").join("node");
    let Ok(entries) = fs::read_dir(&versions_dir) else {
        return Vec::new();
    };
    let mut bins = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|file_type| file_type.is_dir())?;
            Some(entry.path().join("bin"))
        })
        .collect::<Vec<_>>();
    bins.sort();
    bins.reverse();
    bins
}

fn prepend_process_path(
    paths: impl IntoIterator<Item = PathBuf>,
    include_missing: bool,
) -> Vec<PathBuf> {
    let mut added: Vec<PathBuf> = Vec::new();
    let current_path = env::var_os("PATH").unwrap_or_default();
    let mut existing = env::split_paths(&current_path).collect::<Vec<_>>();

    for path in paths {
        if path.as_os_str().is_empty() || (!include_missing && !path.exists()) {
            continue;
        }
        if existing.iter().any(|existing| paths_equal(existing, &path))
            || added.iter().any(|existing| paths_equal(existing, &path))
        {
            continue;
        }
        added.push(path);
    }

    if added.is_empty() {
        return added;
    }

    let mut new_paths = added.clone();
    new_paths.append(&mut existing);
    if let Ok(joined) = env::join_paths(new_paths) {
        env::set_var("PATH", joined);
        added
    } else {
        Vec::new()
    }
}

fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn cargo_config_path() -> PathBuf {
    env::var("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".cargo"))
        .join("config.toml")
}

fn npmrc_path() -> PathBuf {
    home_dir().join(".npmrc")
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

fn append_lines_if_missing(label: &str, path: &Path, lines: &[String]) -> Result<(), ForgeError> {
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
        println!("{label} 已包含 rsenvforge 环境配置：{}", path.display());
        return Ok(());
    }
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str("\n# rsenvforge environment\n");
    existing.push_str(&missing.join("\n"));
    existing.push('\n');
    write_text(path, &existing)?;
    println!("已更新 {label}：{}", path.display());
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
    use super::{
        append_lines_if_missing, refresh_node_process_environment,
        refresh_rust_process_environment, write_cargo_config_if_empty,
    };
    use std::sync::Mutex;
    use std::{env, ffi::OsString, fs};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn restore_env(name: &str, value: Option<OsString>) {
        match value {
            Some(value) => env::set_var(name, value),
            None => env::remove_var(name),
        }
    }

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

        append_lines_if_missing("bashrc", &path, &[". \"$HOME/.cargo/env\"".to_string()]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("export DEMO=1"));
        assert!(contents.contains(". \"$HOME/.cargo/env\""));

        append_lines_if_missing("bashrc", &path, &[". \"$HOME/.cargo/env\"".to_string()]).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents.matches(". \"$HOME/.cargo/env\"").count(), 1);

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn appends_missing_npmrc_lines() {
        let temp =
            std::env::temp_dir().join(format!("rsenvforge-envfile-npmrc-{}", std::process::id()));
        let path = temp.join(".npmrc");
        fs::create_dir_all(&temp).unwrap();
        fs::write(&path, "strict-ssl=false\n").unwrap();

        append_lines_if_missing(
            "npmrc",
            &path,
            &["registry=https://mirror.com/npm/".to_string()],
        )
        .unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("strict-ssl=false"));
        assert!(contents.contains("registry=https://mirror.com/npm/"));

        append_lines_if_missing(
            "npmrc",
            &path,
            &["registry=https://mirror.com/npm/".to_string()],
        )
        .unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents.matches("registry=https://mirror.com/npm/").count(),
            1
        );

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn refreshes_rust_environment_for_current_process() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp =
            std::env::temp_dir().join(format!("rsenvforge-envfile-rust-{}", std::process::id()));
        let cargo_home = temp.join(".cargo");
        let old_userprofile = env::var_os("USERPROFILE");
        let old_home = env::var_os("HOME");
        let old_cargo_home = env::var_os("CARGO_HOME");
        let old_rustup_home = env::var_os("RUSTUP_HOME");
        let old_path = env::var_os("PATH");

        env::set_var("USERPROFILE", &temp);
        env::set_var("HOME", &temp);
        env::remove_var("CARGO_HOME");
        env::remove_var("RUSTUP_HOME");
        env::set_var("PATH", "");

        refresh_rust_process_environment();

        assert_eq!(env::var_os("CARGO_HOME").unwrap(), cargo_home.as_os_str());
        assert_eq!(
            env::var_os("RUSTUP_HOME").unwrap(),
            temp.join(".rustup").as_os_str()
        );
        let paths = env::split_paths(&env::var_os("PATH").unwrap()).collect::<Vec<_>>();
        assert!(paths.iter().any(|path| path == &cargo_home.join("bin")));

        restore_env("USERPROFILE", old_userprofile);
        restore_env("HOME", old_home);
        restore_env("CARGO_HOME", old_cargo_home);
        restore_env("RUSTUP_HOME", old_rustup_home);
        restore_env("PATH", old_path);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn refreshes_node_environment_for_current_process() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp =
            std::env::temp_dir().join(format!("rsenvforge-envfile-node-{}", std::process::id()));
        let expected_path = if cfg!(windows) {
            temp.join("nvm")
        } else {
            temp.join(".nvm")
                .join("versions")
                .join("node")
                .join("v20.17.0")
                .join("bin")
        };
        fs::create_dir_all(&expected_path).unwrap();
        let old_userprofile = env::var_os("USERPROFILE");
        let old_home = env::var_os("HOME");
        let old_nvm_dir = env::var_os("NVM_DIR");
        let old_nvm_home = env::var_os("NVM_HOME");
        let old_path = env::var_os("PATH");

        env::set_var("USERPROFILE", &temp);
        env::set_var("HOME", &temp);
        if cfg!(windows) {
            env::set_var("NVM_HOME", &expected_path);
        } else {
            env::remove_var("NVM_DIR");
        }
        env::set_var("PATH", "");

        refresh_node_process_environment();

        let paths = env::split_paths(&env::var_os("PATH").unwrap()).collect::<Vec<_>>();
        assert!(paths.iter().any(|path| path == &expected_path));

        restore_env("USERPROFILE", old_userprofile);
        restore_env("HOME", old_home);
        restore_env("NVM_DIR", old_nvm_dir);
        restore_env("NVM_HOME", old_nvm_home);
        restore_env("PATH", old_path);
        let _ = fs::remove_dir_all(temp);
    }
}
