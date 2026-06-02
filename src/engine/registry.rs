use std::path::PathBuf;

use super::error::ForgeError;
use super::fsutil::{create_dir_all, read_to_string, write_file};
use super::models::{InstallKind, RegistryEntry};
use super::paths::registry_path;

pub fn read_registry() -> Result<Vec<RegistryEntry>, ForgeError> {
    let path = registry_path();
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for (line_number, line) in read_to_string(&path)?.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() != 6 {
            return Err(ForgeError::Parse(format!(
                "{}:{}：registry 行必须有 6 列",
                path.display(),
                line_number + 1
            )));
        }
        let kind = InstallKind::parse(columns[1])?;
        let targets = if columns[4].is_empty() {
            Vec::new()
        } else {
            columns[4].split('|').map(PathBuf::from).collect()
        };
        let installed_at = columns[5].parse::<u64>().map_err(|error| {
            ForgeError::Parse(format!(
                "{}:{}：时间戳无效：{error}",
                path.display(),
                line_number + 1
            ))
        })?;
        entries.push(RegistryEntry {
            name: columns[0].to_string(),
            kind,
            source: columns[2].to_string(),
            profile: columns[3].to_string(),
            targets,
            installed_at,
        });
    }
    Ok(entries)
}

pub fn write_registry(entries: &[RegistryEntry]) -> Result<(), ForgeError> {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let mut output = String::new();
    for entry in entries {
        let targets = entry
            .targets
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("|");
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            entry.name,
            entry.kind.as_str(),
            entry.source,
            entry.profile,
            targets,
            entry.installed_at
        ));
    }
    write_file(&path, &output)
}

pub(crate) fn append_registry(entry: RegistryEntry) -> Result<(), ForgeError> {
    let mut entries = read_registry()?;
    entries.push(entry);
    write_registry(&entries)
}
