use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::error::ForgeError;
use super::fsutil::{create_dir_all, read_to_string, remove_dir_all, write_file};
use super::models::{AptMirrorDef, InstallConfig};

const DEFAULT_SOURCE_FILE: &str = "/etc/apt/sources.list.d/rsenvforge.sources";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptMirrorPreview {
    pub distribution: String,
    pub codename: String,
    pub architecture: String,
    pub source_file: PathBuf,
    pub source_contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AptSystem {
    distribution: String,
    codename: String,
    architecture: String,
}

pub fn apt_mirror_preview(config: &InstallConfig) -> Result<AptMirrorPreview, ForgeError> {
    let system = detect_apt_system()?;
    build_preview(&config.apt_mirror, &system)
}

pub fn check_apt_mirror(preview: &AptMirrorPreview) -> Result<(), ForgeError> {
    let temp_dir = temp_dir("apt-mirror-check")?;
    let source_file = temp_dir.join("rsenvforge.sources");
    let lists_dir = temp_dir.join("lists");
    let result = (|| {
        create_dir_all(&lists_dir.join("partial"))?;
        write_file(&source_file, &preview.source_contents)?;

        let output = Command::new("apt-get")
            .arg("-o")
            .arg(format!("Dir::Etc::sourcelist={}", source_file.display()))
            .arg("-o")
            .arg("Dir::Etc::sourceparts=-")
            .arg("-o")
            .arg(format!("Dir::State::lists={}", lists_dir.display()))
            .arg("-o")
            .arg("APT::Get::List-Cleanup=0")
            .arg("update")
            .output()
            .map_err(|source| ForgeError::Io {
                path: PathBuf::from("apt-get"),
                source,
            })?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(ForgeError::Command(format!(
            "临时 APT 源验证失败：{}",
            if stderr.is_empty() { stdout } else { stderr }
        )))
    })();
    let cleanup = remove_dir_all(&temp_dir);
    if let Err(error) = cleanup {
        eprintln!(
            "提示：无法删除 APT 验证临时目录 {}：{error}",
            temp_dir.display()
        );
    }
    result
}

pub fn apply_apt_mirror(preview: &AptMirrorPreview) -> Result<(), ForgeError> {
    let parent = preview.source_file.parent().ok_or_else(|| {
        ForgeError::Config(format!(
            "APT 源文件路径无效：{}",
            preview.source_file.display()
        ))
    })?;
    create_dir_all(parent)?;
    write_file(&preview.source_file, &preview.source_contents)
}

fn detect_apt_system() -> Result<AptSystem, ForgeError> {
    if cfg!(windows) {
        return Err(ForgeError::Config(
            "APT 镜像配置仅支持 Linux；请在 WSL 或 Linux 系统内运行 rsenvforge".to_string(),
        ));
    }

    let os_release = read_to_string(&PathBuf::from("/etc/os-release"))?;
    let values = parse_os_release(&os_release);
    let distribution = values
        .get("ID")
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ForgeError::Config("/etc/os-release 缺少 ID".to_string()))?;
    let codename = values
        .get("VERSION_CODENAME")
        .or_else(|| values.get("UBUNTU_CODENAME"))
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ForgeError::Config(
                "/etc/os-release 缺少 VERSION_CODENAME 或 UBUNTU_CODENAME，无法选择 APT suite"
                    .to_string(),
            )
        })?;
    let output = Command::new("dpkg")
        .arg("--print-architecture")
        .output()
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from("dpkg"),
            source,
        })?;
    if !output.status.success() {
        return Err(ForgeError::Command(
            "执行 dpkg --print-architecture 失败，当前系统可能不是 Debian/Ubuntu 系列".to_string(),
        ));
    }
    let architecture = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if architecture.is_empty() {
        return Err(ForgeError::Command(
            "dpkg --print-architecture 未返回架构信息".to_string(),
        ));
    }

    Ok(AptSystem {
        distribution,
        codename,
        architecture,
    })
}

fn build_preview(
    mirror: &AptMirrorDef,
    system: &AptSystem,
) -> Result<AptMirrorPreview, ForgeError> {
    let uri = expand_required(mirror.uri.as_deref(), "uri", system)?;
    if !(uri.starts_with("http://") || uri.starts_with("https://")) {
        return Err(ForgeError::Config(
            "apt_mirror.uri 必须以 http:// 或 https:// 开头".to_string(),
        ));
    }
    validate_scalar("apt_mirror.uri", &uri)?;
    let suites = expand_list(&mirror.suites, "suites", system)?;
    let components = expand_list(&mirror.components, "components", system)?;
    let architectures = if mirror.architectures.is_empty() {
        vec![system.architecture.clone()]
    } else {
        expand_list(&mirror.architectures, "architectures", system)?
    };
    let signed_by = mirror
        .signed_by
        .as_deref()
        .map(|value| expand_template(value, system))
        .transpose()?;
    if let Some(value) = &signed_by {
        validate_scalar("apt_mirror.signed_by", value)?;
    }
    let source_file = mirror.source_file.as_deref().unwrap_or(DEFAULT_SOURCE_FILE);
    if !source_file.starts_with('/') {
        return Err(ForgeError::Config(
            "apt_mirror.source_file 必须是 Linux 绝对路径".to_string(),
        ));
    }

    let mut contents = format!(
        "Types: deb\nURIs: {uri}\nSuites: {}\nComponents: {}\nArchitectures: {}\n",
        suites.join(" "),
        components.join(" "),
        architectures.join(" ")
    );
    if let Some(signed_by) = signed_by {
        contents.push_str(&format!("Signed-By: {signed_by}\n"));
    }

    Ok(AptMirrorPreview {
        distribution: system.distribution.clone(),
        codename: system.codename.clone(),
        architecture: system.architecture.clone(),
        source_file: PathBuf::from(source_file),
        source_contents: contents,
    })
}

fn expand_required(
    value: Option<&str>,
    name: &str,
    system: &AptSystem,
) -> Result<String, ForgeError> {
    let value = value.ok_or_else(|| ForgeError::Config(format!("apt_mirror 缺少 {name}")))?;
    expand_template(value, system)
}

fn expand_list(
    values: &[String],
    name: &str,
    system: &AptSystem,
) -> Result<Vec<String>, ForgeError> {
    if values.is_empty() {
        return Err(ForgeError::Config(format!("apt_mirror.{name} 不能为空")));
    }
    values
        .iter()
        .map(|value| {
            let expanded = expand_template(value, system)?;
            validate_scalar(&format!("apt_mirror.{name}"), &expanded)?;
            Ok(expanded)
        })
        .collect()
}

fn expand_template(value: &str, system: &AptSystem) -> Result<String, ForgeError> {
    let value = value
        .replace("{distribution}", &system.distribution)
        .replace("{codename}", &system.codename)
        .replace("{architecture}", &system.architecture);
    if value.contains('{') || value.contains('}') {
        return Err(ForgeError::Config(format!(
            "APT 镜像配置包含未知变量：{value}"
        )));
    }
    Ok(value)
}

fn validate_scalar(name: &str, value: &str) -> Result<(), ForgeError> {
    if value.trim().is_empty() || value.chars().any(char::is_whitespace) {
        return Err(ForgeError::Config(format!("{name} 不能包含空白字符")));
    }
    Ok(())
}

fn parse_os_release(contents: &str) -> std::collections::BTreeMap<String, String> {
    contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            (
                key.trim().to_string(),
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            )
        })
        .collect()
}

fn temp_dir(prefix: &str) -> Result<PathBuf, ForgeError> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ForgeError::Config(format!("系统时间异常：{error}")))?
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "rsenvforge-{prefix}-{}-{nonce}",
        std::process::id()
    ));
    create_dir_all(&path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{build_preview, AptSystem};
    use crate::AptMirrorDef;

    #[test]
    fn renders_deb822_source_with_system_variables() {
        let mirror = AptMirrorDef {
            uri: Some("https://apt.example.internal/{distribution}".to_string()),
            suites: vec!["{codename}".to_string(), "{codename}-updates".to_string()],
            components: vec!["main".to_string(), "universe".to_string()],
            architectures: Vec::new(),
            signed_by: Some("/usr/share/keyrings/internal.gpg".to_string()),
            source_file: Some("/etc/apt/sources.list.d/rsenvforge.sources".to_string()),
        };
        let system = AptSystem {
            distribution: "ubuntu".to_string(),
            codename: "noble".to_string(),
            architecture: "amd64".to_string(),
        };

        let preview = build_preview(&mirror, &system).unwrap();

        assert_eq!(
            preview.source_contents,
            "Types: deb\nURIs: https://apt.example.internal/ubuntu\nSuites: noble noble-updates\nComponents: main universe\nArchitectures: amd64\nSigned-By: /usr/share/keyrings/internal.gpg\n"
        );
    }
}
