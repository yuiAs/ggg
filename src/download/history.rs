//! Download history module
//!
//! Stores completed, failed, and deleted downloads for display in the Completed node.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use uuid::Uuid;

use super::task::DownloadTask;

/// Download history storage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadHistory {
    /// List of historical download items (completed, failed, deleted)
    pub items: Vec<DownloadTask>,
}

impl DownloadHistory {
    /// Creates a new empty history
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a task to history
    pub fn add(&mut self, task: DownloadTask) {
        // Avoid duplicates by ID
        if !self.items.iter().any(|t| t.id == task.id) {
            self.items.push(task);
        }
    }

    /// Removes a task from history by ID
    pub fn remove(&mut self, id: Uuid) -> Option<DownloadTask> {
        if let Some(pos) = self.items.iter().position(|t| t.id == id) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    /// Gets a task by ID
    pub fn get(&self, id: Uuid) -> Option<&DownloadTask> {
        self.items.iter().find(|t| t.id == id)
    }

    /// Gets a mutable reference to a task by ID
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut DownloadTask> {
        self.items.iter_mut().find(|t| t.id == id)
    }

    /// Returns all history items
    pub fn all(&self) -> &[DownloadTask] {
        &self.items
    }

    /// Returns the number of items in history
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if history is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clears all history items
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Loads history from a TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(path)?;
        let history: DownloadHistory = toml::from_str(&content)?;
        Ok(history)
    }

    /// Saves history to a TOML file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::task::DownloadStatus;
    use std::path::PathBuf;

    fn create_test_task(status: DownloadStatus) -> DownloadTask {
        let mut task = DownloadTask::new(
            "http://example.com/file.txt".to_string(),
            PathBuf::from("/tmp/test"),
        );
        task.status = status;
        task
    }

    #[test]
    fn test_history_new() {
        let history = DownloadHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_history_add_and_get() {
        let mut history = DownloadHistory::new();
        let task = create_test_task(DownloadStatus::Completed);
        let id = task.id;

        history.add(task);

        assert_eq!(history.len(), 1);
        assert!(history.get(id).is_some());
    }

    #[test]
    fn test_history_no_duplicates() {
        let mut history = DownloadHistory::new();
        let task = create_test_task(DownloadStatus::Completed);
        let id = task.id;

        history.add(task.clone());
        history.add(task);

        assert_eq!(history.len(), 1);
        assert!(history.get(id).is_some());
    }

    #[test]
    fn test_history_remove() {
        let mut history = DownloadHistory::new();
        let task = create_test_task(DownloadStatus::Completed);
        let id = task.id;

        history.add(task);
        let removed = history.remove(id);

        assert!(removed.is_some());
        assert!(history.is_empty());
    }

    #[test]
    fn test_history_clear() {
        let mut history = DownloadHistory::new();
        history.add(create_test_task(DownloadStatus::Completed));
        history.add(create_test_task(DownloadStatus::Error));

        assert_eq!(history.len(), 2);

        history.clear();

        assert!(history.is_empty());
    }
}
