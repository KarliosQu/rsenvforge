use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs, path::PathBuf};

#[test]
fn init_creates_default_config() {
    let temp = test_dir("init_creates_default_config");
    fs::create_dir_all(&temp).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .current_dir(&temp)
        .args(["init"])
        .output()
        .unwrap();

    assert_success(&output);
    let config = temp.join("rsenvforge.toml");
    assert!(config.is_file());
    let text = fs::read_to_string(&config).unwrap();
    let expected =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rsenvforge.toml"))
            .unwrap();
    assert_eq!(text, expected);
    assert!(text.contains("[profiles.standard]"));
    assert!(text.contains("nodejs"));
    assert!(text.contains("cargo-llvm-cov"));
    assert!(!text.contains("openspec"));

    let second = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .current_dir(&temp)
        .args(["init"])
        .output()
        .unwrap();
    assert!(!second.status.success());

    let forced = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .current_dir(&temp)
        .args(["init", "--force"])
        .output()
        .unwrap();
    assert_success(&forced);

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn doctor_reports_proxy_settings() {
    let temp = test_dir("doctor_reports_proxy_settings");
    let home = temp.join("home");
    let cargo_home = temp.join("cargo-home");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cargo_home).unwrap();
    fs::write(
        cargo_home.join("config.toml"),
        "[http]\nproxy = \"http://user:pass@127.0.0.1:7890\"\n[build]\nrustflags = [\"-C\", \"target-cpu=native\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
        .env("RSENVFORGE_HOME", temp.join("forge-home"))
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("CARGO_HOME", &cargo_home)
        .env("http_proxy", "http://127.0.0.1:7890")
        .env("https_proxy", "http://127.0.0.1:7891")
        .args(["doctor"])
        .output()
        .unwrap();

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("代理检查"));
    assert!(stdout.contains("http_proxy：http://127.0.0.1:7890"));
    assert!(stdout.contains("https_proxy：http://127.0.0.1:7891"));
    assert!(stdout.contains("Cargo config："));
    assert!(stdout.contains("----- begin config.toml -----"));
    assert!(stdout.contains("proxy = \"http://***@127.0.0.1:7890\""));
    assert!(stdout.contains("[build]"));
    assert!(stdout.contains("rustflags = [\"-C\", \"target-cpu=native\"]"));
    assert!(stdout.contains("----- end config.toml -----"));

    fs::remove_dir_all(temp).unwrap();
}

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
fn preinstall_commands_are_profile_scoped() {
    let temp = test_dir("preinstall_commands_are_profile_scoped");
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
        [preinstall.standard.windows]
        commands = ["echo preinstall-standard"]
        [preinstall.standard.linux]
        commands = ["echo preinstall-standard"]
        [[tools]]
        name = "missing-demo"
        check_windows = "definitely-missing-rsenvforge-demo --version"
        check_linux = "definitely-missing-rsenvforge-demo --version"
        install_windows = "echo install missing-demo"
        install_linux = "echo install missing-demo"
        "#,
    )
    .unwrap();

    let light = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "Y\n",
    );
    assert_success(&light);
    assert!(!String::from_utf8_lossy(&light.stdout).contains("preinstall-standard"));

    let standard = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "standard", "--config", config.to_str().unwrap()]),
        "Y\n",
    );
    assert_success(&standard);
    assert!(String::from_utf8_lossy(&standard.stdout).contains("preinstall-standard"));

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
fn failed_tool_install_can_be_skipped() {
    let temp = test_dir("failed_tool_install_can_be_skipped");
    let home = temp.join("home");
    let config = temp.join("rsenvforge.toml");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &config,
        r#"
        [profiles.light]
        tools = ["fail-tool", "ok-tool"]
        skills = []
        items = []
        [profiles.standard]
        tools = ["fail-tool", "ok-tool"]
        skills = []
        items = []
        [profiles.full]
        tools = ["fail-tool", "ok-tool"]
        skills = []
        items = []
        [[tools]]
        name = "fail-tool"
        check = "definitely-missing-rsenvforge-fail-tool --version"
        install = "definitely-missing-rsenvforge-fail-tool-install"
        [[tools]]
        name = "ok-tool"
        check = "definitely-missing-rsenvforge-ok-tool --version"
        install = "echo install-ok-tool"
        "#,
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "Y\nY\n",
    );

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("是否跳过该工具继续安装"));
    assert!(stdout.contains("已跳过工具：fail-tool"));
    assert!(stdout.contains("开始安装工具：ok-tool"));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn failed_tag_check_can_skip_tool_before_install() {
    let temp = test_dir("failed_tag_check_can_skip_tool_before_install");
    let home = temp.join("home");
    let config = temp.join("rsenvforge.toml");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &config,
        r#"
        [profiles.light]
        tools = ["tagged-tool", "ok-tool"]
        skills = []
        items = []
        [profiles.standard]
        tools = ["tagged-tool", "ok-tool"]
        skills = []
        items = []
        [profiles.full]
        tools = ["tagged-tool", "ok-tool"]
        skills = []
        items = []
        [tag_checks.proxy]
        check = "definitely-missing-rsenvforge-proxy-check"
        [[tools]]
        name = "tagged-tool"
        tags = ["proxy"]
        check = "definitely-missing-rsenvforge-tagged-tool --version"
        install = "echo install-tagged-tool"
        [[tools]]
        name = "ok-tool"
        check = "definitely-missing-rsenvforge-ok-tool --version"
        install = "echo install-ok-tool"
        "#,
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "Y\nY\n",
    );

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("标签检查 proxy 未通过"));
    assert!(stdout.contains("安装前检查未通过，是否跳过该工具继续安装"));
    assert!(stdout.contains("已跳过工具：tagged-tool"));
    assert!(!stdout.contains("开始安装工具：tagged-tool"));
    assert!(stdout.contains("开始安装工具：ok-tool"));

    fs::remove_dir_all(temp).unwrap();
}

#[cfg(windows)]
#[test]
fn unsupported_windows_tag_check_can_skip_tool() {
    let temp = test_dir("unsupported_windows_tag_check_can_skip_tool");
    let home = temp.join("home");
    let config = temp.join("rsenvforge.toml");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &config,
        r#"
        [profiles.light]
        tools = ["apt-dependent"]
        skills = []
        items = []
        [profiles.standard]
        tools = ["apt-dependent"]
        skills = []
        items = []
        [profiles.full]
        tools = []
        skills = []
        items = []
        [tag_checks.apt-mirror]
        check_windows = "0"
        check_linux = "true"
        [[tools]]
        name = "apt-dependent"
        tags = ["apt-mirror"]
        check = "definitely-missing-rsenvforge-apt-dependent --version"
        install = "echo install-apt-dependent"
        "#,
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "Y\nY\n",
    );

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("标签 apt-mirror 不支持当前平台测试"));
    assert!(stdout.contains("已跳过工具：apt-dependent"));
    assert!(!stdout.contains("开始安装工具：apt-dependent"));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn tool_post_install_runs_after_install() {
    let temp = test_dir("tool_post_install_runs_after_install");
    let home = temp.join("home");
    let config = temp.join("rsenvforge.toml");
    let marker = temp.join("post-install-marker.txt");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &config,
        r#"
            [profiles.light]
            tools = ["post-demo"]
            skills = []
            items = []
            [profiles.standard]
            tools = ["post-demo"]
            skills = []
            items = []
            [profiles.full]
            tools = ["post-demo"]
            skills = []
            items = []
            [[tools]]
            name = "post-demo"
            check = "definitely-missing-rsenvforge-post-demo --version"
            install = "echo install-post-demo"
            post_install = "echo post-demo > post-install-marker.txt"
            "#,
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .current_dir(&temp)
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "Y\n",
    );

    assert_success(&output);
    assert!(marker.is_file());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("开始运行工具安装后命令：post-demo"));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn installed_tool_post_install_runs_after_confirmation() {
    let temp = test_dir("installed_tool_post_install_runs_after_confirmation");
    let home = temp.join("home");
    let config = temp.join("rsenvforge.toml");
    let marker = temp.join("installed-post-marker.txt");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &config,
        r#"
            [profiles.light]
            tools = ["installed-post-demo"]
            skills = []
            items = []
            [profiles.standard]
            tools = ["installed-post-demo"]
            skills = []
            items = []
            [profiles.full]
            tools = ["installed-post-demo"]
            skills = []
            items = []
            [[tools]]
            name = "installed-post-demo"
            check = "echo installed-post-demo 1.0.0"
            install = "echo install-installed-post-demo"
            post_install = "echo installed-post-demo > installed-post-marker.txt"
            "#,
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .current_dir(&temp)
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "Y\n",
    );

    assert_success(&output);
    assert!(marker.is_file());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("已安装且配置了安装后命令，是否运行该命令"));
    assert!(stdout.contains("开始运行工具安装后命令：installed-post-demo"));

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn installed_tool_post_install_can_be_declined() {
    let temp = test_dir("installed_tool_post_install_can_be_declined");
    let home = temp.join("home");
    let config = temp.join("rsenvforge.toml");
    let marker = temp.join("installed-post-marker.txt");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &config,
        r#"
            [profiles.light]
            tools = ["installed-post-demo"]
            skills = []
            items = []
            [profiles.standard]
            tools = ["installed-post-demo"]
            skills = []
            items = []
            [profiles.full]
            tools = ["installed-post-demo"]
            skills = []
            items = []
            [[tools]]
            name = "installed-post-demo"
            check = "echo installed-post-demo 1.0.0"
            install = "echo install-installed-post-demo"
            post_install = "echo installed-post-demo > installed-post-marker.txt"
            "#,
    )
    .unwrap();

    let output = command_with_input(
        Command::new(env!("CARGO_BIN_EXE_rsenvforge"))
            .current_dir(&temp)
            .env("RSENVFORGE_HOME", &home)
            .args(["install", "light", "--config", config.to_str().unwrap()]),
        "N\n",
    );

    assert_success(&output);
    assert!(!marker.exists());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("已跳过工具安装后命令：installed-post-demo"));

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
