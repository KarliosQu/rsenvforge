use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{self, Display};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub const CONFIG_FILE: &str = "rsenvforge.toml";
pub const REGISTRY_FILE: &str = "registry.tsv";
pub const SKILL_FILE: &str = "SKILL.md";

pub const BUILTIN_CONFIG: &str = r#"
[profiles.light]
tools = ["rust"]
skills = []
items = []

[profiles.standard]
tools = [
  "rust",
  "cargo-llvm-cov",
  "python",
  "bindgen-cli",
  "cargo-audit",
  "cargo-deny",
  "cargo-geiger",
  "cargo-udeps",
  "cargo-bloat",
  "flamegraph-rs",
  "perf",
  "cargo-msrv",
  "cargo-semver-checks",
  "cpp2rust-demo",
  "c2rust-demo",
  "rust-checker",
  "gitnexus",
]
skills = ["openspec", "oh-my-opencode", "superpowers"]
items = []

[profiles.full]
tools = [
  "rust",
  "cargo-llvm-cov",
  "python",
  "bindgen-cli",
  "cargo-audit",
  "cargo-deny",
  "cargo-geiger",
  "cargo-udeps",
  "cargo-bloat",
  "flamegraph-rs",
  "perf",
  "cargo-msrv",
  "cargo-semver-checks",
  "cpp2rust-demo",
  "c2rust-demo",
  "rust-checker",
  "gitnexus",
  "valgrind",
  "asan",
  "CMake",
  "Ninja",
  "Clang/libclang",
  "llvm-tools-preview",
  "clang++/g++",
]
skills = ["openspec", "oh-my-opencode", "superpowers"]
items = []

[[tools]]
name = "python"
check = "python --version"

[[tools]]
name = "perf"
check_linux = "perf --version"

[[tools]]
name = "valgrind"
check_linux = "valgrind --version"

[[tools]]
name = "asan"
check_linux = "cc --version"
check_windows = "clang --version"

[[tools]]
name = "CMake"
check = "cmake --version"

[[tools]]
name = "Ninja"
check = "ninja --version"

[[tools]]
name = "Clang/libclang"
check = "clang --version"

[[tools]]
name = "clang++/g++"
check_linux = "g++ --version"
check_windows = "clang++ --version"

[[tools]]
name = "gitnexus"
check = "gitnexus --version"
"#;

#[derive(Debug)]
pub enum ForgeError {
    Io { path: PathBuf, source: io::Error },
    Parse(String),
    Config(String),
    Command(String),
}

impl Display for ForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgeError::Io { path, source } => write!(f, "{}: {}", path.display(), source),
            ForgeError::Parse(message) => write!(f, "解析错误：{message}"),
            ForgeError::Config(message) => write!(f, "配置错误：{message}"),
            ForgeError::Command(message) => write!(f, "命令错误：{message}"),
        }
    }
}

impl Error for ForgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ForgeError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

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
    fn parse(value: &str) -> Result<Self, ForgeError> {
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
    fn check_command(&self) -> Option<&str> {
        if cfg!(windows) {
            self.check_windows.as_deref().or(self.check.as_deref())
        } else {
            self.check_linux.as_deref().or(self.check.as_deref())
        }
    }

    fn install_command(&self) -> Option<&str> {
        if cfg!(windows) {
            self.install_windows.as_deref().or(self.install.as_deref())
        } else {
            self.install_linux.as_deref().or(self.install.as_deref())
        }
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
    pub items: Vec<InstallItem>,
    pub tools: Vec<ToolDef>,
    pub skills: Vec<SkillDef>,
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

impl InstallPreview {
    pub fn missing_tools(&self) -> Vec<&ToolStatus> {
        self.tools
            .iter()
            .filter(|status| !status.installed)
            .collect()
    }

    pub fn missing_skills(&self) -> Vec<&SkillStatus> {
        self.skills
            .iter()
            .filter(|status| !status.installed && status.installable)
            .collect()
    }
}

pub fn load_config(explicit_path: Option<&Path>) -> Result<LoadedConfig, ForgeError> {
    if let Some(path) = explicit_path {
        return Ok(LoadedConfig {
            config: merge_with_builtin(parse_config(&read_to_string(path)?)?),
            path: Some(path.to_path_buf()),
            builtin: false,
        });
    }

    let local = env::current_dir()
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from("."),
            source,
        })?
        .join(CONFIG_FILE);
    if local.is_file() {
        return Ok(LoadedConfig {
            config: merge_with_builtin(parse_config(&read_to_string(&local)?)?),
            path: Some(local),
            builtin: false,
        });
    }

    let user = config_dir().join(CONFIG_FILE);
    if user.is_file() {
        return Ok(LoadedConfig {
            config: merge_with_builtin(parse_config(&read_to_string(&user)?)?),
            path: Some(user),
            builtin: false,
        });
    }

    Ok(LoadedConfig {
        config: parse_config(BUILTIN_CONFIG)?,
        path: None,
        builtin: true,
    })
}

pub fn parse_config(input: &str) -> Result<InstallConfig, ForgeError> {
    let mut profiles: BTreeMap<String, ProfileDef> = BTreeMap::new();
    let mut items = Vec::new();
    let mut tools = Vec::new();
    let mut skills = Vec::new();
    let mut section = Section::None;
    let mut current_item: Option<RawItem> = None;
    let mut current_tool: Option<RawTool> = None;
    let mut current_skill: Option<RawSkill> = None;

    let normalized = normalize_multiline_arrays(input)?;
    for (line_number, raw_line) in normalized.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if line == "[[items]]" || line == "[[tools]]" || line == "[[skills]]" {
            flush_raw(
                &mut current_item,
                &mut current_tool,
                &mut current_skill,
                &mut items,
                &mut tools,
                &mut skills,
            )?;
            section = match line {
                "[[items]]" => {
                    current_item = Some(RawItem::default());
                    Section::Item
                }
                "[[tools]]" => {
                    current_tool = Some(RawTool::default());
                    Section::Tool
                }
                _ => {
                    current_skill = Some(RawSkill::default());
                    Section::Skill
                }
            };
            continue;
        }

        if line.starts_with("[profiles.") && line.ends_with(']') {
            flush_raw(
                &mut current_item,
                &mut current_tool,
                &mut current_skill,
                &mut items,
                &mut tools,
                &mut skills,
            )?;
            let profile = line
                .trim_start_matches("[profiles.")
                .trim_end_matches(']')
                .trim()
                .to_string();
            if Profile::parse(&profile).is_none() {
                return Err(ForgeError::Parse(format!(
                    "第 {} 行：未知 profile：{profile}",
                    line_number + 1
                )));
            }
            profiles.entry(profile.clone()).or_default();
            section = Section::Profile(profile);
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(ForgeError::Parse(format!(
                "第 {} 行：需要 key = value",
                line_number + 1
            )));
        };
        let key = key.trim();
        let value = value.trim();

        match &mut section {
            Section::Profile(profile) => {
                let profile = profiles.entry(profile.clone()).or_default();
                match key {
                    "tools" => profile.tools = parse_string_array(value)?,
                    "skills" => profile.skills = parse_string_array(value)?,
                    "items" => profile.items = parse_string_array(value)?,
                    _ => {
                        return Err(ForgeError::Parse(format!(
                            "第 {} 行：profiles 只支持 tools/skills/items",
                            line_number + 1
                        )))
                    }
                }
            }
            Section::Item => {
                let Some(item) = current_item.as_mut() else {
                    return Err(ForgeError::Parse("item 字段不在 [[items]] 中".to_string()));
                };
                match key {
                    "name" => item.name = Some(parse_string(value)?),
                    "kind" => item.kind = Some(parse_string(value)?),
                    "source" => item.source = Some(parse_string(value)?),
                    "agents" => item.agents = parse_string_array(value)?,
                    "bins" => item.bins = parse_string_array(value)?,
                    _ => return Err(ForgeError::Parse(format!("未知 item 字段：{key}"))),
                }
            }
            Section::Tool => {
                let Some(tool) = current_tool.as_mut() else {
                    return Err(ForgeError::Parse("tool 字段不在 [[tools]] 中".to_string()));
                };
                match key {
                    "name" => tool.name = Some(parse_string(value)?),
                    "check" => tool.check = Some(parse_string(value)?),
                    "check_windows" => tool.check_windows = Some(parse_string(value)?),
                    "check_linux" => tool.check_linux = Some(parse_string(value)?),
                    "install" => tool.install = Some(parse_string(value)?),
                    "install_windows" => tool.install_windows = Some(parse_string(value)?),
                    "install_linux" => tool.install_linux = Some(parse_string(value)?),
                    _ => return Err(ForgeError::Parse(format!("未知 tool 字段：{key}"))),
                }
            }
            Section::Skill => {
                let Some(skill) = current_skill.as_mut() else {
                    return Err(ForgeError::Parse(
                        "skill 字段不在 [[skills]] 中".to_string(),
                    ));
                };
                match key {
                    "name" => skill.name = Some(parse_string(value)?),
                    "source" => skill.source = Some(parse_string(value)?),
                    "agents" => skill.agents = parse_string_array(value)?,
                    _ => return Err(ForgeError::Parse(format!("未知 skill 字段：{key}"))),
                }
            }
            Section::None => {
                return Err(ForgeError::Parse(format!(
                    "第 {} 行：字段不在任何 section 中",
                    line_number + 1
                )));
            }
        }
    }

    flush_raw(
        &mut current_item,
        &mut current_tool,
        &mut current_skill,
        &mut items,
        &mut tools,
        &mut skills,
    )?;

    for profile in ["light", "standard", "full"] {
        profiles.entry(profile.to_string()).or_default();
    }

    Ok(InstallConfig {
        profiles,
        items,
        tools,
        skills,
    })
}

pub fn install_profile(options: &InstallOptions) -> Result<Vec<RegistryEntry>, ForgeError> {
    let loaded = load_config(options.config_path.as_deref())?;
    let config = resolve_config_sources(loaded.config, loaded.path.as_deref());
    let preview = preview_install(&config, options.profile)?;
    print_preview(&preview);

    let missing_tools = preview.missing_tools();
    let missing_skills = preview.missing_skills();
    if missing_tools.is_empty() && missing_skills.is_empty() {
        println!("所有工具和 skill 均已安装。");
        install_legacy_items(&config, options)?;
        return read_registry();
    }

    println!(
        "缺少的工具：{}",
        join_names(missing_tools.iter().map(|status| status.name.as_str()))
    );
    println!(
        "缺少的 skill：{}",
        join_names(missing_skills.iter().map(|status| status.name.as_str()))
    );

    let not_installable: Vec<&ToolStatus> = missing_tools
        .iter()
        .copied()
        .filter(|status| !status.installable)
        .collect();
    if !not_installable.is_empty() {
        return Err(ForgeError::Config(format!(
            "以下工具没有确定的安装命令，请在 {CONFIG_FILE} 的 [[tools]] 中配置官方安装命令后重试：{}",
            join_names(not_installable.iter().map(|status| status.name.as_str()))
        )));
    }

    if !confirm_install()? {
        println!("用户取消安装。");
        return Ok(Vec::new());
    }

    install_missing_tools(&config, options.profile, &preview)?;
    install_missing_skills(&config, options.profile, &preview, options.force)?;
    install_legacy_items(&config, options)?;
    read_registry()
}

pub fn preview_install(
    config: &InstallConfig,
    profile: Profile,
) -> Result<InstallPreview, ForgeError> {
    let profile_def = config
        .profiles
        .get(profile.as_str())
        .ok_or_else(|| ForgeError::Config(format!("缺少 profile：{}", profile.as_str())))?;
    let tools = tools_for_names(config, &profile_def.tools)?;
    let skills = skills_for_names(config, &profile_def.skills)?;
    let mut tool_status = Vec::new();
    let mut skill_status = Vec::new();

    for tool in tools {
        tool_status.push(check_tool(&tool));
    }
    for skill in skills {
        for agent in &skill.agents {
            skill_status.push(check_skill(&skill, *agent));
        }
    }

    Ok(InstallPreview {
        tools: tool_status,
        skills: skill_status,
    })
}

pub fn print_preview(preview: &InstallPreview) {
    println!("工具检测结果：");
    for status in &preview.tools {
        if status.installed {
            println!(
                "  已安装：{} ({})",
                status.name,
                status.version.as_deref().unwrap_or("无法读取版本")
            );
        } else {
            println!("  尚未安装：{}", status.name);
        }
    }

    println!("Skill 检测结果：");
    for status in &preview.skills {
        if !status.installable {
            println!(
                "  跳过：{} -> {}（未找到默认 skill 文件夹）",
                status.name,
                status.agent.as_str()
            );
        } else if status.installed {
            println!("  已安装：{} -> {}", status.name, status.agent.as_str());
        } else {
            println!("  尚未安装：{} -> {}", status.name, status.agent.as_str());
        }
    }
}

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

pub fn install_skill_source(
    source: &str,
    agents: &[Agent],
    force: bool,
) -> Result<Vec<RegistryEntry>, ForgeError> {
    let item = InstallItem {
        name: source_name(source),
        kind: InstallKind::Skill,
        source: source.to_string(),
        agents: agents.to_vec(),
        bins: Vec::new(),
    };
    install_skill_item(&item, "manual", force, false)
}

pub fn install_crate_source(
    source: &str,
    bins: &[String],
    force: bool,
    norustup: bool,
) -> Result<Vec<RegistryEntry>, ForgeError> {
    let item = InstallItem {
        name: source_name(source),
        kind: InstallKind::Crate,
        source: source.to_string(),
        agents: Vec::new(),
        bins: bins.to_vec(),
    };
    install_crate_item(&item, "manual", force, norustup, false)
}

pub fn update_installed(force: bool, norustup: bool) -> Result<Vec<RegistryEntry>, ForgeError> {
    let existing = read_registry()?;
    let mut seen = BTreeSet::new();
    let mut updated = Vec::new();

    for entry in existing {
        let key = format!("{}\t{}\t{}", entry.kind.as_str(), entry.name, entry.source);
        if !seen.insert(key) {
            continue;
        }
        match entry.kind {
            InstallKind::Skill => {
                let agents = agents_from_targets(&entry.targets);
                let item = InstallItem {
                    name: entry.name,
                    kind: InstallKind::Skill,
                    source: entry.source,
                    agents,
                    bins: Vec::new(),
                };
                updated.extend(install_skill_item(&item, &entry.profile, force, true)?);
            }
            InstallKind::Crate => {
                let bins = entry
                    .targets
                    .iter()
                    .filter_map(|target| target.file_stem().and_then(OsStr::to_str))
                    .map(str::to_string)
                    .collect();
                let item = InstallItem {
                    name: entry.name,
                    kind: InstallKind::Crate,
                    source: entry.source,
                    agents: Vec::new(),
                    bins,
                };
                updated.extend(install_crate_item(
                    &item,
                    &entry.profile,
                    force,
                    norustup,
                    true,
                )?);
            }
        }
    }

    Ok(updated)
}

pub fn doctor_report() -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("rsenvforge 数据目录：{}", app_home().display()));
    lines.push(format!("托管 bin 目录：{}", managed_bin_dir().display()));
    lines.push(format!("安装记录：{}", registry_path().display()));
    lines.push(format!("git：{}", command_status_text("git")));
    lines.push(format!("cargo：{}", command_status_text("cargo")));
    lines.push(format!("rustup：{}", command_status_text("rustup")));
    lines.push(format!("claude：{}", command_status_text("claude")));
    lines.push(format!("opencode：{}", command_status_text("opencode")));
    lines.push("请将托管 bin 目录加入 PATH，以便直接运行已安装 Rust 工具。".to_string());
    lines
}

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

pub fn managed_bin_dir() -> PathBuf {
    env::var("RSENVFORGE_BIN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| app_home().join("bin"))
}

pub fn registry_path() -> PathBuf {
    app_home().join(REGISTRY_FILE)
}

fn merge_with_builtin(config: InstallConfig) -> InstallConfig {
    let mut builtin = parse_config(BUILTIN_CONFIG).expect("内置配置必须可解析");
    for (profile, def) in config.profiles {
        builtin.profiles.insert(profile, def);
    }
    extend_or_replace_tools(&mut builtin.tools, config.tools);
    extend_or_replace_skills(&mut builtin.skills, config.skills);
    builtin.items = config.items;
    builtin
}

fn extend_or_replace_tools(base: &mut Vec<ToolDef>, incoming: Vec<ToolDef>) {
    for tool in incoming {
        if let Some(existing) = base.iter_mut().find(|existing| existing.name == tool.name) {
            *existing = tool;
        } else {
            base.push(tool);
        }
    }
}

fn extend_or_replace_skills(base: &mut Vec<SkillDef>, incoming: Vec<SkillDef>) {
    for skill in incoming {
        if let Some(existing) = base.iter_mut().find(|existing| existing.name == skill.name) {
            *existing = skill;
        } else {
            base.push(skill);
        }
    }
}

fn resolve_config_sources(mut config: InstallConfig, config_path: Option<&Path>) -> InstallConfig {
    if let Some(base_dir) = config_path.and_then(Path::parent) {
        for item in &mut config.items {
            item.source = resolve_source(&item.source, base_dir);
        }
        for skill in &mut config.skills {
            skill.source = resolve_source(&skill.source, base_dir);
        }
    }
    config
}

fn tools_for_names(config: &InstallConfig, names: &[String]) -> Result<Vec<ToolDef>, ForgeError> {
    let mut tools = Vec::new();
    for name in names {
        if let Some(tool) = builtin_cargo_tool(name)
            .or_else(|| config.tools.iter().find(|tool| &tool.name == name).cloned())
        {
            tools.push(tool);
        } else {
            tools.push(ToolDef {
                name: name.clone(),
                check: Some(format!("{name} --version")),
                check_windows: None,
                check_linux: None,
                install: None,
                install_windows: None,
                install_linux: None,
            });
        }
    }
    Ok(tools)
}

fn skills_for_names(config: &InstallConfig, names: &[String]) -> Result<Vec<SkillDef>, ForgeError> {
    let mut skills = Vec::new();
    for name in names {
        let skill = config
            .skills
            .iter()
            .find(|skill| &skill.name == name)
            .cloned()
            .unwrap_or_else(|| SkillDef {
                name: name.clone(),
                source: String::new(),
                agents: vec![Agent::Claude, Agent::OpenCode],
            });
        skills.push(skill);
    }
    Ok(skills)
}

fn builtin_cargo_tool(name: &str) -> Option<ToolDef> {
    let (check, install) = match name {
        "cargo-llvm-cov" => ("cargo llvm-cov --version", "cargo install cargo-llvm-cov"),
        "bindgen-cli" => ("bindgen --version", "cargo install bindgen-cli"),
        "cargo-audit" => ("cargo audit --version", "cargo install cargo-audit"),
        "cargo-deny" => ("cargo deny --version", "cargo install cargo-deny"),
        "cargo-geiger" => ("cargo geiger --version", "cargo install cargo-geiger"),
        "cargo-udeps" => ("cargo udeps --version", "cargo install cargo-udeps"),
        "cargo-bloat" => ("cargo bloat --version", "cargo install cargo-bloat"),
        "flamegraph-rs" => ("flamegraph --version", "cargo install flamegraph"),
        "cargo-msrv" => ("cargo msrv --version", "cargo install cargo-msrv"),
        "cargo-semver-checks" => (
            "cargo semver-checks --version",
            "cargo install cargo-semver-checks",
        ),
        "cpp2rust-demo" => (
            "cpp2rust-demo --version",
            "cargo install --git https://github.com/LuuuXXX/cpp2rust-demo",
        ),
        "c2rust-demo" => (
            "c2rust-demo --version",
            "cargo install --git https://github.com/LuuuXXX/c2rust-demo",
        ),
        "rust-checker" => (
            "rust-checker --version",
            "cargo install --git https://github.com/LuuuXXX/rust-checker",
        ),
        "llvm-tools-preview" => (
            "rustup component list --installed",
            "rustup component add llvm-tools-preview",
        ),
        "rust" => (
            "rustup --version",
            "rustup toolchain install stable && rustup component add rustfmt clippy",
        ),
        _ => return None,
    };

    Some(ToolDef {
        name: name.to_string(),
        check: Some(check.to_string()),
        check_windows: None,
        check_linux: None,
        install: Some(install.to_string()),
        install_windows: None,
        install_linux: None,
    })
}

fn check_tool(tool: &ToolDef) -> ToolStatus {
    if tool.name == "rust" {
        return check_rust_toolchain(tool);
    }
    let Some(command) = tool.check_command() else {
        return ToolStatus {
            name: tool.name.clone(),
            installed: false,
            version: None,
            installable: tool.install_command().is_some(),
        };
    };
    let result = run_shell_capture(command);
    ToolStatus {
        name: tool.name.clone(),
        installed: result.is_ok(),
        version: result.ok().map(first_line),
        installable: tool.install_command().is_some(),
    }
}

fn check_rust_toolchain(tool: &ToolDef) -> ToolStatus {
    let checks = [
        ("rustup", "rustup --version"),
        ("cargo", "cargo --version"),
        ("rustfmt", "rustfmt --version"),
        ("clippy", "cargo clippy --version"),
    ];
    let mut versions = Vec::new();
    let mut missing = Vec::new();
    for (name, command) in checks {
        match run_shell_capture(command) {
            Ok(output) => versions.push(format!("{name}: {}", first_line(output))),
            Err(_) => missing.push(name),
        }
    }
    ToolStatus {
        name: tool.name.clone(),
        installed: missing.is_empty(),
        version: if versions.is_empty() {
            None
        } else {
            Some(versions.join("; "))
        },
        installable: tool.install_command().is_some(),
    }
}

fn check_skill(skill: &SkillDef, agent: Agent) -> SkillStatus {
    let agent_dir = default_agent_skill_dir(agent);
    let installed = agent_dir
        .as_ref()
        .map(|dir| dir.join(&skill.name).join(SKILL_FILE).is_file())
        .unwrap_or(false);
    SkillStatus {
        name: skill.name.clone(),
        agent,
        installable: agent_dir.is_some() && !skill.source.is_empty(),
        agent_dir,
        installed,
    }
}

fn confirm_install() -> Result<bool, ForgeError> {
    println!("以上为目前工具安装情况，请问是否安装缺失工具？(Y/N)");
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from("stdin"),
            source,
        })?;
    Ok(matches!(answer.trim(), "Y" | "y"))
}

fn install_missing_tools(
    config: &InstallConfig,
    profile: Profile,
    preview: &InstallPreview,
) -> Result<(), ForgeError> {
    let missing_names: BTreeSet<&str> = preview
        .missing_tools()
        .into_iter()
        .filter(|status| status.installable)
        .map(|status| status.name.as_str())
        .collect();
    if missing_names.is_empty() {
        return Ok(());
    }
    let profile_def = config
        .profiles
        .get(profile.as_str())
        .ok_or_else(|| ForgeError::Config(format!("缺少 profile：{}", profile.as_str())))?;
    let tools = tools_for_names(config, &profile_def.tools)?;
    for tool in tools {
        if !missing_names.contains(tool.name.as_str()) {
            continue;
        }
        let Some(command) = tool.install_command() else {
            continue;
        };
        println!("开始安装工具：{}", tool.name);
        run_shell(command).map_err(|error| {
            ForgeError::Command(format!("工具 {} 安装失败：{error}", tool.name))
        })?;
        println!("工具 {} 安装完成。", tool.name);
    }
    Ok(())
}

fn install_missing_skills(
    config: &InstallConfig,
    profile: Profile,
    preview: &InstallPreview,
    force: bool,
) -> Result<(), ForgeError> {
    let missing: Vec<(&str, Agent)> = preview
        .missing_skills()
        .into_iter()
        .map(|status| (status.name.as_str(), status.agent))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let profile_def = config
        .profiles
        .get(profile.as_str())
        .ok_or_else(|| ForgeError::Config(format!("缺少 profile：{}", profile.as_str())))?;
    let skills = skills_for_names(config, &profile_def.skills)?;
    for skill in skills {
        let agents: Vec<Agent> = missing
            .iter()
            .filter_map(|(name, agent)| (*name == skill.name).then_some(*agent))
            .collect();
        if agents.is_empty() {
            continue;
        }
        let item = InstallItem {
            name: skill.name.clone(),
            kind: InstallKind::Skill,
            source: skill.source.clone(),
            agents,
            bins: Vec::new(),
        };
        install_skill_item(&item, profile.as_str(), force, false).map_err(|error| {
            ForgeError::Command(format!("skill {} 安装失败：{error}", skill.name))
        })?;
    }
    Ok(())
}

fn install_legacy_items(
    config: &InstallConfig,
    options: &InstallOptions,
) -> Result<Vec<RegistryEntry>, ForgeError> {
    let profile_def = config
        .profiles
        .get(options.profile.as_str())
        .ok_or_else(|| ForgeError::Config(format!("缺少 profile：{}", options.profile.as_str())))?;
    let selected = items_for_names(config, &profile_def.items)?;
    let mut installed = Vec::new();
    for item in selected {
        match item.kind {
            InstallKind::Skill => installed.extend(install_skill_item(
                &item,
                options.profile.as_str(),
                options.force,
                false,
            )?),
            InstallKind::Crate => installed.extend(install_crate_item(
                &item,
                options.profile.as_str(),
                options.force,
                options.norustup,
                false,
            )?),
        }
    }
    Ok(installed)
}

fn items_for_names(
    config: &InstallConfig,
    names: &[String],
) -> Result<Vec<InstallItem>, ForgeError> {
    let by_name: BTreeMap<&str, &InstallItem> = config
        .items
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect();
    let mut selected = Vec::new();
    for name in names {
        let item = by_name
            .get(name.as_str())
            .ok_or_else(|| ForgeError::Config(format!("profile 引用了不存在的 item：{name}")))?;
        selected.push((*item).clone());
    }
    Ok(selected)
}

fn install_skill_item(
    item: &InstallItem,
    profile: &str,
    force: bool,
    update_source: bool,
) -> Result<Vec<RegistryEntry>, ForgeError> {
    if item.agents.is_empty() {
        return Err(ForgeError::Config(format!(
            "skill {} 必须配置 agents",
            item.name
        )));
    }

    let source_dir = prepare_source(&item.source, update_source)?;
    let candidates = discover_skills(&source_dir)?;
    if candidates.is_empty() {
        return Err(ForgeError::Config(format!(
            "{} 中没有找到 skill",
            source_dir.display()
        )));
    }

    let mut targets = Vec::new();
    for skill in candidates {
        for agent in &item.agents {
            let Some(agent_root) = default_agent_skill_dir(*agent) else {
                println!(
                    "未找到 {} 默认 skill 文件夹，跳过安装 {}。",
                    agent.as_str(),
                    skill.name
                );
                continue;
            };
            let target = agent_root.join(&skill.name);
            copy_dir(&skill.path, &target, force)?;
            targets.push(target);
        }
    }

    let entry = RegistryEntry {
        name: item.name.clone(),
        kind: InstallKind::Skill,
        source: item.source.clone(),
        profile: profile.to_string(),
        targets,
        installed_at: now_secs(),
    };
    append_registry(entry.clone())?;
    Ok(vec![entry])
}

fn install_crate_item(
    item: &InstallItem,
    profile: &str,
    force: bool,
    norustup: bool,
    update_source: bool,
) -> Result<Vec<RegistryEntry>, ForgeError> {
    let source_dir = prepare_source(&item.source, update_source)?;
    let crates = discover_crates(&source_dir)?;
    if crates.is_empty() {
        return Err(ForgeError::Config(format!(
            "{} 中没有找到 Cargo.toml",
            source_dir.display()
        )));
    }

    if !norustup && run_shell_capture("rustup --version").is_err() {
        println!("提示：未检测到 rustup，将继续尝试预编译二进制或已有 cargo。");
    }

    create_dir_all(&managed_bin_dir())?;
    let requested_bins: BTreeSet<String> = item.bins.iter().cloned().collect();
    let mut targets = Vec::new();

    for krate in crates {
        let bins: Vec<String> = if requested_bins.is_empty() {
            krate.bins.clone()
        } else {
            krate
                .bins
                .iter()
                .filter(|bin| requested_bins.contains(*bin))
                .cloned()
                .collect()
        };
        for bin in bins {
            let target = managed_bin_dir().join(exe_name(&bin));
            if target.exists() && !force {
                return Err(ForgeError::Config(format!(
                    "{} 已存在，请添加 --force 覆盖",
                    target.display()
                )));
            }
            if let Some(prebuilt) = find_prebuilt_binary(&source_dir, &bin) {
                copy_file(&prebuilt, &target)?;
            } else {
                build_and_copy_binary(&krate.manifest_path, &bin, &target).map_err(|error| {
                    ForgeError::Command(format!("工具 {bin} 安装失败：{error}"))
                })?;
            }
            targets.push(target);
        }
    }

    if !requested_bins.is_empty() && targets.is_empty() {
        return Err(ForgeError::Config(format!(
            "没有找到指定 binary：{}",
            item.bins.join(", ")
        )));
    }

    let entry = RegistryEntry {
        name: item.name.clone(),
        kind: InstallKind::Crate,
        source: item.source.clone(),
        profile: profile.to_string(),
        targets,
        installed_at: now_secs(),
    };
    append_registry(entry.clone())?;
    Ok(vec![entry])
}

fn prepare_source(source: &str, update: bool) -> Result<PathBuf, ForgeError> {
    let path = PathBuf::from(source);
    if path.exists() {
        return Ok(path);
    }
    if !looks_like_git(source) {
        return Err(ForgeError::Config(format!(
            "source 不存在，且不是 git 地址：{source}"
        )));
    }
    if run_shell_capture("git --version").is_err() {
        return Err(ForgeError::Command(
            "安装 git source 需要 git，但未检测到 git。".to_string(),
        ));
    }

    let cache_path = app_home()
        .join("sources")
        .join(format!("{:016x}", fnv1a(source)));
    if cache_path.exists() {
        if update {
            run_shell(&format!(
                "git -C {} pull --ff-only",
                shell_quote(&cache_path)
            ))?;
        }
        return Ok(cache_path);
    }

    if let Some(parent) = cache_path.parent() {
        create_dir_all(parent)?;
    }
    run_shell(&format!(
        "git clone --depth 1 {} {}",
        shell_quote_str(source),
        shell_quote(&cache_path)
    ))?;
    Ok(cache_path)
}

fn build_and_copy_binary(manifest: &Path, bin: &str, target: &Path) -> Result<(), ForgeError> {
    if run_shell_capture("cargo --version").is_err() {
        return Err(ForgeError::Command(
            "未找到 cargo，且没有匹配的预编译二进制。".to_string(),
        ));
    }
    run_shell(&format!(
        "cargo build --release --manifest-path {} --bin {}",
        shell_quote(manifest),
        shell_quote_str(bin)
    ))?;
    let built = manifest
        .parent()
        .ok_or_else(|| ForgeError::Config(format!("manifest 路径无效：{}", manifest.display())))?
        .join("target")
        .join("release")
        .join(exe_name(bin));
    copy_file(&built, target)
}

fn default_agent_skill_dir(agent: Agent) -> Option<PathBuf> {
    let override_var = match agent {
        Agent::Claude => "RSENVFORGE_CLAUDE_DIR",
        Agent::OpenCode => "RSENVFORGE_OPENCODE_DIR",
    };
    if let Ok(path) = env::var(override_var) {
        let path = PathBuf::from(path);
        return path.is_dir().then_some(path);
    }

    let path = match agent {
        Agent::Claude => home_dir().join(".claude").join("skills"),
        Agent::OpenCode => {
            if cfg!(windows) {
                env::var("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| home_dir().join("AppData").join("Roaming"))
                    .join("opencode")
                    .join("skills")
            } else {
                home_dir().join(".config").join("opencode").join("skills")
            }
        }
    };
    path.is_dir().then_some(path)
}

fn find_prebuilt_binary(root: &Path, bin: &str) -> Option<PathBuf> {
    let names = [exe_name(bin), bin.to_string()];
    for base in [
        root.join("bin"),
        root.join("dist"),
        root.join("release"),
        root.join("target").join("release"),
    ] {
        for name in &names {
            let candidate = base.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
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

fn agents_from_targets(targets: &[PathBuf]) -> Vec<Agent> {
    let mut agents = Vec::new();
    for target in targets {
        let text = target.display().to_string().replace('\\', "/");
        if text.contains(".claude/skills") && !agents.contains(&Agent::Claude) {
            agents.push(Agent::Claude);
        }
        if (text.contains("opencode/skills") || text.contains(".config/opencode/skills"))
            && !agents.contains(&Agent::OpenCode)
        {
            agents.push(Agent::OpenCode);
        }
    }
    if agents.is_empty() {
        agents.push(Agent::Claude);
    }
    agents
}

fn append_registry(entry: RegistryEntry) -> Result<(), ForgeError> {
    let mut entries = read_registry()?;
    entries.push(entry);
    write_registry(&entries)
}

fn copy_dir(source: &Path, target: &Path, force: bool) -> Result<(), ForgeError> {
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

fn copy_file(source: &Path, target: &Path) -> Result<(), ForgeError> {
    if let Some(parent) = target.parent() {
        create_dir_all(parent)?;
    }
    fs::copy(source, target).map_err(|source_error| ForgeError::Io {
        path: source.to_path_buf(),
        source: source_error,
    })?;
    Ok(())
}

fn read_dir(path: &Path) -> Result<Vec<Result<PathBuf, ForgeError>>, ForgeError> {
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

fn create_dir_all(path: &Path) -> Result<(), ForgeError> {
    fs::create_dir_all(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn remove_dir_all(path: &Path) -> Result<(), ForgeError> {
    fs::remove_dir_all(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_to_string(path: &Path) -> Result<String, ForgeError> {
    fs::read_to_string(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file(path: &Path, contents: &str) -> Result<(), ForgeError> {
    fs::write(path, contents).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn run_shell(command: &str) -> Result<(), ForgeError> {
    let output = shell_command(command)
        .output()
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from(command),
            source,
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ForgeError::Command(format!(
            "{}\n{}",
            command,
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

fn run_shell_capture(command: &str) -> Result<String, ForgeError> {
    let output = shell_command(command)
        .output()
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from(command),
            source,
        })?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(if stdout.trim().is_empty() {
            stderr.to_string()
        } else {
            stdout.to_string()
        })
    } else {
        Err(ForgeError::Command(format!(
            "{}\n{}",
            command,
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

fn shell_command(command: &str) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

fn command_status_text(command: &str) -> &'static str {
    if run_shell_capture(&format!("{command} --version")).is_ok() {
        "已找到"
    } else {
        "未找到"
    }
}

fn home_dir() -> PathBuf {
    env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn exe_name(bin: &str) -> String {
    if cfg!(windows) && !bin.ends_with(".exe") {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    }
}

fn source_name(source: &str) -> String {
    source
        .trim_end_matches(".git")
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("manual")
        .to_string()
}

fn looks_like_git(source: &str) -> bool {
    source.ends_with(".git")
        || source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("ssh://")
        || source.starts_with("git@")
}

fn resolve_source(source: &str, base_dir: &Path) -> String {
    let path = PathBuf::from(source);
    if source.is_empty() || path.is_absolute() || looks_like_git(source) {
        source.to_string()
    } else {
        base_dir.join(path).display().to_string()
    }
}

fn shell_quote(path: &Path) -> String {
    shell_quote_str(&path.display().to_string())
}

fn shell_quote_str(value: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn first_line(value: String) -> String {
    value.lines().next().unwrap_or("").trim().to_string()
}

fn join_names<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let names = names.collect::<Vec<_>>();
    if names.is_empty() {
        "无".to_string()
    } else {
        names.join(", ")
    }
}

fn fnv1a(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn strip_comment(line: &str) -> &str {
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

fn parse_string(value: &str) -> Result<String, ForgeError> {
    let value = value.trim();
    if let Some(stripped) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        Ok(stripped.to_string())
    } else {
        Err(ForgeError::Parse(format!("需要字符串，得到：{value}")))
    }
}

fn parse_string_array(value: &str) -> Result<Vec<String>, ForgeError> {
    let value = value.trim();
    let Some(inner) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Err(ForgeError::Parse(format!("需要数组，得到：{value}")));
    };
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .filter(|value| !value.trim().is_empty())
        .map(|value| parse_string(value.trim()))
        .collect()
}

fn normalize_multiline_arrays(input: &str) -> Result<String, ForgeError> {
    let mut output = String::new();
    let mut pending = String::new();
    let mut in_array = false;

    for raw_line in input.lines() {
        let line = strip_comment(raw_line);
        if in_array {
            pending.push(' ');
            pending.push_str(line.trim());
            if line.contains(']') {
                output.push_str(&pending);
                output.push('\n');
                pending.clear();
                in_array = false;
            }
            continue;
        }

        if let Some((_, value)) = line.split_once('=') {
            let value = value.trim();
            if value.starts_with('[') && !value.contains(']') {
                pending.push_str(line.trim());
                in_array = true;
                continue;
            }
        }

        output.push_str(raw_line);
        output.push('\n');
    }

    if in_array {
        return Err(ForgeError::Parse("数组缺少结束 ]".to_string()));
    }

    Ok(output)
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn flush_raw(
    current_item: &mut Option<RawItem>,
    current_tool: &mut Option<RawTool>,
    current_skill: &mut Option<RawSkill>,
    items: &mut Vec<InstallItem>,
    tools: &mut Vec<ToolDef>,
    skills: &mut Vec<SkillDef>,
) -> Result<(), ForgeError> {
    if let Some(item) = current_item.take() {
        items.push(item.into_item()?);
    }
    if let Some(tool) = current_tool.take() {
        tools.push(tool.into_tool()?);
    }
    if let Some(skill) = current_skill.take() {
        skills.push(skill.into_skill()?);
    }
    Ok(())
}

#[derive(Debug)]
enum Section {
    None,
    Profile(String),
    Item,
    Tool,
    Skill,
}

#[derive(Debug, Default)]
struct RawItem {
    name: Option<String>,
    kind: Option<String>,
    source: Option<String>,
    agents: Vec<String>,
    bins: Vec<String>,
}

impl RawItem {
    fn into_item(self) -> Result<InstallItem, ForgeError> {
        let name = required(self.name, "item.name")?;
        let kind = InstallKind::parse(&required(self.kind, "item.kind")?)?;
        let source = required(self.source, "item.source")?;
        let agents = self
            .agents
            .iter()
            .map(|agent| Agent::parse(agent))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(InstallItem {
            name,
            kind,
            source,
            agents,
            bins: self.bins,
        })
    }
}

#[derive(Debug, Default)]
struct RawTool {
    name: Option<String>,
    check: Option<String>,
    check_windows: Option<String>,
    check_linux: Option<String>,
    install: Option<String>,
    install_windows: Option<String>,
    install_linux: Option<String>,
}

impl RawTool {
    fn into_tool(self) -> Result<ToolDef, ForgeError> {
        Ok(ToolDef {
            name: required(self.name, "tool.name")?,
            check: self.check,
            check_windows: self.check_windows,
            check_linux: self.check_linux,
            install: self.install,
            install_windows: self.install_windows,
            install_linux: self.install_linux,
        })
    }
}

#[derive(Debug, Default)]
struct RawSkill {
    name: Option<String>,
    source: Option<String>,
    agents: Vec<String>,
}

impl RawSkill {
    fn into_skill(self) -> Result<SkillDef, ForgeError> {
        let agents = self
            .agents
            .iter()
            .map(|agent| Agent::parse(agent))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SkillDef {
            name: required(self.name, "skill.name")?,
            source: required(self.source, "skill.source")?,
            agents,
        })
    }
}

fn required(value: Option<String>, field: &str) -> Result<String, ForgeError> {
    value.ok_or_else(|| ForgeError::Config(format!("缺少必填字段 {field}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profiles_tools_and_skills() {
        let config = parse_config(
            r#"
            [profiles.light]
            tools = ["rust"]
            skills = []
            items = []

            [profiles.standard]
            tools = ["rust", "python"]
            skills = ["openspec"]
            items = []

            [profiles.full]
            tools = ["rust", "python", "CMake"]
            skills = ["openspec"]
            items = []

            [[tools]]
            name = "python"
            check = "python --version"
            install_windows = "echo install python"

            [[skills]]
            name = "openspec"
            source = "./openspec"
            agents = ["claude", "opencode"]
            "#,
        )
        .unwrap();

        assert_eq!(config.profiles["standard"].tools, vec!["rust", "python"]);
        assert_eq!(config.tools[0].name, "python");
        assert_eq!(
            config.skills[0].agents,
            vec![Agent::Claude, Agent::OpenCode]
        );
    }

    #[test]
    fn builtin_profiles_are_cumulative() {
        let config = parse_config(BUILTIN_CONFIG).unwrap();
        let light = &config.profiles["light"].tools;
        let standard = &config.profiles["standard"].tools;
        let full = &config.profiles["full"].tools;
        assert!(standard.iter().all(|tool| full.contains(tool)));
        assert!(light.iter().all(|tool| standard.contains(tool)));
        for tool in ["cpp2rust-demo", "c2rust-demo", "rust-checker"] {
            assert!(standard.contains(&tool.to_string()));
            assert!(full.contains(&tool.to_string()));
            assert!(builtin_cargo_tool(tool).is_some());
        }
    }

    #[test]
    fn discovers_local_skills_and_crates() {
        let temp = test_dir("discovers_local_skills_and_crates");
        let skill = temp.join("skills").join("demo");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join(SKILL_FILE),
            "---\nname: demo-skill\n---\n# demo\n",
        )
        .unwrap();
        let krate = temp.join("crates").join("demo-tool");
        fs::create_dir_all(krate.join("src")).unwrap();
        fs::write(
            krate.join("Cargo.toml"),
            "[package]\nname = \"demo-tool\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(krate.join("src").join("main.rs"), "fn main() {}\n").unwrap();

        let skills = discover_skills(&temp).unwrap();
        let crates = discover_crates(&temp).unwrap();
        assert_eq!(skills[0].name, "demo-skill");
        assert_eq!(crates[0].bins, vec!["demo-tool"]);

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn writes_and_reads_registry() {
        let temp = test_dir("writes_and_reads_registry");
        env::set_var("RSENVFORGE_HOME", &temp);

        let entry = RegistryEntry {
            name: "demo".to_string(),
            kind: InstallKind::Skill,
            source: "./demo".to_string(),
            profile: "standard".to_string(),
            targets: vec![temp.join("target")],
            installed_at: 42,
        };
        write_registry(std::slice::from_ref(&entry)).unwrap();
        assert_eq!(read_registry().unwrap(), vec![entry]);

        env::remove_var("RSENVFORGE_HOME");
        fs::remove_dir_all(temp).unwrap();
    }

    fn test_dir(name: &str) -> PathBuf {
        let nanos = now_secs();
        env::temp_dir().join(format!("rsenvforge-{name}-{nanos}-{}", std::process::id()))
    }
}
