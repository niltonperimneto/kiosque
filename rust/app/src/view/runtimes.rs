//! Runtimes, shown transparently: what an install will download, what a
//! removal leaves unused, and which installed runtimes serve which apps.
//! Qt-free; `bridge::runtime_planner` and `bridge::runtimes_model` adapt it.

use kiosque_core::entries::format_size;
use kiosque_core::flatpak::{
    InstalledRuntime, Operation, OperationPlan, RefKind, StepAction, runtime_display_name,
};
use serde_json::{Value, json};

use crate::services::services;

/// What installing `app_id` from `remote_hint` (its origin, or "") would do.
pub async fn plan_install(app_id: &str, remote_hint: &str) -> Result<OperationPlan, String> {
    let (remote, scope) = services().install_target_from(app_id, remote_hint);
    services()
        .backend
        .plan(&Operation::Install {
            remote,
            id: app_id.to_string(),
            scope,
        })
        .await
        .map_err(|e| e.to_string())
}

/// What removing `app_id` would remove, with the runtimes it leaves unused.
pub async fn plan_removal(app_id: &str) -> Result<OperationPlan, String> {
    services()
        .backend
        .plan(&Operation::Uninstall {
            id: app_id.to_string(),
            scope: None,
        })
        .await
        .map_err(|e| e.to_string())
}

/// The installation summary for the detail page. `runtime` is the full ref
/// of the runtime the app runs on (from its metadata), if known.
pub fn install_summary(plan: &OperationPlan, runtime: &str) -> Value {
    let runtimes: Vec<Value> = plan
        .runtimes()
        .filter(|s| s.action != StepAction::Uninstall)
        .map(|s| {
            json!({
                "reference": s.reference,
                "name": s.display_name(),
                "download": format_size(s.download_size),
                "installed": format_size(s.installed_size),
                "eol": s.eol,
            })
        })
        .collect();
    let runtime_needed = !runtime.is_empty() && plan.steps.iter().any(|s| s.reference == runtime);
    let eol = plan
        .steps
        .iter()
        .find(|s| !s.eol.is_empty())
        .map(|s| format!("{}: {}", s.display_name(), s.eol))
        .unwrap_or_default();
    json!({
        "known": !plan.steps.is_empty(),
        "download": format_size(plan.total_download()),
        "installed": format_size(plan.total_installed()),
        "runtime": if runtime.is_empty() { String::new() } else { runtime_display_name(runtime) },
        // The app's runtime is already installed when the plan omits it.
        "runtimeInstalled": !runtime.is_empty() && !runtime_needed,
        "runtimes": runtimes,
        "eol": eol,
    })
}

/// Runtimes a removal would leave unused, for the confirmation dialog.
pub fn removal_leftovers(plan: &OperationPlan) -> Vec<Value> {
    plan.runtimes()
        .filter(|s| s.action == StepAction::Uninstall)
        .map(|s| {
            json!({
                "reference": s.reference,
                "name": s.display_name(),
                "size": format_size(s.installed_size),
            })
        })
        .collect()
}

/// One installed runtime as the Installed page lists it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeRow {
    pub reference: String,
    pub name: String,
    pub size: String,
    /// Readable list of the applications using it ("" when none).
    pub used_by: String,
    pub unused: bool,
    pub eol: String,
    pub scope: String,
}

/// Installed runtimes, unused ones first, then by name. App ids are shown by
/// their installed names when known.
pub fn rows(runtimes: Vec<InstalledRuntime>, app_name: impl Fn(&str) -> String) -> Vec<RuntimeRow> {
    let mut rows: Vec<RuntimeRow> = runtimes
        .into_iter()
        .filter(|r| r.info.kind == RefKind::Runtime)
        .map(|r| {
            let reference = r.info.ref_string();
            RuntimeRow {
                name: runtime_display_name(&reference),
                size: format_size(r.info.installed_size),
                used_by: r
                    .used_by
                    .iter()
                    .map(|id| app_name(id))
                    .collect::<Vec<_>>()
                    .join(", "),
                unused: r.unused,
                eol: r.eol,
                scope: r.info.scope.as_str().to_string(),
                reference,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.unused
            .cmp(&a.unused)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    rows
}

pub async fn installed_runtimes() -> Result<Vec<RuntimeRow>, String> {
    let runtimes = services()
        .backend
        .runtimes()
        .await
        .map_err(|e| e.to_string())?;
    let installed = services().registry.installed();
    Ok(rows(runtimes, |id| {
        installed
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| id.to_string())
    }))
}

/// Queue removal of the given runtimes and wait for it.
pub async fn remove(refs: Vec<String>) -> Result<(), String> {
    if refs.is_empty() {
        return Ok(());
    }
    let title = kiosque_core::l10n::i18n("unused runtimes", &[]);
    use crate::view::ops::JobOutcome;
    match crate::view::ops::run(Operation::RemoveRuntimes { refs }, &title, |_, _, _| {}).await {
        JobOutcome::Done => Ok(()),
        JobOutcome::Failed(error) => Err(error),
        JobOutcome::Cancelled => Err("operation cancelled".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiosque_core::flatpak::{InstalledApp, PlannedStep, Scope};

    fn step(reference: &str, kind: RefKind, action: StepAction, size: u64) -> PlannedStep {
        PlannedStep {
            reference: reference.into(),
            kind,
            action,
            download_size: size,
            installed_size: size * 3,
            ..Default::default()
        }
    }

    const GNOME: &str = "runtime/org.gnome.Platform/x86_64/48";

    #[test]
    fn install_summary_names_the_runtime_and_whether_it_downloads() {
        let plan = OperationPlan {
            steps: vec![
                step(
                    "app/org.example.App/x86_64/stable",
                    RefKind::App,
                    StepAction::Install,
                    10_000_000,
                ),
                step(GNOME, RefKind::Runtime, StepAction::Install, 400_000_000),
            ],
        };
        let summary = install_summary(&plan, GNOME);
        assert_eq!(summary["runtime"], "GNOME Platform 48");
        assert_eq!(summary["runtimeInstalled"], false);
        assert_eq!(summary["download"], "410.0 MB");
        assert_eq!(summary["runtimes"][0]["name"], "GNOME Platform 48");

        // Already installed: the plan only contains the app.
        let plan = OperationPlan {
            steps: vec![step(
                "app/org.example.App/x86_64/stable",
                RefKind::App,
                StepAction::Install,
                1,
            )],
        };
        let summary = install_summary(&plan, GNOME);
        assert_eq!(summary["runtimeInstalled"], true);
        assert_eq!(summary["runtimes"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn removals_offer_the_runtimes_left_unused() {
        let plan = OperationPlan {
            steps: vec![
                step(
                    "app/org.example.App/x86_64/stable",
                    RefKind::App,
                    StepAction::Uninstall,
                    0,
                ),
                PlannedStep {
                    installed_size: 900_000_000,
                    ..step(GNOME, RefKind::Runtime, StepAction::Uninstall, 0)
                },
            ],
        };
        let leftovers = removal_leftovers(&plan);
        assert_eq!(leftovers.len(), 1);
        assert_eq!(leftovers[0]["reference"], GNOME);
        assert_eq!(leftovers[0]["size"], "900.0 MB");
    }

    #[test]
    fn rows_list_unused_first_with_app_names() {
        let runtime = |id: &str, used_by: &[&str], unused: bool| InstalledRuntime {
            info: InstalledApp {
                id: id.into(),
                arch: "x86_64".into(),
                branch: "48".into(),
                kind: RefKind::Runtime,
                scope: Scope::System,
                installed_size: 1_000_000,
                ..Default::default()
            },
            used_by: used_by.iter().map(|s| s.to_string()).collect(),
            unused,
            eol: String::new(),
        };
        let rows = rows(
            vec![
                runtime("org.gnome.Platform", &["org.example.App"], false),
                runtime("org.kde.Platform", &[], true),
            ],
            |id| {
                if id == "org.example.App" {
                    "Example".into()
                } else {
                    id.into()
                }
            },
        );
        assert_eq!(rows[0].name, "KDE Platform 48");
        assert!(rows[0].unused);
        assert_eq!(rows[1].used_by, "Example");
        assert_eq!(rows[1].scope, "system");
    }
}
