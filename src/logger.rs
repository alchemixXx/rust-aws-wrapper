use std::fmt::{Display, Formatter, Result};

use env_logger::Env;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug)]
pub struct Logger {}

impl Logger {
    pub fn new() -> Self {
        Self {}
    }

    pub fn init(level: LogLevel) {
        env_logger::Builder::from_env(Env::default().default_filter_or(level.to_string())).init();
    }

    pub fn debug<S: AsRef<str>>(&self, message: S) {
        log::debug!("{}", message.as_ref());
    }

    pub fn info<S: AsRef<str>>(&self, message: S) {
        log::info!("{}", message.as_ref());
    }

    pub fn warn<S: AsRef<str>>(&self, message: S) {
        log::warn!("{}", message.as_ref());
    }

    pub fn error<S: AsRef<str>>(&self, message: S) {
        log::error!("{}", message.as_ref());
    }
}

pub trait LoggerTrait {
    fn get_logger(&self) -> Logger {
        Logger::new()
    }
}
