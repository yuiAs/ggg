use super::task::DownloadTask;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Wrapper for TOML serialization (TOML requires root to be a table, not an array)
#[derive(Debug, Serialize, Deserialize)]
struct QueueFile {
    tasks: Vec<DownloadTask>,
}

#[derive(Clone)]
pub struct DownloadQueue {
    pub(crate) tasks: Arc<RwLock<VecDeque<DownloadTask>>>,
}

impl DownloadQueue {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    pub async fn add(&self, task: DownloadTask) {
        let mut tasks = self.tasks.write().await;
        tasks.push_back(task);
    }

    pub async fn get_all(&self) -> Vec<DownloadTask> {
        // Minimize lock time by cloning VecDeque structure first, then releasing lock
        let tasks_clone = {
            let tasks = self.tasks.read().await;
            tasks.clone()  // Fast: just clones the deque structure, not elements yet
        };
        // Lock released here - clone elements outside the lock to reduce contention
        tasks_clone.into_iter().collect()
    }

    pub async fn remove(&self, id: uuid::Uuid) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().await;
        if let Some(pos) = tasks.iter().position(|t| t.id == id) {
            tasks.remove(pos)
        } else {
            None
        }
    }

    pub async fn get_by_id(&self, id: uuid::Uuid) -> Option<DownloadTask> {
        let tasks = self.tasks.read().await;
        tasks.iter().find(|t| t.id == id).cloned()
    }

    pub async fn update(&self, task: DownloadTask) {
        let mut tasks = self.tasks.write().await;
        if let Some(pos) = tasks.iter().position(|t| t.id == task.id) {
            tasks[pos] = task;
        }
    }

    pub async fn save_to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let tasks = self.tasks.read().await;
        let json = serde_json::to_string_pretty(&tasks.iter().collect::<Vec<_>>())?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub async fn load_from_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let json = std::fs::read_to_string(path)?;
        let loaded_tasks: Vec<DownloadTask> = serde_json::from_str(&json)?;

        let mut tasks = self.tasks.write().await;
        tasks.clear();
        tasks.extend(loaded_tasks);

        Ok(())
    }

    /// Save queue partitioned by folder_id to folder-specific TOML files
    ///
    /// Each folder gets its own queue.toml file in {config_dir}/{folder_id}/queue.toml
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Failed to get config directory
    /// - Failed to create folder directory
    /// - Failed to serialize tasks to TOML
    /// - Failed to write TOML file
    pub async fn save_to_folder_files(&self) -> anyhow::Result<()> {
        let tasks = self.tasks.read().await;

        // Partition tasks by folder_id
        let mut by_folder: std::collections::HashMap<String, Vec<DownloadTask>> =
            std::collections::HashMap::new();
        for task in tasks.iter() {
            by_folder
                .entry(task.folder_id.clone())
                .or_default()
                .push(task.clone());
        }

        // Clean up queue files for folders that no longer have tasks
        let config_dir = crate::util::paths::find_config_directory()?;
        if let Ok(entries) = std::fs::read_dir(&config_dir) {
            for entry in entries.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }

                let folder_name = entry.file_name();
                let folder_id = folder_name.to_string_lossy().to_string();
                let queue_file = entry.path().join("queue.toml");

                // If queue.toml exists but folder has no tasks, delete it
                if queue_file.exists() && !by_folder.contains_key(&folder_id) {
                    if let Err(e) = tokio::fs::remove_file(&queue_file).await {
                        tracing::warn!(
                            "Failed to remove old queue file {}: {}",
                            queue_file.display(),
                            e
                        );
                    } else {
                        tracing::debug!(
                            "Removed queue file for folder with no tasks: {}",
                            queue_file.display()
                        );
                    }
                }
            }
        }

        // Write each folder's queue
        for (folder_id, folder_tasks) in by_folder {
            let queue_path = crate::util::paths::get_folder_queue_path(&folder_id)?;

            // Create folder directory if needed
            if let Some(parent) = queue_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            // Serialize to TOML (wrap in QueueFile to satisfy TOML root table requirement)
            let task_count = folder_tasks.len();
            let queue_file = QueueFile { tasks: folder_tasks };
            let toml = toml::to_string_pretty(&queue_file)?;

            // Atomic write: temp file + rename
            let temp_path = queue_path.with_extension("toml.tmp");
            tokio::fs::write(&temp_path, toml).await?;
            tokio::fs::rename(&temp_path, &queue_path).await?;

            tracing::debug!(
                "Saved {} tasks to folder queue: {}",
                task_count,
                queue_path.display()
            );
        }

        Ok(())
    }

    /// Load all folder queues and merge into in-memory queue
    ///
    /// Scans {config_dir}/ for subdirectories and loads queue.toml from each.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Failed to get config directory
    /// - Failed to read directory
    /// - Failed to parse TOML file
    pub async fn load_from_folder_files(&self) -> anyhow::Result<()> {
        let config_dir = crate::util::paths::find_config_directory()?;
        let mut all_tasks = VecDeque::new();

        // Scan for folder subdirectories
        let entries = std::fs::read_dir(&config_dir)?;
        for entry in entries.flatten() {
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let queue_file = entry.path().join("queue.toml");
            if !queue_file.exists() {
                continue;
            }

            // Load this folder's queue (unwrap QueueFile wrapper)
            let content = tokio::fs::read_to_string(&queue_file).await?;
            let queue_file_data: QueueFile = toml::from_str(&content)?;

            tracing::debug!(
                "Loaded {} tasks from folder queue: {}",
                queue_file_data.tasks.len(),
                queue_file.display()
            );

            all_tasks.extend(queue_file_data.tasks);
        }

        // Replace in-memory queue
        let mut queue = self.tasks.write().await;
        *queue = all_tasks;

        tracing::info!("Loaded total {} tasks from folder queues", queue.len());

        Ok(())
    }
}

impl Default for DownloadQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_task(url: &str) -> DownloadTask {
        DownloadTask::new(url.to_string(), PathBuf::from("/tmp"))
    }

    // Construction & Basic Operations

    #[tokio::test]
    async fn test_queue_new_empty() {
        let queue = DownloadQueue::new();
        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 0);
    }

    #[tokio::test]
    async fn test_queue_add_single() {
        let queue = DownloadQueue::new();
        let task = create_test_task("http://example.com/file.zip");
        let task_id = task.id;

        queue.add(task).await;

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, task_id);
    }

    #[tokio::test]
    async fn test_queue_add_multiple() {
        let queue = DownloadQueue::new();
        let task1 = create_test_task("http://example.com/file1.zip");
        let task2 = create_test_task("http://example.com/file2.zip");
        let id1 = task1.id;
        let id2 = task2.id;

        queue.add(task1).await;
        queue.add(task2).await;

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, id1);
        assert_eq!(tasks[1].id, id2);
    }

    #[tokio::test]
    async fn test_queue_get_all() {
        let queue = DownloadQueue::new();
        queue.add(create_test_task("http://example.com/file1.zip")).await;
        queue.add(create_test_task("http://example.com/file2.zip")).await;
        queue.add(create_test_task("http://example.com/file3.zip")).await;

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 3);
    }

    #[tokio::test]
    async fn test_queue_get_by_id_found() {
        let queue = DownloadQueue::new();
        let task = create_test_task("http://example.com/file.zip");
        let task_id = task.id;

        queue.add(task).await;

        let found = queue.get_by_id(task_id).await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, task_id);
    }

    #[tokio::test]
    async fn test_queue_get_by_id_not_found() {
        let queue = DownloadQueue::new();
        queue.add(create_test_task("http://example.com/file.zip")).await;

        let nonexistent_id = uuid::Uuid::new_v4();
        let found = queue.get_by_id(nonexistent_id).await;
        assert!(found.is_none());
    }

    // Modification

    #[tokio::test]
    async fn test_queue_remove_existing() {
        let queue = DownloadQueue::new();
        let task = create_test_task("http://example.com/file.zip");
        let task_id = task.id;

        queue.add(task).await;

        let removed = queue.remove(task_id).await;
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, task_id);

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 0);
    }

    #[tokio::test]
    async fn test_queue_remove_nonexistent() {
        let queue = DownloadQueue::new();
        queue.add(create_test_task("http://example.com/file.zip")).await;

        let nonexistent_id = uuid::Uuid::new_v4();
        let removed = queue.remove(nonexistent_id).await;
        assert!(removed.is_none());

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_queue_update_existing() {
        let queue = DownloadQueue::new();
        let mut task = create_test_task("http://example.com/file.zip");
        let task_id = task.id;

        queue.add(task.clone()).await;

        // Update task
        task.downloaded = 1024;
        queue.update(task).await;

        let updated = queue.get_by_id(task_id).await.unwrap();
        assert_eq!(updated.downloaded, 1024);
    }

    #[tokio::test]
    async fn test_queue_update_nonexistent() {
        let queue = DownloadQueue::new();
        let task = create_test_task("http://example.com/file.zip");

        // Update without adding first - should be no-op
        queue.update(task.clone()).await;

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 0);
    }

    #[tokio::test]
    async fn test_queue_update_preserves_order() {
        let queue = DownloadQueue::new();
        let task1 = create_test_task("http://example.com/file1.zip");
        let mut task2 = create_test_task("http://example.com/file2.zip");
        let task3 = create_test_task("http://example.com/file3.zip");
        let id1 = task1.id;
        let id2 = task2.id;
        let id3 = task3.id;

        queue.add(task1).await;
        queue.add(task2.clone()).await;
        queue.add(task3).await;

        // Update middle task
        task2.downloaded = 5000;
        queue.update(task2).await;

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, id1);
        assert_eq!(tasks[1].id, id2);
        assert_eq!(tasks[2].id, id3);
        assert_eq!(tasks[1].downloaded, 5000);
    }

    // Persistence

    #[tokio::test]
    async fn test_queue_save_creates_file() {
        let queue = DownloadQueue::new();
        queue.add(create_test_task("http://example.com/file.zip")).await;

        let temp_dir = tempfile::tempdir().unwrap();
        let queue_path = temp_dir.path().join("queue.json");

        queue.save_to_file(&queue_path).await.unwrap();

        assert!(queue_path.exists());
    }

    #[tokio::test]
    async fn test_queue_load_restores_tasks() {
        let queue = DownloadQueue::new();
        let task = create_test_task("http://example.com/file.zip");
        let task_id = task.id;
        queue.add(task).await;

        let temp_dir = tempfile::tempdir().unwrap();
        let queue_path = temp_dir.path().join("queue.json");

        queue.save_to_file(&queue_path).await.unwrap();

        // Create new queue and load
        let new_queue = DownloadQueue::new();
        new_queue.load_from_file(&queue_path).await.unwrap();

        let tasks = new_queue.get_all().await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, task_id);
    }

    #[tokio::test]
    async fn test_queue_load_missing_file_ok() {
        let queue = DownloadQueue::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let queue_path = temp_dir.path().join("nonexistent.json");

        // Should not error
        let result = queue.load_from_file(&queue_path).await;
        assert!(result.is_ok());

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 0);
    }

    #[tokio::test]
    async fn test_queue_load_clears_existing() {
        let queue = DownloadQueue::new();
        queue.add(create_test_task("http://example.com/old.zip")).await;

        let temp_dir = tempfile::tempdir().unwrap();
        let queue_path = temp_dir.path().join("queue.json");

        // Save different task
        let save_queue = DownloadQueue::new();
        let new_task = create_test_task("http://example.com/new.zip");
        let new_id = new_task.id;
        save_queue.add(new_task).await;
        save_queue.save_to_file(&queue_path).await.unwrap();

        // Load should replace existing
        queue.load_from_file(&queue_path).await.unwrap();

        let tasks = queue.get_all().await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, new_id);
    }

    #[tokio::test]
    async fn test_queue_save_load_roundtrip() {
        let queue = DownloadQueue::new();
        let task1 = create_test_task("http://example.com/file1.zip");
        let task2 = create_test_task("http://example.com/file2.zip");
        let id1 = task1.id;
        let id2 = task2.id;

        queue.add(task1).await;
        queue.add(task2).await;

        let temp_dir = tempfile::tempdir().unwrap();
        let queue_path = temp_dir.path().join("queue.json");

        queue.save_to_file(&queue_path).await.unwrap();

        let new_queue = DownloadQueue::new();
        new_queue.load_from_file(&queue_path).await.unwrap();

        let tasks = new_queue.get_all().await;
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, id1);
        assert_eq!(tasks[1].id, id2);
    }

    #[tokio::test]
    async fn test_queue_save_all_fields() {
        let queue = DownloadQueue::new();
        let mut task = create_test_task("http://example.com/file.zip");
        task.downloaded = 1024;
        task.size = Some(2048);
        task.folder_id = "images".to_string();
        task.status = crate::download::task::DownloadStatus::Downloading;

        queue.add(task.clone()).await;

        let temp_dir = tempfile::tempdir().unwrap();
        let queue_path = temp_dir.path().join("queue.json");

        queue.save_to_file(&queue_path).await.unwrap();

        let new_queue = DownloadQueue::new();
        new_queue.load_from_file(&queue_path).await.unwrap();

        let tasks = new_queue.get_all().await;
        assert_eq!(tasks[0].downloaded, 1024);
        assert_eq!(tasks[0].size, Some(2048));
        assert_eq!(tasks[0].folder_id, "images");
    }

    #[test]
    fn test_toml_serialization_wrapper() {
        // Test that QueueFile wrapper fixes TOML top-level array issue
        let task1 = create_test_task("http://example.com/file1.zip");
        let task2 = create_test_task("http://example.com/file2.zip");

        let queue_file = QueueFile {
            tasks: vec![task1.clone(), task2.clone()],
        };

        // Should not panic with "unsupported rust type" error
        let toml_str = toml::to_string_pretty(&queue_file).expect("TOML serialization should succeed");

        // Verify it contains expected data
        assert!(toml_str.contains("[[tasks]]"));
        assert!(toml_str.contains(&task1.url));
        assert!(toml_str.contains(&task2.url));

        // Test deserialization
        let deserialized: QueueFile = toml::from_str(&toml_str).expect("TOML deserialization should succeed");
        assert_eq!(deserialized.tasks.len(), 2);
        assert_eq!(deserialized.tasks[0].id, task1.id);
        assert_eq!(deserialized.tasks[1].id, task2.id);
    }
}
