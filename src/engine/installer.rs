use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{self, ClearType};

use super::apt_mirror::{apply_apt_mirror, apt_mirror_preview, check_apt_mirror};
use super::config::{load_config, resolve_config_sources, skills_for_names, tools_for_names};
use super::constants::{CONFIG_FILE, SKILL_FILE};
use super::discovery::{discover_crates, discover_skills};
use super::envfile::{
    apply_after_rust_install_environment, apply_install_start_environment,
    refresh_node_process_environment,
};
use super::error::ForgeError;
use super::fsutil::{copy_dir, copy_file, create_dir_all, remove_dir_all, remove_file};
use super::input::read_user_line;
use super::models::{
    Agent, InstallConfig, InstallItem, InstallKind, InstallOptions, InstallPreview, InstallReport,
    Profile, ProfileDef, RegistryEntry, SkillDef, SkillStatus, ToolDef, ToolStatus,
};
use super::paths::{app_home, managed_bin_dir, registry_path};
use super::process::{
    command_status_text, run_shell_capture, run_shell_labeled, run_shell_labeled_quiet,
    ShellRunStatus,
};
use super::proxy::{print_proxy_report, proxy_report};
use super::registry::{append_registry, read_registry, write_registry};
use super::util::{
    exe_name, first_line, fnv1a, home_dir, join_names, looks_like_git, now_secs, shell_quote,
    shell_quote_str, source_name,
};

pub fn install_profile(options: &InstallOptions) -> Result<InstallReport, ForgeError> {
    let loaded = load_config(options.config_path.as_deref())?;
    let config = resolve_config_sources(loaded.config, loaded.path.as_deref());
    apply_install_start_environment(&config)?;
    print_proxy_report();
    let preview = preview_install(&config, options.profile)?;
    print_preview(&preview);

    let missing_tools = preview.missing_tools();
    let missing_skills = preview.missing_skills();
    if missing_tools.is_empty() && missing_skills.is_empty() {
        println!("所有工具和 skill 均已安装。");
        let selection = InstallSelection::all(&preview);
        let mut progress = InstallProgress::new(0);
        process_profile_tools(
            &config,
            options.profile,
            &preview,
            &selection,
            &mut progress,
        )?;
        let entries = install_legacy_items(&config, options)?;
        let final_preview = preview_install(&config, options.profile)?;
        print_install_complete(&final_preview);
        return Ok(InstallReport {
            entries,
            final_preview,
        });
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

    let selection = select_install_components(&preview)?;
    if selection.is_empty() {
        println!("未选择任何组件，取消安装。");
        return Ok(InstallReport {
            entries: Vec::new(),
            final_preview: preview,
        });
    }

    let rust_missing = preview
        .tools
        .iter()
        .any(|status| is_rust_toolchain(&status.name) && status.supported && !status.installed);
    let use_apt_mirror = confirm_apt_mirror_for_install(&config)?;
    let preinstall_steps = preinstall_step_count(&config, options.profile, &preview, &selection);
    let mut progress = InstallProgress::new(
        selection.step_count() + preinstall_steps + usize::from(use_apt_mirror),
    );
    if use_apt_mirror {
        apply_apt_mirror_for_install(&config, &mut progress)?;
    }
    run_preinstall_commands(
        &config,
        options.profile,
        &preview,
        &selection,
        &mut progress,
    )?;
    process_profile_tools(
        &config,
        options.profile,
        &preview,
        &selection,
        &mut progress,
    )?;
    if rust_missing {
        apply_after_rust_install_environment(&config)?;
    }
    install_missing_skills(
        &config,
        options.profile,
        &preview,
        &selection,
        options.force,
        &mut progress,
    )?;
    let entries = install_legacy_items(&config, options)?;
    let final_preview = preview_install(&config, options.profile)?;
    print_install_complete(&final_preview);
    Ok(InstallReport {
        entries,
        final_preview,
    })
}

pub fn preview_install(
    config: &InstallConfig,
    profile: Profile,
) -> Result<InstallPreview, ForgeError> {
    let profile_def = merged_profile(config, profile)?;
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
        if !status.supported {
            println!("  {}：不支持{}环境", status.name, current_platform_name());
        } else if status.installed {
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

fn print_install_complete(preview: &InstallPreview) {
    println!("已安装完成");
    for status in &preview.tools {
        if !status.supported {
            println!("{}：不支持{}环境", status.name, current_platform_name());
        } else if status.installed {
            println!(
                "已安装【{}】+【{}】",
                status.name,
                status.version.as_deref().unwrap_or("无法读取版本")
            );
        }
    }
    for status in &preview.skills {
        if status.installed {
            println!(
                "已安装【{} -> {}】+【无版本信息】",
                status.name,
                status.agent.as_str()
            );
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

pub fn remove_installed(
    name: &str,
    kind: Option<InstallKind>,
    force: bool,
) -> Result<Vec<RegistryEntry>, ForgeError> {
    let entries = read_registry()?;
    let (matched, remaining): (Vec<_>, Vec<_>) = entries.into_iter().partition(|entry| {
        entry.name == name && kind.is_none_or(|expected| entry.kind == expected)
    });

    if matched.is_empty() {
        return Err(ForgeError::Config(format!("没有找到安装记录：{}", name)));
    }

    println!("将删除以下安装记录：");
    for entry in &matched {
        println!("  {} {}", entry.kind.as_str(), entry.name);
        for target in &entry.targets {
            println!("    {}", target.display());
        }
    }

    if !force && !confirm_remove()? {
        println!("用户取消删除。");
        return Ok(Vec::new());
    }

    for entry in &matched {
        for target in &entry.targets {
            remove_target(target)?;
        }
    }
    write_registry(&remaining)?;
    Ok(matched)
}

pub fn doctor_report() -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("rsenvforge 数据目录：{}", app_home().display()));
    lines.push(format!("托管 bin 目录：{}", managed_bin_dir().display()));
    lines.push(format!("安装记录：{}", registry_path().display()));
    lines.extend(proxy_report());
    lines.push(format!("git：{}", command_status_text("git")));
    lines.push(format!("cargo：{}", command_status_text("cargo")));
    lines.push(format!("rustup：{}", command_status_text("rustup")));
    lines.push(format!("claude：{}", command_status_text("claude")));
    lines.push(format!("opencode：{}", command_status_text("opencode")));
    lines.push("请将托管 bin 目录加入 PATH，以便直接运行已安装 Rust 工具。".to_string());
    lines
}

fn confirm_remove() -> Result<bool, ForgeError> {
    println!("请确认是否删除以上安装项？(Y/N)");
    let answer = read_user_line()?;
    Ok(matches!(answer.trim(), "Y" | "y"))
}

fn remove_target(target: &Path) -> Result<(), ForgeError> {
    if target.is_dir() {
        remove_dir_all(target)
    } else if target.is_file() {
        remove_file(target)
    } else {
        println!("目标不存在，跳过：{}", target.display());
        Ok(())
    }
}

fn check_tool(tool: &ToolDef) -> ToolStatus {
    if !tool.supports_current_platform() {
        return ToolStatus {
            name: tool.name.clone(),
            installed: false,
            version: Some(format!("不支持{}环境", current_platform_name())),
            installable: false,
            supported: false,
        };
    }
    if is_rust_toolchain(&tool.name) {
        return check_rust_toolchain(tool);
    }
    let Some(command) = tool.check_command() else {
        return ToolStatus {
            name: tool.name.clone(),
            installed: false,
            version: None,
            installable: tool.install_command().is_some(),
            supported: true,
        };
    };
    let result = run_shell_capture(command);
    ToolStatus {
        name: tool.name.clone(),
        installed: result.is_ok(),
        version: result.ok().map(first_line),
        installable: tool.install_command().is_some(),
        supported: true,
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
        supported: true,
    }
}

fn current_platform_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else {
        "linux"
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
    let answer = read_user_line()?;
    Ok(matches!(answer.trim(), "Y" | "y"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallSelection {
    tools: BTreeSet<String>,
    skills: BTreeSet<String>,
}

impl InstallSelection {
    fn all(preview: &InstallPreview) -> Self {
        Self {
            tools: preview
                .missing_tools()
                .into_iter()
                .filter(|status| status.installable)
                .map(|status| status.name.clone())
                .collect(),
            skills: preview
                .missing_skills()
                .into_iter()
                .map(|status| skill_selection_key(&status.name, status.agent))
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.tools.is_empty() && self.skills.is_empty()
    }

    fn includes_tool(&self, name: &str) -> bool {
        self.tools.contains(name)
    }

    fn includes_skill(&self, name: &str, agent: Agent) -> bool {
        self.skills.contains(&skill_selection_key(name, agent))
    }

    fn step_count(&self) -> usize {
        self.tools.len() + self.skills.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectionKind {
    Tool(String),
    Skill(String, Agent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectionChoice {
    kind: SelectionKind,
    label: String,
    selected: bool,
}

fn select_install_components(preview: &InstallPreview) -> Result<InstallSelection, ForgeError> {
    let mut choices = selectable_install_choices(preview);
    if choices.is_empty() {
        return Ok(InstallSelection::all(preview));
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        if confirm_install()? {
            return Ok(InstallSelection::all(preview));
        }
        return Ok(InstallSelection {
            tools: BTreeSet::new(),
            skills: BTreeSet::new(),
        });
    }

    run_interactive_selection_menu(&mut choices)
}

fn selectable_install_choices(preview: &InstallPreview) -> Vec<SelectionChoice> {
    let mut choices = Vec::new();
    choices.extend(
        preview
            .missing_tools()
            .into_iter()
            .filter(|status| status.installable)
            .map(|status| SelectionChoice {
                kind: SelectionKind::Tool(status.name.clone()),
                label: format!("工具：{}", status.name),
                selected: true,
            }),
    );
    choices.extend(
        preview
            .missing_skills()
            .into_iter()
            .map(|status| SelectionChoice {
                kind: SelectionKind::Skill(status.name.clone(), status.agent),
                label: format!("Skill：{} -> {}", status.name, status.agent.as_str()),
                selected: true,
            }),
    );
    choices
}

fn run_interactive_selection_menu(
    choices: &mut [SelectionChoice],
) -> Result<InstallSelection, ForgeError> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()
        .map_err(|error| ForgeError::Command(format!("无法启用交互选择模式：{error}")))?;
    execute!(stdout, cursor::Hide, terminal::EnterAlternateScreen)
        .map_err(|error| ForgeError::Command(format!("无法绘制安装选择菜单：{error}")))?;

    let result = run_selection_event_loop(choices, &mut stdout);

    let _ = execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show);
    let _ = terminal::disable_raw_mode();

    let selection = result?;
    if selection.is_empty() {
        println!("已选择安装组件：无");
    } else {
        println!("已选择安装组件：{}", selection.step_count());
    }
    Ok(selection)
}

fn run_selection_event_loop(
    choices: &mut [SelectionChoice],
    stdout: &mut io::Stdout,
) -> Result<InstallSelection, ForgeError> {
    let mut cursor_index = 0usize;
    loop {
        render_selection_menu(choices, cursor_index, stdout)?;
        let event = event::read()
            .map_err(|error| ForgeError::Command(format!("读取安装选择输入失败：{error}")))?;
        let Event::Key(key) = event else {
            continue;
        };
        match key.code {
            KeyCode::Up => cursor_index = cursor_index.saturating_sub(1),
            KeyCode::Down => {
                if cursor_index + 1 < choices.len() {
                    cursor_index += 1;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(choice) = choices.get_mut(cursor_index) {
                    choice.selected = !choice.selected;
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let all_selected = choices.iter().all(|choice| choice.selected);
                for choice in choices.iter_mut() {
                    choice.selected = !all_selected;
                }
            }
            KeyCode::Enter => return Ok(selection_from_choices(choices)),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                for choice in choices.iter_mut() {
                    choice.selected = false;
                }
                return Ok(selection_from_choices(choices));
            }
            _ => {}
        }
    }
}

fn render_selection_menu(
    choices: &[SelectionChoice],
    cursor_index: usize,
    stdout: &mut io::Stdout,
) -> Result<(), ForgeError> {
    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::All)
    )
    .map_err(|error| ForgeError::Command(format!("绘制安装选择菜单失败：{error}")))?;
    writeln!(
        stdout,
        "请选择本次要安装的组件（↑/↓移动，空格切换，A 全选/全不选，Enter 确认，Esc/Q 取消）"
    )
    .map_err(|error| ForgeError::Command(format!("输出安装选择菜单失败：{error}")))?;
    writeln!(stdout)
        .map_err(|error| ForgeError::Command(format!("输出安装选择菜单失败：{error}")))?;
    for (index, choice) in choices.iter().enumerate() {
        let cursor = if index == cursor_index { ">" } else { " " };
        let mark = if choice.selected { "x" } else { " " };
        writeln!(stdout, "{cursor} [{mark}] {}", choice.label)
            .map_err(|error| ForgeError::Command(format!("输出安装选择菜单失败：{error}")))?;
    }
    stdout
        .flush()
        .map_err(|error| ForgeError::Command(format!("刷新安装选择菜单失败：{error}")))
}

fn selection_from_choices(choices: &[SelectionChoice]) -> InstallSelection {
    let mut selection = InstallSelection {
        tools: BTreeSet::new(),
        skills: BTreeSet::new(),
    };
    for choice in choices.iter().filter(|choice| choice.selected) {
        match &choice.kind {
            SelectionKind::Tool(name) => {
                selection.tools.insert(name.clone());
            }
            SelectionKind::Skill(name, agent) => {
                selection.skills.insert(skill_selection_key(name, *agent));
            }
        }
    }
    selection
}

fn skill_selection_key(name: &str, agent: Agent) -> String {
    format!("{name}\t{}", agent.as_str())
}

fn confirm_apt_mirror_for_install(config: &InstallConfig) -> Result<bool, ForgeError> {
    if !apt_mirror_available_for_install(config) {
        return Ok(false);
    }

    println!("是否使用内部apt镜像？如果未配置proxy，不使用apt镜像可能导致部分工具安装失败。(Y/N)");
    let answer = read_user_line()?;
    if !matches!(answer.trim(), "Y" | "y") {
        println!("已跳过内部 APT 镜像配置。");
        return Ok(false);
    }
    Ok(true)
}

fn apt_mirror_available_for_install(config: &InstallConfig) -> bool {
    cfg!(target_os = "linux")
        && (config.apt_mirror.uri.is_some()
            || !config.apt_mirror.lines.is_empty()
            || !config.apt_mirror.rules.is_empty())
}

fn apply_apt_mirror_for_install(
    config: &InstallConfig,
    progress: &mut InstallProgress,
) -> Result<(), ForgeError> {
    progress.next("配置", "APT 镜像");
    let preview = apt_mirror_preview(config)?;
    println!("开始验证内部 APT 镜像，不会直接修改系统源文件。");
    println!("APT 镜像源文件：{}", preview.source_file.display());
    check_apt_mirror(&preview)?;
    apply_apt_mirror(&preview)?;
    println!("已写入内部 APT 镜像配置：{}", preview.source_file.display());
    Ok(())
}

#[derive(Default)]
struct InstallSession {
    installed_tools: BTreeSet<String>,
}

impl InstallSession {
    fn mark_installed(&mut self, name: &str) {
        self.installed_tools.insert(name.to_string());
    }

    fn installed_nvm_this_run(&self) -> bool {
        self.installed_tools.contains("nvm")
    }

    fn installed_node_this_run(&self) -> bool {
        self.installed_tools.contains("nodejs")
    }
}

fn command_for_install_session(command: &str, session: &InstallSession) -> String {
    command_for_install_session_on_platform(command, session, cfg!(target_os = "linux"))
}

fn command_for_install_session_on_platform(
    command: &str,
    session: &InstallSession,
    is_linux: bool,
) -> String {
    if !is_linux || !command_uses_node_environment(command) {
        return command.to_string();
    }
    if !session.installed_nvm_this_run()
        && !session.installed_node_this_run()
        && !command_uses_nvm(command)
    {
        return command.to_string();
    }
    format!(
        "export NVM_DIR=\"${{NVM_DIR:-$HOME/.nvm}}\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"; {command}"
    )
}

fn command_uses_nvm(command: &str) -> bool {
    command_uses_token(command, "nvm")
}

fn command_uses_node_environment(command: &str) -> bool {
    ["nvm", "node", "npm"]
        .iter()
        .any(|token| command_uses_token(command, token))
}

fn command_uses_token(command: &str, token: &str) -> bool {
    command
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        })
        .any(|part| part == token)
}

fn process_profile_tools(
    config: &InstallConfig,
    profile: Profile,
    preview: &InstallPreview,
    selection: &InstallSelection,
    progress: &mut InstallProgress,
) -> Result<(), ForgeError> {
    let missing_names: BTreeSet<&str> = preview
        .missing_tools()
        .into_iter()
        .filter(|status| status.installable)
        .filter(|status| selection.includes_tool(&status.name))
        .map(|status| status.name.as_str())
        .collect();
    let installed_names: BTreeSet<&str> = preview
        .tools
        .iter()
        .filter(|status| status.supported && status.installed)
        .map(|status| status.name.as_str())
        .collect();
    let profile_def = merged_profile(config, profile)?;
    let tools = tools_for_names(config, &profile_def.tools)?;
    let mut passed_tags = BTreeSet::new();
    let mut session = InstallSession::default();
    for tool in tools {
        if missing_names.contains(tool.name.as_str()) {
            progress.next("安装工具", &tool.name);
            install_tool(config, &tool, &mut passed_tags, &mut session)?;
            continue;
        }

        if !installed_names.contains(tool.name.as_str()) || tool.post_install_command().is_none() {
            continue;
        }

        if confirm_run_installed_tool_post(&tool.name)? {
            run_tool_post_install(&tool, &session)?;
        } else {
            println!("已跳过工具安装后命令：{}", tool.name);
        }
    }
    Ok(())
}

fn install_tool(
    config: &InstallConfig,
    tool: &ToolDef,
    passed_tags: &mut BTreeSet<String>,
    session: &mut InstallSession,
) -> Result<(), ForgeError> {
    let Some(command) = tool.install_command() else {
        return Ok(());
    };
    if !run_tool_tag_checks(config, tool, passed_tags, session)? {
        println!("已跳过工具：{}", tool.name);
        return Ok(());
    }
    println!("开始安装工具：{}", tool.name);
    let command = command_for_install_session(command, session);
    match run_shell_labeled(&tool.name, &command) {
        Ok(ShellRunStatus::Completed) => {}
        Ok(ShellRunStatus::Skipped) => {
            println!("已跳过工具：{}", tool.name);
            return Ok(());
        }
        Err(error) => {
            println!("工具 {} 安装失败：{error}", tool.name);
            if confirm_skip_tool(&tool.name)? {
                println!("已跳过工具：{}", tool.name);
                return Ok(());
            }
            return Err(ForgeError::Command(format!(
                "工具 {} 安装失败：{error}",
                tool.name
            )));
        }
    }
    session.mark_installed(&tool.name);
    if is_rust_toolchain(&tool.name) {
        apply_after_rust_install_environment(config)?;
        println!("Rust 环境已写入配置文件，并已刷新当前安装进程。");
        println!("如果当前终端仍找不到 cargo/rustup，请执行 source ~/.bashrc 或重新打开终端。");
    }
    if is_node_environment_tool(&tool.name) {
        refresh_node_process_environment();
        println!("Node.js 环境已刷新到当前安装进程。");
    }
    run_tool_post_install(tool, session)?;
    println!("工具 {} 安装完成。", tool.name);
    Ok(())
}

fn run_tool_tag_checks(
    config: &InstallConfig,
    tool: &ToolDef,
    passed_tags: &mut BTreeSet<String>,
    session: &InstallSession,
) -> Result<bool, ForgeError> {
    for tag in &tool.tags {
        if passed_tags.contains(tag) {
            continue;
        }
        let Some(tag_check) = config.tag_checks.get(tag) else {
            println!("工具 {} 的标签 {} 未配置测试指令。", tool.name, tag);
            if confirm_skip_tool_tag_check(&tool.name)? {
                return Ok(false);
            }
            return Err(ForgeError::Config(format!(
                "工具 {} 的标签 {} 未配置测试指令",
                tool.name, tag
            )));
        };
        let Some(command) = tag_check.check_command() else {
            println!("工具 {} 的标签 {} 不支持当前平台测试。", tool.name, tag);
            if confirm_skip_tool_tag_check(&tool.name)? {
                return Ok(false);
            }
            return Err(ForgeError::Config(format!(
                "工具 {} 的标签 {} 不支持当前平台测试",
                tool.name, tag
            )));
        };
        if !tag_check.supports_current_platform() {
            println!("工具 {} 的标签 {} 不支持当前平台测试。", tool.name, tag);
            if confirm_skip_tool_tag_check(&tool.name)? {
                return Ok(false);
            }
            return Err(ForgeError::Config(format!(
                "工具 {} 的标签 {} 不支持当前平台测试",
                tool.name, tag
            )));
        }

        println!("执行工具 {} 的标签检查：{}", tool.name, tag);
        let command = command_for_install_session(command, session);
        match run_shell_capture(&command) {
            Ok(output) => {
                let version = first_line(output);
                if version.trim().is_empty() {
                    println!("标签检查通过：{}", tag);
                } else {
                    println!("标签检查通过：{}（{}）", tag, version);
                }
                passed_tags.insert(tag.clone());
            }
            Err(error) => {
                println!("工具 {} 的标签检查 {} 未通过：{error}", tool.name, tag);
                if confirm_skip_tool_tag_check(&tool.name)? {
                    return Ok(false);
                }
                return Err(ForgeError::Command(format!(
                    "工具 {} 的标签检查 {} 未通过：{error}",
                    tool.name, tag
                )));
            }
        }
    }
    Ok(true)
}

fn run_tool_post_install(tool: &ToolDef, session: &InstallSession) -> Result<(), ForgeError> {
    let Some(post_command) = tool.post_install_command() else {
        return Ok(());
    };
    println!("开始运行工具安装后命令：{}", tool.name);
    let post_command = command_for_install_session(post_command, session);
    match run_shell_labeled(&format!("{} 安装后命令", tool.name), &post_command) {
        Ok(ShellRunStatus::Completed) => {}
        Ok(ShellRunStatus::Skipped) => {
            println!("已跳过工具安装后命令：{}", tool.name);
            return Ok(());
        }
        Err(error) => {
            println!("工具 {} 安装后命令失败：{error}", tool.name);
            if confirm_skip_tool(&tool.name)? {
                println!("已跳过工具安装后命令：{}", tool.name);
                return Ok(());
            }
            return Err(ForgeError::Command(format!(
                "工具 {} 安装后命令失败：{error}",
                tool.name
            )));
        }
    }
    println!("工具 {} 安装后命令完成。", tool.name);
    Ok(())
}

fn confirm_run_installed_tool_post(name: &str) -> Result<bool, ForgeError> {
    println!("工具 {name} 已安装且配置了安装后命令，是否运行该命令？(Y/N)");
    let answer = read_user_line()?;
    Ok(matches!(answer.trim(), "Y" | "y"))
}

fn confirm_skip_tool(name: &str) -> Result<bool, ForgeError> {
    println!("工具 {name} 安装失败，是否跳过该工具继续安装？(Y/N)");
    let answer = read_user_line()?;
    Ok(matches!(answer.trim(), "Y" | "y"))
}

fn confirm_skip_tool_tag_check(name: &str) -> Result<bool, ForgeError> {
    println!("工具 {name} 安装前检查未通过，是否跳过该工具继续安装？(Y/N)");
    let answer = read_user_line()?;
    Ok(matches!(answer.trim(), "Y" | "y"))
}

struct InstallProgress {
    current: usize,
    total: usize,
}

impl InstallProgress {
    fn new(total: usize) -> Self {
        Self { current: 0, total }
    }

    fn next(&mut self, action: &str, name: &str) {
        if self.total == 0 {
            return;
        }
        self.current += 1;
        println!("Step {}/{}：{} {}", self.current, self.total, action, name);
    }
}

fn run_preinstall_commands(
    config: &InstallConfig,
    profile: Profile,
    preview: &InstallPreview,
    selection: &InstallSelection,
    progress: &mut InstallProgress,
) -> Result<(), ForgeError> {
    let commands = selected_preinstall_commands(config, profile, preview, selection);
    if commands.is_empty() {
        return Ok(());
    }

    progress.next("运行", "安装前置命令");
    for command in commands {
        if run_shell_labeled_quiet("安装前置命令", &command)? == ShellRunStatus::Skipped {
            println!("已跳过安装前置命令。");
        }
    }
    Ok(())
}

fn preinstall_step_count(
    config: &InstallConfig,
    profile: Profile,
    preview: &InstallPreview,
    selection: &InstallSelection,
) -> usize {
    usize::from(!selected_preinstall_commands(config, profile, preview, selection).is_empty())
}

fn selected_preinstall_commands(
    config: &InstallConfig,
    profile: Profile,
    preview: &InstallPreview,
    selection: &InstallSelection,
) -> Vec<String> {
    let commands = config.preinstall.commands_for_current_platform(profile);
    if commands.is_empty() {
        return Vec::new();
    }

    let missing_names: BTreeSet<&str> = preview
        .missing_tools()
        .into_iter()
        .filter(|status| status.installable)
        .filter(|status| selection.includes_tool(&status.name))
        .map(|status| status.name.as_str())
        .collect();
    if missing_names.is_empty() {
        return Vec::new();
    }

    commands
}

fn is_rust_toolchain(name: &str) -> bool {
    name == "rust-toolchain" || name == "rust"
}

fn is_node_environment_tool(name: &str) -> bool {
    name == "nvm" || name == "nodejs"
}

fn install_missing_skills(
    config: &InstallConfig,
    profile: Profile,
    preview: &InstallPreview,
    selection: &InstallSelection,
    force: bool,
    progress: &mut InstallProgress,
) -> Result<(), ForgeError> {
    let missing: Vec<(&str, Agent)> = preview
        .missing_skills()
        .into_iter()
        .filter(|status| selection.includes_skill(&status.name, status.agent))
        .map(|status| (status.name.as_str(), status.agent))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let profile_def = merged_profile(config, profile)?;
    let skills = skills_for_names(config, &profile_def.skills)?;
    for skill in skills {
        let agents: Vec<Agent> = missing
            .iter()
            .filter_map(|(name, agent)| (*name == skill.name).then_some(*agent))
            .collect();
        if agents.is_empty() {
            continue;
        }
        progress.next("安装 Skill", &skill.name);
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
    let profile_def = merged_profile(config, options.profile)?;
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

fn merged_profile(config: &InstallConfig, profile: Profile) -> Result<ProfileDef, ForgeError> {
    let mut merged = ProfileDef::default();
    for profile in included_profiles(profile) {
        let profile_def = config
            .profiles
            .get(profile.as_str())
            .ok_or_else(|| ForgeError::Config(format!("缺少 profile：{}", profile.as_str())))?;
        extend_unique(&mut merged.tools, &profile_def.tools);
        extend_unique(&mut merged.skills, &profile_def.skills);
        extend_unique(&mut merged.items, &profile_def.items);
    }
    Ok(merged)
}

fn included_profiles(profile: Profile) -> Vec<Profile> {
    match profile {
        Profile::Light => vec![Profile::Light],
        Profile::Standard => vec![Profile::Light, Profile::Standard],
        Profile::Full => vec![Profile::Light, Profile::Standard, Profile::Full],
    }
}

fn extend_unique(target: &mut Vec<String>, incoming: &[String]) {
    for item in incoming {
        if !target.contains(item) {
            target.push(item.clone());
        }
    }
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
            let _ = run_shell_labeled(
                &source_name(source),
                &format!("git -C {} pull --ff-only", shell_quote(&cache_path)),
            )?;
        }
        return Ok(cache_path);
    }

    if let Some(parent) = cache_path.parent() {
        create_dir_all(parent)?;
    }
    let _ = run_shell_labeled(
        &source_name(source),
        &format!(
            "git clone --depth 1 {} {}",
            shell_quote_str(source),
            shell_quote(&cache_path)
        ),
    )?;
    Ok(cache_path)
}

fn build_and_copy_binary(manifest: &Path, bin: &str, target: &Path) -> Result<(), ForgeError> {
    if run_shell_capture("cargo --version").is_err() {
        return Err(ForgeError::Command(
            "未找到 cargo，且没有匹配的预编译二进制。".to_string(),
        ));
    }
    let _ = run_shell_labeled(
        bin,
        &format!(
            "cargo build --release --manifest-path {} --bin {}",
            shell_quote(manifest),
            shell_quote_str(bin)
        ),
    )?;
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

#[cfg(test)]
mod tests {
    use super::{
        command_for_install_session_on_platform, command_uses_node_environment, command_uses_nvm,
        InstallSession,
    };

    #[test]
    fn wraps_nvm_commands_after_nvm_is_installed_by_session() {
        let command = "nvm install 20.17.0 && nvm use 20.17.0";
        let mut session = InstallSession::default();

        let preexisting_wrapped = command_for_install_session_on_platform(command, &session, true);
        assert!(preexisting_wrapped.contains("NVM_DIR"));
        assert!(preexisting_wrapped.ends_with(command));

        session.mark_installed("nvm");
        let wrapped = command_for_install_session_on_platform(command, &session, true);
        assert!(wrapped.contains("NVM_DIR"));
        assert!(wrapped.ends_with(command));

        assert_eq!(
            command_for_install_session_on_platform(command, &session, false),
            command
        );
    }

    #[test]
    fn wraps_node_and_npm_commands_after_node_is_installed_by_session() {
        let mut session = InstallSession::default();
        let node_check = "node -e 'console.log(process.version)'";
        let npm_install = "npm install -g gitnexus";

        assert_eq!(
            command_for_install_session_on_platform(node_check, &session, true),
            node_check
        );
        assert_eq!(
            command_for_install_session_on_platform(npm_install, &session, true),
            npm_install
        );

        session.mark_installed("nodejs");
        let wrapped_node = command_for_install_session_on_platform(node_check, &session, true);
        let wrapped_npm = command_for_install_session_on_platform(npm_install, &session, true);
        assert!(wrapped_node.contains("NVM_DIR"));
        assert!(wrapped_node.ends_with(node_check));
        assert!(wrapped_npm.contains("NVM_DIR"));
        assert!(wrapped_npm.ends_with(npm_install));

        assert_eq!(
            command_for_install_session_on_platform(npm_install, &session, false),
            npm_install
        );
    }

    #[test]
    fn detects_nvm_as_a_shell_command_token() {
        assert!(command_uses_nvm("nvm --version"));
        assert!(command_uses_nvm("nvm install 20.17.0"));
        assert!(!command_uses_nvm("echo NVM_NODEJS_ORG_MIRROR"));
        assert!(!command_uses_nvm("echo my-nvm-helper"));
    }

    #[test]
    fn detects_node_environment_shell_command_tokens() {
        assert!(command_uses_node_environment("node -e 'console.log(1)'"));
        assert!(command_uses_node_environment("npm install -g gitnexus"));
        assert!(command_uses_node_environment("nvm use 20.17.0"));
        assert!(!command_uses_node_environment("echo npm_config_registry"));
        assert!(!command_uses_node_environment("echo node-version"));
    }
}
