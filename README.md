# rsenvforge

`rsenvforge` 是一个用于准备 Rust 开发环境的命令行工具。它会按照 `rsenvforge.toml` 中定义的安装表单，先检查工具和 agent skill 的安装状态，再询问是否安装缺失项。

默认命令是：

```powershell
rsenvforge install
```

无参数时会使用 `standard` 等级。

## 构建与运行

在项目目录中执行：

```powershell
cargo build --release
.\target\release\rsenvforge.exe doctor
.\target\release\rsenvforge.exe install
```

Linux 下可使用：

```bash
cargo build --release
./target/release/rsenvforge doctor
./target/release/rsenvforge install
```

如果已经把二进制所在目录加入 `PATH`，可以直接使用 `rsenvforge`。

## 命令

```text
rsenvforge install [light|standard|full] [--config <path>] [--force] [--norustup]
rsenvforge install-skill <source> --agent <claude|opencode|both> [--force]
rsenvforge install-crate <source> [--norustup] [--force] [--bin <name>]
rsenvforge update [--force] [--norustup]
rsenvforge list
rsenvforge doctor
rsenvforge help
rsenvforge version
```

常用示例：

```powershell
rsenvforge install
rsenvforge install light
rsenvforge install full
rsenvforge install --config D:\path\rsenvforge.toml

rsenvforge install-skill D:\skills\openspec --agent both
rsenvforge install-crate https://github.com/example/tool.git --bin tool-name

rsenvforge update
rsenvforge list
rsenvforge doctor
```

## 安装流程

`install` 会先检查当前等级中的工具和 skill：

- 已安装的工具会显示检测到的版本信息。
- 未安装的工具会被列为缺失项。
- 已安装的 skill 会显示已存在。
- 未安装的 skill 会被列为缺失项。
- 如果 agent 的默认 skill 目录不存在，该 agent 下的 skill 安装会被跳过并给出提示。

检查完成后，命令会询问：

```text
以上为目前工具安装情况，请问是否安装缺失工具？(Y/N)
```

只有输入 `Y` 或 `y` 才会继续安装。输入 `N` 或其他内容会取消安装。安装过程中如果某个工具或 skill 失败，流程会立即停止，并说明失败项。

## 安装等级

`rsenvforge` 使用三个固定等级：

- `light`：轻量环境。
- `standard`：标准环境，包含 `light` 的内容。
- `full`：完整环境，包含 `standard` 的内容。

内置等级内容如下。

`light`：

- `rust`

`standard`：

- `rust`
- `cargo-llvm-cov`
- `python`
- `bindgen-cli`
- `cargo-audit`
- `cargo-deny`
- `cargo-geiger`
- `cargo-udeps`
- `cargo-bloat`
- `flamegraph-rs`
- `perf`
- `cargo-msrv`
- `cargo-semver-checks`
- `cpp2rust-demo`
- `c2rust-demo`
- `rust-checker`
- `gitnexus`
- `openspec`
- `oh-my-opencode`
- `superpowers`

`full`：

- `standard` 的全部内容
- `valgrind`
- `asan`
- `CMake`
- `Ninja`
- `Clang/libclang`
- `llvm-tools-preview`
- `clang++/g++`

## 配置文件

配置文件名为 `rsenvforge.toml`。发现顺序如下：

1. `--config <path>` 指定的文件。
2. 当前目录下的 `rsenvforge.toml`。
3. 项目根目录下的 [rsenvforge.toml](rsenvforge.toml)。
4. 用户配置目录下的 `rsenvforge.toml`。
5. 内置配置。

这个仓库自带的默认安装表单位于根目录 [rsenvforge.toml](rsenvforge.toml)。直接从本项目构建出的二进制，即使在其他目录运行，也会在没有显式配置和当前目录配置时回退读取该文件。

## 配置格式

配置由 `profiles`、`tools`、`skills` 和 `items` 组成：

```toml
[profiles.light]
tools = ["rust"]
skills = []
items = []

[profiles.standard]
tools = ["rust", "cargo-audit", "python"]
skills = ["openspec"]
items = []

[profiles.full]
tools = ["rust", "cargo-audit", "python", "CMake"]
skills = ["openspec"]
items = []

[[tools]]
name = "python"
check = "python --version"
# install_windows = "winget install Python.Python.3.12"
# install_linux = "sudo apt-get update && sudo apt-get install -y python3 python3-pip"

[[skills]]
name = "openspec"
source = "D:/your/local/skills/openspec"
agents = ["claude", "opencode"]
```

`profiles.<name>.tools` 引用 `[[tools]]` 或内置工具。`profiles.<name>.skills` 引用 `[[skills]]`。`profiles.<name>.items` 用于兼容旧的自定义 skill/crate 安装项。

自定义配置会和内置配置合并。同名 `tool` 或 `skill` 会覆盖内置定义。

## 工具安装

工具会先通过 `check` 命令检测版本。内置的 Rust/Cargo 工具会使用确定的安装命令，例如：

- `rust`：`rustup toolchain install stable`，并安装 `rustfmt` 和 `clippy`。
- `cargo-audit`：`cargo install cargo-audit`。
- `cargo-llvm-cov`：`cargo install cargo-llvm-cov`。
- `cpp2rust-demo`：`cargo install --git https://github.com/LuuuXXX/cpp2rust-demo`。
- `c2rust-demo`：`cargo install --git https://github.com/LuuuXXX/c2rust-demo`。
- `rust-checker`：`cargo install --git https://github.com/LuuuXXX/rust-checker`。
- `llvm-tools-preview`：`rustup component add llvm-tools-preview`。

对安装方式没有内置确定来源的系统工具，例如 `python`、`perf`、`valgrind`、`CMake`、`Ninja`、`Clang/libclang`、`clang++/g++`、`asan` 和 `gitnexus`，需要在 `[[tools]]` 中提供官方安装命令。缺失这些工具且没有安装命令时，`rsenvforge` 会停止并提示补充配置。

`[[tools]]` 支持的常用字段：

```toml
[[tools]]
name = "CMake"
check = "cmake --version"
check_windows = "cmake --version"
check_linux = "cmake --version"
install = "..."
install_windows = "..."
install_linux = "..."
```

平台专用字段优先级高于通用字段。

## Skill 安装

`rsenvforge` 支持安装到 Claude Code 和 OpenCode 的默认 skill 目录：

- Claude：`~/.claude/skills`
- OpenCode：Windows 为 `%APPDATA%\opencode\skills`，Linux 为 `~/.config/opencode/skills`

如果默认目录不存在，工具会提示并跳过该 agent 的 skill 安装，不会自动创建目录。

`[[skills]]` 示例：

```toml
[[skills]]
name = "superpowers"
source = "https://github.com/example/superpowers.git"
agents = ["claude", "opencode"]
```

`source` 可以是 Git 地址或本地路径。扫描 skill 时会查找：

- 根目录 `SKILL.md`
- `skills/*/SKILL.md`
- `.claude/skills/*/SKILL.md`

## Crate 安装

`install-crate` 可从 Git 地址或本地路径安装 Rust binary：

```powershell
rsenvforge install-crate D:\source\my-tool
rsenvforge install-crate https://github.com/example/my-tool.git --bin my-tool
rsenvforge install-crate https://github.com/example/my-tool.git --norustup
```

扫描 crate 时会查找：

- 根目录 `Cargo.toml`
- `crates/*/Cargo.toml`

如果仓库中存在预编译二进制，会优先复制；否则在本机有 `cargo` 时执行 `cargo build --release`。生成的可执行文件会复制到 `rsenvforge` 管理的 bin 目录中。可以通过 `doctor` 查看该目录。

`--norustup` 会跳过 rustup 检查，适合无法使用 rustup 的环境。

## 更新与清单

`rsenvforge` 会记录自己安装过的项目。记录内容包括名称、类型、来源、等级、目标路径、安装时间以及 Git commit 或本地路径。

```powershell
rsenvforge list
rsenvforge update
```

`update` 只更新 registry 中已有的安装项，不会扫描和安装配置文件里的新项目。

## 环境变量

可以通过环境变量覆盖默认路径：

- `RSENVFORGE_HOME`：工具数据目录。
- `RSENVFORGE_CONFIG_DIR`：用户配置目录。
- `RSENVFORGE_BIN_DIR`：工具管理的 bin 目录。
- `RSENVFORGE_CLAUDE_DIR`：Claude skill 目录。
- `RSENVFORGE_OPENCODE_DIR`：OpenCode skill 目录。

## 诊断

执行：

```powershell
rsenvforge doctor
```

该命令会显示数据目录、bin 目录、registry 路径，以及 `git`、`cargo`、`rustup`、`claude`、`opencode` 的检测状态。它也会提示是否需要把 `rsenvforge` 管理的 bin 目录加入 `PATH`。
