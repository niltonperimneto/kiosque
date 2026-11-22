//! System tray icon: `org.kde.StatusNotifierItem` and its
//! `com.canonical.dbusmenu` menu, served on the daemon's bus connection.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kiosque_core::l10n::i18n;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;
use zbus::object_server::SignalEmitter;
use zbus::{OwnedObjectPath, OwnedVariant, Type, Variant, interface};

use crate::shell::{Command, TrayState};

pub const ITEM_PATH: &str = "/StatusNotifierItem";
pub const MENU_PATH: &str = "/MenuBar";
const ICON: &str = "system-software-update";

/// Menu entries; ids are stable for the life of the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Open = 1,
    Check = 2,
    UpdateAll = 3,
    Separator = 4,
    Quit = 5,
}

const ENTRIES: [Entry; 5] = [
    Entry::Open,
    Entry::Check,
    Entry::UpdateAll,
    Entry::Separator,
    Entry::Quit,
];

impl Entry {
    fn from_id(id: i32) -> Option<Self> {
        ENTRIES.into_iter().find(|e| *e as i32 == id)
    }

    fn command(self) -> Option<Command> {
        match self {
            Entry::Open => Some(Command::OpenStore),
            Entry::Check => Some(Command::Check),
            Entry::UpdateAll => Some(Command::UpdateAll),
            Entry::Quit => Some(Command::Quit),
            Entry::Separator => None,
        }
    }

    fn properties(self, state: &TrayState) -> HashMap<String, OwnedVariant> {
        let mut p = HashMap::new();
        let mut put = |k: &str, v: Variant<'_>| {
            if let Ok(v) = OwnedVariant::try_from(v) {
                p.insert(k.to_string(), v);
            }
        };
        let (label, icon, enabled) = match self {
            Entry::Separator => {
                put("type", Variant::from("separator"));
                return p;
            }
            Entry::Open => (i18n("Open Store", &[]), "org.kiosque.Kiosque", true),
            Entry::Check => (
                i18n("Check for Updates", &[]),
                "view-refresh",
                state.can_check,
            ),
            Entry::UpdateAll => (i18n("Update All", &[]), ICON, state.can_update),
            Entry::Quit => (i18n("Quit", &[]), "application-exit", true),
        };
        put("label", Variant::from(label));
        put("icon-name", Variant::from(icon));
        put("enabled", Variant::from(enabled));
        put("visible", Variant::from(true));
        p
    }
}

/// SNI tooltip: `(icon name, icon pixmaps (w, h, ARGB32), title, description)`.
type ToolTip = (String, Vec<(i32, i32, Vec<u8>)>, String, String);

/// dbusmenu layout node `(ia{sv}av)`; children are variants of the same shape.
#[derive(Debug, Serialize, Type)]
pub struct Layout(i32, HashMap<String, OwnedVariant>, Vec<OwnedVariant>);

fn layout(state: &TrayState) -> Layout {
    let children = ENTRIES
        .iter()
        .filter_map(|e| {
            let node = Layout(*e as i32, e.properties(state), vec![]);
            let v = Variant::from(
                zbus::wire::StructureBuilder::new()
                    .add_field(node.0)
                    .add_field(node.1)
                    .add_field(node.2)
                    .build()
                    .ok()?,
            );
            OwnedVariant::try_from(v).ok()
        })
        .collect();
    let mut root = HashMap::new();
    if let Ok(v) = OwnedVariant::try_from(Variant::from("submenu")) {
        root.insert("children-display".to_string(), v);
    }
    Layout(0, root, children)
}

/// State shared by the item, the menu and the main loop.
pub struct Shared {
    pub state: Mutex<TrayState>,
    pub revision: Mutex<u32>,
    pub commands: UnboundedSender<Command>,
}

pub struct Item(pub Arc<Shared>);
pub struct Menu(pub Arc<Shared>);

#[interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property)]
    fn category(&self) -> &str {
        "SystemServices"
    }
    #[zbus(property)]
    fn id(&self) -> &str {
        "org.kiosque.Kiosque.Update"
    }
    #[zbus(property)]
    fn title(&self) -> String {
        i18n("Kiosque Update Notifier", &[])
    }
    #[zbus(property)]
    fn status(&self) -> String {
        self.0.state.lock().unwrap().status.to_string()
    }
    #[zbus(property)]
    fn icon_name(&self) -> &str {
        ICON
    }
    #[zbus(property)]
    fn attention_icon_name(&self) -> &str {
        ICON
    }
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(MENU_PATH).expect("static path")
    }
    /// `(icon name, icon pixmaps, title, description)`.
    #[zbus(property)]
    fn tool_tip(&self) -> ToolTip {
        let tip = self.0.state.lock().unwrap().tooltip.clone();
        (ICON.into(), vec![], i18n("Kiosque", &[]), tip)
    }

    fn activate(&self, _x: i32, _y: i32) {
        let _ = self.0.commands.send(Command::OpenStore);
    }
    fn secondary_activate(&self, _x: i32, _y: i32) {
        let _ = self.0.commands.send(Command::Check);
    }
    fn context_menu(&self, _x: i32, _y: i32) {}
    fn scroll(&self, _delta: i32, _orientation: &str) {}

    #[zbus(signal)]
    pub async fn new_status(emitter: &SignalEmitter<'_>, status: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    pub async fn new_tool_tip(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

#[interface(name = "com.canonical.dbusmenu")]
impl Menu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }
    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }
    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }
    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        vec![]
    }

    fn get_layout(
        &self,
        _parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> (u32, Layout) {
        let state = self.0.state.lock().unwrap().clone();
        (*self.0.revision.lock().unwrap(), layout(&state))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, OwnedVariant>)> {
        let state = self.0.state.lock().unwrap().clone();
        ENTRIES
            .iter()
            .filter(|e| ids.is_empty() || ids.contains(&(**e as i32)))
            .map(|e| (*e as i32, e.properties(&state)))
            .collect()
    }

    fn get_property(&self, id: i32, name: &str) -> zbus::fdo::Result<OwnedVariant> {
        let state = self.0.state.lock().unwrap().clone();
        Entry::from_id(id)
            .and_then(|e| e.properties(&state).remove(name))
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs(format!("no property {name} on {id}")))
    }

    fn event(&self, id: i32, event_id: &str, _data: OwnedVariant, _timestamp: u32) {
        if event_id == "clicked"
            && let Some(command) = Entry::from_id(id).and_then(Entry::command)
        {
            let _ = self.0.commands.send(command);
        }
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedVariant, u32)>) -> Vec<i32> {
        let mut unknown = vec![];
        for (id, event_id, _, _) in events {
            match Entry::from_id(id).and_then(Entry::command) {
                Some(command) if event_id == "clicked" => {
                    let _ = self.0.commands.send(command);
                }
                Some(_) => {}
                None => unknown.push(id),
            }
        }
        unknown
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (vec![], vec![])
    }

    #[zbus(signal)]
    pub async fn layout_updated(
        emitter: &SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}

/// Serve the item and menu and register with the StatusNotifierWatcher.
/// A missing watcher (no tray host yet) is not an error: hosts re-scan
/// registered items when they start.
pub async fn serve(conn: &zbus::Connection, shared: Arc<Shared>) -> zbus::Result<()> {
    conn.object_server()
        .at(ITEM_PATH, Item(shared.clone()))
        .await?;
    conn.object_server().at(MENU_PATH, Menu(shared)).await?;
    let service = conn
        .unique_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    if let Err(e) = conn
        .call_method(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            Some("org.kde.StatusNotifierWatcher"),
            "RegisterStatusNotifierItem",
            &(service.as_str(),),
        )
        .await
    {
        kiosque_core::klog!("tray: no StatusNotifierWatcher: {}", e);
    }
    Ok(())
}

/// Publish a new tray state: property change signals plus a menu relayout.
pub async fn update(conn: &zbus::Connection, shared: &Shared, state: TrayState) {
    *shared.state.lock().unwrap() = state.clone();
    let revision = {
        let mut r = shared.revision.lock().unwrap();
        *r += 1;
        *r
    };
    if let Ok(emitter) = SignalEmitter::new(conn, ITEM_PATH) {
        let _ = Item::new_status(&emitter, state.status).await;
        let _ = Item::new_tool_tip(&emitter).await;
    }
    if let Ok(emitter) = SignalEmitter::new(conn, MENU_PATH) {
        let _ = Menu::layout_updated(&emitter, revision, 0).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(can_update: bool) -> TrayState {
        TrayState {
            status: "Active",
            tooltip: String::new(),
            can_update,
            can_check: true,
        }
    }

    #[test]
    fn layout_signature_and_entries() {
        assert_eq!(Layout::SIGNATURE.to_string(), "(ia{sv}av)");
        let l = layout(&state(false));
        assert_eq!(l.0, 0);
        assert_eq!(l.2.len(), ENTRIES.len());
        // The whole reply must serialize as D-Bus `u(ia{sv}av)`.
        let msg = zbus::Message::signal(MENU_PATH, "com.canonical.dbusmenu", "Probe")
            .build(&(1u32, l))
            .unwrap();
        assert_eq!(msg.body().signature().to_string(), "(u(ia{sv}av))");
    }

    #[test]
    fn update_all_follows_state() {
        let off = Entry::UpdateAll.properties(&state(false));
        let on = Entry::UpdateAll.properties(&state(true));
        assert!(!bool::try_from(&off["enabled"]).unwrap());
        assert!(bool::try_from(&on["enabled"]).unwrap());
        assert!(
            Entry::Separator
                .properties(&state(true))
                .contains_key("type")
        );
        assert_eq!(
            Entry::from_id(5).and_then(Entry::command),
            Some(Command::Quit)
        );
        assert_eq!(Entry::from_id(4).and_then(Entry::command), None);
        assert_eq!(Entry::from_id(42), None);
    }
}
