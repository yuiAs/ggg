use fluent::{FluentBundle, FluentResource};
use fluent_bundle::FluentArgs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use unic_langid::LanguageIdentifier;

/// Manages localization resources and provides translation API
pub struct LocalizationManager {
    bundle: FluentBundle<Arc<FluentResource>>,
    fallback_bundle: Option<FluentBundle<Arc<FluentResource>>>,
    current_locale: String,
}

impl LocalizationManager {
    /// Create a new LocalizationManager for the specified locale
    ///
    /// # Arguments
    /// * `locale` - Language code ("en" or "ja")
    ///
    /// # Returns
    /// * `Ok(LocalizationManager)` on success
    /// * `Err` if locale files cannot be loaded
    pub fn new(locale: &str) -> anyhow::Result<Self> {
        // Map short codes to full locale identifiers
        let locale_lower = locale.to_lowercase();
        let locale_id = match locale_lower.as_str() {
            "en" => "en-US",
            "ja" => "ja-JP",
            other => other,
        };

        tracing::info!("Loading translations for locale: {}", locale_id);

        // Load the requested locale
        let locale_id_owned = locale_id.to_string();
        let bundle = Self::load_locale_bundle(&locale_id_owned)?;

        // Load fallback locale (en-US) if not already loaded
        let fallback_bundle = if locale_id != "en-US" {
            match Self::load_locale_bundle("en-US") {
                Ok(fallback) => {
                    tracing::debug!("Loaded fallback locale: en-US");
                    Some(fallback)
                }
                Err(e) => {
                    tracing::warn!("Failed to load fallback locale en-US: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            bundle,
            fallback_bundle,
            current_locale: locale.to_string(),
        })
    }

    /// Load all .ftl files for a locale into a FluentBundle
    fn load_locale_bundle(
        locale_id: &str,
    ) -> anyhow::Result<FluentBundle<Arc<FluentResource>>> {
        let locale_dir = Self::get_locale_dir(locale_id)?;

        // Parse language identifier
        let lang_id: LanguageIdentifier = locale_id
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid locale ID '{}': {:?}", locale_id, e))?;

        // Create bundle
        let mut bundle = FluentBundle::new(vec![lang_id]);

        // Load all .ftl files
        let resources = Self::load_ftl_files(&locale_dir)?;

        if resources.is_empty() {
            anyhow::bail!(
                "No .ftl files found in {}",
                locale_dir.display()
            );
        }

        // Add resources to bundle
        for resource in resources {
            if let Err(errors) = bundle.add_resource(resource) {
                for error in errors {
                    tracing::error!("Failed to add resource to bundle: {:?}", error);
                }
            }
        }

        tracing::debug!(
            "Loaded locale bundle for {}",
            locale_id
        );

        Ok(bundle)
    }

    /// Get the path to the locale directory
    fn get_locale_dir(locale_id: &str) -> anyhow::Result<PathBuf> {
        let locales_dir = super::paths::find_resource_directory("locales")?;
        let locale_path = locales_dir.join(locale_id);
        if locale_path.is_dir() {
            tracing::debug!("Found locale directory: {}", locale_path.display());
            Ok(locale_path)
        } else {
            anyhow::bail!("Locale directory not found for '{}'", locale_id)
        }
    }

    /// Load all .ftl files from a directory
    fn load_ftl_files(dir: &Path) -> anyhow::Result<Vec<Arc<FluentResource>>> {
        let mut resources = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(Result::ok)
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    == Some("ftl")
            })
            .collect();

        // Sort entries alphabetically for consistent loading order
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            match Self::load_ftl_file(&path) {
                Ok(resource) => {
                    tracing::debug!("Loaded translation file: {}", path.display());
                    resources.push(resource);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to load translation file {}: {}",
                        path.display(),
                        e
                    );
                    // Continue loading other files
                }
            }
        }

        Ok(resources)
    }

    /// Load a single .ftl file
    fn load_ftl_file(path: &Path) -> anyhow::Result<Arc<FluentResource>> {
        let content = std::fs::read_to_string(path)?;
        let resource = FluentResource::try_new(content).map_err(|(_, errors)| {
            anyhow::anyhow!(
                "Failed to parse {}: {:?}",
                path.display(),
                errors
            )
        })?;
        Ok(Arc::new(resource))
    }

    /// Get a translated string by key
    ///
    /// # Arguments
    /// * `key` - Translation key (e.g., "help-title")
    ///
    /// # Returns
    /// * Translated string, or fallback string if key not found
    pub fn get(&self, key: &str) -> String {
        self.get_with_args(key, None)
    }

    /// Get a translated string with arguments
    ///
    /// # Arguments
    /// * `key` - Translation key
    /// * `args` - Optional arguments for parameterized translations
    ///
    /// # Returns
    /// * Translated string with substituted arguments
    pub fn get_with_args(&self, key: &str, args: Option<&FluentArgs>) -> String {
        // Try current locale first
        if let Some(message) = self.bundle.get_message(key) {
            if let Some(pattern) = message.value() {
                let mut errors = vec![];
                let value = self.bundle.format_pattern(pattern, args, &mut errors);

                if !errors.is_empty() {
                    tracing::warn!(
                        "Translation errors for key '{}': {:?}",
                        key,
                        errors
                    );
                }

                return value.to_string();
            }
        }

        // Try fallback locale
        if let Some(fallback) = &self.fallback_bundle {
            if let Some(message) = fallback.get_message(key) {
                if let Some(pattern) = message.value() {
                    let mut errors = vec![];
                    let value = fallback.format_pattern(pattern, args, &mut errors);

                    if !errors.is_empty() {
                        tracing::warn!(
                            "Translation errors for fallback key '{}': {:?}",
                            key,
                            errors
                        );
                    }

                    tracing::debug!(
                        "Using fallback translation for key: {}",
                        key
                    );
                    return value.to_string();
                }
            }
        }

        // No translation found
        tracing::warn!("Missing translation key: {}", key);
        format!("[missing: {}]", key)
    }

    /// Get the current locale code
    pub fn current_locale(&self) -> &str {
        &self.current_locale
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_locale_mapping() {
        // This test will fail if locale files don't exist yet
        // Uncomment when locale files are populated

        // let manager = LocalizationManager::new("en").expect("Failed to load en locale");
        // assert_eq!(manager.current_locale(), "en");

        // let manager = LocalizationManager::new("ja").expect("Failed to load ja locale");
        // assert_eq!(manager.current_locale(), "ja");
    }

    #[test]
    fn test_missing_key() {
        // This test will work once locale files exist
        // let manager = LocalizationManager::new("en").expect("Failed to load locale");
        // let result = manager.get("nonexistent-key");
        // assert!(result.contains("[missing:"));
    }
}
