mod apt_mirror;
mod config;
mod constants;
mod discovery;
mod envfile;
mod error;
mod fsutil;
mod input;
mod installer;
mod models;
mod paths;
mod process;
mod proxy;
mod registry;
mod util;

pub use apt_mirror::{apply_apt_mirror, apt_mirror_preview, check_apt_mirror, AptMirrorPreview};
pub use config::{init_config, load_config, parse_config};
pub use constants::{BUILTIN_CONFIG, CONFIG_FILE, REGISTRY_FILE, SKILL_FILE};
pub use discovery::{discover_crates, discover_skills};
pub use error::ForgeError;
pub use installer::{
    doctor_report, install_crate_source, install_profile, install_skill_source, preview_install,
    print_preview, remove_installed, update_installed,
};
pub use models::{
    Agent, AptMirrorDef, AptMirrorRuleDef, CrateCandidate, InstallConfig, InstallItem, InstallKind,
    InstallOptions, InstallPreview, InstallReport, LoadedConfig, Profile, ProfileDef,
    RegistryEntry, SkillCandidate, SkillDef, SkillStatus, TagCheckDef, ToolDef, ToolStatus,
};
pub use paths::{app_home, config_dir, managed_bin_dir, manifest_config_path, registry_path};
pub use registry::{read_registry, write_registry};

#[cfg(test)]
mod tests {
    use super::config::builtin_cargo_tool;
    use super::util::now_secs;
    use super::*;
    use std::sync::Mutex;
    use std::{env, fs, path::PathBuf};

    static CWD_LOCK: Mutex<()> = Mutex::new(());

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

            [preinstall.linux]
            commands = ["sudo apt-get update"]

            [preinstall.standard.linux]
            commands = ["sudo apt-get install -y pkg-config libssl-dev"]

            [environment]
            cargo_config = ["[net]", "git-fetch-with-cli = true"]
            bashrc = [". \"$HOME/.cargo/env\""]
            npmrc = ["registry=https://mirror.com/npm/"]

            [tag_checks.proxy]
            check = "echo proxy-ok"

            [[tools]]
            name = "python"
            tags = ["proxy"]
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
        assert_eq!(config.preinstall.linux, vec!["sudo apt-get update"]);
        assert_eq!(
            config.preinstall.standard.linux,
            vec!["sudo apt-get install -y pkg-config libssl-dev"]
        );
        assert_eq!(
            config.environment.cargo_config,
            vec!["[net]", "git-fetch-with-cli = true"]
        );
        assert_eq!(config.environment.bashrc, vec![". \"$HOME/.cargo/env\""]);
        assert_eq!(
            config.environment.npmrc,
            vec!["registry=https://mirror.com/npm/"]
        );
        assert_eq!(config.tools[0].name, "python");
        assert_eq!(config.tools[0].tags, vec!["proxy"]);
        assert!(config.tag_checks.contains_key("proxy"));
        assert_eq!(
            config.skills[0].agents,
            vec![Agent::Claude, Agent::OpenCode]
        );
    }

    #[test]
    fn preview_install_inherits_lower_profiles() {
        let config = parse_config(
            r#"
            [profiles.light]
            tools = ["light-tool"]
            skills = []
            items = []

            [profiles.standard]
            tools = ["standard-tool"]
            skills = []
            items = []

            [profiles.full]
            tools = ["full-tool"]
            skills = []
            items = []

            [[tools]]
            name = "light-tool"
            check = "0"
            install = "0"

            [[tools]]
            name = "standard-tool"
            check = "0"
            install = "0"

            [[tools]]
            name = "full-tool"
            check = "0"
            install = "0"
            "#,
        )
        .unwrap();

        let standard = preview_install(&config, Profile::Standard).unwrap();
        let full = preview_install(&config, Profile::Full).unwrap();

        assert_eq!(
            standard
                .tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["light-tool", "standard-tool"]
        );
        assert_eq!(
            full.tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["light-tool", "standard-tool", "full-tool"]
        );
    }

    #[test]
    fn parses_apt_mirror_configuration() {
        let config = parse_config(
            r#"
            [apt_mirror]
            uri = "https://apt.internal.example/{distribution}"
            lines = []
            suites = ["{codename}", "{codename}-updates"]
            components = ["main", "universe"]
            architectures = ["{architecture}"]
            signed_by = "/usr/share/keyrings/internal.gpg"
            source_file = "/etc/apt/sources.list.d/rsenvforge.sources"

            [[apt_mirror.rules]]
            distribution = "ubuntu"
            architecture = "amd64"
            uri = "https://amd64.internal.example/ubuntu"
            lines = [
              "deb https://amd64.internal.example/ubuntu {codename} main universe",
              "deb https://amd64.internal.example/ubuntu {codename}-updates main universe",
            ]
            "#,
        )
        .unwrap();

        assert_eq!(
            config.apt_mirror.uri.as_deref(),
            Some("https://apt.internal.example/{distribution}")
        );
        assert_eq!(
            config.apt_mirror.suites,
            vec!["{codename}".to_string(), "{codename}-updates".to_string()]
        );
        assert_eq!(
            config.apt_mirror.components,
            vec!["main".to_string(), "universe".to_string()]
        );
        assert_eq!(
            config.apt_mirror.architectures,
            vec!["{architecture}".to_string()]
        );
        assert_eq!(config.apt_mirror.rules.len(), 1);
        assert_eq!(
            config.apt_mirror.rules[0].uri.as_deref(),
            Some("https://amd64.internal.example/ubuntu")
        );
        assert_eq!(
            config.apt_mirror.rules[0].lines,
            vec![
                "deb https://amd64.internal.example/ubuntu {codename} main universe".to_string(),
                "deb https://amd64.internal.example/ubuntu {codename}-updates main universe"
                    .to_string()
            ]
        );
    }

    #[test]
    fn builtin_profiles_match_configured_tool_lists() {
        let config = parse_config(BUILTIN_CONFIG).unwrap();
        let light = &config.profiles["light"].tools;
        let standard = &config.profiles["standard"].tools;
        let full = &config.profiles["full"].tools;
        assert!(full.is_empty());
        for tool in ["gnu", "msvc"] {
            assert!(light.contains(&tool.to_string()));
            assert!(config.tools.iter().any(|candidate| candidate.name == tool));
        }
        assert!(light.contains(&"cargo-llvm-cov".to_string()));
        for tool in [
            "cargo-llvm-cov",
            "bindgen-cli",
            "cargo-audit",
            "cargo-deny",
            "cargo-geiger",
            "rust-analyzer",
            "miri",
            "cargo-expand",
            "cargo-fuzz",
            "cargo-udeps",
            "cargo-bloat",
            "flamegraph-rs",
            "cargo-msrv",
            "cargo-semver-checks",
            "rust-checker-cli",
            "rustbot-cli",
        ] {
            assert!(light.contains(&tool.to_string()));
            assert!(builtin_cargo_tool(tool).is_some());
        }
        for tool in ["python", "ninja", "valgrind"] {
            assert!(light.contains(&tool.to_string()));
            assert!(config.tools.iter().any(|candidate| candidate.name == tool));
        }
        assert!(!light.contains(&"cpp2rust-demo".to_string()));
        assert!(!light.contains(&"c2rust-demo".to_string()));
        for tool in [
            "rust-build-base",
            "rust-toolchain",
            "nvm",
            "nodejs",
            "gitnexus",
        ] {
            assert!(standard.contains(&tool.to_string()));
        }
        let nvm_index = standard.iter().position(|tool| tool == "nvm").unwrap();
        let nodejs_index = standard.iter().position(|tool| tool == "nodejs").unwrap();
        let gitnexus_index = standard.iter().position(|tool| tool == "gitnexus").unwrap();
        assert!(nvm_index < nodejs_index);
        assert!(nodejs_index < gitnexus_index);

        let tool_names = config
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>();
        let nvm_def_index = tool_names.iter().position(|tool| *tool == "nvm").unwrap();
        let nodejs_def_index = tool_names
            .iter()
            .position(|tool| *tool == "nodejs")
            .unwrap();
        let gitnexus_def_index = tool_names
            .iter()
            .position(|tool| *tool == "gitnexus")
            .unwrap();
        assert!(nvm_def_index < nodejs_def_index);
        assert!(nodejs_def_index < gitnexus_def_index);
    }

    #[test]
    fn builtin_config_puts_environment_and_preinstall_first() {
        let environment_index = BUILTIN_CONFIG.find("[environment]").unwrap();
        let preinstall_index = BUILTIN_CONFIG.find("[preinstall.light.linux]").unwrap();
        let profiles_index = BUILTIN_CONFIG.find("[profiles.light]").unwrap();

        assert!(environment_index < preinstall_index);
        assert!(preinstall_index < profiles_index);
    }

    #[test]
    fn builtin_tag_checks_match_install_commands() {
        let config = parse_config(BUILTIN_CONFIG).unwrap();
        for tag in [
            "rustup-mirror",
            "cargo-mirror",
            "cargo-install",
            "nvm-mirror",
            "node20",
            "apt-mirror",
            "npm-mirror",
        ] {
            let check = config.tag_checks.get(tag).unwrap();
            assert!(check.check_command().is_some());
        }
        for tool in [
            "cargo-llvm-cov",
            "bindgen-cli",
            "cargo-audit",
            "cargo-deny",
            "cargo-geiger",
            "cargo-expand",
            "cargo-fuzz",
            "cargo-udeps",
            "cargo-bloat",
            "flamegraph-rs",
            "cargo-msrv",
            "cargo-semver-checks",
            "rust-checker-cli",
            "rustbot-cli",
        ] {
            let tool = config
                .tools
                .iter()
                .find(|candidate| candidate.name == tool)
                .unwrap();
            assert_eq!(tool.tags, vec!["cargo-install".to_string()]);
        }
        let rust = config
            .tools
            .iter()
            .find(|candidate| candidate.name == "rust-toolchain")
            .unwrap();
        assert!(rust.tags.is_empty());
        for tool in ["rust-analyzer", "miri"] {
            let tool = config
                .tools
                .iter()
                .find(|candidate| candidate.name == tool)
                .unwrap();
            assert_eq!(tool.tags, vec!["rustup-mirror".to_string()]);
        }
        let nodejs = config
            .tools
            .iter()
            .find(|candidate| candidate.name == "nodejs")
            .unwrap();
        assert!(nodejs.tags.is_empty());
        let nvm = config
            .tools
            .iter()
            .find(|candidate| candidate.name == "nvm")
            .unwrap();
        assert!(nvm.tags.is_empty());
        let gitnexus = config
            .tools
            .iter()
            .find(|candidate| candidate.name == "gitnexus")
            .unwrap();
        assert_eq!(
            gitnexus.tags,
            vec!["node20".to_string(), "npm-mirror".to_string()]
        );
        assert!(gitnexus
            .install_linux
            .as_deref()
            .unwrap()
            .contains("--ignore-scripts"));
        assert!(gitnexus
            .install_windows
            .as_deref()
            .unwrap()
            .contains("--ignore-scripts"));
    }

    #[test]
    fn load_config_falls_back_to_manifest_config() {
        let _guard = CWD_LOCK.lock().unwrap();
        let temp = test_dir("load_config_falls_back_to_manifest_config");
        let config_dir = temp.join("config");
        fs::create_dir_all(&config_dir).unwrap();
        let old_dir = env::current_dir().unwrap();
        let old_config_dir = env::var_os("RSENVFORGE_CONFIG_DIR");

        env::set_current_dir(&temp).unwrap();
        env::set_var("RSENVFORGE_CONFIG_DIR", &config_dir);
        let loaded = load_config(None).unwrap();

        env::set_current_dir(old_dir).unwrap();
        match old_config_dir {
            Some(value) => env::set_var("RSENVFORGE_CONFIG_DIR", value),
            None => env::remove_var("RSENVFORGE_CONFIG_DIR"),
        }
        fs::remove_dir_all(temp).unwrap();

        assert_eq!(loaded.path, Some(manifest_config_path()));
        assert!(!loaded.builtin);
    }

    #[test]
    fn custom_config_without_apt_mirror_does_not_inherit_builtin_mirror() {
        let _guard = CWD_LOCK.lock().unwrap();
        let temp = test_dir("custom_config_without_apt_mirror_does_not_inherit_builtin_mirror");
        let config = temp.join(CONFIG_FILE);
        fs::create_dir_all(&temp).unwrap();
        fs::write(
            &config,
            r#"
            [profiles.light]
            tools = []
            skills = []
            items = []

            [profiles.standard]
            tools = []
            skills = []
            items = []

            [profiles.full]
            tools = []
            skills = []
            items = []
            "#,
        )
        .unwrap();

        let builtin = parse_config(BUILTIN_CONFIG).unwrap();
        let loaded = load_config(Some(&config)).unwrap();

        fs::remove_dir_all(temp).unwrap();

        assert!(builtin.apt_mirror.uri.is_some());
        assert!(loaded.config.apt_mirror.uri.is_none());
        assert!(loaded.config.apt_mirror.lines.is_empty());
        assert!(loaded.config.apt_mirror.rules.is_empty());
    }

    #[test]
    fn zero_check_marks_tool_as_unsupported() {
        let config = parse_config(
            r#"
            [profiles.light]
            tools = ["unsupported-demo"]
            skills = []
            items = []

            [profiles.standard]
            tools = ["unsupported-demo"]
            skills = []
            items = []

            [profiles.full]
            tools = ["unsupported-demo"]
            skills = []
            items = []

            [[tools]]
            name = "unsupported-demo"
            check_windows = "0"
            check_linux = "0"
            install_windows = "0"
            install_linux = "0"
            "#,
        )
        .unwrap();

        let preview = preview_install(&config, Profile::Light).unwrap();
        assert_eq!(preview.tools[0].name, "unsupported-demo");
        assert!(!preview.tools[0].supported);
        assert!(preview.missing_tools().is_empty());
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
