use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
pub struct Config {
    pub terminal: Option<String>,
    /// Notification method chosen during `tlink install claude-notification`
    /// Values: "terminal-notifier" | "osascript" | "dunstify" | "notify-send"
    pub notification_method: Option<String>,
    /// Telemetry opt-in. `true` = enabled, `false` = disabled, `None` = undecided.
    pub telemetry_enabled: Option<bool>,
    /// Sentry DSN for error tracking (optional, set via TLINK_SENTRY_DSN or config).
    pub sentry_dsn: Option<String>,
}

pub fn config_path() -> PathBuf {
    // Allow override via TLINK_CONFIG env var (useful for testing)
    if let Ok(override_path) = std::env::var("TLINK_CONFIG") {
        return PathBuf::from(override_path);
    }
    dirs::home_dir()
        .expect("home dir not found")
        .join(".config/tlink/config.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&content)?)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, toml::to_string(config)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_when_file_missing() {
        let fake_path = std::env::temp_dir().join("tlink-nonexistent-99999/config.toml");
        assert!(!fake_path.exists());
        let config: Config = if !fake_path.exists() {
            Config::default()
        } else {
            toml::from_str(&std::fs::read_to_string(&fake_path).unwrap()).unwrap()
        };
        assert_eq!(config, Config::default());
    }

    #[test]
    fn test_roundtrip_serialization() {
        let config = Config {
            terminal: Some("iTerm2".to_string()),
            ..Default::default()
        };
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.terminal.as_deref(), Some("iTerm2"));
    }

    #[test]
    fn test_default_has_no_terminal() {
        let c = Config::default();
        assert!(c.terminal.is_none());
    }
}
