use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::util::home_dir;

const PROXY_KEYS: [&str; 4] = ["http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY"];

pub fn proxy_report() -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("代理检查：".to_string());
    lines.extend(environment_proxy_lines());
    if cfg!(windows) {
        lines.push("Windows 环境：已检查环境变量和 Cargo config.toml。".to_string());
    } else {
        lines.extend(bashrc_proxy_lines(&home_dir().join(".bashrc")));
    }
    lines.extend(cargo_config_proxy_lines());
    lines
}

pub fn print_proxy_report() {
    for line in proxy_report() {
        println!("{line}");
    }
}

fn environment_proxy_lines() -> Vec<String> {
    vec![
        format!(
            "  http_proxy：{}",
            env_proxy_value("http_proxy", "HTTP_PROXY")
        ),
        format!(
            "  https_proxy：{}",
            env_proxy_value("https_proxy", "HTTPS_PROXY")
        ),
    ]
}

fn env_proxy_value(lower: &str, upper: &str) -> String {
    env::var(lower)
        .or_else(|_| env::var(upper))
        .map(|value| mask_proxy_secrets(&value))
        .unwrap_or_else(|_| "未设置".to_string())
}

fn bashrc_proxy_lines(path: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    match fs::read_to_string(path) {
        Ok(contents) => {
            let matches = proxy_matches(&contents);
            if matches.is_empty() {
                lines.push(format!("  ~/.bashrc：未找到代理配置（{}）", path.display()));
            } else {
                lines.push(format!("  ~/.bashrc：找到 {} 行代理配置", matches.len()));
                for value in matches {
                    lines.push(format!("    {}", mask_proxy_secrets(&value)));
                }
            }
        }
        Err(_) => lines.push(format!("  ~/.bashrc：未找到文件（{}）", path.display())),
    }
    lines
}

fn cargo_config_proxy_lines() -> Vec<String> {
    let mut lines = Vec::new();
    let candidates = cargo_config_candidates();
    let Some(path) = candidates.iter().find(|path| path.exists()) else {
        let checked = candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("；");
        lines.push(format!("  Cargo config：未找到（检查路径：{checked}）"));
        return lines;
    };

    match fs::read_to_string(path) {
        Ok(contents) => {
            let matches = proxy_matches(&contents);
            if matches.is_empty() {
                lines.push(format!(
                    "  Cargo config：存在但未找到 proxy 配置（{}）",
                    path.display()
                ));
            } else {
                lines.push(format!(
                    "  Cargo config：找到 {} 行 proxy 配置（{}）",
                    matches.len(),
                    path.display()
                ));
                for value in matches {
                    lines.push(format!("    {}", mask_proxy_secrets(&value)));
                }
            }
        }
        Err(error) => lines.push(format!(
            "  Cargo config：无法读取 {}：{error}",
            path.display()
        )),
    }
    lines
}

fn cargo_config_candidates() -> Vec<PathBuf> {
    let cargo_home = env::var("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".cargo"));
    vec![cargo_home.join("config.toml"), cargo_home.join("config")]
}

fn proxy_matches(contents: &str) -> Vec<String> {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("proxy")
                || PROXY_KEYS
                    .iter()
                    .any(|key| lower.contains(&key.to_ascii_lowercase()))
        })
        .map(str::to_string)
        .collect()
}

fn mask_proxy_secrets(value: &str) -> String {
    let Some(scheme_end) = value.find("://") else {
        return value.to_string();
    };
    let auth_start = scheme_end + 3;
    let Some(at_offset) = value[auth_start..].find('@') else {
        return value.to_string();
    };
    let at = auth_start + at_offset;
    let Some(host_end_offset) = value[auth_start..].find(['/', '?', '#', ' ', '"', '\'']) else {
        return format!("{}***@{}", &value[..auth_start], &value[at + 1..]);
    };
    let host_end = auth_start + host_end_offset;
    if at > host_end {
        return value.to_string();
    }
    format!("{}***@{}", &value[..auth_start], &value[at + 1..])
}

#[cfg(test)]
mod tests {
    use super::{mask_proxy_secrets, proxy_matches};

    #[test]
    fn masks_proxy_credentials() {
        assert_eq!(
            mask_proxy_secrets("http://user:pass@127.0.0.1:7890"),
            "http://***@127.0.0.1:7890"
        );
        assert_eq!(
            mask_proxy_secrets("proxy = \"http://user:pass@example.com:8080\""),
            "proxy = \"http://***@example.com:8080\""
        );
    }

    #[test]
    fn finds_proxy_lines() {
        let matches = proxy_matches(
            r#"
            # http_proxy=http://ignored
            [http]
            proxy = "http://127.0.0.1:7890"
            export https_proxy=http://127.0.0.1:7891
            "#,
        );
        assert_eq!(matches.len(), 2);
    }
}
