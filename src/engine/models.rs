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
    pub tags: Vec<String>,
    pub check: Option<String>,
    pub check_windows: Option<String>,
    pub check_linux: Option<String>,
    pub install: Option<String>,
    pub install_windows: Option<String>,
    pub install_linux: Option<String>,
    pub post_install: Option<String>,
    pub post_install_windows: Option<String>,
    pub post_install_linux: Option<String>,
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

    pub(crate) fn post_install_command(&self) -> Option<&str> {
        if cfg!(windows) {
            self.post_install_windows
                .as_deref()
                .or(self.post_install.as_deref())
        } else {
            self.post_install_linux
                .as_deref()
                .or(self.post_install.as_deref())
        }
    }

    pub(crate) fn supports_current_platform(&self) -> bool {
        !matches!(self.check_command(), Some("0")) && !matches!(self.install_command(), Some("0"))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagCheckDef {
    pub check: Option<String>,
    pub check_windows: Option<String>,
    pub check_linux: Option<String>,
}

impl TagCheckDef {
    pub(crate) fn check_command(&self) -> Option<&str> {
        if cfg!(windows) {
            self.check_windows.as_deref().or(self.check.as_deref())
        } else {
            self.check_linux.as_deref().or(self.check.as_deref())
        }
    }

    pub(crate) fn supports_current_platform(&self) -> bool {
        !matches!(self.check_command(), Some("0"))
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
    pub environment: EnvironmentDef,
    pub apt_mirror: AptMirrorDef,
    pub tag_checks: BTreeMap<String, TagCheckDef>,
    pub items: Vec<InstallItem>,
    pub tools: Vec<ToolDef>,
    pub skills: Vec<SkillDef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AptMirrorDef {
    pub uri: Option<String>,
    pub lines: Vec<String>,
    pub suites: Vec<String>,
    pub components: Vec<String>,
    pub architectures: Vec<String>,
    pub signed_by: Option<String>,
    pub source_file: Option<String>,
    pub rules: Vec<AptMirrorRuleDef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AptMirrorRuleDef {
    pub distribution: Option<String>,
    pub codename: Option<String>,
    pub architecture: Option<String>,
    pub uri: Option<String>,
    pub lines: Vec<String>,
    pub suites: Vec<String>,
    pub components: Vec<String>,
    pub architectures: Vec<String>,
    pub signed_by: Option<String>,
    pub source_file: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvironmentDef {
    pub windows: PlatformEnvironmentDef,
    pub linux: PlatformEnvironmentDef,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformEnvironmentDef {
    pub cargo_config: Vec<String>,
    pub bashrc: Vec<String>,
    pub npmrc: Vec<String>,
    pub variables: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreinstallDef {
    pub windows: Vec<String>,
    pub linux: Vec<String>,
    pub light: ProfilePreinstallDef,
    pub standard: ProfilePreinstallDef,
    pub full: ProfilePreinstallDef,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfilePreinstallDef {
    pub windows: Vec<String>,
    pub linux: Vec<String>,
}

impl PreinstallDef {
    pub(crate) fn commands_for_current_platform(&self, profile: Profile) -> Vec<String> {
        let mut commands = current_platform_commands(&self.windows, &self.linux).to_vec();
        for profile_def in self.included_profiles(profile) {
            commands.extend(
                current_platform_commands(&profile_def.windows, &profile_def.linux).to_vec(),
            );
        }
        commands
    }

    pub(crate) fn profile_mut(&mut self, profile: Profile) -> &mut ProfilePreinstallDef {
        match profile {
            Profile::Light => &mut self.light,
            Profile::Standard => &mut self.standard,
            Profile::Full => &mut self.full,
        }
    }

    fn included_profiles(&self, profile: Profile) -> Vec<&ProfilePreinstallDef> {
        match profile {
            Profile::Light => vec![&self.light],
            Profile::Standard => vec![&self.light, &self.standard],
            Profile::Full => vec![&self.light, &self.standard, &self.full],
        }
    }
}

fn current_platform_commands<'a>(windows: &'a [String], linux: &'a [String]) -> &'a [String] {
    if cfg!(windows) {
        windows
    } else {
        linux
    }
}

impl ProfilePreinstallDef {
    pub(crate) fn commands_mut(&mut self, platform: PreinstallPlatform) -> &mut Vec<String> {
        match platform {
            PreinstallPlatform::Windows => &mut self.windows,
            PreinstallPlatform::Linux => &mut self.linux,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreinstallPlatform {
    Windows,
    Linux,
}

impl PreinstallPlatform {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "windows" => Some(Self::Windows),
            "linux" => Some(Self::Linux),
            _ => None,
        }
    }
}

impl PreinstallDef {
    pub(crate) fn commands_mut(&mut self, platform: PreinstallPlatform) -> &mut Vec<String> {
        match platform {
            PreinstallPlatform::Windows => &mut self.windows,
            PreinstallPlatform::Linux => &mut self.linux,
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
    pub status_bar: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            profile: Profile::Standard,
            config_path: None,
            force: false,
            norustup: false,
            status_bar: true,
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
