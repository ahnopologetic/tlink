use std::path::PathBuf;

/// Canonical raw GitHub URL for the tlink notification logo.
/// Downloaded on first access and cached at `~/.tlink/notification-logo.png`.
pub const ICON_URL: &str =
    "https://raw.githubusercontent.com/ahnopologetic/tlink/main/assets/notification-logo.png";

/// Returns the path to the notification logo, downloading it from the
/// raw GitHub URL on first access. The icon is cached at `~/.tlink/notification-logo.png`.
pub fn ensure_icon() -> std::io::Result<PathBuf> {
    let tlink_dir = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home dir not found"))?
        .join(".tlink");

    // Create ~/.tlink if it doesn't exist.
    std::fs::create_dir_all(&tlink_dir)?;

    let icon_path = tlink_dir.join("notification-logo.png");

    // Download the icon only if it doesn't already exist (idempotent).
    if !icon_path.exists() {
        let status = std::process::Command::new("curl")
            .args(["-fsSL", "-o"])
            .arg(&icon_path)
            .arg(ICON_URL)
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other(format!(
                "curl failed with exit code {}",
                status
            )));
        }
    }

    Ok(icon_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_url_is_https() {
        assert!(ICON_URL.starts_with("https://"));
    }

    #[test]
    fn icon_url_points_to_tlink_repo_asset() {
        assert!(ICON_URL.contains("ahnopologetic/tlink"));
        assert!(ICON_URL.ends_with("notification-logo.png"));
    }
}
