use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use super::constants::{BUILTIN_CONFIG, CONFIG_FILE};
use super::error::ForgeError;
use super::fsutil::{read_to_string, write_file};
use super::models::{
    Agent, EnvironmentDef, InstallConfig, InstallItem, InstallKind, LoadedConfig, PreinstallDef,
    PreinstallPlatform, Profile, ProfileDef, SkillDef, ToolDef,
};
use super::paths::{config_dir, manifest_config_path};
use super::util::resolve_source;

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

    let manifest = manifest_config_path();
    if manifest.is_file() {
        return Ok(LoadedConfig {
            config: merge_with_builtin(parse_config(&read_to_string(&manifest)?)?),
            path: Some(manifest),
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

pub fn init_config(force: bool) -> Result<PathBuf, ForgeError> {
    let path = env::current_dir()
        .map_err(|source| ForgeError::Io {
            path: PathBuf::from("."),
            source,
        })?
        .join(CONFIG_FILE);
    if path.exists() && !force {
        return Err(ForgeError::Config(format!(
            "{} 已存在，如需覆盖请添加 --force",
            path.display()
        )));
    }
    write_file(&path, BUILTIN_CONFIG)?;
    Ok(path)
}

pub fn parse_config(input: &str) -> Result<InstallConfig, ForgeError> {
    let mut profiles: BTreeMap<String, ProfileDef> = BTreeMap::new();
    let mut preinstall = PreinstallDef::default();
    let mut environment = EnvironmentDef::default();
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

        if line.starts_with("[preinstall.") && line.ends_with(']') {
            flush_raw(
                &mut current_item,
                &mut current_tool,
                &mut current_skill,
                &mut items,
                &mut tools,
                &mut skills,
            )?;
            let scope = line
                .trim_start_matches("[preinstall.")
                .trim_end_matches(']')
                .trim();
            section = parse_preinstall_section(scope, line_number + 1)?;
            continue;
        }

        if line == "[environment]" {
            flush_raw(
                &mut current_item,
                &mut current_tool,
                &mut current_skill,
                &mut items,
                &mut tools,
                &mut skills,
            )?;
            section = Section::Environment;
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
            Section::Preinstall { profile, platform } => match key {
                "commands" => {
                    let commands = parse_string_array(value)?;
                    match profile.as_deref().and_then(Profile::parse) {
                        Some(profile) => {
                            *preinstall.profile_mut(profile).commands_mut(*platform) = commands
                        }
                        None => *preinstall.commands_mut(*platform) = commands,
                    }
                }
                _ => {
                    return Err(ForgeError::Parse(format!(
                        "第 {} 行：preinstall 只支持 commands",
                        line_number + 1
                    )))
                }
            },
            Section::Environment => match key {
                "cargo_config" => environment.cargo_config = parse_string_array(value)?,
                "bashrc" => environment.bashrc = parse_string_array(value)?,
                _ => {
                    return Err(ForgeError::Parse(format!(
                        "第 {} 行：environment 只支持 cargo_config/bashrc",
                        line_number + 1
                    )))
                }
            },
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
        preinstall,
        environment,
        items,
        tools,
        skills,
    })
}

fn merge_with_builtin(config: InstallConfig) -> InstallConfig {
    let mut builtin = parse_config(BUILTIN_CONFIG).expect("内置配置必须可解析");
    for (profile, def) in config.profiles {
        builtin.profiles.insert(profile, def);
    }
    builtin.preinstall = config.preinstall;
    builtin.environment = config.environment;
    extend_or_replace_tools(&mut builtin.tools, config.tools);
    extend_or_replace_skills(&mut builtin.skills, config.skills);
    builtin.items = config.items;
    builtin
}

fn parse_preinstall_section(scope: &str, line_number: usize) -> Result<Section, ForgeError> {
    let parts = scope.split('.').map(str::trim).collect::<Vec<_>>();
    match parts.as_slice() {
        [platform] => {
            let platform = PreinstallPlatform::parse(platform).ok_or_else(|| {
                ForgeError::Parse(format!(
                    "第 {line_number} 行：preinstall 只支持 windows/linux"
                ))
            })?;
            Ok(Section::Preinstall {
                profile: None,
                platform,
            })
        }
        [profile, platform] => {
            if Profile::parse(profile).is_none() {
                return Err(ForgeError::Parse(format!(
                    "第 {line_number} 行：未知 preinstall profile：{profile}"
                )));
            }
            let platform = PreinstallPlatform::parse(platform).ok_or_else(|| {
                ForgeError::Parse(format!(
                    "第 {line_number} 行：preinstall 只支持 windows/linux"
                ))
            })?;
            Ok(Section::Preinstall {
                profile: Some((*profile).to_string()),
                platform,
            })
        }
        _ => Err(ForgeError::Parse(format!(
            "第 {line_number} 行：preinstall 格式应为 [preinstall.<platform>] 或 [preinstall.<profile>.<platform>]"
        ))),
    }
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

pub(crate) fn resolve_config_sources(
    mut config: InstallConfig,
    config_path: Option<&Path>,
) -> InstallConfig {
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

pub(crate) fn tools_for_names(
    config: &InstallConfig,
    names: &[String],
) -> Result<Vec<ToolDef>, ForgeError> {
    let mut tools = Vec::new();
    for name in names {
        if let Some(tool) = config
            .tools
            .iter()
            .find(|tool| &tool.name == name)
            .cloned()
            .or_else(|| builtin_cargo_tool(name))
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

pub(crate) fn skills_for_names(
    config: &InstallConfig,
    names: &[String],
) -> Result<Vec<SkillDef>, ForgeError> {
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

pub(crate) fn builtin_cargo_tool(name: &str) -> Option<ToolDef> {
    if name == "rust-build-base" {
        return Some(ToolDef {
            name: name.to_string(),
            check: None,
            check_windows: Some("0".to_string()),
            check_linux: Some("dpkg -s build-essential pkg-config libssl-dev".to_string()),
            install: None,
            install_windows: Some("0".to_string()),
            install_linux: Some(
                "apt-get update && apt-get install -y build-essential pkg-config libssl-dev"
                    .to_string(),
            ),
        });
    }

    if name == "rust-lldb" {
        return Some(ToolDef {
            name: name.to_string(),
            check: None,
            check_windows: Some("0".to_string()),
            check_linux: Some("rust-lldb --version".to_string()),
            install: None,
            install_windows: Some("0".to_string()),
            install_linux: Some("rustup component add rustc".to_string()),
        });
    }

    let (check, install) = match name {
        "cargo-llvm-cov" => ("cargo llvm-cov --version", "cargo install cargo-llvm-cov"),
        "bindgen-cli" => ("bindgen --version", "cargo install bindgen-cli"),
        "cargo-audit" => ("cargo audit --version", "cargo install cargo-audit"),
        "cargo-deny" => ("cargo deny --version", "cargo install cargo-deny"),
        "cargo-geiger" => ("cargo geiger --version", "cargo install cargo-geiger"),
        "rust-analyzer" => (
            "rust-analyzer --version",
            "rustup component add rust-analyzer",
        ),
        "miri" => (
            "cargo +nightly miri --version",
            "rustup toolchain install nightly && rustup +nightly component add miri",
        ),
        "cargo-expand" => ("cargo expand --version", "cargo install cargo-expand"),
        "cargo-fuzz" => ("cargo fuzz --version", "cargo install cargo-fuzz"),
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
        unescape_string(stripped)
    } else {
        Err(ForgeError::Parse(format!("需要字符串，得到：{value}")))
    }
}

fn unescape_string(value: &str) -> Result<String, ForgeError> {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }
        let Some(escaped) = chars.next() else {
            return Err(ForgeError::Parse("字符串转义缺少后续字符".to_string()));
        };
        match escaped {
            '"' => result.push('"'),
            '\\' => result.push('\\'),
            'n' => result.push('\n'),
            't' => result.push('\t'),
            _ => {
                result.push('\\');
                result.push(escaped);
            }
        }
    }
    Ok(result)
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
            if array_is_closed(&pending) {
                output.push_str(&pending);
                output.push('\n');
                pending.clear();
                in_array = false;
            }
            continue;
        }

        if let Some((_, value)) = line.split_once('=') {
            let value = value.trim();
            if value.starts_with('[') && !array_is_closed(value) {
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

fn array_is_closed(value: &str) -> bool {
    let mut depth = 0;
    let mut in_quote = false;
    let mut escaped = false;

    for ch in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_quote && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_quote = !in_quote;
            continue;
        }
        if in_quote {
            continue;
        }
        match ch {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ => {}
        }
    }

    depth == 0
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
    Preinstall {
        profile: Option<String>,
        platform: PreinstallPlatform,
    },
    Environment,
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
