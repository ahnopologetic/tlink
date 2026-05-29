use super::adapter::{NotificationAdapter, NotificationRequest};
use anyhow::Result;

pub struct NotifySendAdapter;

impl NotifySendAdapter {
    pub fn build_args(&self, req: &NotificationRequest) -> Vec<String> {
        vec![
            req.title.clone(),
            format!("{}\n{}", req.message, req.location),
            "--urgency=normal".into(),
            "--icon=utilities-terminal".into(),
            "--app-name=Claude Code".into(),
        ]
    }
}

impl NotificationAdapter for NotifySendAdapter {
    fn name(&self) -> &str {
        "notify-send"
    }

    fn notify(&self, req: &NotificationRequest) -> Result<()> {
        let args = self.build_args(req);
        std::process::Command::new("notify-send")
            .args(&args)
            .status()?;
        Ok(())
    }
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
    fn build_args_first_is_title() {
        let args = NotifySendAdapter.build_args(&req());
        assert_eq!(args[0], "Title");
    }

    #[test]
    fn build_args_second_combines_message_and_location() {
        let args = NotifySendAdapter.build_args(&req());
        assert!(args[1].contains("Msg"));
        assert!(args[1].contains("s > w > 0"));
    }

    #[test]
    fn build_args_has_urgency_and_icon() {
        let args = NotifySendAdapter.build_args(&req());
        assert!(args.contains(&"--urgency=normal".to_string()));
        assert!(args.contains(&"--icon=utilities-terminal".to_string()));
    }

    #[test]
    fn build_args_has_app_name() {
        let args = NotifySendAdapter.build_args(&req());
        assert!(args.contains(&"--app-name=Claude Code".to_string()));
    }

    #[test]
    fn name_returns_notify_send() {
        assert_eq!(NotifySendAdapter.name(), "notify-send");
    }
}
