use tokio::process::Command;

/// Represents a single Flatpak app from `flatpak list --json`.
/// The JSON output schema varies across flatpak versions, so every field
/// except the ID and name is optional, and we accept common aliases.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct FlatpakJsonApp {
    /// The reverse-DNS application ID. Some flatpak versions output
    /// `"application"` instead of `"application_id"`.
    #[serde(alias = "application")]
    pub application_id: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub branch: Option<String>,

    #[serde(default)]
    pub origin: Option<String>,

    #[serde(default)]
    pub installation: Option<String>,

    #[serde(default)]
    pub arch: Option<String>,

    /// Size string like "565.4 MB". Some flatpak versions call this
    /// `"installed-size"` (with a hyphen) or `"installed_size"`.
    #[serde(default, alias = "installed-size")]
    pub installed_size: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct Remote {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub description: String,
}

/// List all installed Flatpak applications as structured JSON.
pub async fn list_installed() -> Result<String, String> {
    let output = Command::new("flatpak")
        .env("LC_ALL", "C")
        .args([
            "list", "--app", "-j", "--columns=application,name,version,branch,origin,installation,arch,size",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to execute `flatpak list`: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("flatpak list exited with {}: {}", output.status, stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// List installed apps, returning parsed structs. Uses the in-memory cache
/// when the data is still fresh (within the installed TTL).
pub async fn list_installed_cached() -> Result<Vec<FlatpakJsonApp>, String> {
    let cache = crate::cache::app_cache();

    // Try cache first
    if let Some(cached) = cache.get_installed_list().await {
        return Ok(cached);
    }

    // Cache miss — call flatpak CLI
    eprintln!("[kiosque] CACHE MISS installed_list");
    let stdout = list_installed().await?;

    if stdout.trim().is_empty() {
        eprintln!("[kiosque] list_installed_cached: flatpak list returned empty output");
        let empty = vec![];
        cache.put_installed_list(empty.clone()).await;
        return Ok(empty);
    }

    match serde_json::from_str::<Vec<FlatpakJsonApp>>(&stdout) {
        Ok(parsed) => {
            eprintln!("[kiosque] list_installed_cached: parsed {} installed apps", parsed.len());
            cache.put_installed_list(parsed.clone()).await;
            Ok(parsed)
        }
        Err(e) => {
            eprintln!("[kiosque] ERROR list_installed_cached: JSON parse failed: {}", e);
            eprintln!("[kiosque]   raw output (first 500 bytes): {}", &stdout[..stdout.len().min(500)]);
            Err(format!("JSON parse failed: {}", e))
        }
    }
}

/// Check whether a specific app ID is installed, using the cached installed
/// set when available. Falls back to a `flatpak info` subprocess on cache miss.
pub async fn is_installed_cached(app_id: &str) -> bool {
    let cache = crate::cache::app_cache();

    // Try the cached set first
    if let Some(result) = cache.is_installed(app_id).await {
        return result;
    }

    // Cache miss — try to populate the cache by fetching the full list
    eprintln!("[kiosque] CACHE MISS is_installed(\"{}\"), populating installed cache", app_id);
    if let Ok(apps) = list_installed_cached().await {
        // The list_installed_cached call already populated the cache,
        // so check again
        if let Some(result) = cache.is_installed(app_id).await {
            return result;
        }
        // Fallback: check the list directly
        return apps.iter().any(|a| a.application_id == app_id);
    }

    // Final fallback — use the CLI directly
    is_installed(app_id).await
}

/// Check whether a specific app ID is installed.
pub async fn is_installed(app_id: &str) -> bool {
    let result = Command::new("flatpak")
        .args(["info", app_id])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    match result {
        Ok(s) => s.success(),
        Err(_) => false,
    }
}

/// Install a Flatpak app by ID from the flathub remote.
/// Invalidates the installed cache on success.
pub async fn install_app(app_id: &str) -> Result<(), String> {
    let output = Command::new("flatpak")
        .args(["install", "--assumeyes", "--noninteractive", "flathub", app_id])
        .output()
        .await
        .map_err(|e| format!("Failed to execute `flatpak install`: {}", e))?;

    if output.status.success() {
        // Invalidate installed cache so the next query reflects the change
        crate::cache::app_cache().invalidate_installed().await;
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("flatpak install failed: {}", stderr))
    }
}

/// Uninstall a Flatpak app by ID.
/// Invalidates the installed cache on success.
pub async fn uninstall_app(app_id: &str) -> Result<(), String> {
    let output = Command::new("flatpak")
        .args(["uninstall", "--assumeyes", "--noninteractive", app_id])
        .output()
        .await
        .map_err(|e| format!("Failed to execute `flatpak uninstall`: {}", e))?;

    if output.status.success() {
        // Invalidate installed cache so the next query reflects the change
        crate::cache::app_cache().invalidate_installed().await;
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("flatpak uninstall failed: {}", stderr))
    }
}

/// Launch a Flatpak app in the background.
pub async fn launch_app(app_id: &str) -> Result<(), String> {
    Command::new("flatpak")
        .args(["run", app_id])
        .spawn()
        .map_err(|e| format!("Failed to launch {}: {}", app_id, e))?;
    Ok(())
}

/// Add a Flatpak remote repository (user-scoped).
pub async fn add_repository(name: &str, url: &str) -> Result<(), String> {
    let output = Command::new("flatpak")
        .args(["remote-add", "--if-not-exists", "--user", name, url])
        .output()
        .await
        .map_err(|e| format!("Failed to execute `flatpak remote-add`: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("flatpak remote-add failed: {}", stderr))
    }
}

/// Remove a Flatpak remote repository.
pub async fn remove_repository(name: &str) -> Result<(), String> {
    let output = Command::new("flatpak")
        .args(["remote-delete", "--user", "--force", name])
        .output()
        .await
        .map_err(|e| format!("Failed to execute `flatpak remote-delete`: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let output2 = Command::new("flatpak")
            .args(["remote-delete", "--system", "--force", name])
            .output()
            .await
            .map_err(|e| format!("Failed to execute `flatpak remote-delete --system`: {}", e))?;
        if output2.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("flatpak remote-delete failed: {}", stderr))
        }
    }
}

/// List all configured Flatpak remotes.
pub async fn list_remotes() -> Result<Vec<Remote>, String> {
    let output = Command::new("flatpak")
        .args(["remotes", "-j", "-d"])
        .output()
        .await
        .map_err(|e| format!("Failed to execute `flatpak remotes`: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("flatpak remotes failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    match serde_json::from_str::<Vec<Remote>>(&stdout) {
        Ok(parsed) => Ok(parsed),
        Err(e) => Err(format!("Failed to parse remotes JSON: {}", e)),
    }
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct UpdateEntry {
    #[serde(alias = "application")]
    pub application_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default, alias = "installed-size")]
    pub installed_size: Option<String>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default, rename = "ref")]
    pub ref_string: Option<String>,
}

/// List all Application IDs and Runtimes that have updates available.
pub async fn list_updates() -> Result<Vec<UpdateEntry>, String> {
    let output = Command::new("flatpak")
        .env("LC_ALL", "C")
        .args(["remote-ls", "--updates", "--columns=application,name,version,origin,installed-size,ref", "-j"])
        .output()
        .await
        .map_err(|e| format!("Failed to execute `flatpak remote-ls --updates`: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("flatpak remote-ls exited with {}: {}", output.status, stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    match serde_json::from_str::<Vec<UpdateEntry>>(&stdout) {
        Ok(parsed) => Ok(parsed),
        Err(e) => {
            if stdout.trim() == "[]" {
                Ok(vec![])
            } else {
                Err(format!("Failed to parse updates JSON: {}", e))
            }
        }
    }
}

/// Update specific Flatpak apps by their Application IDs, providing status updates line-by-line.
/// Invalidates the installed cache on success.
pub async fn update_apps<F>(app_ids: &[String], mut on_status: F) -> Result<(), String>
where
    F: FnMut(String) + Send + 'static,
{
    if app_ids.is_empty() {
        return Ok(());
    }

    let mut args = vec!["update", "--assumeyes", "--noninteractive"];
    let mut app_id_refs = Vec::new();
    for id in app_ids {
        app_id_refs.push(id.as_str());
    }
    args.extend(app_id_refs);

    let mut child = Command::new("flatpak")
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute `flatpak update`: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to open stdout of flatpak update")?;
    
    use tokio::io::AsyncBufReadExt;
    let mut reader = tokio::io::BufReader::new(stdout).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            on_status(trimmed.to_string());
        }
    }

    let status = child.wait().await
        .map_err(|e| format!("Failed to wait for flatpak update: {}", e))?;

    if status.success() {
        crate::cache::app_cache().invalidate_installed().await;
        Ok(())
    } else {
        let mut stderr_str = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut stderr_str).await;
        }
        Err(format!("flatpak update failed: {}", stderr_str))
    }
}


