use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not determine a config directory for this platform")]
    NoConfigDir,

    #[error("could not read config file at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write config file at {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not create config directory at {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not back up config file to {path}")]
    Backup {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Parse(String),

    #[error("{0}")]
    Validation(String),
}
