# rsenvforge

`rsenvforge` 是一个 Rust 环境安装器。它按 `rsenvforge.toml` 中定义的轻量、标准、全量三个等级，先检测工具和 agent skill，再询问用户是否安装缺失项。

默认命令：

```powershell
rsenvforge install
```

无参数时使用 `standard` 等级。安装前会先检测并列出缺失项，然后提示：

```text
以上为目前工具安装情况，请问是否安装缺失工具？(Y/N)
```

输入 `Y` 或 `y` 才会继续安装；输入 `N` 或其他内容会取消安装。

## 安装等级

轻量 `light`：

- `rust` stable
- 确保 `cargo`、`rustup`、`rustfmt`、`clippy` 可用

标准 `standard` 包含轻量，并增加：

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
- `openspec`、`oh-my-opencode`、`superpowers` skills

全量 `full` 包含标准，并增加：

- `valgrind`
- `asan`
- `CMake`
- `Ninja`
- `Clang/libclang`
- `llvm-tools-preview`
- `clang++/g++`

## 命令

```powershell
rsenvforge install
rsenvforge install light
rsenvforge install full
rsenvforge install --config D:\path\rsenvforge.toml

rsenvforge install-skill <git-or-local-path> --agent claude
rsenvforge install-skill <git-or-local-path> --agent opencode
rsenvforge install-skill <git-or-local-path> --agent both

rsenvforge install-crate <git-or-local-path>
rsenvforge install-crate <git-or-local-path> --norustup
rsenvforge install-crate <git-or-local-path> --bin tool-name

rsenvforge update
rsenvforge list
rsenvforge doctor
```

## 检测和确认

`install` 会先输出：

- 已安装工具及版本
- 尚未安装的工具
- 已安装 skill
- 尚未安装 skill
- 未找到默认 skill 文件夹而跳过的 skill

如果存在缺失项，工具会等待用户输入 `Y` 或 `N`。只有用户确认后才执行安装。

安装过程中任何一个工具或 skill 失败，流程会立即停止，并说明失败项。

## 配置文件

配置文件名为 `rsenvforge.toml`。读取顺序：

1. `--config <path>` 指定的文件
2. 当前目录下的 `rsenvforge.toml`
3. 用户配置目录下的 `rsenvforge.toml`
4. 内置默认配置

示例见 `examples/rsenvforge.toml`。

## tools 字段

无法通过 `cargo install` 确定安装方式的系统工具放入 `[[tools]]`。

```toml
[profiles.standard]
tools = ["python"]
skills = []
items = []

[[tools]]
name = "python"
check = "python --version"
install_windows = "winget install Python.Python.3.12"
install_linux = "sudo apt-get update && sudo apt-get install -y python3 python3-pip"
```

如果工具缺失但没有配置安装命令，`rsenvforge` 会停止并提示你补充官方安装命令，避免在安装源不确定时擅自选择来源。

## skills 字段

内置 profile 会尝试安装：

- `openspec`
- `oh-my-opencode`
- `superpowers`

这些 skill 的来源不在工具内硬编码。你需要在配置文件中提供本地路径或 Git 地址：

```toml
[profiles.standard]
tools = []
skills = ["openspec", "superpowers"]
items = []

[[skills]]
name = "openspec"
source = "D:/your/local/skills/openspec"
agents = ["claude", "opencode"]

[[skills]]
name = "superpowers"
source = "https://github.com/you/superpowers-skill.git"
agents = ["claude", "opencode"]
```

如果默认 skill 文件夹不存在，`rsenvforge` 会提示并跳过安装，不会自动创建该目录。

## --norustup

当用户无法使用 `rustup` 时，可以使用：

```powershell
rsenvforge install-crate <source> --norustup
rsenvforge install --norustup
```

`--norustup` 会跳过 rustup 检查，优先寻找仓库中的预编译二进制；如果没有预编译二进制但本机存在 `cargo`，则会尝试 `cargo build --release` 后复制产物。
