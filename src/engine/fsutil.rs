use std::fs;
use std::path::{Path, PathBuf};

use super::error::ForgeError;

pub(crate) fn copy_dir(source: &Path, target: &Path, force: bool) -> Result<(), ForgeError> {
    if target.exists() {
        if !force {
            return Err(ForgeError::Config(format!(
                "{} 已存在，请添加 --force 覆盖",
                target.display()
            )));
        }
        remove_dir_all(target)?;
    }
    create_dir_all(target)?;
    for entry in read_dir(source)? {
        let path = entry?;
        let file_name = path
            .file_name()
            .ok_or_else(|| ForgeError::Config(format!("路径无效：{}", path.display())))?;
        let dest = target.join(file_name);
        if path.is_dir() {
            copy_dir(&path, &dest, force)?;
        } else {
            copy_file(&path, &dest)?;
        }
    }
    Ok(())
}

pub(crate) fn copy_file(source: &Path, target: &Path) -> Result<(), ForgeError> {
    if let Some(parent) = target.parent() {
        create_dir_all(parent)?;
    }
    fs::copy(source, target).map_err(|source_error| ForgeError::Io {
        path: source.to_path_buf(),
        source: source_error,
    })?;
    Ok(())
}

pub(crate) fn read_dir(path: &Path) -> Result<Vec<Result<PathBuf, ForgeError>>, ForgeError> {
    let entries = fs::read_dir(path)
        .map_err(|source| ForgeError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|source| ForgeError::Io {
                    path: path.to_path_buf(),
                    source,
                })
        })
        .collect();
    Ok(entries)
}

pub(crate) fn create_dir_all(path: &Path) -> Result<(), ForgeError> {
    fs::create_dir_all(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn remove_dir_all(path: &Path) -> Result<(), ForgeError> {
    fs::remove_dir_all(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn remove_file(path: &Path) -> Result<(), ForgeError> {
    fs::remove_file(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn read_to_string(path: &Path) -> Result<String, ForgeError> {
    fs::read_to_string(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn write_file(path: &Path, contents: &str) -> Result<(), ForgeError> {
    fs::write(path, contents).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}
