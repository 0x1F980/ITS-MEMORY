use std::fmt;

#[derive(Debug)]
pub enum MemError {
    Usage(String),
    NotFound(String),
    Pipe(String),
    Wire(String),
    Store(String),
    Coin(String),
    Io(std::io::Error),
}

pub type Result<T> = std::result::Result<T, MemError>;

impl fmt::Display for MemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(m) => write!(f, "{m}"),
            Self::NotFound(m) => write!(f, "not found: {m}"),
            Self::Pipe(m) => write!(f, "subprocess: {m}"),
            Self::Wire(m) => write!(f, "wire: {m}"),
            Self::Store(m) => write!(f, "store: {m}"),
            Self::Coin(m) => write!(f, "coin: {m}"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for MemError {}

impl From<std::io::Error> for MemError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for MemError {
    fn from(e: toml::de::Error) -> Self {
        Self::Wire(e.to_string())
    }
}

impl From<toml::ser::Error> for MemError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Wire(e.to_string())
    }
}
