use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use super::config::{load_config, resolve_config_sources, skills_for_names, tools_for_names};
use super::constants::{CONFIG_FILE, SKILL_FILE};
use super::discovery::{discover_crates, discover_skills};
use super::error::ForgeError;
use super::fsutil::{copy_dir, copy_file, create_dir_all};
use super::models::{
    Agent, InstallConfig, InstallItem, InstallKind, InstallOptions, InstallPreview, Profile,
    RegistryEntry, SkillDef, SkillStatus, ToolDef, ToolStatus,
};
use super::paths::{app_home, managed_bin_dir, registry_path};
use super::process::{command_status_text, run_shell, run_shell_capture};
use super::registry::{append_registry, read_registry};
use super::util::{
    exe_name, first_line, fnv1a, home_dir, join_names, looks_like_git, now_secs, shell_quote,
    shell_quote_str, source_name,
};

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
