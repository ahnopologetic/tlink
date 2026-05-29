use super::adapter::{NotificationAdapter, NotificationRequest};
use super::utils::sh_quote;
use anyhow::Result;

const NOTIFICATION_LOGO: &str =
    "https://raw.githubusercontent.com/ahnopologetic/tlink/main/assets/notification-logo.png";

pub struct AlerterAdapter;

impl AlerterAdapter {
    /// Returns the shell script that runs alerter and handles the result.
    /// Separated from `notify` so the command string can be tested without spawning.
    pub fn build_script(&self, req: &NotificationRequest) -> String {
        let actions = if req.choices.is_empty() {
            "Open".to_string()
        } else {
            req.choices.join(",")
        };

        let mk_target = format!(
            "TARGET=$(printf '%s' {dl} | sed 's|tmux://||; s|/|:|; s|/|.|')",
            dl = sh_quote(&req.deeplink),
        );

        let handler = match req.notification_type.as_str() {
            "permission_prompt" => format!(
                "{mk}; case \"$result\" in \
                   Allow) tmux send-keys -t \"$TARGET\" 'y' Enter ;; \
                   Deny) tmux send-keys -t \"$TARGET\" 'n' Enter ;; \
                   @CONTENTCLICKED|@ACTIONCLICKED) open {dl} ;; \
                 esac",
                mk = mk_target,
                dl = sh_quote(&req.deeplink),
            ),
            "elicitation_dialog" if !req.choices.is_empty() => format!(
                "{mk}; case \"$result\" in \
                   @CONTENTCLICKED|@ACTIONCLICKED) open {dl} ;; \
                   @*) ;; \
                   *) tmux send-keys -t \"$TARGET\" \"$result\" Enter ;; \
                 esac",
                mk = mk_target,
                dl = sh_quote(&req.deeplink),
            ),
            _ => format!(
                "case \"$result\" in @CONTENTCLICKED|@ACTIONCLICKED) open {dl} ;; esac",
                dl = sh_quote(&req.deeplink),
            ),
        };

        format!(
            "result=$(alerter --title {t} --subtitle {loc} --message {m} --app-icon {icon} \
                --actions {actions} --close-label 'Dismiss' --sound 'Glass' --timeout 60); \
             {handler}",
            t = sh_quote(&req.title),
            loc = sh_quote(&req.location),
            m = sh_quote(&req.message),
            icon = sh_quote(NOTIFICATION_LOGO),
            actions = sh_quote(&actions),
            handler = handler,
        )
    }
}

impl NotificationAdapter for AlerterAdapter {
    fn name(&self) -> &str {
        "alerter"
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

    fn req(notification_type: &str, choices: Vec<String>) -> NotificationRequest {
        NotificationRequest {
            title: "Test Title".into(),
            message: "Test message".into(),
            location: "main > win > 0".into(),
            deeplink: "tmux://main/win/0".into(),
            notification_type: notification_type.into(),
            choices,
        }
    }

    #[test]
    fn build_script_contains_alerter() {
        let script = AlerterAdapter.build_script(&req("idle_prompt", vec![]));
        assert!(script.contains("alerter --title"));
    }

    #[test]
    fn build_script_default_action_open() {
        let script = AlerterAdapter.build_script(&req("idle_prompt", vec![]));
        assert!(script.contains("'Open'"));
    }

    #[test]
    fn build_script_permission_prompt_has_allow_deny() {
        let script = AlerterAdapter.build_script(&req("permission_prompt", vec![]));
        assert!(script.contains("Allow) tmux send-keys"));
        assert!(script.contains("Deny) tmux send-keys"));
    }

    #[test]
    fn build_script_elicitation_with_choices_uses_send_keys() {
        let script = AlerterAdapter
            .build_script(&req("elicitation_dialog", vec!["Yes".into(), "No".into()]));
        assert!(script.contains("tmux send-keys"));
        assert!(script.contains("'Yes,No'"));
    }

    #[test]
    fn build_script_elicitation_no_choices_falls_through_to_default() {
        // elicitation_dialog with no choices hits the _ arm
        let script = AlerterAdapter.build_script(&req("elicitation_dialog", vec![]));
        assert!(script.contains("@CONTENTCLICKED|@ACTIONCLICKED) open"));
        assert!(!script.contains("tmux send-keys"));
    }

    #[test]
    fn build_script_unknown_type_uses_content_click_open() {
        let script = AlerterAdapter.build_script(&req("stop", vec![]));
        assert!(script.contains("@CONTENTCLICKED|@ACTIONCLICKED) open"));
    }

    #[test]
    fn build_script_includes_logo_icon() {
        let script = AlerterAdapter.build_script(&req("idle_prompt", vec![]));
        assert!(script.contains("notification-logo.png"));
    }

    #[test]
    fn name_returns_alerter() {
        assert_eq!(AlerterAdapter.name(), "alerter");
    }
}
