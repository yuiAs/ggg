use crate::app::keybindings::KeybindingsConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Application-level configuration (saved to config/settings.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationConfig {
    pub general: GeneralConfig,
    pub download: DownloadConfig,
    pub network: NetworkConfig,
    pub scripts: ScriptConfig,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

/// Complete configuration (Application settings + Folder settings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub download: DownloadConfig,
    pub network: NetworkConfig,
    pub scripts: ScriptConfig,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
    #[serde(default)]
    pub folders: HashMap<String, FolderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub language: String,
    pub theme: String,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    #[serde(default = "default_skip_download_preview")]
    pub skip_download_preview: bool,
    /// Auto-launch ggg-dnd GUI on startup (Windows only)
    #[serde(default)]
    pub auto_launch_dnd: bool,
}

fn default_skip_download_preview() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub default_directory: PathBuf,
    pub max_concurrent: usize,
    pub retry_count: u32,
    pub retry_delay: u64,
    pub user_agent: String,
    pub bandwidth_limit: u64,
    #[serde(default)]
    pub max_concurrent_per_folder: Option<usize>,
    #[serde(default)]
    pub parallel_folder_count: Option<usize>,
    #[serde(default = "default_max_redirects")]
    pub max_redirects: u32,
}

fn default_max_redirects() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub proxy_enabled: bool,
    pub proxy_type: String,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub proxy_auth: bool,
    pub proxy_user: String,
    pub proxy_pass: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptConfig {
    pub enabled: bool,
    pub directory: PathBuf,
    pub timeout: u64,
    /// Per-script file enable/disable settings
    /// Maps filename (without path) to enabled status
    #[serde(default)]
    pub script_files: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderConfig {
    pub save_path: PathBuf,
    #[serde(default)]
    pub auto_date_directory: bool,
    #[serde(default)]
    pub auto_start_downloads: bool,
    #[serde(default)]
    pub scripts_enabled: Option<bool>,
    #[serde(default)]
    pub script_files: Option<HashMap<String, bool>>,
    #[serde(default)]
    pub max_concurrent: Option<usize>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub default_headers: HashMap<String, String>,
}

impl Default for FolderConfig {
    fn default() -> Self {
        Self {
            save_path: crate::util::paths::resolve_default_download_directory(),
            auto_date_directory: false,
            auto_start_downloads: false,
            scripts_enabled: None,
            script_files: None,
            max_concurrent: None,
            user_agent: None,
            default_headers: HashMap::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                language: "en".to_string(),
                theme: "classic".to_string(),
                minimize_to_tray: true,
                start_minimized: false,
                skip_download_preview: true,
                auto_launch_dnd: false,
            },
            download: DownloadConfig {
                default_directory: crate::util::paths::resolve_default_download_directory(),
                max_concurrent: 3,
                retry_count: 3,
                retry_delay: 5,
                user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
                bandwidth_limit: 0,
                max_concurrent_per_folder: None,
                parallel_folder_count: None,
                max_redirects: 5,
            },
            network: NetworkConfig {
                proxy_enabled: false,
                proxy_type: "http".to_string(),
                proxy_host: String::new(),
                proxy_port: 8080,
                proxy_auth: false,
                proxy_user: String::new(),
                proxy_pass: String::new(),
            },
            scripts: ScriptConfig {
                enabled: true,
                directory: crate::util::paths::resolve_default_scripts_directory(),
                timeout: 30,
                script_files: HashMap::new(),
            },
            keybindings: KeybindingsConfig::default(),
            folders: HashMap::new(),
        }
    }
}

impl Config {
    /// Load configuration from multi-file structure
    pub fn load() -> anyhow::Result<Self> {
        // Step 1: Load application-level config
        let app_config = Self::load_application_config()?;

        // Step 2: Load all folder configs
        let mut folders = Self::load_all_folder_configs()?;

        // Step 3: Ensure "default" folder exists
        if !folders.contains_key("default") {
            tracing::info!("Creating default folder config");
            folders.insert(
                "default".to_string(),
                FolderConfig {
                    save_path: app_config.download.default_directory.clone(),
                    auto_date_directory: false,
                    auto_start_downloads: false,
                    scripts_enabled: None,
                    script_files: None,
                    max_concurrent: None,
                    user_agent: None,
                    default_headers: HashMap::new(),
                },
            );
        }

        // Step 4: Construct Config
        let config = Self {
            general: app_config.general,
            download: app_config.download,
            network: app_config.network,
            scripts: app_config.scripts,
            keybindings: app_config.keybindings,
            folders,
        };

        // Step 5: Validate
        if let Err(errors) = crate::app::settings::validate_folder_config(&config) {
            return Err(anyhow::anyhow!(
                "Invalid configuration: {}",
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        Ok(config)
    }

    /// Save configuration to multi-file structure
    pub fn save(&self) -> anyhow::Result<()> {
        // Step 1: Validate before saving
        if let Err(errors) = crate::app::settings::validate_folder_config(self) {
            return Err(anyhow::anyhow!(
                "Cannot save invalid config: {}",
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Step 2: Save application-level config
        Self::save_application_config(self)?;

        // Step 3: Save each folder config
        for (folder_name, folder_config) in &self.folders {
            Self::save_folder_config(folder_name, folder_config)?;
        }

        Ok(())
    }

    // --- Helper Methods ---

    fn load_application_config() -> anyhow::Result<ApplicationConfig> {
        use anyhow::Context;

        let config_path = crate::util::paths::get_app_config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .context(format!("Failed to read {:?}", config_path))?;
            let mut app_config: ApplicationConfig = toml::from_str(&content)
                .context(format!("Failed to parse {:?}", config_path))?;

            // Resolve relative scripts directory against config directory
            app_config.scripts.directory =
                crate::util::paths::resolve_relative_to_config(&app_config.scripts.directory);

            Ok(app_config)
        } else {
            tracing::info!("Application config not found, using defaults");
            Ok(ApplicationConfig {
                general: GeneralConfig {
                    language: "en".to_string(),
                    theme: "classic".to_string(),
                    minimize_to_tray: true,
                    start_minimized: false,
                    skip_download_preview: true,
                    auto_launch_dnd: false,
                },
                download: DownloadConfig {
                    default_directory: crate::util::paths::resolve_default_download_directory(),
                    max_concurrent: 3,
                    retry_count: 3,
                    retry_delay: 5,
                    user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
                    bandwidth_limit: 0,
                    max_concurrent_per_folder: None,
                    parallel_folder_count: None,
                    max_redirects: 5,
                },
                network: NetworkConfig {
                    proxy_enabled: false,
                    proxy_type: "http".to_string(),
                    proxy_host: String::new(),
                    proxy_port: 8080,
                    proxy_auth: false,
                    proxy_user: String::new(),
                    proxy_pass: String::new(),
                },
                scripts: ScriptConfig {
                    enabled: true,
                    directory: crate::util::paths::resolve_default_scripts_directory(),
                    timeout: 30,
                    script_files: HashMap::new(),
                },
                keybindings: KeybindingsConfig::default(),
            })
        }
    }

    fn save_application_config(&self) -> anyhow::Result<()> {
        use anyhow::Context;

        let config_path = crate::util::paths::get_app_config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let app_config = ApplicationConfig {
            general: self.general.clone(),
            download: self.download.clone(),
            network: self.network.clone(),
            scripts: self.scripts.clone(),
            keybindings: self.keybindings.clone(),
        };

        let content = toml::to_string_pretty(&app_config)?;

        // Atomic write using temp file + rename
        let temp_path = config_path.with_extension("toml.tmp");
        std::fs::write(&temp_path, &content)
            .context("Failed to write temp config file")?;
        std::fs::rename(&temp_path, &config_path)
            .context("Failed to rename temp config file")?;

        tracing::info!("Saved application config to {:?}", config_path);
        Ok(())
    }

    fn load_all_folder_configs() -> anyhow::Result<HashMap<String, FolderConfig>> {
        let config_dir = crate::util::paths::find_config_directory()?;
        let mut folders = HashMap::new();

        // Scan for subdirectories in config/
        if !config_dir.exists() {
            return Ok(folders);
        }

        for entry in std::fs::read_dir(&config_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Only treat directories containing settings.toml as folders
                if !path.join("settings.toml").exists() {
                    continue;
                }

                let folder_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid folder name"))?
                    .to_string();

                match Self::load_folder_config(&folder_name) {
                    Ok(folder_config) => {
                        folders.insert(folder_name.clone(), folder_config);
                        tracing::debug!("Loaded folder config: {}", folder_name);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load folder '{}': {}", folder_name, e);
                        // Continue loading other folders
                    }
                }
            }
        }

        Ok(folders)
    }

    fn load_folder_config(folder_name: &str) -> anyhow::Result<FolderConfig> {
        use anyhow::Context;

        let folder_path = crate::util::paths::get_folder_config_path(folder_name)?;

        if folder_path.exists() {
            let content = std::fs::read_to_string(&folder_path)?;
            toml::from_str(&content)
                .context(format!("Failed to parse folder config: {}", folder_name))
        } else {
            // No config file - use defaults
            Ok(FolderConfig::default())
        }
    }

    fn save_folder_config(
        folder_name: &str,
        folder_config: &FolderConfig,
    ) -> anyhow::Result<()> {
        use anyhow::Context;

        let folder_path = crate::util::paths::get_folder_config_path(folder_name)?;

        // Ensure parent directory exists
        if let Some(parent) = folder_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(folder_config)?;

        // Atomic write
        let temp_path = folder_path.with_extension("toml.tmp");
        std::fs::write(&temp_path, &content)
            .context("Failed to write temp folder config")?;
        std::fs::rename(&temp_path, &folder_path)
            .context("Failed to rename temp folder config")?;

        tracing::debug!("Saved folder config: {}", folder_name);
        Ok(())
    }

    #[cfg(test)]
    fn load_from(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    #[cfg(test)]
    fn save_to(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serial_test::serial;

    fn create_test_config_toml() -> &'static str {
        r#"
[general]
language = "ja"
theme = "classic"
minimize_to_tray = false
start_minimized = true
skip_download_preview = false

[download]
default_directory = "D:\\MyDownloads"
max_concurrent = 5
retry_count = 10
retry_delay = 3
user_agent = "CustomAgent/1.0"
bandwidth_limit = 1000000
max_concurrent_per_folder = 3
parallel_folder_count = 2
max_redirects = 10

[network]
proxy_enabled = true
proxy_type = "socks5"
proxy_host = "localhost"
proxy_port = 1080
proxy_auth = true
proxy_user = "testuser"
proxy_pass = "testpass"

[scripts]
enabled = false
directory = "./custom_scripts"
timeout = 60
"#
    }

    #[test]
    #[serial]
    fn test_config_default_values() {
        let config = Config::default();

        assert_eq!(config.general.language, "en");
        assert_eq!(config.general.theme, "classic");
        assert_eq!(config.general.minimize_to_tray, true);
        assert_eq!(config.general.start_minimized, false);

        assert_eq!(config.download.default_directory, crate::util::paths::resolve_default_download_directory());
        assert_eq!(config.download.max_concurrent, 3);
        assert_eq!(config.download.retry_count, 3);
        assert_eq!(config.download.retry_delay, 5);
        assert_eq!(config.download.bandwidth_limit, 0);

        assert_eq!(config.network.proxy_enabled, false);
        assert_eq!(config.network.proxy_type, "http");
        assert_eq!(config.network.proxy_port, 8080);

        assert_eq!(config.scripts.enabled, true);
        assert_eq!(config.scripts.directory, crate::util::paths::resolve_default_scripts_directory());
        assert_eq!(config.scripts.timeout, 30);
    }

    #[test]
    fn test_config_load_missing_file_uses_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("nonexistent.toml");

        // Load from non-existent path should return defaults
        let config = Config::load_from(&config_path).unwrap();

        // Should use defaults
        assert_eq!(config.general.language, "en");
        assert_eq!(config.download.max_concurrent, 3);
    }

    #[test]
    fn test_config_load_valid_toml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        std::fs::write(&config_path, create_test_config_toml()).unwrap();

        let config = Config::load_from(&config_path).unwrap();

        assert_eq!(config.general.language, "ja");
        assert_eq!(config.general.theme, "classic");
        assert_eq!(config.general.minimize_to_tray, false);
        assert_eq!(config.general.start_minimized, true);
        assert_eq!(config.general.skip_download_preview, false);

        assert_eq!(config.download.default_directory, PathBuf::from("D:\\MyDownloads"));
        assert_eq!(config.download.max_concurrent, 5);
        assert_eq!(config.download.retry_count, 10);
        assert_eq!(config.download.retry_delay, 3);
        assert_eq!(config.download.max_redirects, 10);
        assert_eq!(config.download.max_concurrent_per_folder, Some(3));
        assert_eq!(config.download.parallel_folder_count, Some(2));

        assert_eq!(config.network.proxy_enabled, true);
        assert_eq!(config.network.proxy_type, "socks5");
        assert_eq!(config.network.proxy_host, "localhost");
        assert_eq!(config.network.proxy_port, 1080);
        assert_eq!(config.network.proxy_auth, true);
        assert_eq!(config.network.proxy_user, "testuser");
        assert_eq!(config.network.proxy_pass, "testpass");

        assert_eq!(config.scripts.enabled, false);
        assert_eq!(config.scripts.timeout, 60);
        assert_eq!(config.scripts.directory, PathBuf::from("./custom_scripts"));
    }

    #[test]
    fn test_config_load_invalid_toml_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // Write invalid TOML
        std::fs::write(&config_path, "this is not valid toml [[[").unwrap();

        let result = Config::load_from(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_save_creates_toml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config = Config::default();
        config.save_to(&config_path).unwrap();

        assert!(config_path.exists());
    }

    #[test]
    fn test_config_save_load_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut config = Config::default();
        config.general.language = "en".to_string();
        config.download.max_concurrent = 7;
        config.network.proxy_enabled = true;

        config.save_to(&config_path).unwrap();

        let loaded_config = Config::load_from(&config_path).unwrap();

        assert_eq!(loaded_config.general.language, "en");
        assert_eq!(loaded_config.download.max_concurrent, 7);
        assert_eq!(loaded_config.network.proxy_enabled, true);
    }

    #[test]
    fn test_config_all_sections_present() {
        let config = Config::default();

        // Verify all sections exist and have expected structure
        let _ = config.general.language;
        let _ = config.download.default_directory;
        let _ = config.network.proxy_enabled;
        let _ = config.scripts.enabled;
    }

    #[test]
    fn test_config_custom_values_preserved() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut config = Config::default();
        config.general.theme = "modern".to_string();
        config.download.retry_count = 99;
        config.network.proxy_port = 9999;
        config.scripts.directory = PathBuf::from("/custom/path");

        config.save_to(&config_path).unwrap();

        let loaded = Config::load_from(&config_path).unwrap();

        assert_eq!(loaded.general.theme, "modern");
        assert_eq!(loaded.download.retry_count, 99);
        assert_eq!(loaded.network.proxy_port, 9999);
        assert_eq!(loaded.scripts.directory, PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_application_config_serialization() {
        let app_config = ApplicationConfig {
            general: GeneralConfig {
                language: "en".to_string(),
                theme: "dark".to_string(),
                minimize_to_tray: false,
                start_minimized: true,
                skip_download_preview: true,
                auto_launch_dnd: false,
            },
            download: DownloadConfig {
                default_directory: PathBuf::from("C:\\Downloads"),
                max_concurrent: 5,
                retry_count: 3,
                retry_delay: 5,
                user_agent: "TestAgent".to_string(),
                bandwidth_limit: 0,
                max_concurrent_per_folder: Some(2),
                parallel_folder_count: Some(2),
                max_redirects: 10,
            },
            network: NetworkConfig {
                proxy_enabled: false,
                proxy_type: "http".to_string(),
                proxy_host: String::new(),
                proxy_port: 8080,
                proxy_auth: false,
                proxy_user: String::new(),
                proxy_pass: String::new(),
            },
            scripts: ScriptConfig {
                enabled: true,
                directory: PathBuf::from("./scripts"),
                timeout: 30,
                script_files: HashMap::new(),
            },
            keybindings: KeybindingsConfig::default(),
        };

        // Should serialize and deserialize correctly
        let serialized = toml::to_string_pretty(&app_config).unwrap();
        let deserialized: ApplicationConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.general.language, "en");
        assert_eq!(deserialized.download.max_concurrent, 5);
        assert_eq!(deserialized.download.max_redirects, 10);
    }

    #[test]
    fn test_folder_config_with_overrides() {
        let mut folder_config = FolderConfig::default();
        folder_config.max_concurrent = Some(5);
        folder_config.scripts_enabled = Some(false);
        folder_config.user_agent = Some("CustomAgent".to_string());

        let serialized = toml::to_string_pretty(&folder_config).unwrap();
        let deserialized: FolderConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.max_concurrent, Some(5));
        assert_eq!(deserialized.scripts_enabled, Some(false));
        assert_eq!(deserialized.user_agent, Some("CustomAgent".to_string()));
    }

    #[test]
    fn test_folder_config_inherits_when_none() {
        let folder_config = FolderConfig {
            save_path: PathBuf::from("C:\\Test"),
            auto_date_directory: true,
            auto_start_downloads: false,
            scripts_enabled: None, // Should inherit from app
            script_files: None,     // Should inherit from app
            max_concurrent: None,   // Should inherit from app
            user_agent: None,       // Should inherit from app
            default_headers: HashMap::new(),
        };

        let serialized = toml::to_string_pretty(&folder_config).unwrap();
        let deserialized: FolderConfig = toml::from_str(&serialized).unwrap();

        // None values should remain None (inheritance handled at runtime)
        assert_eq!(deserialized.scripts_enabled, None);
        assert_eq!(deserialized.max_concurrent, None);
        assert_eq!(deserialized.user_agent, None);
    }

    #[test]
    fn test_default_folder_creation() {
        let config = Config::default();

        // Default config should have no folders initially
        assert_eq!(config.folders.len(), 0);
    }

    #[test]
    fn test_config_validation_integration() {
        use crate::app::settings::validate_folder_config;

        let mut config = Config::default();
        config.download.max_concurrent = 10;
        config.download.max_concurrent_per_folder = Some(5);
        config.download.parallel_folder_count = Some(2);

        // Valid configuration
        let result = validate_folder_config(&config);
        assert!(result.is_ok());

        // Invalid: per_folder * parallel > global
        config.download.max_concurrent_per_folder = Some(6);
        let result = validate_folder_config(&config);
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_load_all_folder_configs_requires_settings_toml() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().to_path_buf();
        std::fs::create_dir_all(&config_dir).unwrap();

        // Use config dir override instead of CWD change
        crate::util::paths::set_config_dir_override(Some(config_dir.clone()));
        unsafe { std::env::set_var("GGG_TEST_MODE", "1") };

        // Folders WITH settings.toml (should be recognized)
        std::fs::create_dir_all(config_dir.join("folder1")).unwrap();
        std::fs::write(
            config_dir.join("folder1").join("settings.toml"),
            r#"
save_path = "C:\\Folder1"
auto_date_directory = false
auto_start_downloads = false
"#,
        )
        .unwrap();

        std::fs::create_dir_all(config_dir.join("folder2")).unwrap();
        std::fs::write(
            config_dir.join("folder2").join("settings.toml"),
            r#"
save_path = "C:\\Folder2"
auto_date_directory = false
auto_start_downloads = false
"#,
        )
        .unwrap();

        // Directories WITHOUT settings.toml (should be ignored)
        std::fs::create_dir_all(config_dir.join(".logs")).unwrap();
        std::fs::create_dir_all(config_dir.join("scripts")).unwrap();
        std::fs::create_dir_all(config_dir.join("empty_dir")).unwrap();

        // Load folder configs
        let folders = Config::load_all_folder_configs().unwrap();

        // Clean up
        crate::util::paths::set_config_dir_override(None);
        unsafe { std::env::remove_var("GGG_TEST_MODE") };

        // Only directories with settings.toml should be loaded
        assert_eq!(folders.len(), 2);
        assert!(folders.contains_key("folder1"));
        assert!(folders.contains_key("folder2"));
        assert!(!folders.contains_key(".logs"));
        assert!(!folders.contains_key("scripts"));
        assert!(!folders.contains_key("empty_dir"));
    }
}
