use crate::download::task::DownloadTask;
use serde_json;

/// Format bytes into human-readable string (KB, MB, GB)
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a single download task for display
pub fn format_download(task: &DownloadTask, detailed: bool) -> String {
    let mut output = String::new();

    if detailed {
        output.push_str(&format!("ID: {}\n", task.id));
        output.push_str(&format!("URL: {}\n", task.url));
        output.push_str(&format!("Filename: {}\n", task.filename));
        output.push_str(&format!("Folder: {}\n", task.folder_id));
        output.push_str(&format!("Status: {:?}\n", task.status));

        if let Some(total) = task.size {
            output.push_str(&format!("Size: {}\n", format_bytes(total)));
        }

        if task.downloaded > 0 {
            output.push_str(&format!("Downloaded: {}\n", format_bytes(task.downloaded)));

            if let Some(total) = task.size {
                let progress = (task.downloaded as f64 / total as f64 * 100.0) as u8;
                output.push_str(&format!("Progress: {}%\n", progress));
            }
        }

        if task.resume_supported {
            output.push_str("Resume: Supported\n");
        }

        output.push_str(&format!("Created: {}\n", task.created_at.format("%Y-%m-%d %H:%M:%S")));

        if let Some(started) = task.started_at {
            output.push_str(&format!("Started: {}\n", started.format("%Y-%m-%d %H:%M:%S")));
        }

        if let Some(completed) = task.completed_at {
            output.push_str(&format!("Completed: {}\n", completed.format("%Y-%m-%d %H:%M:%S")));
        }
    } else {
        // Compact format for lists
        let status_icon = match task.status {
            crate::download::task::DownloadStatus::Pending => "⏸",
            crate::download::task::DownloadStatus::Downloading => "⬇",
            crate::download::task::DownloadStatus::Completed => "✓",
            crate::download::task::DownloadStatus::Error => "✗",
            crate::download::task::DownloadStatus::Paused => "⏸",
            crate::download::task::DownloadStatus::Deleted => "🗑",
        };

        let progress_str = if let Some(total) = task.size {
            let progress = (task.downloaded as f64 / total as f64 * 100.0) as u8;
            format!("{}%", progress)
        } else {
            format_bytes(task.downloaded)
        };

        output.push_str(&format!("{} {} [{}] {}",
            status_icon,
            task.id,
            progress_str,
            task.filename
        ));
    }

    output
}

/// Format multiple downloads for display (human or JSON)
pub fn format_downloads(tasks: &[DownloadTask], json: bool) -> String {
    if json {
        serde_json::to_string_pretty(tasks).unwrap_or_else(|_| "[]".to_string())
    } else {
        if tasks.is_empty() {
            return "No downloads in queue.".to_string();
        }

        tasks.iter()
            .map(|task| format_download(task, false))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
