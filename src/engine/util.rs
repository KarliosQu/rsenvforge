use std::env;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn home_dir() -> PathBuf {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(crate) fn exe_name(bin: &str) -> String {
    if cfg!(windows) && !bin.ends_with(".exe") {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    }
}

pub(crate) fn source_name(source: &str) -> String {
    source
        .trim_end_matches(".git")
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("manual")
        .to_string()
}

pub(crate) fn looks_like_git(source: &str) -> bool {
    source.ends_with(".git")
        || source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("ssh://")
        || source.starts_with("git@")
}

pub(crate) fn resolve_source(source: &str, base_dir: &Path) -> String {
    let path = PathBuf::from(source);
    if source.is_empty() || path.is_absolute() || looks_like_git(source) {
        source.to_string()
    } else {
        base_dir.join(path).display().to_string()
    }
}

pub(crate) fn shell_quote(path: &Path) -> String {
    shell_quote_str(&path.display().to_string())
}

pub(crate) fn shell_quote_str(value: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

pub(crate) fn first_line(value: String) -> String {
    value.lines().next().unwrap_or("").trim().to_string()
}

pub(crate) fn join_names<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let names = names.collect::<Vec<_>>();
    if names.is_empty() {
        "无".to_string()
    } else {
        names.join(", ")
    }
}

pub(crate) fn fnv1a(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn strip_comment(line: &str) -> &str {
    let mut in_quote = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '"' => in_quote = !in_quote,
            '#' if !in_quote => return &line[..index],
            _ => {}
        }
    }
    line
}

pub(crate) fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}
