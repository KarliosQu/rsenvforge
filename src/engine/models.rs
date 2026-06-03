use std::collections::BTreeMap;
use std::path::PathBuf;

use super::error::ForgeError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Profile {
    Light,
    #[default]
    Standard,
    Full,
}

impl Profile {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "light" => Some(Self::Light),
            "standard" => Some(Self::Standard),
            "full" => Some(Self::Full),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Standard => "standard",
            Self::Full => "full",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    OpenCode,
}

impl Agent {
    pub fn parse(value: &str) -> Result<Self, ForgeError> {
        match value {
            "claude" => Ok(Self::Claude),
            "opencode" => Ok(Self::OpenCode),
            _ => Err(ForgeError::Config(format!("未知 agent：{value}"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    Skill,
    Crate,
}

impl InstallKind {
    pub(crate) fn parse(value: &str) -> Result<Self, ForgeError> {
        match value {
            "skill" => Ok(Self::Skill),
            "crate" => Ok(Self::Crate),
            _ => Err(ForgeError::Config(format!("未知安装项类型：{value}"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Crate => "crate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallItem {
    pub name: String,
    pub kind: InstallKind,
    pub source: String,
    pub agents: Vec<Agent>,
    pub bins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDef {
    pub name: String,
    pub check: Option<String>,
    pub check_windows: Option<String>,
    pub check_linux: Option<String>,
    pub install: Option<String>,
    pub install_windows: Option<String>,
    pub install_linux: Option<String>,
}

impl ToolDef {
    pub(crate) fn check_command(&self) -> Option<&str> {
        if cfg!(windows) {
            self.check_windows.as_deref().or(self.check.as_deref())
        } else {
            self.check_linux.as_deref().or(self.check.as_deref())
        }
    }

    pub(crate) fn install_command(&self) -> Option<&str> {
        if cfg!(windows) {
            self.install_windows.as_deref().or(self.install.as_deref())
        } else {
            self.install_linux.as_deref().or(self.install.as_deref())
        }
    }

    pub(crate) fn supports_current_platform(&self) -> bool {
        !matches!(self.check_command(), Some("0")) && !matches!(self.install_command(), Some("0"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDef {
    pub name: String,
    pub source: String,
    pub agents: Vec<Agent>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileDef {
    pub tools: Vec<String>,
    pub skills: Vec<String>,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallConfig {
    pub profiles: BTreeMap<String, ProfileDef>,
    pub preinstall: PreinstallDef,
    pub items: Vec<InstallItem>,
    pub tools: Vec<ToolDef>,
    pub skills: Vec<SkillDef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreinstallDef {
    pub windows: Vec<String>,
    pub linux: Vec<String>,
}

impl PreinstallDef {
    pub(crate) fn commands_for_current_platform(&self) -> &[String] {
        if cfg!(windows) {
            &self.windows
        } else {
            &self.linux
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub config: InstallConfig,
    pub path: Option<PathBuf>,
    pub builtin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCandidate {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateCandidate {
    pub package_name: String,
    pub manifest_path: PathBuf,
    pub bins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub name: String,
    pub kind: InstallKind,
    pub source: String,
    pub profile: String,
    pub targets: Vec<PathBuf>,
    pub installed_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOptions {
    pub profile: Profile,
    pub config_path: Option<PathBuf>,
    pub force: bool,
    pub norustup: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            profile: Profile::Standard,
            config_path: None,
            force: false,
            norustup: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub installable: bool,
    pub supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillStatus {
    pub name: String,
    pub agent: Agent,
    pub agent_dir: Option<PathBuf>,
    pub installed: bool,
    pub installable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPreview {
    pub tools: Vec<ToolStatus>,
    pub skills: Vec<SkillStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    pub entries: Vec<RegistryEntry>,
    pub final_preview: InstallPreview,
}

impl InstallPreview {
    pub fn missing_tools(&self) -> Vec<&ToolStatus> {
        self.tools
            .iter()
            .filter(|status| status.supported && !status.installed)
            .collect()
    }

    pub fn missing_skills(&self) -> Vec<&SkillStatus> {
        self.skills
            .iter()
            .filter(|status| !status.installed && status.installable)
            .collect()
    }
}
