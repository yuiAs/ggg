use super::config::Config;
use crate::script::{executor, message::ScriptRequest};
use crate::util::i18n::LocalizationManager;
use anyhow::Result;
use std::sync::{mpsc, Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    /// Shared internationalization manager
    pub i18n: Arc<LocalizationManager>,
    /// Channel sender for script execution requests
    ///
    /// When Some, scripts are enabled and requests are sent to the executor thread.
    /// The executor thread runs in a separate OS thread with its own ScriptManager.
    pub script_sender: Option<mpsc::Sender<ScriptRequest>>,
}

impl AppState {
    /// Create LocalizationManager with fallback to English
    fn create_i18n(language: &str) -> Arc<LocalizationManager> {
        Arc::new(
            LocalizationManager::new(language).unwrap_or_else(|e| {
                tracing::error!("Failed to load translations for '{}': {}", language, e);
                tracing::info!("Falling back to English");
                LocalizationManager::new("en").expect("Failed to load fallback locale")
            }),
        )
    }

    pub fn new(config: Config, language: &str) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            i18n: Self::create_i18n(language),
            script_sender: None,
        }
    }

    pub async fn new_with_scripts(config: Config, language: &str) -> Result<Self> {
        // Spawn script executor thread if scripts enabled
        let script_sender = if config.scripts.enabled {
            let (tx, rx) = std::sync::mpsc::channel();

            let script_config = config.scripts.clone();

            // Spawn in a dedicated OS thread since ScriptManager (!Send) cannot cross thread boundaries
            std::thread::spawn(move || {
                // Create ScriptManager
                let mut script_manager = match crate::script::ScriptManager::new(&script_config) {
                    Ok(sm) => {
                        tracing::info!("ScriptManager created successfully");
                        sm
                    }
                    Err(e) => {
                        tracing::error!("Failed to create ScriptManager: {}", e);
                        tracing::warn!("Script executor thread exiting due to initialization failure");
                        return;
                    }
                };

                // Load all scripts
                if let Err(e) = script_manager.load_all_scripts() {
                    tracing::error!("Failed to load scripts: {}", e);
                } else {
                    tracing::info!("Scripts loaded successfully");
                }

                // Run executor loop (no tokio runtime needed)
                executor::script_executor_loop(rx, script_manager);
            });

            tracing::info!("Script executor thread spawned");
            Some(tx)
        } else {
            None
        };

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            i18n: Self::create_i18n(language),
            script_sender,
        })
    }

    /// Get translated string by key
    pub fn t(&self, key: &str) -> String {
        self.i18n.get(key)
    }

    /// Get translated string with arguments
    pub fn t_with_args(&self, key: &str, args: Option<&fluent_bundle::FluentArgs>) -> String {
        self.i18n.get_with_args(key, args)
    }

    /// Reload all scripts from disk
    pub async fn reload_scripts(&self) -> Result<()> {
        if let Some(ref sender) = self.script_sender {
            let (response_tx, response_rx) = std::sync::mpsc::channel();
            let sender = sender.clone();

            // Send request and receive response in blocking task
            tokio::task::spawn_blocking(move || {
                sender
                    .send(ScriptRequest::Reload {
                        response: response_tx,
                    })
                    .map_err(|e| anyhow::anyhow!("Failed to send reload request: {}", e))?;

                response_rx
                    .recv()
                    .map_err(|e| anyhow::anyhow!("Failed to receive reload response: {}", e))?
                    .map_err(|e| anyhow::anyhow!("Script reload failed: {}", e))?;

                Ok::<(), anyhow::Error>(())
            })
            .await
            .map_err(|e| anyhow::anyhow!("Blocking task failed: {}", e))??;

            tracing::info!("Scripts reloaded successfully");
            Ok(())
        } else {
            Err(anyhow::anyhow!("Scripts are not enabled"))
        }
    }
}
