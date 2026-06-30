# rsenvforge 使用手册

`rsenvforge` 是一个基于 TOML 安装表单的开发环境准备工具。它可以检测并安装 Rust 工具、agent skill 和其他配置项；所有需要安装或修改系统状态的操作都会以中文输出进度并要求确认。

## 安装与启动

在项目根目录编译并安装：

```powershell
cargo install --path .
rsenvforge version
```

如果只想在当前源码目录运行：

```powershell
cargo run -- help
```

Linux 使用相同的 Cargo 命令。需要配置 APT 源时，请在 Linux 或 WSL 内安装并运行 Linux 版二进制；Windows `.exe` 不会自动进入 WSL。

## 命令总览

| 命令 | 作用 |
| --- | --- |
| `init [--force]` | 在当前目录生成二进制内置的默认配置。 |
| `install [light|standard|full] [--config <path>]` | 检测并安装选定 profile；省略等级时使用 `standard`。 |
| `install-skill <source> --agent <claude|opencode|both>` | 从本地路径或 Git 地址安装 agent skill。 |
| `install-crate <source> [--bin <name>] [--norustup]` | 从本地路径或 Git 地址安装 Rust binary crate。 |
| `update [--force] [--norustup]` | 更新由 rsenvforge 记录过的安装项。 |
| `remove <name> [--kind <skill|crate>] [--force]` | 删除由 rsenvforge 记录过的安装项。 |
| `list` | 显示本地安装记录。 |
| `doctor` | 显示代理、Cargo 配置、管理目录与工具诊断信息。 |
| `apt-mirror show|check|apply [--config <path>]` | 显示、临时验证或写入内部 APT 镜像配置。 |
| `help` / `version` | 显示帮助或版本。 |

`install`、`install-skill`、`install-crate`、`update` 和 `remove` 支持 `--force` 的范围以各自命令帮助为准。`--force` 不等于自动确认安装；普通 `install` 仍会等待 `Y/N`。

安装某个工具时，界面会持续显示“安装过程中可输入 T 后回车，强制跳过当前工具”。在命令仍在运行时输入 `T` 或 `t` 后回车，rsenvforge 会终止当前安装命令、跳过该工具，并继续处理后续工具。

## 配置文件发现

配置文件名固定为 `rsenvforge.toml`，读取顺序为：

1. 命令传入的 `--config <path>`。
2. 当前工作目录的 `rsenvforge.toml`。
3. 与当前二进制源码关联的项目根目录配置。
4. 用户配置目录的 `rsenvforge.toml`。
5. 二进制内置的默认配置。

`init` 写出的内容是编译时嵌入二进制的默认表单。修改项目根目录的 `rsenvforge.toml` 后，必须重新编译，新的 `init` 才会生成修改后的版本。

## Profile

每个 profile 都显式列出工具、skill 与旧式 item 名称；profile 之间不会自动继承工具。

```toml
[profiles.light]
tools = ["cargo-audit"]
skills = []
items = []

[profiles.standard]
tools = ["rust", "nodejs"]
skills = []
items = []

[profiles.full]
tools = ["nvm", "gitnexus"]
skills = []
items = []
```

运行 `rsenvforge install light` 只处理 `light` 列表；运行 `rsenvforge install` 只处理 `standard` 列表。

## 工具定义

工具以 `[[tools]]` 定义。`check_*` 用于检测版本或可用性，`install_*` 用于安装，`post_install_*` 用于安装成功后的后置操作。

```toml
[[tools]]
name = "cargo-audit"
tags = ["cargo-install", "cargo-mirror"]
check_windows = "cargo audit --version"
check_linux = "cargo audit --version"
install_windows = "cargo install cargo-audit"
install_linux = "cargo install cargo-audit"
```

字段说明：

| 字段 | 说明 |
| --- | --- |
| `name` | 与 profile 中引用的工具名称。 |
| `tags` | 安装前需要执行的标签检查列表。 |
| `check` | 两个平台通用的检测命令。 |
| `check_windows` / `check_linux` | 平台专用检测命令，优先于 `check`。 |
| `install` | 两个平台通用的安装命令。 |
| `install_windows` / `install_linux` | 平台专用安装命令，优先于 `install`。 |
| `post_install*` | 仅在安装成功后运行；若工具本来已安装，会先询问是否运行。 |

平台专用的 `check_*` 或 `install_*` 写为 `"0"` 时，表示工具不支持该平台，rsenvforge 不会执行该命令。

## 标签检查与镜像

`[tag_checks.<名称>]` 定义工具安装前检查。检查失败时会询问是否跳过当前工具；同一标签在一次安装中通过后不会重复执行。

```toml
[tag_checks.github]
check_windows = "git ls-remote https://github.com/example/project.git HEAD"
check_linux = "git ls-remote https://github.com/example/project.git HEAD"
```

默认表单包含：

| 标签 | 用途 |
| --- | --- |
| `proxy` | 检查代理环境变量是否存在。 |
| `github` | 检查 GitHub Git 访问。 |
| `cargo-install` | 检查 `cargo install --list`。 |
| `cargo-mirror` | 检查 Cargo 配置与 registry 查询。 |
| `rustup-mirror` | 检查 `RUSTUP_DIST_SERVER` 与 stable channel。 |
| `nvm-mirror` | 检查 Node 镜像与 NVM 查询。 |
| `node20` | 检查 Node.js 主版本是否 >= 20。 |
| `npm-mirror` | 检查 npm registry 与 `npm ping`。 |
| `apt-mirror` | Linux 下临时验证 APT 源；Windows 不支持。 |

标签需要在工具项的 `tags` 字段中显式绑定。当前默认配置已经为 `cargo install`、`rustup` 与 `gitnexus` 的 npm 安装命令绑定了相应检查。

## 环境与预安装

`[environment]` 在 Rust 安装后可写入 Cargo 配置和 `~/.bashrc`：

```toml
[environment]
cargo_config = [
  "[net]",
  "git-fetch-with-cli = true",
]
bashrc = [
  "export RUSTUP_DIST_SERVER=https://rustup.internal.example",
  ". \"$HOME/.cargo/env\"",
]
npmrc = [
  "registry=https://mirror.com/npm/",
]
```

`cargo_config` 写入 Cargo 配置，`bashrc` 写入 Linux 的 `~/.bashrc`，`npmrc` 追加写入用户级 `.npmrc`。APT 镜像使用独立的 `[apt_mirror]`，因为它需要写入系统 APT source 文件。

`[preinstall.<profile>.<platform>]` 用于工具安装前的命令：

```toml
[preinstall.standard.linux]
commands = ["apt-get update", "apt-get install -y pkg-config libssl-dev"]
```

预安装命令按等级累积：`standard` 会执行 `light + standard`，`full` 会执行 `light + standard + full`。Linux root 环境中，rsenvforge 会去掉 apt 命令前的 `sudo`。

## 内部 APT 镜像

APT 镜像配置仅面向 Debian/Ubuntu 系列 Linux。它通过 `/etc/os-release` 读取发行版和版本代号，并使用 `dpkg --print-architecture` 读取架构。内核版本不参与镜像选择。

在配置中添加：

```toml
[apt_mirror]
uri = "https://apt.internal.example/{distribution}"
suites = ["{codename}", "{codename}-updates", "{codename}-security"]
components = ["main", "restricted", "universe", "multiverse"]
architectures = ["{architecture}"]
signed_by = "/usr/share/keyrings/internal-archive-keyring.gpg"
source_file = "/etc/apt/sources.list.d/rsenvforge.sources"
```

如果同一系统的不同架构要使用不同镜像，可以把公共字段放在 `[apt_mirror]`，差异字段放在 `[[apt_mirror.rules]]`：

```toml
[apt_mirror]
suites = ["{codename}", "{codename}-updates", "{codename}-security"]
components = ["main", "restricted", "universe", "multiverse"]
architectures = ["{architecture}"]
source_file = "/etc/apt/sources.list.d/rsenvforge.sources"

[[apt_mirror.rules]]
distribution = "ubuntu"
architecture = "amd64"
uri = "https://mirror-amd64.example/ubuntu"

[[apt_mirror.rules]]
distribution = "ubuntu"
architecture = "arm64"
uri = "https://mirror-arm64.example/ubuntu"
```

规则按书写顺序匹配，命中第一条即使用。`distribution`、`codename`、`architecture` 都是可选匹配条件；未在 rule 中填写的 `suites/components/architectures/signed_by/source_file` 会继承 `[apt_mirror]` 中的默认值。

变量：

| 变量 | 来源 |
| --- | --- |
| `{distribution}` | `/etc/os-release` 的 `ID`，例如 `ubuntu`、`debian`。 |
| `{codename}` | `VERSION_CODENAME` 或 `UBUNTU_CODENAME`，例如 `noble`、`bookworm`。 |
| `{architecture}` | `dpkg --print-architecture`，例如 `amd64`、`arm64`。 |

安全使用顺序：

```bash
rsenvforge apt-mirror show --config /path/to/rsenvforge.toml
rsenvforge apt-mirror check --config /path/to/rsenvforge.toml
sudo rsenvforge apt-mirror apply --config /path/to/rsenvforge.toml
```

Linux 下运行 `install` 时，如果配置了 `[apt_mirror]`，也会在执行安装前准备命令之前询问是否使用内部 APT 镜像。确认后会先验证镜像，再写入 source 文件；拒绝后继续普通安装流程。

`show` 不执行网络请求；`check` 将生成的 Deb822 源文件写入临时目录，再让 `apt-get update` 仅读取该临时源和临时索引目录，验证结束后删除临时目录；`apply` 先完成相同验证，再等待输入 `Y`，随后写入 `source_file`。

`apply` 不会替换、删除或禁用 `/etc/apt/sources.list` 和其他 `sources.list.d` 文件。因此默认行为是“新增内网源”，不是“强制只使用内网源”。如需完全切换源，应由运维策略在验证成功后处理旧源文件。rsenvforge 不会设置 `trusted=yes` 或关闭签名校验，`signed_by` 指向的 GPG key 必须由受信任的渠道预先部署。

## Skill 与 crate

`install-skill` 支持 Claude Code 和 OpenCode。Claude Code 默认目录为 `~/.claude/skills`；OpenCode 在 Windows 为 `%APPDATA%\opencode\skills`，在 Linux 为 `~/.config/opencode/skills`。如果目标 agent 目录不存在，rsenvforge 会提示并跳过，不会自动创建。

`install-crate` 可以从本地路径或 Git 地址扫描根 `Cargo.toml` 与 `crates/*/Cargo.toml`。`--bin` 可限定要复制的 binary；`--norustup` 会跳过 rustup 检查，优先使用预编译 binary 或已有 Cargo。

## 常见问题

**为什么 `apt-mirror apply` 失败？**

通常是没有 `/etc/apt/sources.list.d` 的写权限、镜像未同步对应 suite、内部 GPG key 缺失，或网络/代理不通。先运行 `apt-mirror check` 获取错误，再使用 `doctor` 查看代理与 Cargo 配置；写入系统目录时使用 root 权限。

**为什么工具安装前被标签检查阻止？**

标签验证的是安装前提。例如 `cargo-install` 要求 Cargo 已经可用，`node20` 要求 Node.js 主版本至少为 20。可以修复前提条件，或在提示时选择跳过当前工具。标签由配置控制，可以按实际环境调整。
