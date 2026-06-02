use std::env;
use std::path::PathBuf;

use super::constants::{CONFIG_FILE, REGISTRY_FILE};
use super::util::home_dir;

pub fn app_home() -> PathBuf {
    if let Ok(path) = env::var("RSENVFORGE_HOME") {
        return PathBuf::from(path);
    }
    if cfg!(windows) {
        if let Ok(path) = env::var("LOCALAPPDATA") {
            return PathBuf::from(path).join("rsenvforge");
        }
    }
    if let Ok(path) = env::var("XDG_DATA_HOME") {
        return PathBuf::from(path).join("rsenvforge");
    }
    home_dir().join(".local").join("share").join("rsenvforge")
}

pub fn config_dir() -> PathBuf {
    if let Ok(path) = env::var("RSENVFORGE_CONFIG_DIR") {
        return PathBuf::from(path);
    }
    if cfg!(windows) {
        if let Ok(path) = env::var("APPDATA") {
            return PathBuf::from(path).join("rsenvforge");
        }
    }
    if let Ok(path) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(path).join("rsenvforge");
    }
    home_dir().join(".config").join("rsenvforge")
}

pub fn manifest_config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(CONFIG_FILE)
}

pub fn managed_bin_dir() -> PathBuf {
    env::var("RSENVFORGE_BIN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| app_home().join("bin"))
}

pub fn registry_path() -> PathBuf {
    app_home().join(REGISTRY_FILE)
}
