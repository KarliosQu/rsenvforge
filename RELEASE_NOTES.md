# Release Notes

## Unreleased

### 新增

- standard 等级新增 `rust-analyzer`、`miri`、`cargo-expand`、`cargo-fuzz`、`rust-lldb`。

## 0.1.2 - 2026-06-05

本版本增强了初始化、安装前诊断和长时间安装体验。

### 新增

- 新增 `rsenvforge init [--force]`，可在当前目录生成默认 `rsenvforge.toml`。
- `doctor` 和 `install` 会输出代理检查信息，包括 `http_proxy`、`https_proxy`、Cargo 配置文件代理，以及 Linux 下的 `~/.bashrc` 代理线索。
- 新增分级 `preinstall`，支持 `[preinstall.light.*]`、`[preinstall.standard.*]`、`[preinstall.full.*]`。
- 安装命令运行较久时会在 `120`、`240`、`480`、`960` 秒等节点输出当前进度，但不会中断安装。

### 改进

- 默认内置配置直接来自仓库根目录 `rsenvforge.toml`，保证 `init` 生成内容与当前版本默认配置一致。
- 代理地址输出会脱敏 `user:password@` 形式的凭据信息。
- Linux 下 `pkg-config` 和 `libssl-dev` 被放入 `standard` 级别的预安装命令，用于满足 `cargo-geiger` 的系统依赖；`light` 不会执行该命令。

## 0.1.1 - 2026-06-04

本版本完善了默认安装表单、配置结构和安装项管理能力。

### 新增

- 新增 `rsenvforge remove <name> [--kind <skill|crate>] [--force]`，支持删除由 `rsenvforge` 记录的已安装项。
- 默认配置加入 Rust 开发常用工具，包括 `cargo-llvm-cov`、`bindgen-cli`、`cargo-audit`、`cargo-deny`、`cargo-geiger`、`cargo-udeps`、`cargo-bloat`、`flamegraph-rs`、`cargo-msrv`、`cargo-semver-checks`、`nodejs`、`cpp2rust-demo`、`c2rust-demo`、`rust-checker` 等。
- 默认配置加入 full 等级工具和 skills，包括 `python`、`perf`、`gitnexus`、`valgrind`、`CMake`、`Ninja`、`Clang/libclang`、`llvm-tools-preview`、`clang++/g++`、`openspec`、`oh-my-opencode`、`superpowers`。
- 支持 `tools` 字段中的平台专用 `check_windows`、`check_linux`、`install_windows`、`install_linux`。
- 支持用 `"0"` 标记某个工具不支持当前平台。

### 改进

- 将 `rsenvforge.toml` 从示例目录移动到仓库根目录，作为可直接使用的默认配置。
- 重构代码结构，将原本臃肿的 `lib.rs` 拆分为 `config`、`discovery`、`installer`、`models`、`paths`、`process`、`registry` 等模块。
- 安装前会先检测工具和 skill 状态，列出缺失项，并等待用户输入 `Y` 或 `N`。
- 安装完成后输出 `已安装完成`，并显示已安装组件及版本信息。
- README 改为客观说明工具用法、配置格式和默认工具列表。

## 0.1.0 - 2026-06-02

初始版本，完成 `rsenvforge` 的基础安装器能力。

### 新增

- 提供 `install`、`install-skill`、`install-crate`、`update`、`list`、`doctor`、`help`、`version` 命令。
- 支持 `light`、`standard`、`full` 三个安装等级，默认 `install` 使用 `standard`。
- 支持从 Git 地址或本地路径安装 agent skill。
- 支持 Claude Code 和 OpenCode 的默认 skill 目录。
- 支持从 Git 地址或本地路径安装 Rust binary crate。
- 支持 `--norustup`，在无法使用 rustup 时优先尝试预编译二进制，并在本机有 cargo 时尝试构建。
- 支持安装 registry，记录已安装项的名称、类型、来源、profile、目标路径和安装时间。
- `update` 可根据 registry 更新曾由 `rsenvforge` 安装过的项目。
- `doctor` 可检查本地目录和常用命令状态。
