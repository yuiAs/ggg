//! `ggg bridge install / uninstall / status` — manages the Chrome Native
//! Messaging host registration that lets the browser extension talk to ggg
//! through `ggg-bridge.exe`.
//!
//! What gets installed:
//!   1. `%LOCALAPPDATA%\ggg\com.ggg.bridge.json` — the host manifest.
//!   2. A registry value at
//!      `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.ggg.bridge`
//!      whose default value is the absolute path to that JSON file.
//!
//! Chromium-derived browsers (Edge, Brave) read the same manifest format
//! from their own registry hives — extending this to those is a future
//! follow-up; for now we register under Chrome only.

use super::error;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

const HOST_NAME: &str = "com.ggg.bridge";
const MANIFEST_FILENAME: &str = "com.ggg.bridge.json";
const REGISTRY_KEY: &str = r"Software\Google\Chrome\NativeMessagingHosts\com.ggg.bridge";

#[derive(Debug, Serialize)]
struct HostManifest {
    name: String,
    description: String,
    path: String,
    #[serde(rename = "type")]
    kind: &'static str,
    allowed_origins: Vec<String>,
}

#[cfg(windows)]
pub async fn handle_install(
    extension_ids: Vec<String>,
    bridge_path: Option<PathBuf>,
) -> Result<i32> {
    let bridge = resolve_bridge_path(bridge_path)?;
    if !bridge.exists() {
        return Err(anyhow!(
            "ggg-bridge.exe not found at {}\nBuild it first: cargo build --release -p ggg-bridge",
            bridge.display()
        ));
    }

    if extension_ids.is_empty() {
        // Refuse rather than installing a wide-open manifest. Without an
        // ID the host would accept messages from any extension, which
        // defeats the point of using Native Messaging over localhost HTTP.
        return Err(anyhow!(
            "At least one --extension-id is required.\n\
             Load the unpacked extension first, copy the ID from chrome://extensions, then re-run."
        ));
    }

    let allowed_origins: Vec<String> = extension_ids
        .iter()
        .map(|id| format!("chrome-extension://{}/", id))
        .collect();

    let manifest = HostManifest {
        name: HOST_NAME.to_string(),
        description: "ggg URL bridge".to_string(),
        path: bridge.to_string_lossy().into_owned(),
        kind: "stdio",
        allowed_origins,
    };

    let manifest_path = manifest_dir()?.join(MANIFEST_FILENAME);
    write_manifest(&manifest_path, &manifest)?;
    write_registry(&manifest_path)?;

    println!("Installed Chrome Native Messaging host:");
    println!("  manifest:  {}", manifest_path.display());
    println!("  bridge:    {}", manifest.path);
    println!("  registry:  HKCU\\{}", REGISTRY_KEY);
    for origin in &manifest.allowed_origins {
        println!("  origin:    {}", origin);
    }
    Ok(error::SUCCESS)
}

#[cfg(windows)]
pub async fn handle_uninstall() -> Result<i32> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.delete_subkey_all(REGISTRY_KEY) {
        Ok(()) => println!("Deleted registry key HKCU\\{}", REGISTRY_KEY),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("Registry key already absent: HKCU\\{}", REGISTRY_KEY);
        }
        Err(e) => return Err(anyhow!("Failed to delete registry key: {}", e)),
    }

    let manifest_path = manifest_dir()?.join(MANIFEST_FILENAME);
    if manifest_path.exists() {
        std::fs::remove_file(&manifest_path)
            .with_context(|| format!("Failed to delete {}", manifest_path.display()))?;
        println!("Deleted manifest {}", manifest_path.display());
    } else {
        println!("Manifest already absent: {}", manifest_path.display());
    }

    Ok(error::SUCCESS)
}

#[cfg(windows)]
pub async fn handle_status() -> Result<i32> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let manifest_path = manifest_dir()?.join(MANIFEST_FILENAME);
    println!("Manifest path: {}", manifest_path.display());
    if manifest_path.exists() {
        match std::fs::read_to_string(&manifest_path) {
            Ok(contents) => println!("Manifest contents:\n{}", contents),
            Err(e) => println!("  (failed to read: {})", e),
        }
    } else {
        println!("  (not installed)");
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey(REGISTRY_KEY) {
        Ok(key) => {
            let value: Result<String, _> = key.get_value("");
            match value {
                Ok(v) => println!("Registry HKCU\\{} = {}", REGISTRY_KEY, v),
                Err(e) => println!("Registry key exists but has no default value: {}", e),
            }
        }
        Err(_) => println!("Registry HKCU\\{} = (not installed)", REGISTRY_KEY),
    }

    Ok(error::SUCCESS)
}

#[cfg(not(windows))]
pub async fn handle_install(
    _extension_ids: Vec<String>,
    _bridge_path: Option<PathBuf>,
) -> Result<i32> {
    Err(anyhow!("ggg bridge is only supported on Windows"))
}

#[cfg(not(windows))]
pub async fn handle_uninstall() -> Result<i32> {
    Err(anyhow!("ggg bridge is only supported on Windows"))
}

#[cfg(not(windows))]
pub async fn handle_status() -> Result<i32> {
    Err(anyhow!("ggg bridge is only supported on Windows"))
}

#[cfg(windows)]
fn manifest_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("Could not resolve %LOCALAPPDATA%"))?;
    let dir = base.join("ggg");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create {}", dir.display()))?;
    Ok(dir)
}

#[cfg(not(windows))]
fn manifest_dir() -> Result<PathBuf> {
    Err(anyhow!("ggg bridge is only supported on Windows"))
}

/// Resolve where `ggg-bridge.exe` should live. If the user passed an explicit
/// path, use it; otherwise look next to the running ggg.exe.
fn resolve_bridge_path(override_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return Ok(p);
    }
    let exe = std::env::current_exe().context("Could not locate ggg.exe")?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("ggg.exe has no parent directory"))?;
    Ok(dir.join("ggg-bridge.exe"))
}

#[cfg(windows)]
fn write_manifest(path: &Path, manifest: &HostManifest) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)
        .context("Failed to serialize host manifest")?;
    std::fs::write(path, json)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

#[cfg(windows)]
fn write_registry(manifest_path: &Path) -> Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _disp) = hkcu
        .create_subkey_with_flags(REGISTRY_KEY, KEY_WRITE)
        .with_context(|| format!("Failed to create registry key HKCU\\{}", REGISTRY_KEY))?;
    key.set_value("", &manifest_path.to_string_lossy().to_string())
        .context("Failed to set registry default value")?;
    Ok(())
}
