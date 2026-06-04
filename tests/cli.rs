use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs, path::PathBuf};

#[test]
fn install_defaults_to_standard_profile() {
    let temp = test_dir("install_defaults_to_standard_profile");
    let source = temp.join("source");
    let skill = source.join("skills").join("standard-skill");
    let home = temp.join("home");
    let claude = temp.join("claude-skills");
    fs::create_dir_all(&claude).unwrap();
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: standard-skill\n---\n# standard\n",
    )
    .unwrap();
    fs::write(
        temp.join("rsenvforge.toml"),
        format!(
            r#"
            [profiles.light]
            items = []
            [profiles.standard]
            items = ["standard-skill"]
            [profiles.full]
            items = ["standard-skill"]
            [[items]]
            name = "standard-skill"
            kind = "skill"
            source = "{}"
            agents = ["claude"]
            "#,
            path_for_config(&source)
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .current_dir(&temp)
        .env("RSENVFORGE_HOME", &home)
        .env("RSENVFORGE_CLAUDE_DIR", &claude)
        .args(["install", "--force"])
        .output()
        .unwrap();

    assert_success(&output);
    assert!(claude.join("standard-skill").join("SKILL.md").is_file());
    assert!(home.join("registry.tsv").is_file());

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn install_light_and_full_use_selected_profiles() {
    let temp = test_dir("install_light_and_full_use_selected_profiles");
    let source = temp.join("source");
    let light_skill = source.join("skills").join("light-skill");
    let full_skill = source.join("skills").join("full-skill");
    let home = temp.join("home");
    let claude = temp.join("claude-skills");
    fs::create_dir_all(&claude).unwrap();
    fs::create_dir_all(&light_skill).unwrap();
    fs::create_dir_all(&full_skill).unwrap();
    fs::write(
        light_skill.join("SKILL.md"),
        "---\nname: light-skill\n---\n# light\n",
    )
    .unwrap();
    fs::write(
        full_skill.join("SKILL.md"),
        "---\nname: full-skill\n---\n# full\n",
    )
    .unwrap();
    let config = temp.join("rsenvforge.toml");
    fs::write(
        &config,
        format!(
            r#"
            [profiles.light]
            items = ["light"]
            [profiles.standard]
            items = ["light"]
            [profiles.full]
            items = ["light", "full"]
            [[items]]
            name = "light"
            kind = "skill"
            source = "{}"
            agents = ["claude"]
            [[items]]
            name = "full"
            kind = "skill"
            source = "{}"
            agents = ["claude"]
            "#,
            path_for_config(&light_skill),
            path_for_config(&full_skill)
        ),
    )
    .unwrap();

    let light = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .env("RSENVFORGE_CLAUDE_DIR", &claude)
        .args([
            "install",
            "light",
            "--config",
            config.to_str().unwrap(),
            "--force",
        ])
        .output()
        .unwrap();
    assert_success(&light);
    assert!(claude.join("light-skill").is_dir());
    assert!(!claude.join("full-skill").exists());

    let full = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .env("RSENVFORGE_CLAUDE_DIR", &claude)
        .args([
            "install",
            "full",
            "--config",
            config.to_str().unwrap(),
            "--force",
        ])
        .output()
        .unwrap();
    assert_success(&full);
    assert!(claude.join("full-skill").is_dir());

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn install_skill_supports_opencode_target() {
    let temp = test_dir("install_skill_supports_opencode_target");
    let source = temp.join("source");
    let skill = source.join("skills").join("open-skill");
    let home = temp.join("home");
    let opencode = temp.join("opencode-skills");
    fs::create_dir_all(&opencode).unwrap();
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: open-skill\n---\n# open\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .env("RSENVFORGE_OPENCODE_DIR", &opencode)
        .args([
            "install-skill",
            source.to_str().unwrap(),
            "--agent",
            "opencode",
            "--force",
        ])
        .output()
        .unwrap();

    assert_success(&output);
    assert!(opencode.join("open-skill").join("SKILL.md").is_file());

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn install_crate_uses_prebuilt_binary_with_norustup() {
    let temp = test_dir("install_crate_uses_prebuilt_binary_with_norustup");
    let source = temp.join("tool");
    let home = temp.join("home");
    let bin_dir = temp.join("managed-bin");
    fs::create_dir_all(source.join("src")).unwrap();
    fs::create_dir_all(source.join("dist")).unwrap();
    fs::write(
        source.join("Cargo.toml"),
        "[package]\nname = \"demo-tool\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(source.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(source.join("dist").join(exe_name("demo-tool")), "prebuilt").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .env("RSENVFORGE_BIN_DIR", &bin_dir)
        .args([
            "install-crate",
            source.to_str().unwrap(),
            "--norustup",
            "--force",
        ])
        .output()
        .unwrap();

    assert_success(&output);
    assert!(bin_dir.join(exe_name("demo-tool")).is_file());

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn update_reinstalls_registry_items() {
    let temp = test_dir("update_reinstalls_registry_items");
    let source = temp.join("source");
    let skill = source.join("skills").join("updatable");
    let home = temp.join("home");
    let claude = temp.join("claude-skills");
    fs::create_dir_all(&claude).unwrap();
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), "---\nname: updatable\n---\n# v1\n").unwrap();

    let install = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .env("RSENVFORGE_CLAUDE_DIR", &claude)
        .args([
            "install-skill",
            source.to_str().unwrap(),
            "--agent",
            "claude",
            "--force",
        ])
        .output()
        .unwrap();
    assert_success(&install);

    fs::write(skill.join("extra.txt"), "updated").unwrap();
    let update = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .env("RSENVFORGE_CLAUDE_DIR", &claude)
        .args(["update", "--force"])
        .output()
        .unwrap();

    assert_success(&update);
    assert!(claude.join("updatable").join("extra.txt").is_file());

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn remove_deletes_registry_targets() {
    let temp = test_dir("remove_deletes_registry_targets");
    let source = temp.join("source");
    let skill = source.join("skills").join("removable");
    let home = temp.join("home");
    let claude = temp.join("claude-skills");
    fs::create_dir_all(&claude).unwrap();
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: removable\n---\n# demo\n",
    )
    .unwrap();

    let install = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .env("RSENVFORGE_CLAUDE_DIR", &claude)
        .args([
            "install-skill",
            source.to_str().unwrap(),
            "--agent",
            "claude",
            "--force",
        ])
        .output()
        .unwrap();
    assert_success(&install);
    assert!(claude.join("removable").is_dir());

    let remove = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .args(["remove", "source", "--kind", "skill", "--force"])
        .output()
        .unwrap();
    assert_success(&remove);
    assert!(!claude.join("removable").exists());

    let list = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", &home)
        .args(["list"])
        .output()
        .unwrap();
    assert_success(&list);
    assert!(!String::from_utf8_lossy(&list.stdout).contains("source"));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn install_declines_when_user_answers_n() {
    let temp = test_dir("install_declines_when_user_answers_n");
    let home = temp.join("home");
    let config = temp.join("rsenvforge.toml");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &config,
        r#"
        [profiles.light]
        tools = ["missing-demo"]
        skills = []
        items = []
        [profiles.standard]
        tools = ["missing-demo"]
        skills = []
        items = []
        [profiles.full]
        tools = ["missing-demo"]
        skills = []
        items = []
        [[tools]]
        name = "missing-demo"
        check = "definitely-missing-rsenvforge-demo --version"
        install = "echo install-demo"
        "#,
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "N\n",
    );

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Y/N"));
    assert!(!home.join("registry.tsv").exists());

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn non_installable_tool_stops_with_message() {
    let temp = test_dir("non_installable_tool_stops_with_message");
    let home = temp.join("home");
    let config = temp.join("rsenvforge.toml");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &config,
        r#"
        [profiles.light]
        tools = ["unknown-system-tool"]
        skills = []
        items = []
        [profiles.standard]
        tools = ["unknown-system-tool"]
        skills = []
        items = []
        [profiles.full]
        tools = ["unknown-system-tool"]
        skills = []
        items = []
        [[tools]]
        name = "unknown-system-tool"
        check = "definitely-missing-rsenvforge-demo --version"
        "#,
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "Y\n",
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("安装命令"));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn missing_agent_skill_dir_skips_skill_install() {
    let temp = test_dir("missing_agent_skill_dir_skips_skill_install");
    let source = temp.join("source");
    let skill = source.join("skills").join("demo-skill");
    let home = temp.join("home");
    let missing_claude = temp.join("missing-claude-skills");
    let config = temp.join("rsenvforge.toml");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: demo-skill\n---\n# demo\n",
    )
    .unwrap();
    fs::write(
        &config,
        format!(
            r#"
            [profiles.light]
            tools = []
            skills = ["demo-skill"]
            items = []
            [profiles.standard]
            tools = []
            skills = ["demo-skill"]
            items = []
            [profiles.full]
            tools = []
            skills = ["demo-skill"]
            items = []
            [[skills]]
            name = "demo-skill"
            source = "{}"
            agents = ["claude"]
            "#,
            path_for_config(&source)
        ),
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .env("RSENVFORGE_HOME", &home)
            .env("RSENVFORGE_CLAUDE_DIR", &missing_claude)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "Y\n",
    );

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skill"));
    assert!(!missing_claude.exists());

    fs::remove_dir_all(temp).unwrap();
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn command_with_input(command: &mut Command, input: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn path_for_config(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn exe_name(bin: &str) -> String {
    if cfg!(windows) {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    }
}

fn test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!(
        "rsenvforge-cli-{name}-{nanos}-{}",
        std::process::id()
    ))
}
