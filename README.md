# rsenvforge

`rsenvforge` 是一个用于准备 Rust 开发环境的命令行工具。它读取 `rsenvforge.toml` 中的安装表单，先检测工具和 agent skill 的安装状态，再询问是否安装缺失项。

默认命令：

```powershell
rsenvforge install
```

无参数时使用 `standard` 等级。

## 安装与运行

Windows：

```powershell
cargo install --path .
rsenvforge doctor
rsenvforge install
```

Linux：

```bash
cargo install --path .
rsenvforge doctor
rsenvforge install
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
rsenvforge apt-mirror <show|check|apply> [--config <path>]
rsenvforge help
rsenvforge version
```

完整的命令与配置字段说明见 [USAGE.md](USAGE.md)。

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

如果某个 profile 工具安装失败，`rsenvforge` 会询问是否跳过该工具继续安装。输入 `Y` 或 `y` 会跳过当前工具并继续后续工具；输入 `N` 或其他内容会停止安装。

工具安装命令运行期间，界面会持续提示“安装过程中可输入 T 后回车，强制跳过当前工具”。输入 `T` 或 `t` 后回车会终止当前安装命令，标记该工具为已跳过，并继续安装后续工具。

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
- 如果找到 Cargo 配置文件，会直接输出整个文件内容，便于确认 registry、source、net、http 等完整配置。

如果代理地址中包含 `user:password@`，输出时会脱敏为 `***@`。

## 内部 APT 镜像

内部 APT 镜像只支持 Linux；在 Windows 主机上请进入 WSL 后运行 Linux 版 `rsenvforge`。默认配置中的 `[apt_mirror]` 是注释示例，取消注释并填写内网 URI、suite、组件和签名 key 后使用：

```bash
rsenvforge apt-mirror show
rsenvforge apt-mirror check
sudo rsenvforge apt-mirror apply
```

`show` 只显示根据 `/etc/os-release` 和 `dpkg --print-architecture` 生成的候选 Deb822 源文件。`check` 使用临时目录运行 APT 验证，不写入 `/etc`，临时文件会在结束后删除。`apply` 会先执行相同验证，得到 `Y` 确认后写入 `source_file`，默认是 `/etc/apt/sources.list.d/rsenvforge.sources`。

该功能不会删除、禁用或替换已有系统源；若要让内网镜像成为唯一来源，应在确认镜像可用后自行按运维策略停用原有源。不会关闭 APT 签名校验，`signed_by` 指向的内部镜像 GPG key 文件必须已存在。

## 安装等级

三个等级各自安装配置文件中对应的工具清单：

- `light`：Cargo/Rust 辅助工具。
- `standard`：Rust 构建基础、Rust 与 Node.js 环境。
- `full`：当前为空。

### light

| 类型 | 名称 | 默认安装方式 |
| --- | --- | --- |
| 工具 | `cargo-llvm-cov` | `cargo install cargo-llvm-cov` |
| 工具 | `bindgen-cli` | `cargo install bindgen-cli` |
| 工具 | `cargo-audit` | `cargo install cargo-audit` |
| 工具 | `cargo-deny` | `cargo install cargo-deny` |
| 工具 | `cargo-geiger` | `cargo install cargo-geiger` |
| 工具 | `rust-analyzer` | `rustup component add rust-analyzer` |
| 工具 | `miri` | `rustup toolchain install nightly && rustup +nightly component add miri` |
| 工具 | `cargo-expand` | `cargo install cargo-expand` |
| 工具 | `cargo-fuzz` | `cargo install cargo-fuzz` |
| 工具 | `cargo-udeps` | `cargo install cargo-udeps` |
| 工具 | `cargo-bloat` | `cargo install cargo-bloat` |
| 工具 | `flamegraph-rs` | `cargo install flamegraph` |
| 工具 | `cargo-msrv` | `cargo install cargo-msrv` |
| 工具 | `cargo-semver-checks` | `cargo install cargo-semver-checks` |
| 工具 | `cpp2rust-demo` | `cargo install cpp2rust-demo` |
| 工具 | `c2rust-demo` | `cargo install c2rust-demo` |
| 工具 | `rust-checker-cli` | `cargo install rust-checker-cli` |

### standard

| 类型 | 名称 | 默认安装方式 |
| --- | --- | --- |
| 工具 | `rust-build-base` | Linux: `apt-get update && apt-get install -y build-essential pkg-config libssl-dev`；Windows: 不支持 |
| 工具 | `rust` | `rustup toolchain install stable && rustup component add rustfmt clippy` |
| 工具 | `nvm` | Windows: `winget install -e --id CoreyButler.NVMforWindows`；Linux: `set -o pipefail && curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.5/install.sh \| PROFILE="$HOME/.bashrc" bash` |
| 工具 | `nodejs` | 通过 `nvm install 20.17.0` 安装并启用 Node.js 20.17 |

### full

当前 `full` 为空，没有配置工具或 skill。

## 配置文件

配置文件名为 `rsenvforge.toml`。发现顺序如下：

1. `--config <path>` 指定的文件。
2. 当前目录下的 `rsenvforge.toml`。
3. 项目根目录下的 [rsenvforge.toml](rsenvforge.toml)。
4. 用户配置目录下的 `rsenvforge.toml`。
5. 内置配置。

## 配置格式

配置由 `profiles`、`preinstall`、`environment`、`tag_checks`、`tools`、`skills` 和 `items` 组成：

```toml
[profiles.standard]
tools = ["rust", "cargo-audit", "nvm", "nodejs"]
skills = []
items = []

[environment]
cargo_config = [
  "[net]",
  "git-fetch-with-cli = true",
]
bashrc = [
  ". \"$HOME/.cargo/env\"",
]

[tag_checks.github]
check_windows = "git ls-remote https://github.com/github/gitignore.git HEAD"
check_linux = "git ls-remote https://github.com/github/gitignore.git HEAD"

[[tools]]
name = "nvm"
tags = ["github"]
check_windows = "nvm version"
check_linux = ". \"$HOME/.nvm/nvm.sh\" && nvm --version"
install_windows = "winget install -e --id CoreyButler.NVMforWindows"
install_linux = "set -o pipefail && curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.5/install.sh | PROFILE=\"$HOME/.bashrc\" bash"

[[tools]]
name = "nodejs"
check_windows = "node --version | findstr /C:\"v20.17.\" && npm --version"
check_linux = ". \"$HOME/.nvm/nvm.sh\" && nvm current | grep '^v20\\.17\\.' && node --version && npm --version"
install_windows = "set \"PATH=%NVM_HOME%;%LocalAppData%\\nvm;%APPDATA%\\nvm;%ProgramFiles%\\nodejs;%PATH%\" && nvm install 20.17.0 && nvm use 20.17.0"
install_linux = ". \"$HOME/.nvm/nvm.sh\" && nvm install 20.17.0 && nvm alias default 20.17.0 && nvm use 20.17.0"
post_install_linux = "node --version && npm --version"

[[skills]]
name = "superpowers"
source = "https://github.com/obra/superpowers.git"
agents = ["claude", "opencode"]
```

`preinstall` 支持全局和分级两种写法。全局写法是 `[preinstall.linux]` / `[preinstall.windows]`，分级写法是 `[preinstall.light.linux]`、`[preinstall.standard.linux]`、`[preinstall.full.linux]`，Windows 同理。

分级预安装命令会按安装等级累进执行：`standard` 会执行 `light + standard`，`full` 会执行 `light + standard + full`。命令只会在用户确认安装后、当前等级存在缺失工具时执行。

当前默认配置把 Linux Rust 构建基础依赖放在 `standard` 等级的 `rust-build-base` 工具中，会执行 `apt-get update && apt-get install -y build-essential pkg-config libssl-dev`。

Linux 下如果当前用户已经是 `root`，`rsenvforge` 会在执行安装命令前自动去掉 apt 相关命令前的 `sudo`，例如把 `sudo apt-get install -y cmake` 转为 `apt-get install -y cmake`。

## 环境文件

`[environment]` 用于声明安装时要写入的本机环境文件内容：

| 字段 | 说明 |
| --- | --- |
| `cargo_config` | 写入 `$CARGO_HOME/config.toml`；未设置 `CARGO_HOME` 时写入 `~/.cargo/config.toml` |
| `bashrc` | Linux 下 rust 安装完成后追加到 `~/.bashrc` |

运行 `install` 时，如果 Cargo `config.toml` 不存在或内容为空，`rsenvforge` 会创建该文件并写入 `cargo_config`。Linux 下也会在执行 `preinstall` 之前，把 `bashrc` 中缺失的行追加到 `~/.bashrc`。如果检测到用户原本没有安装 `rust`，在 rust 安装完成后会再次确保这些环境文件已经写入。

Linux 下安装命令优先使用 `bash -lc` 执行，因此可以在 `preinstall` 中使用 `source ~/.bashrc` 让刚写入的环境变量对后续命令生效；如果系统没有 `bash`，会回退到 `sh -c`，此时应使用 `. ~/.bashrc`。

如果 Cargo `config.toml` 已存在且非空，`rsenvforge` 不会覆盖它；内网 registry、source 或代理配置可以直接写在 `cargo_config` 中，例如用转义引号表达 Cargo 配置行：

```toml
cargo_config = [
  "[source.crates-io]",
  "replace-with = \"internal\"",
  "[source.internal]",
  "registry = \"sparse+https://example.internal/crates.io-index/\"",
]
```

## 工具配置

| 字段 | 说明 |
| --- | --- |
| `name` | 工具名称，必须与 profile 中引用的名称一致 |
| `tags` | 工具安装前要运行的标签检查名称列表，例如 `["github", "npm"]` |
| `check_windows` | Windows 专用检测命令 |
| `check_linux` | Linux 专用检测命令 |
| `install_windows` | Windows 专用安装命令 |
| `install_linux` | Linux 专用安装命令 |
| `post_install` | 工具主安装命令成功后的通用后置命令 |
| `post_install_windows` | Windows 专用安装后命令 |
| `post_install_linux` | Linux 专用安装后命令 |

`check_windows/check_linux/install_windows/install_linux` 可以填写 `"0"`，表示该工具不支持对应平台。当前平台遇到 `"0"` 时，`rsenvforge` 不会执行检测或安装命令。

`post_install*` 会在对应工具的 `install*` 命令成功后立即运行，适合安装完成后才能使用的新命令，例如 node.js 安装完成后运行 npm 相关命令。如果工具在本次安装前已经存在，且配置了 `post_install*`，`rsenvforge` 会先询问是否运行该后置命令，得到 `Y` 或 `y` 后才会执行。后置命令失败时，工具会询问是否跳过当前工具继续安装。

## 标签检查

`[tag_checks.<名称>]` 用于定义工具安装前的测试指令。工具只有显式配置了 `tags = [...]` 才会触发对应检查；`rsenvforge` 不会在运行时自行推断或添加标签。

```toml
[tag_checks.github]
check_windows = "git ls-remote https://github.com/github/gitignore.git HEAD"
check_linux = "git ls-remote https://github.com/github/gitignore.git HEAD"

[tag_checks.npm]
check_windows = "npm --version"
check_linux = "npm --version"

[[tools]]
name = "demo-from-github"
tags = ["github"]
check = "demo-from-github --version"
install = "cargo install --git https://github.com/example/demo-from-github"
```

安装某个缺失工具前，工具会按 `tags` 顺序运行标签检查。检查通过后，同一次安装流程中相同标签不会重复检查；检查失败时会询问是否跳过当前工具继续安装。

默认配置提供以下标签检查。其中 `cargo-install`、`rustup-mirror` 和 `nvm-mirror` 已按当前工具安装命令显式绑定；其他标签可按需要手动添加：

| 标签 | 验证内容 | Windows | Linux |
| --- | --- | --- | --- |
| `rustup-mirror` | `rustup`、`RUSTUP_DIST_SERVER` 和 stable channel 文件连通性 | 支持 | 支持 |
| `cargo-mirror` | `cargo`、Cargo registry/source 配置及 `cargo search serde` | 支持 | 支持 |
| `cargo-install` | `cargo install --list`，确认 Cargo 安装子命令可用 | 支持 | 支持 |
| `nvm-mirror` | `nvm`、Node 镜像设置及 `index.tab` 连通性 | 支持 | 支持 |
| `apt-mirror` | `apt-get` 与 apt 源更新；索引仅下载到临时目录，结束后删除 | 不支持 | 支持 |
| `npm-mirror` | `npm`、registry 配置及 `npm ping` | 支持 | 支持 |

例如，要在现有 `cargo-audit` 工具块内，在已有 `cargo-install` 检查之外增加 Cargo 镜像验证：

```toml
tags = ["cargo-install", "cargo-mirror"]
```

当前配置没有安装命令调用 npm，因此没有工具绑定 `npm-mirror`。以后添加 `npm install`、`npm ci` 等命令时，可为对应工具显式添加该标签。标签的当前平台字段为 `"0"` 时，程序会明确提示该标签不支持当前平台，并继续询问是否跳过当前工具。

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
