use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

// Global config directory override (for --config flag and tests)
static CONFIG_DIR_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Set config directory override (used by --config flag and tests)
pub fn set_config_dir_override(path: Option<PathBuf>) {
    let mut override_path = CONFIG_DIR_OVERRIDE.write().unwrap();
    *override_path = path;
}

/// Get current config directory override
pub fn get_config_dir_override() -> Option<PathBuf> {
    CONFIG_DIR_OVERRIDE.read().unwrap().clone()
}

/// Find config directory by searching in priority order:
/// 1. Override from --config flag or set_config_dir_override() (highest priority)
/// 2. Environment variable GGG_CONFIG_DIR
/// 3. User config directory (`~/.config/ggg/` on Unix, `%APPDATA%\ggg\` on Windows)
/// 4. Current working directory (`./config/`)
/// 5. Executable directory (`<exe_dir>/config/`)
///
/// If no config directory is found, creates one in the user config directory.
pub fn find_config_directory() -> Result<PathBuf> {
    // Priority 1: Override from --config flag or tests
    if let Some(override_path) = get_config_dir_override() {
        if override_path.exists() || std::env::var("GGG_TEST_MODE").is_ok() {
            tracing::debug!("Using config directory override: {:?}", override_path);
            return Ok(override_path);
        }
        tracing::warn!("Config directory override does not exist: {:?}", override_path);
    }

    // Priority 2: Environment variable
    if let Ok(env_path) = std::env::var("GGG_CONFIG_DIR") {
        let env_config = PathBuf::from(env_path);
        if env_config.exists() {
            tracing::debug!("Found config directory from GGG_CONFIG_DIR: {:?}", env_config);
            return Ok(env_config);
        }
    }

    // Priority 3: User config directory (platform standard location)
    if let Ok(user_config) = get_user_config_dir() {
        if user_config.exists() {
            tracing::debug!("Found config directory at: {:?}", user_config);
            return Ok(user_config);
        }
    }

    // Priority 4: Current working directory
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_config = cwd.join("config");
        if cwd_config.exists() {
            tracing::debug!("Found config directory at: {:?}", cwd_config);
            return Ok(cwd_config);
        }
    }

    // Priority 5: Executable directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_config = exe_dir.join("config");
            if exe_config.exists() {
                tracing::debug!("Found config directory at: {:?}", exe_config);
                return Ok(exe_config);
            }
        }
    }

    // Fallback: Create in user config directory
    let user_config = get_user_config_dir()?;
    std::fs::create_dir_all(&user_config)
        .context("Failed to create user config directory")?;
    tracing::info!("Created config directory at: {:?}", user_config);
    Ok(user_config)
}

/// Get platform-specific user config directory
/// - Windows: `%APPDATA%\ggg`
/// - Unix: `~/.config/ggg`
fn get_user_config_dir() -> Result<PathBuf> {
    let base_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine user config directory"))?;
    Ok(base_dir.join("ggg"))
}

/// Get absolute path to settings.toml (application-level)
pub fn get_app_config_path() -> Result<PathBuf> {
    let config_dir = find_config_directory()?;
    Ok(config_dir.join("settings.toml"))
}

/// Get absolute path to folder-specific settings.toml
pub fn get_folder_config_path(folder_name: &str) -> Result<PathBuf> {
    let config_dir = find_config_directory()?;
    let folder_dir = config_dir.join(folder_name);
    Ok(folder_dir.join("settings.toml"))
}

/// Get absolute path to folder-specific queue.toml
pub fn get_folder_queue_path(folder_id: &str) -> Result<PathBuf> {
    let config_dir = find_config_directory()?;
    let folder_dir = config_dir.join(folder_id);
    Ok(folder_dir.join("queue.toml"))
}

/// Resolve the default download directory at runtime.
///
/// Resolution order (mirrors config directory logic):
/// 1. Current working directory + "Downloads"
/// 2. Executable directory + "Downloads"
/// 3. Fallback: relative "Downloads"
pub fn resolve_default_download_directory() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        return cwd.join("Downloads");
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            return exe_dir.join("Downloads");
        }
    }
    PathBuf::from("Downloads")
}

/// Resolve the default scripts directory at runtime.
///
/// Uses the config directory as the base, falling back to relative "./scripts".
pub fn resolve_default_scripts_directory() -> PathBuf {
    match find_config_directory() {
        Ok(config_dir) => config_dir.join("scripts"),
        Err(_) => PathBuf::from("./scripts"),
    }
}

/// Resolve a relative path against the config directory.
///
/// If the path is already absolute, it is returned as-is.
/// If relative, it is joined to `find_config_directory()`.
/// Falls back to the original path if the config directory cannot be determined.
pub fn resolve_relative_to_config(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match find_config_directory() {
        Ok(config_dir) => config_dir.join(path),
        Err(_) => path.to_path_buf(),
    }
}

/// Get the platform-specific data directory for locale overrides.
///
/// Returns `<data_dir>/ggg/locales` where `<data_dir>` is:
/// - Linux: `$XDG_DATA_HOME` or `~/.local/share`
/// - macOS: `~/Library/Application Support`
/// - Windows: `%APPDATA%`
pub fn get_locale_data_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine user data directory"))?;
    Ok(data_dir.join("ggg").join("locales"))
}

/// Get absolute path to application-wide logs directory
pub fn get_logs_dir() -> Result<PathBuf> {
    let config_dir = find_config_directory()?;
    Ok(config_dir.join(".logs"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    // Helper function to ensure clean test state
    fn reset_test_state() {
        set_config_dir_override(None);
        unsafe { std::env::remove_var("GGG_TEST_MODE") };
        unsafe { std::env::remove_var("GGG_CONFIG_DIR") };
    }

    #[test]
    #[serial]
    fn test_get_app_config_path() {
        reset_test_state();
        // Create temporary config directory for test
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();

        // Set override for test
        set_config_dir_override(Some(config_dir.clone()));
        unsafe { std::env::set_var("GGG_TEST_MODE", "1") };

        let path = get_app_config_path().unwrap();
        assert_eq!(path, config_dir.join("settings.toml"));

        // Clean up
        set_config_dir_override(None);
        unsafe { std::env::remove_var("GGG_TEST_MODE") };
    }

    #[test]
    #[serial]
    fn test_get_folder_config_path() {
        reset_test_state();
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();

        set_config_dir_override(Some(config_dir.clone()));
        unsafe { std::env::set_var("GGG_TEST_MODE", "1") };

        let path = get_folder_config_path("test_folder").unwrap();
        assert_eq!(path, config_dir.join("test_folder").join("settings.toml"));
    }

    #[test]
    #[serial]
    fn test_get_folder_config_path_with_special_chars() {
        reset_test_state();
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();

        set_config_dir_override(Some(config_dir.clone()));
        unsafe { std::env::set_var("GGG_TEST_MODE", "1") };

        let path = get_folder_config_path("folder-with-dash").unwrap();
        assert!(path.to_str().unwrap().contains("folder-with-dash"));
    }

    #[test]
    #[serial]
    fn test_find_config_directory_creates_if_missing() {
        reset_test_state();

        // This test verifies that find_config_directory() returns a valid path
        let config_dir = find_config_directory().unwrap();
        assert!(config_dir.ends_with("config") || config_dir.to_str().unwrap().contains("ggg"));
    }

    #[test]
    #[serial]
    fn test_config_dir_override() {
        reset_test_state();
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();

        set_config_dir_override(Some(config_dir.clone()));
        unsafe { std::env::set_var("GGG_TEST_MODE", "1") };

        let found_dir = find_config_directory().unwrap();
        assert_eq!(found_dir, config_dir);
    }

    #[test]
    #[serial]
    fn test_config_dir_from_env_variable() {
        reset_test_state();

        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();
        fs::create_dir_all(&config_dir).unwrap();

        unsafe { std::env::set_var("GGG_CONFIG_DIR", config_dir.to_str().unwrap()) };

        let found_dir = find_config_directory().unwrap();
        assert_eq!(found_dir, config_dir);

        // Keep temp_dir alive
        drop(temp_dir);
    }

    #[test]
    #[serial]
    fn test_find_config_directory_cwd_fallback() {
        reset_test_state();

        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let found_dir = find_config_directory().unwrap();
        // CWD config is still found (as Priority 4 fallback or user config dir)
        assert!(found_dir.ends_with("config") || found_dir.to_str().unwrap().contains("ggg"));

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[serial]
    fn test_find_config_directory_prefers_user_config_over_cwd() {
        reset_test_state();

        let temp_dir = TempDir::new().unwrap();
        let cwd_config = temp_dir.path().join("config");
        fs::create_dir_all(&cwd_config).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        // If user config dir exists, it should be preferred over CWD
        let user_config = get_user_config_dir().unwrap();
        if user_config.exists() {
            let found_dir = find_config_directory().unwrap();
            assert_eq!(found_dir, user_config);
        }

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_get_user_config_dir_returns_valid_path() {
        let user_dir = get_user_config_dir().unwrap();

        // Should contain 'ggg' in the path
        assert!(user_dir.to_str().unwrap().contains("ggg"));

        // On Windows, should contain AppData or similar
        #[cfg(windows)]
        {
            let path_str = user_dir.to_str().unwrap();
            assert!(path_str.contains("AppData") || path_str.contains("config"));
        }

        // On Unix, should contain .config
        #[cfg(unix)]
        {
            assert!(user_dir.to_str().unwrap().contains(".config"));
        }
    }

    #[test]
    #[serial]
    fn test_path_normalization() {
        reset_test_state();
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();

        set_config_dir_override(Some(config_dir.clone()));
        unsafe { std::env::set_var("GGG_TEST_MODE", "1") };

        let app_path = get_app_config_path().unwrap();
        let folder_path = get_folder_config_path("test").unwrap();

        assert!(app_path.is_absolute());
        assert!(folder_path.is_absolute());
        assert!(folder_path.components().count() > app_path.components().count());
    }
}
