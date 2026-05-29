use super::adapter::{NotificationAdapter, NotificationRequest};
use anyhow::Result;

pub struct TerminalNotifierAdapter;

impl TerminalNotifierAdapter {
    /// Returns the argument list for terminal-notifier (without the binary name).
    pub fn build_args(&self, req: &NotificationRequest) -> Vec<String> {
        vec![
            "-title".into(),
            req.title.clone(),
            "-subtitle".into(),
            req.location.clone(),
            "-message".into(),
            req.message.clone(),
            // -execute is broken on macOS 12+. -open invokes the registered URL scheme
            // handler on click, routing through tlink's tmux:// handler without PATH issues.
            "-open".into(),
            req.deeplink.clone(),
        ]
    }
}

impl NotificationAdapter for TerminalNotifierAdapter {
    fn name(&self) -> &str {
        "terminal-notifier"
    }

    fn notify(&self, req: &NotificationRequest) -> Result<()> {
        let args = self.build_args(req);
        std::process::Command::new("terminal-notifier")
            .args(&args)
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
            title: "Title".into(),
            message: "Msg".into(),
            location: "s > w > 0".into(),
            deeplink: "tmux://s/w/0".into(),
            notification_type: "idle_prompt".into(),
            choices: vec![],
        }
    }

    #[test]
    fn build_args_contains_title() {
        let args = TerminalNotifierAdapter.build_args(&req());
        let idx = args.iter().position(|a| a == "-title").unwrap();
        assert_eq!(args[idx + 1], "Title");
    }

    #[test]
    fn build_args_contains_subtitle_as_location() {
        let args = TerminalNotifierAdapter.build_args(&req());
        let idx = args.iter().position(|a| a == "-subtitle").unwrap();
        assert_eq!(args[idx + 1], "s > w > 0");
    }

    #[test]
    fn build_args_contains_message() {
        let args = TerminalNotifierAdapter.build_args(&req());
        let idx = args.iter().position(|a| a == "-message").unwrap();
        assert_eq!(args[idx + 1], "Msg");
    }

    #[test]
    fn build_args_uses_open_not_execute() {
        let args = TerminalNotifierAdapter.build_args(&req());
        assert!(args.contains(&"-open".to_string()));
        assert!(!args.contains(&"-execute".to_string()));
    }

    #[test]
    fn build_args_deeplink_passed_to_open() {
        let args = TerminalNotifierAdapter.build_args(&req());
        let idx = args.iter().position(|a| a == "-open").unwrap();
        assert_eq!(args[idx + 1], "tmux://s/w/0");
    }

    #[test]
    fn name_returns_terminal_notifier() {
        assert_eq!(TerminalNotifierAdapter.name(), "terminal-notifier");
    }
}
