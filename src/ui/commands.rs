use crate::AppState;
use crate::download::{manager::DownloadManager, task::DownloadTask};
use fluent::fluent_args;
use serde::{Deserialize, Serialize};

/// Commands that can be invoked from the TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Command {
    AddDownload { urls: Vec<String> },
    StartDownload { id: String },
    PauseDownload { id: String },
    GetDownloads,
    RemoveDownload { id: String },
    ChangeFolder { id: String, folder_id: String },
    GetConfig,
    UpdateConfig { config: serde_json::Value },

    // Application-level settings
    UpdateMaxConcurrent { value: usize },
    UpdateMaxConcurrentPerFolder { value: Option<usize> },
    UpdateMaxActiveFolders { value: Option<usize> },
    UpdateMaxRedirects { value: u32 },
    UpdateRetryCount { value: u32 },
    UpdateScriptsEnabled { value: bool },
    UpdateSkipDownloadPreview { value: bool },
    UpdateLanguage { value: String },

    // Folder-level settings
    UpdateFolderMaxConcurrent { folder_id: String, value: Option<usize> },

    // Script settings
    ToggleScriptFile { filename: String },
    ToggleFolderScriptFile { folder_id: String, filename: String },
    ReloadScripts,

    // Config management
    ReloadConfig,
}

/// Response to a command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandResponse {
    Success { data: serde_json::Value },
    Error { error: String },
}

pub async fn handle_command(
    command: Command,
    state: AppState,
    download_manager: DownloadManager,
) -> CommandResponse {
    match command {
        Command::AddDownload { urls } => {
            let config = state.config.read().await;
            for url in urls {
                let task = DownloadTask::new(url, config.download.default_directory.clone());
                download_manager.add_download(task).await;
            }
            CommandResponse::Success {
                data: serde_json::json!({"status": "ok"}),
            }
        }
        Command::StartDownload { id } => {
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                match download_manager.start_download(uuid, state.script_sender.clone(), state.config.clone()).await {
                    Ok(_) => CommandResponse::Success {
                        data: serde_json::json!({"status": "ok"}),
                    },
                    Err(e) => CommandResponse::Error {
                        error: state.t_with_args("cmd-error-start-download",
                            Some(&fluent_args!["error" => e.to_string()])),
                    },
                }
            } else {
                CommandResponse::Error {
                    error: state.t("cmd-error-invalid-uuid"),
                }
            }
        }
        Command::PauseDownload { id } => {
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                match download_manager.pause_download(uuid).await {
                    Ok(_) => CommandResponse::Success {
                        data: serde_json::json!({"status": "ok"}),
                    },
                    Err(e) => CommandResponse::Error {
                        error: state.t_with_args("cmd-error-pause-download",
                            Some(&fluent_args!["error" => e.to_string()])),
                    },
                }
            } else {
                CommandResponse::Error {
                    error: state.t("cmd-error-invalid-uuid"),
                }
            }
        }
        Command::GetDownloads => {
            let downloads = download_manager.get_all_downloads().await;
            CommandResponse::Success {
                data: serde_json::to_value(&downloads).unwrap(),
            }
        }
        Command::RemoveDownload { id } => {
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                download_manager.remove_download(uuid).await;
                CommandResponse::Success {
                    data: serde_json::json!({"status": "ok"}),
                }
            } else {
                CommandResponse::Error {
                    error: state.t("cmd-error-invalid-uuid"),
                }
            }
        }
        Command::ChangeFolder { id, folder_id } => {
            if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                match download_manager.change_folder(uuid, folder_id).await {
                    Ok(_) => CommandResponse::Success {
                        data: serde_json::json!({"status": "ok"}),
                    },
                    Err(e) => CommandResponse::Error {
                        error: state.t_with_args("cmd-error-change-folder",
                            Some(&fluent_args!["error" => e.to_string()])),
                    },
                }
            } else {
                CommandResponse::Error {
                    error: state.t("cmd-error-invalid-uuid"),
                }
            }
        }
        Command::GetConfig => {
            let config = state.config.read().await;
            CommandResponse::Success {
                data: serde_json::to_value(&*config).unwrap(),
            }
        }
        Command::UpdateConfig { config } => {
            let mut state_config = state.config.write().await;
            if let Ok(new_config) = serde_json::from_value(config) {
                // Validate before applying
                if let Err(errors) = crate::app::settings::validate_folder_config(&new_config) {
                    let error_str = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                    return CommandResponse::Error {
                        error: state.t_with_args("cmd-error-validation-failed",
                            Some(&fluent_args!["error" => error_str])),
                    };
                }

                *state_config = new_config;
                // Save to disk
                if let Err(e) = state_config.save() {
                    return CommandResponse::Error {
                        error: state.t_with_args("cmd-error-save-config",
                            Some(&fluent_args!["error" => e.to_string()])),
                    };
                }

                CommandResponse::Success {
                    data: serde_json::json!({"status": "ok"}),
                }
            } else {
                CommandResponse::Error {
                    error: state.t("cmd-error-invalid-config"),
                }
            }
        }

        Command::UpdateMaxConcurrent { value } => {
            let mut config = state.config.write().await;
            config.download.max_concurrent = value;

            // Validate constraints
            if let Err(errors) = crate::app::settings::validate_folder_config(&config) {
                let error_str = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-validation-failed",
                        Some(&fluent_args!["error" => error_str])),
                };
            }

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "value": value}),
            }
        }

        Command::UpdateMaxConcurrentPerFolder { value } => {
            let mut config = state.config.write().await;
            config.download.max_concurrent_per_folder = value;

            // Validate constraints
            if let Err(errors) = crate::app::settings::validate_folder_config(&config) {
                let error_str = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-validation-failed",
                        Some(&fluent_args!["error" => error_str])),
                };
            }

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "value": value}),
            }
        }

        Command::UpdateMaxActiveFolders { value } => {
            let mut config = state.config.write().await;
            config.download.parallel_folder_count = value;

            // Validate constraints
            if let Err(errors) = crate::app::settings::validate_folder_config(&config) {
                let error_str = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-validation-failed",
                        Some(&fluent_args!["error" => error_str])),
                };
            }

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "value": value}),
            }
        }

        Command::UpdateMaxRedirects { value } => {
            let mut config = state.config.write().await;
            config.download.max_redirects = value;

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "value": value}),
            }
        }

        Command::UpdateRetryCount { value } => {
            let mut config = state.config.write().await;
            config.download.retry_count = value;

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "value": value}),
            }
        }

        Command::UpdateScriptsEnabled { value } => {
            let mut config = state.config.write().await;
            config.scripts.enabled = value;

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "value": value}),
            }
        }
        Command::UpdateSkipDownloadPreview { value } => {
            let mut config = state.config.write().await;
            config.general.skip_download_preview = value;

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "value": value}),
            }
        }
        Command::UpdateLanguage { value } => {
            let mut config = state.config.write().await;
            config.general.language = value.clone();

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "value": value}),
            }
        }

        Command::UpdateFolderMaxConcurrent { folder_id, value } => {
            let mut config = state.config.write().await;

            // Get or create folder config
            let folder_config = config
                .folders
                .entry(folder_id.clone())
                .or_insert_with(crate::app::config::FolderConfig::default);

            folder_config.max_concurrent = value;

            // Validate constraints
            if let Err(errors) = crate::app::settings::validate_folder_config(&config) {
                let error_str = errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-validation-failed",
                        Some(&fluent_args!["error" => error_str])),
                };
            }

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({"status": "ok", "folder_id": folder_id, "value": value}),
            }
        }

        Command::ToggleScriptFile { filename } => {
            let mut config = state.config.write().await;

            // Toggle the enabled status (default is true if not in map)
            let current_status = config.scripts.script_files.get(&filename).copied().unwrap_or(true);
            let new_status = !current_status;
            config.scripts.script_files.insert(filename.clone(), new_status);

            // Save to disk
            if let Err(e) = config.save() {
                return CommandResponse::Error {
                    error: state.t_with_args("cmd-error-save-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                };
            }

            CommandResponse::Success {
                data: serde_json::json!({
                    "status": "ok",
                    "filename": filename,
                    "enabled": new_status
                }),
            }
        }

        Command::ToggleFolderScriptFile { folder_id, filename } => {
            let mut config = state.config.write().await;

            // Get application-level status first (before mutable borrow)
            let app_status = config.scripts.script_files.get(&filename).copied().unwrap_or(true);

            // Get or create folder config
            if let Some(folder_config) = config.folders.get_mut(&folder_id) {
                // Get or create the script_files map for this folder
                let script_files = folder_config.script_files.get_or_insert_with(std::collections::HashMap::new);

                // Get current effective status (inherit from Application if not overridden)
                let current_status = script_files.get(&filename).copied().unwrap_or(app_status);

                // Toggle: enabled -> disabled, disabled -> remove override (inherit)
                if script_files.contains_key(&filename) {
                    // Already overridden - remove the override to inherit from Application
                    script_files.remove(&filename);
                } else {
                    // Not overridden - set opposite of current inherited value
                    script_files.insert(filename.clone(), !current_status);
                }

                // If script_files becomes empty, set to None to inherit all
                if script_files.is_empty() {
                    folder_config.script_files = None;
                }

                // Save to disk
                if let Err(e) = config.save() {
                    return CommandResponse::Error {
                        error: state.t_with_args("cmd-error-save-config",
                            Some(&fluent_args!["error" => e.to_string()])),
                    };
                }

                CommandResponse::Success {
                    data: serde_json::json!({
                        "status": "ok",
                        "folder_id": folder_id,
                        "filename": filename,
                    }),
                }
            } else {
                CommandResponse::Error {
                    error: state.t_with_args("cmd-error-folder-not-found",
                        Some(&fluent_args!["folder" => folder_id.clone()])),
                }
            }
        }

        Command::ReloadScripts => {
            // Send reload message to script executor
            if let Some(ref script_sender) = state.script_sender {
                let (response_tx, response_rx) = std::sync::mpsc::channel();
                let sender_clone = script_sender.clone();

                // Send request and receive response in blocking task
                match tokio::task::spawn_blocking(move || {
                    if let Err(e) = sender_clone.send(crate::script::message::ScriptRequest::Reload {
                        response: response_tx,
                    }) {
                        return Err(format!("{:?}", e));
                    }

                    response_rx.recv()
                        .map_err(|e| format!("{:?}", e))
                }).await
                {
                    Ok(Ok(Ok(_))) => CommandResponse::Success {
                        data: serde_json::json!({
                            "status": "ok",
                            "message": state.t("cmd-success-scripts-reloaded")
                        }),
                    },
                    Ok(Ok(Err(e))) => CommandResponse::Error {
                        error: state.t_with_args("cmd-error-reload-scripts",
                            Some(&fluent_args!["error" => e.to_string()])),
                    },
                    Ok(Err(e)) => CommandResponse::Error {
                        error: state.t_with_args("cmd-error-script-communication",
                            Some(&fluent_args!["error" => e.clone()])),
                    },
                    Err(e) => CommandResponse::Error {
                        error: state.t_with_args("cmd-error-blocking-task",
                            Some(&fluent_args!["error" => e.to_string()])),
                    },
                }
            } else {
                CommandResponse::Error {
                    error: state.t("cmd-error-scripts-disabled"),
                }
            }
        }

        Command::ReloadConfig => {
            // Check if any downloads are active
            let has_active = download_manager.has_active_downloads().await;
            if has_active {
                return CommandResponse::Error {
                    error: state.t("cmd-error-reload-active-downloads"),
                };
            }

            // Reload configuration from disk
            match crate::app::config::Config::load() {
                Ok(new_config) => {
                    // Update application state
                    let mut config = state.config.write().await;
                    *config = new_config;

                    CommandResponse::Success {
                        data: serde_json::json!({
                            "status": "ok",
                            "message": state.t("cmd-success-config-reloaded")
                        }),
                    }
                }
                Err(e) => CommandResponse::Error {
                    error: state.t_with_args("cmd-error-reload-config",
                        Some(&fluent_args!["error" => e.to_string()])),
                },
            }
        }
    }
}
