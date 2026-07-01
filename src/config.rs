use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::logger::Logger;

const CONFIG_FILE_NAME: &str = ".rust-aws-wrapper.toml";

#[derive(Debug, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    /// When true, SSO authentication is disabled and local AWS credentials
    /// (from environment variables or ~/.aws/credentials) are used instead.
    #[serde(default)]
    pub disable_sso: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self { disable_sso: false }
    }
}

impl AppConfig {
    /// Loads config from `.rust-aws-wrapper.toml` in the current directory,
    /// then walks up parent directories. Returns default config if no file is found.
    pub fn load() -> Self {
        let logger = Logger::new();

        if let Some(path) = Self::find_config_file() {
            logger.debug(format!("Loading config from: {}", path.display()));
            match fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str::<AppConfig>(&contents) {
                    Ok(config) => {
                        logger.debug(format!("Config loaded: {:?}", config));
                        return config;
                    }
                    Err(err) => {
                        logger.warn(format!(
                            "Failed to parse config file {}: {}",
                            path.display(),
                            err
                        ));
                    }
                },
                Err(err) => {
                    logger.warn(format!(
                        "Failed to read config file {}: {}",
                        path.display(),
                        err
                    ));
                }
            }
        } else {
            logger.debug("No config file found, using defaults");
        }

        AppConfig::default()
    }

    /// Searches for the config file starting from the current directory
    /// and walking up to parent directories.
    fn find_config_file() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;

        loop {
            let candidate = dir.join(CONFIG_FILE_NAME);
            if candidate.is_file() {
                return Some(candidate);
            }
            if !dir.pop() {
                break;
            }
        }

        None
    }
}
