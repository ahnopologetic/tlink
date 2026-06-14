use super::adapter::{NotificationAdapter, NotificationRequest};
use anyhow::Result;

pub struct TerminalNotifierAdapter;

impl TerminalNotifierAdapter {
    /// Returns the argument list for terminal-notifier (without the binary name).
    pub fn build_args(&self, req: &NotificationRequest) -> Vec<String> {
        let args = vec![
            "-title".into(),
            req.title.clone(),
            "-subtitle".into(),
            req.location.clone(),
            "-message".into(),
            req.message.clone(),
            // -group enables Notification Center to stack multiple notifications
            // from the same tmux session into a single collapsible group.
            "-group".into(),
            req.session.clone(),
            // -execute is broken on macOS 12+. -open invokes the registered URL scheme
            // handler on click, routing through tlink's tmux:// handler without PATH issues.
            "-open".into(),
            req.deeplink.clone(),
            // -sender uses the TmuxLink.app bundle identifier so macOS pulls
            // the app icon from the bundle (Contents/Resources/AppIcon.png).
            // -appIcon with a raw file path is unreliable across macOS versions.
            "-sender".into(),
            "com.tlink.handler".into(),
        ];
        args
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
            session: "s".into(),
            icon_path: "/tmp/tlink-logo.png".into(),
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
    fn build_args_contains_group_session() {
        let args = TerminalNotifierAdapter.build_args(&req());
        let idx = args.iter().position(|a| a == "-group").unwrap();
        assert_eq!(args[idx + 1], "s");
    }

    #[test]
    fn build_args_uses_sender_not_app_icon() {
        // -sender com.tlink.handler is the reliable way to set the
        // notification icon — macOS pulls it from the app bundle.
        // -appIcon with a file path is unreliable across macOS versions.
        let args = TerminalNotifierAdapter.build_args(&req());
        let idx = args.iter().position(|a| a == "-sender").unwrap();
        assert_eq!(args[idx + 1], "com.tlink.handler");
        assert!(!args.contains(&"-appIcon".to_string()));
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
