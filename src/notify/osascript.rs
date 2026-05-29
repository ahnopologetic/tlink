use super::adapter::{NotificationAdapter, NotificationRequest};
use super::utils::applescript_escape;
use anyhow::Result;

pub struct OsascriptAdapter;

impl OsascriptAdapter {
    pub fn build_script(&self, req: &NotificationRequest) -> String {
        format!(
            "display notification \"{}\" with title \"{}\" subtitle \"{}\" sound name \"Glass\"\n\
             open location \"{}\"",
            applescript_escape(&req.message),
            applescript_escape(&req.title),
            applescript_escape(&req.location),
            applescript_escape(&req.deeplink),
        )
    }
}

impl NotificationAdapter for OsascriptAdapter {
    fn name(&self) -> &str {
        "osascript"
    }

    fn notify(&self, req: &NotificationRequest) -> Result<()> {
        let script = self.build_script(req);
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .status()?;
        Ok(())
    }
}

pub fn alerter_available() -> bool {
    std::process::Command::new("which")
        .arg("alerter")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> NotificationRequest {
        NotificationRequest {
            title: "Title".into(),
            message: "Msg".into(),
            location: "s > w > 0".into(),
            deeplink: "tmux://s/w/0".into(),
            notification_type: "idle_prompt".into(),
            choices: vec![],
        }
    }

    #[test]
    fn build_script_contains_display_notification() {
        let s = OsascriptAdapter.build_script(&req());
        assert!(s.contains("display notification"));
    }

    #[test]
    fn build_script_contains_title_and_message() {
        let s = OsascriptAdapter.build_script(&req());
        assert!(s.contains("\"Msg\""));
        assert!(s.contains("\"Title\""));
    }

    #[test]
    fn build_script_contains_open_location_for_deeplink() {
        let s = OsascriptAdapter.build_script(&req());
        assert!(s.contains("open location"));
        assert!(s.contains("tmux://s/w/0"));
    }

    #[test]
    fn build_script_escapes_special_chars() {
        let r = NotificationRequest {
            title: r#"Say "hi""#.into(),
            message: r"foo\bar".into(),
            location: "s > w > 0".into(),
            deeplink: "tmux://s/w/0".into(),
            notification_type: "idle_prompt".into(),
            choices: vec![],
        };
        let s = OsascriptAdapter.build_script(&r);
        assert!(s.contains(r#"\"hi\""#));
        assert!(s.contains(r"foo\\bar"));
    }

    #[test]
    fn name_returns_osascript() {
        assert_eq!(OsascriptAdapter.name(), "osascript");
    }

    #[test]
    fn alerter_available_returns_bool() {
        // Just verify it doesn't panic; result depends on the environment.
        let _ = alerter_available();
    }
}
