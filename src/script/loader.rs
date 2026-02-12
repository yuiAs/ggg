use crate::script::error::{ScriptError, ScriptResult};
use std::path::{Path, PathBuf};

/// Script file loader
///
/// Handles:
/// - Loading scripts from filesystem
/// - Alphabetical ordering
/// - File filtering (*.js)
/// - Error handling for missing/invalid directories
pub struct ScriptLoader {
    directory: PathBuf,
}

impl ScriptLoader {
    /// Create new script loader for directory
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    /// Get all script files in alphabetical order
    pub fn list_scripts(&self) -> ScriptResult<Vec<PathBuf>> {
        let dir = &self.directory;

        // Check if directory exists
        if !dir.exists() {
            tracing::warn!(
                "Script directory does not exist: {:?}, creating it",
                dir
            );
            std::fs::create_dir_all(dir).map_err(|_e| ScriptError::InvalidScriptDirectory(dir.clone()))?;
            return Ok(Vec::new());
        }

        if !dir.is_dir() {
            return Err(ScriptError::InvalidScriptDirectory(dir.clone()));
        }

        // Read directory entries
        let entries = std::fs::read_dir(dir).map_err(|_e| ScriptError::InvalidScriptDirectory(dir.clone()))?;

        let mut scripts: Vec<PathBuf> = Vec::new();

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Filter for .js files
            if let Some(ext) = path.extension() {
                if ext == "js" {
                    scripts.push(path);
                }
            }
        }

        // Sort alphabetically by filename
        scripts.sort_by(|a, b| {
            let name_a = a.file_name().unwrap_or_default();
            let name_b = b.file_name().unwrap_or_default();
            name_a.cmp(name_b)
        });

        tracing::debug!("Found {} script files in {:?}", scripts.len(), dir);
        Ok(scripts)
    }

    /// Read script file contents
    pub fn read_script(&self, path: &Path) -> ScriptResult<String> {
        std::fs::read_to_string(path).map_err(|e| ScriptError::FileReadError {
            path: path.to_owned(),
            source: e,
        })
    }

    /// Get the script directory path
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_loader_creation() {
        let loader = ScriptLoader::new("./scripts");
        assert_eq!(loader.directory, PathBuf::from("./scripts"));
    }

    #[test]
    fn test_loader_directory_getter() {
        let loader = ScriptLoader::new("./test_scripts");
        assert_eq!(loader.directory(), Path::new("./test_scripts"));
    }

    #[test]
    fn test_list_scripts_empty_directory() {
        let temp_dir = std::env::temp_dir().join("ggg_test_empty");
        fs::create_dir_all(&temp_dir).unwrap();

        let loader = ScriptLoader::new(&temp_dir);
        let scripts = loader.list_scripts().unwrap();

        assert_eq!(scripts.len(), 0);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_list_scripts_filters_js_files() {
        let temp_dir = std::env::temp_dir().join("ggg_test_filter");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create test files
        fs::write(temp_dir.join("script1.js"), "// test").unwrap();
        fs::write(temp_dir.join("script2.js"), "// test").unwrap();
        fs::write(temp_dir.join("readme.txt"), "text file").unwrap();
        fs::write(temp_dir.join("config.json"), "{}").unwrap();

        let loader = ScriptLoader::new(&temp_dir);
        let scripts = loader.list_scripts().unwrap();

        // Should only find .js files
        assert_eq!(scripts.len(), 2);

        for script in &scripts {
            assert_eq!(script.extension().unwrap(), "js");
        }

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_list_scripts_alphabetical_order() {
        let temp_dir = std::env::temp_dir().join("ggg_test_sort");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create files in non-alphabetical order
        fs::write(temp_dir.join("c_third.js"), "// test").unwrap();
        fs::write(temp_dir.join("a_first.js"), "// test").unwrap();
        fs::write(temp_dir.join("b_second.js"), "// test").unwrap();

        let loader = ScriptLoader::new(&temp_dir);
        let scripts = loader.list_scripts().unwrap();

        assert_eq!(scripts.len(), 3);

        // Verify alphabetical order
        assert_eq!(
            scripts[0].file_name().unwrap().to_str().unwrap(),
            "a_first.js"
        );
        assert_eq!(
            scripts[1].file_name().unwrap().to_str().unwrap(),
            "b_second.js"
        );
        assert_eq!(
            scripts[2].file_name().unwrap().to_str().unwrap(),
            "c_third.js"
        );

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_list_scripts_nonexistent_directory_creates() {
        let temp_dir = std::env::temp_dir().join("ggg_test_nonexistent");

        // Ensure directory doesn't exist
        fs::remove_dir_all(&temp_dir).ok();

        let loader = ScriptLoader::new(&temp_dir);
        let scripts = loader.list_scripts().unwrap();

        // Should create directory and return empty list
        assert!(temp_dir.exists());
        assert_eq!(scripts.len(), 0);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_read_script() {
        let temp_dir = std::env::temp_dir().join("ggg_test_read");
        fs::create_dir_all(&temp_dir).unwrap();

        let script_path = temp_dir.join("test.js");
        let script_content = "ggg.on('beforeRequest', function(e) { return true; });";
        fs::write(&script_path, script_content).unwrap();

        let loader = ScriptLoader::new(&temp_dir);
        let content = loader.read_script(&script_path).unwrap();

        assert_eq!(content, script_content);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_read_script_file_not_found() {
        let temp_dir = std::env::temp_dir().join("ggg_test_missing");
        fs::create_dir_all(&temp_dir).unwrap();

        let loader = ScriptLoader::new(&temp_dir);
        let script_path = temp_dir.join("nonexistent.js");

        let result = loader.read_script(&script_path);
        assert!(result.is_err());

        match result.unwrap_err() {
            ScriptError::FileReadError { path, .. } => {
                assert_eq!(path, script_path);
            }
            _ => panic!("Expected FileReadError"),
        }

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_list_scripts_ignores_subdirectories() {
        let temp_dir = std::env::temp_dir().join("ggg_test_subdir");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a file and a subdirectory
        fs::write(temp_dir.join("script.js"), "// test").unwrap();
        fs::create_dir_all(temp_dir.join("subdir")).unwrap();
        fs::write(temp_dir.join("subdir").join("nested.js"), "// nested").unwrap();

        let loader = ScriptLoader::new(&temp_dir);
        let scripts = loader.list_scripts().unwrap();

        // Should only find the top-level script, not the nested one
        assert_eq!(scripts.len(), 1);
        assert_eq!(
            scripts[0].file_name().unwrap().to_str().unwrap(),
            "script.js"
        );

        fs::remove_dir_all(&temp_dir).ok();
    }
}
