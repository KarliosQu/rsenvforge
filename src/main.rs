use std::env;
use std::path::PathBuf;
use std::process;

use rsenvforge::{
    doctor_report, install_crate_source, install_profile, install_skill_source, read_registry,
    remove_installed, update_installed, Agent, InstallKind, InstallOptions, Profile,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("错误：{error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };

    match command {
        "install" => cmd_install(&args[1..]),
        "install-skill" => cmd_install_skill(&args[1..]),
        "install-crate" => cmd_install_crate(&args[1..]),
        "update" => cmd_update(&args[1..]),
        "remove" | "uninstall" => cmd_remove(&args[1..]),
        "list" => cmd_list(&args[1..]),
        "doctor" => cmd_doctor(&args[1..]),
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        "-V" | "--version" | "version" => {
            println!("rsenvforge {VERSION}");
            Ok(())
        }
        unknown => Err(format!("未知命令：{unknown}")),
    }
}

fn cmd_install(args: &[String]) -> Result<(), String> {
    let mut options = InstallOptions::default();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                index += 1;
                options.config_path = Some(PathBuf::from(value_after(args, index, "--config")?));
            }
            "--force" | "-f" => options.force = true,
            "--norustup" => options.norustup = true,
            value if Profile::parse(value).is_some() => {
                options.profile = Profile::parse(value).expect("checked above");
            }
            value if value.starts_with('-') => return Err(format!("未知 install 选项：{value}")),
            value => return Err(format!("未知安装等级：{value}")),
        }
        index += 1;
    }

    let report = install_profile(&options).map_err(|error| error.to_string())?;
    println!(
        "安装流程完成：{} 条安装记录，等级 {}",
        report.entries.len(),
        options.profile.as_str()
    );
    for entry in report.entries {
        println!(
            "{} {} -> {} 个目标",
            entry.kind.as_str(),
            entry.name,
            entry.targets.len()
        );
    }
    Ok(())
}

fn cmd_install_skill(args: &[String]) -> Result<(), String> {
    let mut source = None;
    let mut agents = Vec::new();
    let mut force = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--agent" => {
                index += 1;
                agents = parse_agent_list(value_after(args, index, "--agent")?)?;
            }
            "--force" | "-f" => force = true,
            value if value.starts_with('-') => {
                return Err(format!("未知 install-skill 选项：{value}"))
            }
            value => {
                if source.replace(value.to_string()).is_some() {
                    return Err("install-skill 只能接受一个 source".to_string());
                }
            }
        }
        index += 1;
    }

    let source = source.ok_or_else(|| "缺少 source".to_string())?;
    if agents.is_empty() {
        return Err("install-skill 需要 --agent claude|opencode|both".to_string());
    }
    let entries =
        install_skill_source(&source, &agents, force).map_err(|error| error.to_string())?;
    println!("已安装 {} 条 skill 记录", entries.len());
    Ok(())
}

fn cmd_install_crate(args: &[String]) -> Result<(), String> {
    let mut source = None;
    let mut bins = Vec::new();
    let mut force = false;
    let mut norustup = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--bin" => {
                index += 1;
                bins.push(value_after(args, index, "--bin")?.to_string());
            }
            "--force" | "-f" => force = true,
            "--norustup" => norustup = true,
            value if value.starts_with('-') => {
                return Err(format!("未知 install-crate 选项：{value}"))
            }
            value => {
                if source.replace(value.to_string()).is_some() {
                    return Err("install-crate 只能接受一个 source".to_string());
                }
            }
        }
        index += 1;
    }

    let source = source.ok_or_else(|| "缺少 source".to_string())?;
    let entries =
        install_crate_source(&source, &bins, force, norustup).map_err(|error| error.to_string())?;
    println!("已安装 {} 条 crate 记录", entries.len());
    Ok(())
}

fn cmd_update(args: &[String]) -> Result<(), String> {
    let mut force = false;
    let mut norustup = false;
    for arg in args {
        match arg.as_str() {
            "--force" | "-f" => force = true,
            "--norustup" => norustup = true,
            value => return Err(format!("未知 update 选项：{value}")),
        }
    }

    let entries = update_installed(force, norustup).map_err(|error| error.to_string())?;
    println!("已更新 {} 条安装记录", entries.len());
    Ok(())
}

fn cmd_remove(args: &[String]) -> Result<(), String> {
    let mut name = None;
    let mut kind = None;
    let mut force = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--kind" => {
                index += 1;
                kind = Some(parse_install_kind(value_after(args, index, "--kind")?)?);
            }
            "--force" | "-f" => force = true,
            value if value.starts_with('-') => return Err(format!("未知 remove 选项：{value}")),
            value => {
                if name.replace(value.to_string()).is_some() {
                    return Err("remove 只能接受一个名称".to_string());
                }
            }
        }
        index += 1;
    }

    let name = name.ok_or_else(|| "缺少要删除的安装项名称".to_string())?;
    let removed = remove_installed(&name, kind, force).map_err(|error| error.to_string())?;
    println!("已删除 {} 条安装记录", removed.len());
    Ok(())
}

fn cmd_list(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("list 不接受参数".to_string());
    }
    let entries = read_registry().map_err(|error| error.to_string())?;
    if entries.is_empty() {
        println!("暂无安装记录");
    } else {
        for entry in entries {
            println!(
                "{}\t{}\t{}\t{}\t{} target(s)",
                entry.kind.as_str(),
                entry.name,
                entry.profile,
                entry.source,
                entry.targets.len()
            );
        }
    }
    Ok(())
}

fn cmd_doctor(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("doctor 不接受参数".to_string());
    }
    for line in doctor_report() {
        println!("{line}");
    }
    Ok(())
}

fn parse_install_kind(value: &str) -> Result<InstallKind, String> {
    match value {
        "skill" => Ok(InstallKind::Skill),
        "crate" => Ok(InstallKind::Crate),
        _ => Err(format!("未知安装项类型：{value}，可用值为 skill|crate")),
    }
}

fn parse_agent_list(value: &str) -> Result<Vec<Agent>, String> {
    if value == "both" {
        return Ok(vec![Agent::Claude, Agent::OpenCode]);
    }
    value
        .split(',')
        .map(|agent| Agent::parse(agent.trim()).map_err(|error| error.to_string()))
        .collect()
}

fn value_after<'a>(args: &'a [String], index: usize, option: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{option} 后缺少值"))
}

fn print_help() {
    println!(
        "rsenvforge {VERSION}

用法：
    rsenvforge <command> [options]

命令：
    install [light|standard|full]  从 rsenvforge.toml 检测并安装，默认 standard
    install-skill <source>         从 Git 地址或本地路径安装 agent skill
    install-crate <source>         从 Git 地址或本地路径安装 Rust binary crate
    update                         更新 rsenvforge 记录过的安装项
    remove <name>                  删除 rsenvforge 记录过的安装项
    list                           显示安装记录
    doctor                         检查本地工具和管理目录
    help                           显示帮助
    version                        显示版本

选项：
    install --config <path>        使用自定义安装表单
    install --force                覆盖已有 skill/tool 目标
    install --norustup             crate 安装时跳过 rustup 检查
    install-skill --agent <value>  claude、opencode 或 both
    install-crate --bin <name>     只安装指定 binary
    remove --kind <value>          skill 或 crate
    remove --force                 跳过删除确认
"
    );
}
