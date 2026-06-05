# rsenvforge

`rsenvforge` 是一个用于准备 Rust 开发环境的命令行工具。它读取 `rsenvforge.toml` 中的安装表单，先检测工具和 agent skill 的安装状态，再询问是否安装缺失项。

默认命令：

```powershell
rsenvforge install
```

无参数时使用 `standard` 等级。

## 构建与运行

Windows：

```powershell
cargo build --release
.\target\release\rsenvforge.exe doctor
.\target\release\rsenvforge.exe install
```

Linux：

```bash
cargo build --release
./target/release/rsenvforge doctor
./target/release/rsenvforge install
```

## 命令

```text
rsenvforge init [--force]
rsenvforge install [light|standard|full] [--config <path>] [--force] [--norustup]
rsenvforge install-skill <source> --agent <claude|opencode|both> [--force]
rsenvforge install-crate <source> [--norustup] [--force] [--bin <name>]
rsenvforge update [--force] [--norustup]
rsenvforge remove <name> [--kind <skill|crate>] [--force]
rsenvforge list
rsenvforge doctor
rsenvforge help
rsenvforge version
```

## 初始化配置

`init` 会在当前目录生成默认 `rsenvforge.toml`。该文件来自当前版本随二进制内置的默认安装表单，内容与仓库根目录的 [rsenvforge.toml](rsenvforge.toml) 保持一致。

```powershell
rsenvforge init
```

如果当前目录已经存在 `rsenvforge.toml`，命令会停止并提示。需要覆盖时使用：

```powershell
rsenvforge init --force
```

## 安装流程

`install` 会先输出代理检查，再输出工具和 skill 的检测结果。已安装工具会显示版本，未安装工具会列为缺失项。不支持当前平台的工具会直接提示，例如：

```text
valgrind：不支持windows环境
```

确认安装前会询问：

```text
以上为目前工具安装情况，请问是否安装缺失工具？(Y/N)
```

只有输入 `Y` 或 `y` 才会继续安装。安装成功后会输出：

```text
已安装完成
已安装【组件名称】+【组件版本】
```

安装前已经存在的工具也会出现在最终摘要中。

安装命令不会因为耗时较长而被自动停止。单个安装命令持续运行到 `120` 秒时会输出一次提醒，之后按 `240`、`480`、`960` 秒继续翻倍提醒，并显示当前已经收集到的命令行输出：

```text
目前cargo-geiger的安装已经持续了120秒，请注意，目前进度为：
...
```

## 代理检查

`doctor` 和 `install` 都会输出当前代理配置，便于排查 `cargo install`、Git 下载和系统包安装时的网络问题。

检查内容：

- 当前环境变量中的 `http_proxy` / `HTTP_PROXY`。
- 当前环境变量中的 `https_proxy` / `HTTPS_PROXY`。
- Windows：环境变量和 Cargo 配置文件。
- Linux：`~/.bashrc` 和 Cargo 配置文件。
- Cargo 配置文件优先检查 `$CARGO_HOME/config.toml`，未设置 `CARGO_HOME` 时检查 `~/.cargo/config.toml`，同时兼容旧版 `~/.cargo/config`。

如果代理地址中包含 `user:password@`，输出时会脱敏为 `***@`。

## 安装等级

三个等级是累进关系：

- `light`：轻量环境。
- `standard`：包含 `light` 的全部内容。
- `full`：包含 `standard` 的全部内容。

### light

| 类型 | 名称 | 默认安装方式 |
| --- | --- | --- |
| 工具 | `rust` | `rustup toolchain install stable`，并安装 `rustfmt`、`clippy` |

### standard

| 类型 | 名称 | 默认安装方式 |
| --- | --- | --- |
| 工具 | `rust` | `rustup toolchain install stable && rustup component add rustfmt clippy` |
| 工具 | `cargo-llvm-cov` | `cargo install cargo-llvm-cov` |
| 工具 | `bindgen-cli` | `cargo install bindgen-cli` |
| 工具 | `cargo-audit` | `cargo install cargo-audit` |
| 工具 | `cargo-deny` | `cargo install cargo-deny` |
| 工具 | `cargo-geiger` | `cargo install cargo-geiger` |
| 工具 | `cargo-udeps` | `cargo install cargo-udeps` |
| 工具 | `cargo-bloat` | `cargo install cargo-bloat` |
| 工具 | `flamegraph-rs` | `cargo install flamegraph` |
| 工具 | `cargo-msrv` | `cargo install cargo-msrv` |
| 工具 | `cargo-semver-checks` | `cargo install cargo-semver-checks` |
| 工具 | `nodejs` | Windows: `winget install OpenJS.NodeJS.LTS`；Linux: `sudo apt-get install -y nodejs npm` |
| 工具 | `cpp2rust-demo` | `cargo install --git https://github.com/LuuuXXX/cpp2rust-demo` |
| 工具 | `c2rust-demo` | `cargo install --git https://github.com/LuuuXXX/c2rust-demo` |
| 工具 | `rust-checker` | `cargo install --git https://github.com/LuuuXXX/rust-checker` |

### full

`full` 包含 `standard`，并增加：

| 类型 | 名称 | 默认安装方式 |
| --- | --- | --- |
| 工具 | `python` | Windows: `winget install Python.Python.3.12`；Linux: `sudo apt-get install -y python3 python3-pip` |
| 工具 | `perf` | Windows: 不支持；Linux: `sudo apt-get install -y linux-tools-common linux-tools-generic linux-tools-$(uname -r)` |
| 工具 | `gitnexus` | `npm install -g gitnexus` |
| 工具 | `valgrind` | Windows: 不支持；Linux: `sudo apt-get install -y valgrind` |
| 工具 | `CMake` | Windows: `winget install Kitware.CMake`；Linux: `sudo apt-get install -y cmake` |
| 工具 | `Ninja` | Windows: `winget install Ninja-build.Ninja`；Linux: `sudo apt-get install -y ninja-build` |
| 工具 | `Clang/libclang` | Windows: `winget install LLVM.LLVM`；Linux: `sudo apt-get install -y clang lld libclang-dev` |
| 工具 | `llvm-tools-preview` | `rustup component add llvm-tools-preview` |
| 工具 | `clang++/g++` | Windows: `winget install LLVM.LLVM`；Linux: `sudo apt-get install -y g++ clang` |
| Skill | `openspec` | `https://github.com/Fission-AI/OpenSpec.git` |
| Skill | `oh-my-opencode` | `https://github.com/code-yeongyu/oh-my-openagent.git` |
| Skill | `superpowers` | `https://github.com/obra/superpowers.git` |

## 配置文件

配置文件名为 `rsenvforge.toml`。发现顺序如下：

1. `--config <path>` 指定的文件。
2. 当前目录下的 `rsenvforge.toml`。
3. 项目根目录下的 [rsenvforge.toml](rsenvforge.toml)。
4. 用户配置目录下的 `rsenvforge.toml`。
5. 内置配置。

## 配置格式

配置由 `profiles`、`preinstall`、`tools`、`skills` 和 `items` 组成：

```toml
[profiles.standard]
tools = ["rust", "cargo-audit", "nodejs"]
skills = []
items = []

[preinstall.standard.linux]
commands = [
  "sudo apt-get update",
  "sudo apt-get install -y pkg-config libssl-dev",
]

[[tools]]
name = "nodejs"
check_windows = "node --version && npm --version"
check_linux = "node --version && npm --version"
install_windows = "winget install OpenJS.NodeJS.LTS"
install_linux = "sudo apt-get install -y nodejs npm"

[[skills]]
name = "superpowers"
source = "https://github.com/obra/superpowers.git"
agents = ["claude", "opencode"]
```

`preinstall` 支持全局和分级两种写法。全局写法是 `[preinstall.linux]` / `[preinstall.windows]`，分级写法是 `[preinstall.light.linux]`、`[preinstall.standard.linux]`、`[preinstall.full.linux]`，Windows 同理。

分级预安装命令会按安装等级累进执行：`standard` 会执行 `light + standard`，`full` 会执行 `light + standard + full`。命令只会在用户确认安装后、当前等级存在缺失工具时执行。

当前默认配置把 `sudo apt-get update` 和 `sudo apt-get install -y pkg-config libssl-dev` 放在 `[preinstall.standard.linux]` 中，用于满足 `cargo-geiger` 的 Linux 系统依赖，因此 `light` 安装不会执行这两个命令。

## 工具配置

| 字段 | 说明 |
| --- | --- |
| `name` | 工具名称，必须与 profile 中引用的名称一致 |
| `check_windows` | Windows 专用检测命令 |
| `check_linux` | Linux 专用检测命令 |
| `install_windows` | Windows 专用安装命令 |
| `install_linux` | Linux 专用安装命令 |

`check_windows/check_linux/install_windows/install_linux` 可以填写 `"0"`，表示该工具不支持对应平台。当前平台遇到 `"0"` 时，`rsenvforge` 不会执行检测或安装命令。

## Skill 安装

`rsenvforge` 支持安装到 Claude Code 和 OpenCode 的默认 skill 目录：

| Agent | 默认目录 |
| --- | --- |
| Claude Code | `~/.claude/skills` |
| OpenCode Windows | `%APPDATA%\opencode\skills` |
| OpenCode Linux | `~/.config/opencode/skills` |

如果默认目录不存在，工具会提示并跳过该 agent 的 skill 安装，不会自动创建目录。

扫描 skill 时会查找：

| 位置 | 说明 |
| --- | --- |
| `SKILL.md` | 仓库根目录下的单个 skill |
| `skills/*/SKILL.md` | 通用 skills 目录 |
| `.claude/skills/*/SKILL.md` | Claude 风格 skills 目录 |

## Crate 安装

`install-crate` 可从 Git 地址或本地路径安装 Rust binary：

```powershell
rsenvforge install-crate D:\source\my-tool
rsenvforge install-crate https://github.com/example/my-tool.git --bin my-tool
rsenvforge install-crate https://github.com/example/my-tool.git --norustup
```

扫描 crate 时会查找：

| 位置 | 说明 |
| --- | --- |
| `Cargo.toml` | 仓库根 crate |
| `crates/*/Cargo.toml` | workspace 风格子 crate |

生成的可执行文件会复制到 `rsenvforge` 管理的 bin 目录中。可以通过 `doctor` 查看该目录。

## 更新与清单

```powershell
rsenvforge list
rsenvforge update
```

`update` 只更新 registry 中已有的安装项，不会扫描和安装配置文件里的新项目。

## 删除

`remove` 会删除 `rsenvforge` registry 中记录过的安装项，并从 registry 移除对应记录：

```powershell
rsenvforge remove <name>
rsenvforge remove <name> --kind skill
rsenvforge remove <name> --kind crate --force
```

说明：

- `--kind skill|crate` 用于在同名记录中限定类型。
- `--force` 跳过删除确认。
- 删除只处理 registry 中记录的目标路径，例如 skill 目录或 `install-crate` 复制到托管 bin 目录的二进制。
- 通过系统包管理器、`cargo install` 或 `winget/apt-get/npm` 安装的 profile 工具当前不会写入 registry，因此 `remove` 不会卸载这些系统级工具。

## 环境变量

| 变量 | 说明 |
| --- | --- |
| `RSENVFORGE_HOME` | 工具数据目录 |
| `RSENVFORGE_CONFIG_DIR` | 用户配置目录 |
| `RSENVFORGE_BIN_DIR` | 工具管理的 bin 目录 |
| `RSENVFORGE_CLAUDE_DIR` | Claude skill 目录 |
| `RSENVFORGE_OPENCODE_DIR` | OpenCode skill 目录 |

## 诊断

```powershell
rsenvforge doctor
```

该命令会显示数据目录、bin 目录、registry 路径，以及 `git`、`cargo`、`rustup`、`claude`、`opencode` 的检测状态。
