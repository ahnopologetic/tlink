//! Telemetry: error tracking (Sentry) + local activity log.
//!
//! ## Opt Model
//! Users choose during `tlink setup` or via `tlink telemetry enable|disable`.
//! Default: disabled (opt-in) until the user decides.
//!
//! ## Override Env Var
//! `TLINK_TELEMETRY=1` or `=0` takes priority over config at runtime.
//!
//! ## What's collected (all anonymous)
//! - Command name, success/failure, version, platform
//! - Errors / panics (backtrace, sent to Sentry if DSN is configured)
//!
//! ## Privacy
//! Activity events are written locally to `~/.local/share/tlink/telemetry/events.jsonl`.
//! No network calls are made for activity data. Users can inspect and share
//! the file manually. Sentry only fires when a DSN is set (via `TLINK_SENTRY_DSN`
//! or config `telemetry.sentry_dsn`).

use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Unique machine ID — generated once and stored alongside the event file.
fn machine_id() -> String {
    let dir = data_dir();
    let id_path = dir.join("machine-id");
    if let Ok(existing) = std::fs::read_to_string(&id_path) {
        let trimmed = existing.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&id_path, &id);
    id
}

/// Data directory for telemetry artifacts (`~/.local/share/tlink/telemetry`).
fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(dir).join("tlink/telemetry")
    } else {
        let home = dirs::home_dir().expect("home dir not found");
        home.join(".local/share/tlink/telemetry")
    }
}

/// Path to the JSON-lines event file.
fn event_file_path() -> PathBuf {
    data_dir().join("events.jsonl")
}

/// Path to the Telemetry DSN file (optional, set during `tlink setup`).
pub fn dsn_path() -> PathBuf {
    data_dir().join("sentry-dsn")
}

// ── opt-in check ─────────────────────────────────────────────────────────────

/// Check whether telemetry is currently opted in.
///
/// Priority:
/// 1. `TLINK_TELEMETRY` env var (`1`/`true`/`yes` = enabled)
/// 2. Config `telemetry_enabled` field
/// 3. Default: `false` (opt-in model)
pub fn enabled() -> bool {
    // Env var overrides everything
    if let Ok(val) = std::env::var("TLINK_TELEMETRY") {
        return matches!(val.as_str(), "1" | "true" | "yes");
    }
    // Fall back to config
    crate::config::load()
        .ok()
        .and_then(|c| c.telemetry_enabled)
        .unwrap_or(false)
}

// ── sentry init / shutdown ───────────────────────────────────────────────────

/// Initialize Sentry. No-op if telemetry is disabled or no DSN is available.
pub fn init() {
    if !enabled() {
        return;
    }
    let _ = std::fs::create_dir_all(data_dir());

    let dsn = resolve_dsn();
    let guard = sentry::init(sentry::ClientOptions {
        dsn: dsn.as_deref().and_then(|s| s.parse().ok()),
        release: sentry::release_name!(),
        before_send: Some(std::sync::Arc::new(|mut event| {
            // Strip machine-id from breadcrumbs to avoid leaking PII
            for bc in event.breadcrumbs.iter_mut() {
                bc.data.remove("machine_id");
            }
            Some(event)
        })),
        ..Default::default()
    });
    // Leak the guard so it lives for the process lifetime.
    std::mem::forget(guard);
}

/// Resolve the Sentry DSN from env var, file, or config.
fn resolve_dsn() -> Option<String> {
    // 1. Env var
    if let Ok(dsn) = std::env::var("TLINK_SENTRY_DSN") {
        if !dsn.is_empty() {
            return Some(dsn);
        }
    }
    // 2. DSN file (written by `tlink setup` or manually)
    let dsn_file = dsn_path();
    if let Ok(dsn) = std::fs::read_to_string(&dsn_file) {
        let trimmed = dsn.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    // 3. Config
    if let Ok(cfg) = crate::config::load() {
        if let Some(dsn) = cfg.sentry_dsn {
            if !dsn.is_empty() {
                return Some(dsn);
            }
        }
    }
    None
}

/// Flush Sentry before exit.
pub fn shutdown() {
    if enabled() {
        if let Some(client) = sentry::Hub::current().client() {
            let _ = client.flush(Some(std::time::Duration::from_secs(5)));
        }
    }
}

// ── local event recording ────────────────────────────────────────────────────

/// A single activity event stored as a JSON line.
#[derive(Serialize)]
struct Event {
    pub timestamp: String,
    pub machine_id: String,
    pub version: String,
    pub platform: String,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
}

fn iso_now() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = dur.as_secs();

    // Civil calendar from unix timestamp (avoids pulling in chrono)
    // Based on Howard Hinnant's algorithm.
    let z = total_secs / 86400 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    let time_secs = total_secs % 86400;
    let h = time_secs / 3600;
    let min = (time_secs % 3600) / 60;
    let s = time_secs % 60;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, m, d, h, min, s
    )
}

/// Record an activity event to the local JSONL file.
/// Also adds a Sentry breadcrumb for error context.
/// No-op if telemetry is disabled.
pub fn record_event(name: &str, properties: Option<serde_json::Value>) {
    if !enabled() {
        return;
    }

    let event = Event {
        timestamp: iso_now(),
        machine_id: machine_id(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "other"
        }
        .to_string(),
        event: name.to_string(),
        properties,
    };

    // Write to local file
    let line = serde_json::to_string(&event).unwrap_or_default();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(event_file_path())
    {
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }

    // Sentry breadcrumb for error correlation
    sentry::add_breadcrumb(sentry::Breadcrumb {
        ty: "default".into(),
        category: Some("activity".into()),
        message: Some(name.into()),
        ..Default::default()
    });
}

// ── CLI actions ──────────────────────────────────────────────────────────────

/// Enable telemetry and store the choice in config.
pub fn enable(dsn: Option<String>) -> Result<()> {
    let mut cfg = crate::config::load().unwrap_or_default();
    cfg.telemetry_enabled = Some(true);
    if let Some(dsn) = dsn {
        cfg.sentry_dsn = Some(dsn);
    }
    crate::config::save(&cfg)?;
    println!("Telemetry enabled.");
    println!("Activity events are logged locally to ~/.local/share/tlink/telemetry/events.jsonl");
    if cfg.sentry_dsn.is_some() {
        println!("Error reports will be sent to Sentry.");
    } else {
        println!("No Sentry DSN configured — error tracking off. Set TLINK_SENTRY_DSN or add sentry_dsn to config.");
    }
    // If Sentry wasn't running yet, initialize now
    init();
    record_event("telemetry.enabled", None);
    Ok(())
}

/// Disable telemetry.
pub fn disable() -> Result<()> {
    let mut cfg = crate::config::load().unwrap_or_default();
    cfg.telemetry_enabled = Some(false);
    crate::config::save(&cfg)?;
    println!("Telemetry disabled.");
    Ok(())
}

/// Show current telemetry status.
pub fn status() -> Result<()> {
    let cfg = crate::config::load().unwrap_or_default();
    let env_override = std::env::var("TLINK_TELEMETRY").ok();
    let active = enabled();

    println!("Telemetry status:");
    println!(
        "  Active:              {}",
        if active { "yes" } else { "no" }
    );
    println!(
        "  Config setting:      {}",
        match cfg.telemetry_enabled {
            Some(true) => "enabled",
            Some(false) => "disabled",
            None => "not set (default: disabled)",
        }
    );
    println!(
        "  Env override:        {}",
        env_override.as_deref().unwrap_or("none"),
    );
    println!(
        "  Sentry DSN:          {}",
        match resolve_dsn() {
            Some(_) => "configured",
            None => "not configured",
        }
    );
    println!("  Event file:          {}", event_file_path().display());
    if event_file_path().exists() {
        let line_count = std::fs::read_to_string(event_file_path())
            .map(|s| s.lines().count())
            .unwrap_or(0);
        println!("  Events recorded:     {line_count}");
        let size = std::fs::metadata(event_file_path())
            .map(|m| m.len())
            .unwrap_or(0);
        println!("  File size:           {} bytes", size);
    } else {
        println!("  Events recorded:     0");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_disabled_by_default() {
        std::env::remove_var("TLINK_TELEMETRY");
        assert!(!enabled());
    }

    #[test]
    fn test_telemetry_enabled_via_env() {
        std::env::set_var("TLINK_TELEMETRY", "1");
        assert!(enabled());
        std::env::remove_var("TLINK_TELEMETRY");
    }

    #[test]
    fn test_telemetry_disabled_via_env() {
        std::env::set_var("TLINK_TELEMETRY", "0");
        assert!(!enabled());
        std::env::remove_var("TLINK_TELEMETRY");
    }

    #[test]
    fn test_machine_id_is_stable() {
        let id1 = machine_id();
        let id2 = machine_id();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_iso_now_returns_string() {
        let s = iso_now();
        assert!(!s.is_empty());
        assert!(s.contains('T'));
    }

    #[test]
    fn test_record_event_disabled_no_panic() {
        std::env::remove_var("TLINK_TELEMETRY");
        record_event("test.event", None);
        record_event("test.event", Some(serde_json::json!({"key": "value"})));
    }
}
