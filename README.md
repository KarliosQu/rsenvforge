# rsenvforge

`rsenvforge` 是一个用于准备 Rust 开发环境的命令行工具。它读取 `rsenvforge.toml` 中的安装表单，先检测工具安装状态，再询问是否安装缺失项。

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
rsenvforge install [light|standard|full] [--config <path>] [--force] [--norustup] [--no-status-bar]
rsenvforge install-crate <source> [--norustup] [--force] [--bin <name>]
rsenvforge update [--force] [--norustup]
rsenvforge remove <name> [--kind <crate>] [--force]
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

`install` 会先输出工具检测结果。已安装工具会显示版本，未安装工具会列为缺失项。不支持当前平台的工具会直接提示，例如：

```text
valgrind：不支持windows环境
```

确认安装前，交互终端会显示可勾选安装列表。列表默认全选，可以用方向键移动、空格切换某个组件是否安装，按 `Enter` 确认；按 `A` 可在全选和全不选之间切换，按 `Esc` 或 `Q` 取消本次安装。

在脚本、管道或自动化测试这类非交互终端中，工具会保留轻量的 `Y/N` 确认，并默认安装全部缺失组件：

```text
以上为目前工具安装情况，请问是否安装缺失工具？(Y/N)
```

只有输入 `Y` 或 `y` 才会继续安装。安装过程中会以 `Step 当前/总数` 展示整体组件安装进度；APT 镜像验证/写入、安装前置命令和每个工具安装都会纳入 Step，例如：

```text
Step 1/4：配置 APT 镜像
Step 2/4：运行 安装前置命令
Step 3/4：安装工具 nodejs
Step 4/4：安装工具 gitnexus
```

安装成功后会输出：

```text
已安装完成
已安装【组件名称】+【组件版本】
```

安装前已经存在的工具也会出现在最终摘要中。

如果某个 profile 工具安装失败，`rsenvforge` 会询问是否跳过该工具继续安装。输入 `Y` 或 `y` 会跳过当前工具并继续后续工具；输入 `N` 或其他内容会停止安装。

工具安装命令运行期间，界面会持续提示“安装过程中可输入 T 后回车，强制跳过当前工具”。输入 `T` 或 `t` 后回车会终止当前安装命令，标记该工具为已跳过，并继续安装后续工具。

安装命令不会因为耗时较长而被自动停止。单个安装命令持续运行到 `120` 秒时会输出一次提醒，之后按 `240`、`480`、`960` 秒继续翻倍提醒，并显示当前已经收集到的命令行输出，最多保留最近 `5` 行：

```text
目前cargo-geiger的安装已经持续了120秒，请注意，目前进度为（最多显示最近 5 行）：
...
```

交互式终端默认会在底部显示常驻安装状态栏；在 CI、重定向输出、测试等非交互环境中会自动使用普通文本输出。如果不希望使用状态栏，可以添加 `--no-status-bar`。

## 代理检查

`doctor` 会输出当前代理配置，便于排查 `cargo install`、Git 下载和系统包安装时的网络问题。`install` 不再在界面中显示代理检查结果。

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

`show` 只显示根据 `/etc/os-release` 和 `dpkg --print-architecture` 生成的候选 APT 源文件。`check` 使用临时目录运行 APT 验证，不写入 `/etc`，临时文件会在结束后删除。`apply` 会先执行相同验证，得到 `Y` 确认后写入 `source_file`。Deb822 模式默认写入 `/etc/apt/sources.list.d/rsenvforge.sources`；传统 `deb ...` 行模式默认写入 `/etc/apt/sources.list.d/rsenvforge.list`。

`[apt_mirror]` 支持 `{distribution}`、`{codename}`、`{architecture}` 变量，也支持按系统和架构选择镜像：

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

规则按书写顺序匹配，命中第一条即使用；未在 rule 中填写的 `lines/suites/components/architectures/signed_by/source_file` 会继承 `[apt_mirror]` 中的默认值。

如果内网提供的是传统 `deb ...` 行，可以使用 `lines`。`lines` 支持写在顶层，也支持写在每条 rule 中；命中 rule 后优先使用该 rule 的 `lines`：

```toml
[apt_mirror]
source_file = "/etc/apt/sources.list.d/rsenvforge.list"
lines = [
  "deb https://mirror.example/ubuntu {codename} main restricted universe multiverse",
  "deb https://mirror.example/ubuntu {codename}-updates main restricted universe multiverse",
]

[[apt_mirror.rules]]
distribution = "ubuntu"
architecture = "amd64"
lines = [
  "deb https://mirror-amd64.example/ubuntu {codename} main restricted universe multiverse",
  "deb https://mirror-amd64.example/ubuntu {codename}-updates main restricted universe multiverse",
]
```

Linux 下运行 `install` 时，如果配置了 `[apt_mirror]`，会在执行安装前准备命令之前询问：

```text
是否使用内部apt镜像？如果未配置proxy，不使用apt镜像可能导致部分工具安装失败。(Y/N)
```

输入 `Y` 或 `y` 后会先验证镜像，再写入 APT source 文件；其他输入会跳过 APT 镜像配置并继续安装。

该功能不会删除、禁用或替换已有系统源；若要让内网镜像成为唯一来源，应在确认镜像可用后自行按运维策略停用原有源。不会关闭 APT 签名校验，`signed_by` 指向的内部镜像 GPG key 文件必须已存在。

## 安装等级

三个等级按顺序累积安装配置文件中的工具清单：

- `light`：Windows Rust 编译工具链、Cargo/Rust 辅助工具、Python、Ninja 与 Valgrind。
- `standard`：包含 `light`，并追加 Rust 构建基础、Rust toolchain 与 Node.js 环境。
- `full`：包含 `light + standard`，并追加自定义全量工具；当前默认追加项为空。

### light

| 类型 | 名称 | 默认安装方式 |
| --- | --- | --- |
| 工具 | `gnu` | Windows: 下载内网 WinLibs x64 UCRT ZIP，解压到 `%LOCALAPPDATA%\\rsenvforge\\toolchains\\winlibs` 并写入用户 PATH；Linux: 不支持 |
| 工具 | `msvc` | Windows: `winget install -e --id Microsoft.VisualStudio.2022.BuildTools` 并安装 C++ Build Tools；Linux: 不支持 |
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
| 工具 | `rust-checker-cli` | `cargo install rust-checker-cli` |
| 工具 | `rust-bot` | `cargo install rust-bot` |
| 工具 | `python` | Windows: winget；Linux: `apt-get install -y python3 python3-pip` |
| 工具 | `ninja` | Windows: winget；Linux: `apt-get install -y ninja-build` |
| 工具 | `valgrind` | Linux: `apt-get install -y valgrind`；Windows: 不支持 |

Windows 的 `gnu` 使用内网 WinLibs x64 UCRT ZIP，不依赖 `winget`、MSYS2 或外网时区配置。将 ZIP 放到 `http://internal-host/packages/winlibs-x86_64-ucrt.zip`；ZIP 根目录必须直接包含 `mingw64/`。安装后会解压到 `%LOCALAPPDATA%\rsenvforge\toolchains\winlibs`，rsenvforge 会写入当前用户 PATH 并立即刷新本次安装进程的 PATH。当前提供的包 SHA-256 为 `78eff1e2e804b6a6320c713f084b8f820c662104a24cea6a3bfcab82032bdd60`。

### standard

| 类型 | 名称 | 默认安装方式 |
| --- | --- | --- |
| 工具 | `rust-build-base` | Linux: `apt-get update && apt-get install -y build-essential pkg-config libssl-dev`；Windows: 不支持 |
| 工具 | `rust-toolchain` | Linux: 无 rustup 时使用 `curl -ssf` 调用 rustup-init 安装 Rust 1.89.0；Windows: 通过 `install_windows` 中配置的 URL 下载 Rustup 安装器，并安装 Rust 1.89.0 |
| 工具 | `nodejs` | Linux: 从 Node.js 模板地址下载 20.17.0 压缩包并安装到 `~/.local/opt`；Windows: 使用 winget 安装 Node.js LTS |
| 工具 | `gitnexus` | 复用 `nodejs` 准备好的 Node 20+，按平台选择离线包执行 `npm install -g <package-url>` |

Linux 下安装 `nodejs` 时，会优先读取 `RSENVFORGE_NODEJS_ARCHIVE_URL` 模板地址，替换 `{version}`、`{arch}`、`{platform}`、`{package}` 后下载 Node.js 20.17.0 压缩包到 `~/.local/opt`，并将 `~/.local/bin` 写入 PATH。未配置模板地址时，会使用 `RSENVFORGE_NODEJS_MIRROR` 拼接默认下载路径。`nodejs` 安装完成后，rsenvforge 会刷新当前安装进程的 Node.js 环境，并把 `$(npm prefix -g)/bin` 写入当前用户的 `~/.bashrc`，避免 `npm install -g` 已安装但新终端找不到命令。

Linux 下 `gitnexus` 安装前会运行 `node20` 和 `npm-mirror` 标签检查，因此系统 apt 提供的低版本 Node.js 不会被误认为满足要求。`gitnexus` 默认按平台选择离线 npm 包：Windows、Linux x64、Linux arm64 分别在 `rsenvforge.toml` 的 `install_windows` / `install_linux` 中配置对应 URL。安装时会输出检测到的目标平台，例如 `windows`、`linux-x64` 或 `linux-arm64`。

Windows 下的 `rust-toolchain` 安装命令完全由 TOML 的 `install_windows` 控制，不再由程序生成。默认命令直接下载 GNU Rustup 安装器；如需 MSVC，直接将该命令中的 URL 改为 `.../x86_64-pc-windows-msvc/rustup-init.exe`，并在安装选择界面选择 `msvc` 而不选择 `gnu`。MSVC 检测会同时确认 `cl/link` 是否已在终端可用，或验证 `vcvars64.bat` 是否存在；后一种情况会在 rsenvforge 执行 Rust/Cargo 命令时自动加载 Visual Studio C++ 开发环境。GNU 检测会验证 GCC 目标为 x64 MinGW，避免把 MSYS2、Cygwin 或其他 GCC 误判为 Rust GNU 工具链。`gnu` 和 `msvc` 均位于 `light` 等级，安装前也会显示检测结果。

### full

| 类型 | 名称 | 默认安装方式 |
| --- | --- | --- |
| - | 暂无 | 可在 `rsenvforge.toml` 中自行添加 |

## 配置文件

配置文件名为 `rsenvforge.toml`。发现顺序如下：

1. `--config <path>` 指定的文件。
2. 当前目录下的 `rsenvforge.toml`。
3. 项目根目录下的 [rsenvforge.toml](rsenvforge.toml)。
4. 用户配置目录下的 `rsenvforge.toml`。
5. 内置配置。

## 配置格式

配置由 `environment.windows`、`environment.linux`、`preinstall`、`profiles`、`tag_checks`、`tools` 和 `items` 组成。默认配置把平台环境变量和安装前置命令放在文件最前面，便于先配置内网镜像和基础环境：

```toml
[environment.windows]
cargo_config = [
  "[net]",
  "git-fetch-with-cli = true",
]
variables = [
  "RUSTUP_DIST_SERVER=https://rustup.internal.example",
  "RUSTUP_UPDATE_ROOT=https://rustup.internal.example/rustup",
]
npmrc = [
  "registry=https://mirror.com/npm/",
]

[environment.linux]
cargo_config = [
  "[net]",
  "git-fetch-with-cli = true",
]
bashrc = [
  "export RUSTUP_DIST_SERVER=https://rustup.internal.example",
  "export PATH=\"$HOME/.local/bin:$PATH\"",
  ". \"$HOME/.cargo/env\"",
]

[preinstall.standard.linux]
commands = []

[profiles.standard]
tools = ["rust-toolchain", "cargo-audit", "nodejs"]
items = []

[profiles.full]
tools = []
items = []

[tag_checks.github]
check_windows = "git ls-remote https://github.com/github/gitignore.git HEAD"
check_linux = "git ls-remote https://github.com/github/gitignore.git HEAD"

[[tools]]
name = "nodejs"
check_windows = "node --version && npm --version"
check_linux = "export PATH=\"$HOME/.local/bin:$PATH\"; version=$(node --version) || exit $?; npm --version || exit $?; echo \"$version\"; major=${version#v}; major=${major%%.*}; test \"$major\" -ge 20"
install_windows = "winget install -e --id OpenJS.NodeJS.LTS && node --version && npm --version"
install_linux = "echo 请参考根目录 rsenvforge.toml 中 nodejs 的完整离线安装命令"
post_install_linux = "node --version && npm --version"
```

Linux 下直接安装 Node.js 时，推荐在环境配置中提供 Node.js 压缩包模板地址：

```bash
export RSENVFORGE_NODEJS_ARCHIVE_URL=https://mirrors.com/nodejs/v{version}/node-v{version}-linux-{arch}.tar.gz
export RSENVFORGE_NODEJS_MIRROR=http://7.222.7.221/nodejs
export RSENVFORGE_NODEJS_VERSION=20.17.0
export PATH="$HOME/.local/bin:$PATH"
```

`RSENVFORGE_NODEJS_ARCHIVE_URL` 支持 `{version}`、`{arch}`、`{platform}`、`{package}` 四个变量。`{arch}` 会在 Linux 下自动替换为 `x64` 或 `arm64`，所以同一条模板可以覆盖 x64 和 arm64。压缩包后缀支持 `.tar.gz`、`.tgz` 和 `.tar.xz`。`export PATH="$HOME/.local/bin:$PATH"` 用于让当前安装进程和后续终端优先找到 rsenvforge 直装的 Node.js。

如果只配置 `RSENVFORGE_NODEJS_MIRROR`，rsenvforge 会按默认规则拼接为 `${RSENVFORGE_NODEJS_MIRROR}/v{version}/{package}.tar.gz`。

`gitnexus` 使用离线 npm 包安装。Linux 会根据 `uname -m` 自动区分 `x64` 和 `arm64`，Windows 使用 `install_windows`。如果你的内网包地址不同，只需要修改 `rsenvforge.toml` 中 `gitnexus` 工具的三个 URL：`gitnexus-windows-1.6.8.tgz`、`gitnexus-linux-x64-1.6.8.tgz`、`gitnexus-linux-arm64-1.6.8.tgz`。

`preinstall` 支持全局和分级两种写法。全局写法是 `[preinstall.linux]` / `[preinstall.windows]`，分级写法是 `[preinstall.light.linux]`、`[preinstall.standard.linux]`、`[preinstall.full.linux]`，Windows 同理。

分级预安装命令会按安装等级累进执行：`standard` 会执行 `light + standard`，`full` 会执行 `light + standard + full`。命令只会在用户确认安装后、当前等级存在缺失工具时执行。

当前默认配置把 Linux Rust 构建基础依赖放在 `standard` 等级的 `rust-build-base` 工具中，会执行 `apt-get update && apt-get install -y build-essential pkg-config libssl-dev`。

Linux 下如果当前用户已经是 `root`，`rsenvforge` 会在执行安装命令前自动去掉 apt 相关命令前的 `sudo`，例如把 `sudo apt-get install -y cmake` 转为 `apt-get install -y cmake`。

## 环境文件

`[environment.windows]` 与 `[environment.linux]` 用于声明安装时要写入的本机环境内容。两个区块互不影响，程序只处理当前平台的区块：

| 字段 | 说明 |
| --- | --- |
| `cargo_config` | 写入当前平台的 `$CARGO_HOME/config.toml`；未设置 `CARGO_HOME` 时写入用户目录 `.cargo/config.toml` |
| `npmrc` | 追加写入当前用户的 `.npmrc`，用于 npm registry、strict-ssl 等配置 |
| `variables` | `KEY=value` 列表。Windows 下写入当前安装进程和 `HKCU\Environment`；Linux 下会转为 `export KEY=value` 并追加至 `~/.bashrc` |
| `bashrc` | 仅 Linux 可用，追加到 `~/.bashrc`；其中 `export KEY=value` 也会在本次安装进程中立即生效 |

运行 `install` 时，如果 Cargo `config.toml` 不存在或内容为空，`rsenvforge` 会创建该文件并写入 `cargo_config`。`npmrc` 会按缺失行追加到用户级 `.npmrc`。Linux 下也会在执行 `preinstall` 之前，把 `bashrc` 与 `variables` 中的缺失内容写入 `~/.bashrc`；Windows 下会将 `variables` 写入当前安装进程和当前用户环境变量。Windows 配置的 `RUSTUP_DIST_SERVER` 与 `RUSTUP_UPDATE_ROOT` 会在 Rust toolchain 安装前生效。

已经打开的父终端环境无法被子进程反向修改。Linux 下如果 `rsenvforge install` 结束后当前终端仍找不到 `cargo` 或 `rustup`，请执行 `source ~/.bashrc`、`. "$HOME/.cargo/env"`，或重新打开终端；Windows 下请重新打开 PowerShell/CMD，使当前用户环境变量重新加载。

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

APT 镜像不写在 `[environment]` 中，而是使用独立的 `[apt_mirror]`，因为它需要生成系统源文件。可以通过 `rsenvforge apt-mirror apply` 手动写入，也可以在 Linux `install` 流程中按提示确认后写入。

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

默认配置提供以下标签检查。其中 `cargo-install` 已绑定到 `cargo install` 工具，`rustup-mirror` 已绑定到依赖已有 rustup 的组件工具；其他标签可按需要手动添加：

| 标签 | 验证内容 | Windows | Linux |
| --- | --- | --- | --- |
| `rustup-mirror` | `rustup`、`RUSTUP_DIST_SERVER` 和 stable channel 文件连通性 | 支持 | 支持 |
| `cargo-mirror` | `cargo`、Cargo registry/source 配置及 `cargo search serde` | 支持 | 支持 |
| `cargo-install` | `cargo install --list`，确认 Cargo 安装子命令可用 | 支持 | 支持 |
| `node20` | 检查 Node.js 主版本是否 >= 20 | 支持 | 支持 |
| `apt-mirror` | `apt-get` 与 apt 源更新；索引仅下载到临时目录，结束后删除 | 不支持 | 支持 |
| `npm-mirror` | `npm`、registry 配置及 `npm ping` | 支持 | 支持 |

例如，要在现有 `cargo-audit` 工具块内，在已有 `cargo-install` 检查之外增加 Cargo 镜像验证：

```toml
tags = ["cargo-install", "cargo-mirror"]
```

以后添加 `npm install`、`npm ci` 等命令时，可按工具实际要求显式添加 `node20` 和 `npm-mirror`。如果 Node.js 主版本低于 20，`node20` 检查会失败，并询问是否跳过当前工具。Linux 下默认 `gitnexus` 会使用 `nodejs` 工具直装的 Node 20.17.0。标签的当前平台字段为 `"0"` 时，程序会明确提示该标签不支持当前平台，并继续询问是否跳过当前工具。

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
rsenvforge remove <name> --kind crate --force
```

说明：

- `--kind crate` 用于在同名记录中限定类型。
- `--force` 跳过删除确认。
- 删除只处理 registry 中记录的目标路径，例如 `install-crate` 复制到托管 bin 目录的二进制。
- 通过系统包管理器、`cargo install` 或 `winget/apt-get/npm` 安装的 profile 工具当前不会写入 registry，因此 `remove` 不会卸载这些系统级工具。

## 环境变量

| 变量 | 说明 |
| --- | --- |
| `RSENVFORGE_HOME` | 工具数据目录 |
| `RSENVFORGE_CONFIG_DIR` | 用户配置目录 |
| `RSENVFORGE_BIN_DIR` | 工具管理的 bin 目录 |

## 诊断

```powershell
rsenvforge doctor
```

该命令会显示数据目录、bin 目录、registry 路径，以及 `git`、`cargo`、`rustup` 等基础工具的检测状态。
