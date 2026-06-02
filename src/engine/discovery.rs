use std::ffi::OsStr;
use std::path::Path;

use super::constants::SKILL_FILE;
use super::error::ForgeError;
use super::fsutil::{read_dir, read_to_string};
use super::models::{CrateCandidate, SkillCandidate};
use super::util::{strip_comment, unquote};

pub fn discover_skills(root: &Path) -> Result<Vec<SkillCandidate>, ForgeError> {
    let mut candidates = Vec::new();
    let root_skill = root.join(SKILL_FILE);
    if root_skill.is_file() {
        candidates.push(skill_candidate(root));
    }

    for base in [root.join("skills"), root.join(".claude").join("skills")] {
        if base.is_dir() {
            for entry in read_dir(&base)? {
                let path = entry?;
                if path.join(SKILL_FILE).is_file() {
                    candidates.push(skill_candidate(&path));
                }
            }
        }
    }

    candidates.sort_by(|left, right| left.name.cmp(&right.name).then(left.path.cmp(&right.path)));
    candidates.dedup_by(|left, right| left.path == right.path);
    Ok(candidates)
}

pub fn discover_crates(root: &Path) -> Result<Vec<CrateCandidate>, ForgeError> {
    let mut candidates = Vec::new();
    let root_manifest = root.join("Cargo.toml");
    if root_manifest.is_file() {
        candidates.push(crate_candidate(&root_manifest)?);
    }

    let crates_dir = root.join("crates");
    if crates_dir.is_dir() {
        for entry in read_dir(&crates_dir)? {
            let path = entry?;
            let manifest = path.join("Cargo.toml");
            if manifest.is_file() {
                candidates.push(crate_candidate(&manifest)?);
            }
        }
    }

    candidates.sort_by(|left, right| {
        left.package_name
            .cmp(&right.package_name)
            .then(left.manifest_path.cmp(&right.manifest_path))
    });
    candidates.dedup_by(|left, right| left.manifest_path == right.manifest_path);
    Ok(candidates)
}

fn crate_candidate(manifest: &Path) -> Result<CrateCandidate, ForgeError> {
    let content = read_to_string(manifest)?;
    let package_name = parse_package_name(&content).unwrap_or_else(|| {
        manifest
            .parent()
            .and_then(Path::file_name)
            .and_then(OsStr::to_str)
            .unwrap_or("unnamed")
            .to_string()
    });
    let mut bins = parse_bins(&content);
    if bins.is_empty()
        && manifest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("src")
            .join("main.rs")
            .is_file()
    {
        bins.push(package_name.clone());
    }
    Ok(CrateCandidate {
        package_name,
        manifest_path: manifest.to_path_buf(),
        bins,
    })
}

fn parse_package_name(content: &str) -> Option<String> {
    let mut in_package = false;
    for raw_line in content.lines() {
        let line = strip_comment(raw_line).trim();
        if line == "[package]" {
            in_package = true;
            continue;
        }
        if line.starts_with('[') {
            in_package = false;
        }
        if in_package {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "name" {
                    return parse_string(value.trim()).ok();
                }
            }
        }
    }
    None
}

fn parse_string(value: &str) -> Result<String, ForgeError> {
    let value = value.trim();
    if let Some(stripped) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        Ok(stripped.to_string())
    } else {
        Err(ForgeError::Parse(format!("字符串必须使用双引号：{value}")))
    }
}

fn parse_bins(content: &str) -> Vec<String> {
    let mut bins = Vec::new();
    let mut in_bin = false;
    for raw_line in content.lines() {
        let line = strip_comment(raw_line).trim();
        if line == "[[bin]]" {
            in_bin = true;
            continue;
        }
        if line.starts_with('[') {
            in_bin = false;
        }
        if in_bin {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "name" {
                    if let Ok(name) = parse_string(value.trim()) {
                        bins.push(name);
                    }
                }
            }
        }
    }
    bins
}

fn skill_candidate(path: &Path) -> SkillCandidate {
    let fallback = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("unnamed")
        .to_string();
    let name = read_to_string(&path.join(SKILL_FILE))
        .ok()
        .and_then(|content| parse_frontmatter_name(&content))
        .unwrap_or(fallback);
    SkillCandidate {
        name,
        path: path.to_path_buf(),
    }
}

fn parse_frontmatter_name(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            return None;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.trim() == "name" {
                return Some(unquote(value.trim()).to_string());
            }
        }
    }
    None
}
