use super::adapter::{NotificationAdapter, NotificationRequest};
use super::utils::sh_quote;
use anyhow::Result;

pub struct DunstifyAdapter;

impl DunstifyAdapter {
    pub fn build_script(&self, req: &NotificationRequest) -> String {
        format!(
            "ACTION=$(dunstify {t} {m} --action='default,Go there' \
                --urgency=normal --icon=utilities-terminal --appname='Claude Code'); \
             [ \"$ACTION\" = \"default\" ] && tlink open {dl}",
            t = sh_quote(&req.title),
            m = sh_quote(&req.message),
            dl = sh_quote(&req.deeplink),
        )
    }
}

impl NotificationAdapter for DunstifyAdapter {
    fn name(&self) -> &str {
        "dunstify"
    }

    fn notify(&self, req: &NotificationRequest) -> Result<()> {
        let cmd = self.build_script(req);
        std::process::Command::new("sh")
            .args(["-c", &cmd])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> NotificationRequest {
        NotificationRequest {
            title: "Claude".into(),
            message: "Done".into(),
            location: "s > w > 0".into(),
            deeplink: "tmux://s/w/0".into(),
        }
    }

    #[test]
    fn build_script_calls_dunstify() {
        let s = DunstifyAdapter.build_script(&req());
        assert!(s.contains("dunstify"));
    }

    #[test]
    fn build_script_has_click_action() {
        let s = DunstifyAdapter.build_script(&req());
        assert!(s.contains("tlink open"));
        assert!(s.contains("\"default\""));
    }

    #[test]
    fn build_script_includes_title_and_message() {
        let s = DunstifyAdapter.build_script(&req());
        assert!(s.contains("'Claude'"));
        assert!(s.contains("'Done'"));
    }

    #[test]
    fn build_script_deeplink_passed() {
        let s = DunstifyAdapter.build_script(&req());
        assert!(s.contains("tmux://s/w/0"));
    }

    #[test]
    fn name_returns_dunstify() {
        assert_eq!(DunstifyAdapter.name(), "dunstify");
    }
}
