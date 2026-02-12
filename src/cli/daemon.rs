use crate::download::manager::DownloadManager;
use anyhow::Result;
use tokio::signal;

/// Run in headless daemon mode
pub async fn run_daemon(manager: DownloadManager) -> Result<()> {
    tracing::info!("Starting daemon mode...");
    tracing::info!("Press Ctrl+C to stop");

    // Clone manager for auto-save task
    let manager_clone = manager.clone();

    // Spawn auto-save task
    let auto_save_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

        loop {
            interval.tick().await;

            match manager_clone.save_queue_to_folders().await {
                Ok(_) => tracing::debug!("Queue auto-saved to folder files"),
                Err(e) => tracing::error!("Failed to auto-save queue: {}", e),
            }
        }
    });

    // Wait for Ctrl+C
    match signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!("Received Ctrl+C, shutting down...");
        }
        Err(e) => {
            tracing::error!("Error waiting for Ctrl+C: {}", e);
        }
    }

    // Cancel auto-save task
    auto_save_handle.abort();

    // Save queue one last time
    tracing::info!("Saving queue to folder files...");
    manager.save_queue_to_folders().await?;

    tracing::info!("Daemon stopped");

    Ok(())
}
