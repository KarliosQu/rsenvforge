use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ForgeError {
    Io { path: PathBuf, source: io::Error },
    Parse(String),
    Config(String),
    Command(String),
}

impl Display for ForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgeError::Io { path, source } => write!(f, "{}: {}", path.display(), source),
            ForgeError::Parse(message) => write!(f, "解析错误：{message}"),
            ForgeError::Config(message) => write!(f, "配置错误：{message}"),
            ForgeError::Command(message) => write!(f, "命令错误：{message}"),
        }
    }
}

impl Error for ForgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ForgeError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
