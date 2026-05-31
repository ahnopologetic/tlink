use anyhow::Result;

pub struct NotificationRequest {
    pub title: String,
    pub message: String,
    pub location: String,
    pub deeplink: String,
    pub session: String,
}

pub trait NotificationAdapter: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &str;
    fn notify(&self, req: &NotificationRequest) -> Result<()>;
}
