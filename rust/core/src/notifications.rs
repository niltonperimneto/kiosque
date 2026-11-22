// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Kiosque Contributors

//! Desktop notifications through `org.freedesktop.Notifications`.

use std::collections::HashMap;

use zbus::zvariant::Variant;

/// Desktop entry the notifications are attributed to.
const DESKTOP_ENTRY: &str = "org.kiosque.Kiosque";

/// How long a notification stays visible, in milliseconds.
const EXPIRE_TIMEOUT_MS: i32 = 8000;

/// Show a desktop notification attributed to Kiosque's desktop entry and
/// return its id. Fails when no notification service is on the session bus.
pub async fn notify(title: &str, body: &str, icon: &str) -> Result<u32, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let mut hints: HashMap<&str, Variant<'_>> = HashMap::new();
    hints.insert("desktop-entry", Variant::from(DESKTOP_ENTRY));
    let actions: Vec<&str> = Vec::new();
    let reply = conn
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "Kiosque",
                0u32,
                icon,
                title,
                body,
                actions,
                hints,
                EXPIRE_TIMEOUT_MS,
            ),
        )
        .await
        .map_err(|e| e.to_string())?;
    reply.body().deserialize::<u32>().map_err(|e| e.to_string())
}
