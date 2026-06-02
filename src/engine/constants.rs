pub const CONFIG_FILE: &str = "rsenvforge.toml";
pub const REGISTRY_FILE: &str = "registry.tsv";
pub const SKILL_FILE: &str = "SKILL.md";

pub const BUILTIN_CONFIG: &str = r#"
[profiles.light]
tools = ["rust"]
skills = []
items = []

[profiles.standard]
tools = [
  "rust",
  "cargo-llvm-cov",
  "python",
  "bindgen-cli",
  "cargo-audit",
  "cargo-deny",
  "cargo-geiger",
  "cargo-udeps",
  "cargo-bloat",
  "flamegraph-rs",
  "perf",
  "cargo-msrv",
  "cargo-semver-checks",
  "cpp2rust-demo",
  "c2rust-demo",
  "rust-checker",
  "gitnexus",
]
skills = ["openspec", "oh-my-opencode", "superpowers"]
items = []

[profiles.full]
tools = [
  "rust",
  "cargo-llvm-cov",
  "python",
  "bindgen-cli",
  "cargo-audit",
  "cargo-deny",
  "cargo-geiger",
  "cargo-udeps",
  "cargo-bloat",
  "flamegraph-rs",
  "perf",
  "cargo-msrv",
  "cargo-semver-checks",
  "cpp2rust-demo",
  "c2rust-demo",
  "rust-checker",
  "gitnexus",
  "valgrind",
  "asan",
  "CMake",
  "Ninja",
  "Clang/libclang",
  "llvm-tools-preview",
  "clang++/g++",
]
skills = ["openspec", "oh-my-opencode", "superpowers"]
items = []

[[tools]]
name = "python"
check = "python --version"

[[tools]]
name = "perf"
check_linux = "perf --version"

[[tools]]
name = "valgrind"
check_linux = "valgrind --version"

[[tools]]
name = "asan"
check_linux = "cc --version"
check_windows = "clang --version"

[[tools]]
name = "CMake"
check = "cmake --version"

[[tools]]
name = "Ninja"
check = "ninja --version"

[[tools]]
name = "Clang/libclang"
check = "clang --version"

[[tools]]
name = "clang++/g++"
check_linux = "g++ --version"
check_windows = "clang++ --version"

[[tools]]
name = "gitnexus"
check = "gitnexus --version"
"#;
