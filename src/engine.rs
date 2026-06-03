mod config;
mod constants;
mod discovery;
mod error;
mod fsutil;
mod installer;
mod models;
mod paths;
mod process;
mod registry;
mod util;

pub use config::{load_config, parse_config};
pub use constants::{BUILTIN_CONFIG, CONFIG_FILE, REGISTRY_FILE, SKILL_FILE};
pub use discovery::{discover_crates, discover_skills};
pub use error::ForgeError;
pub use installer::{
    doctor_report, install_crate_source, install_profile, install_skill_source, preview_install,
    print_preview, update_installed,
};
pub use models::{
    Agent, CrateCandidate, InstallConfig, InstallItem, InstallKind, InstallOptions, InstallPreview,
    InstallReport, LoadedConfig, Profile, ProfileDef, RegistryEntry, SkillCandidate, SkillDef,
    SkillStatus, ToolDef, ToolStatus,
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
        assert_eq!(config.preinstall.linux, vec!["sudo apt-get update"]);
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
