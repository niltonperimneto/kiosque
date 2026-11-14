//! Desktop notifications through `org.freedesktop.Notifications`.

use std::collections::HashMap;

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::{Variant, proxy};

use crate::shell::{Command, Notification};

#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
pub trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Variant<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;
}

/// Sends notifications, replacing the previous one, and turns their action
/// buttons into [`Command`]s.
pub struct Notifier {
    proxy: NotificationsProxy<'static>,
    last: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl Notifier {
    pub async fn new(
        conn: &zbus::Connection,
        commands: UnboundedSender<Command>,
    ) -> zbus::Result<Self> {
        let proxy = NotificationsProxy::new(conn).await?;
        let last = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let mut actions = proxy.receive_action_invoked().await?;
        let ours = last.clone();
        tokio::spawn(async move {
            while let Some(signal) = actions.next().await {
                let Ok(args) = signal.args() else { continue };
                // Only react to our own notification.
                if args.id != ours.load(std::sync::atomic::Ordering::SeqCst) {
                    continue;
                }
                if let Some(command) = Command::from_action(&args.action_key) {
                    let _ = commands.send(command);
                }
            }
        });
        Ok(Self { proxy, last })
    }

    pub async fn send(&self, n: &Notification) {
        let mut actions: Vec<&str> = vec![];
        for (key, label) in &n.actions {
            actions.push(key);
            actions.push(label);
        }
        let mut hints = HashMap::new();
        hints.insert("desktop-entry", Variant::from("org.kiosque.Kiosque"));
        let replaces = self.last.load(std::sync::atomic::Ordering::SeqCst);
        match self
            .proxy
            .notify(
                "Kiosque",
                replaces,
                "system-software-update",
                &n.summary,
                &n.body,
                &actions,
                hints,
                -1,
            )
            .await
        {
            Ok(id) => self.last.store(id, std::sync::atomic::Ordering::SeqCst),
            Err(e) => kiosque_core::klog!("notification failed: {}", e),
        }
    }
}
