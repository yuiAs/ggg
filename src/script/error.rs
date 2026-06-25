use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during script execution
#[derive(Error, Debug)]
pub enum ScriptError {
    /// Failed to read script file
    #[error("Failed to read script file {path}: {source}")]
    FileReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    /// Script compilation failed
    #[error("Failed to compile script {path}: {message}")]
    CompilationError { path: PathBuf, message: String },

    /// Script execution failed
    #[error("Script execution error in {script}: {message}")]
    ExecutionError { script: String, message: String },

    /// Script timeout
    #[error("Script timeout after {timeout_ms}ms in {script}")]
    Timeout { script: String, timeout_ms: u64 },

    /// Invalid event name
    #[error("Invalid event name: {0}")]
    InvalidEventName(String),

    /// Invalid filter pattern
    #[error("Invalid URL filter pattern in {script}: {pattern}")]
    InvalidFilter { script: String, pattern: String },

    /// JSON serialization error
    #[error("Failed to serialize context: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Runtime initialization error
    #[error("Failed to initialize JavaScript runtime: {0}")]
    RuntimeInitError(String),

    /// Invalid script directory
    #[error("Invalid script directory: {0}")]
    InvalidScriptDirectory(PathBuf),

    /// Internal error - unexpected state
    #[error("Internal script error: {0}")]
    InternalError(String),
}

/// Result type for script operations
pub type ScriptResult<T> = Result<T, ScriptError>;

impl ScriptError {
    /// Create a compilation error
    pub fn compilation(path: PathBuf, message: impl Into<String>) -> Self {
        Self::CompilationError {
            path,
            message: message.into(),
        }
    }

    /// Create an execution error
    pub fn execution(script: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ExecutionError {
            script: script.into(),
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(script: impl Into<String>, timeout_ms: u64) -> Self {
        Self::Timeout {
            script: script.into(),
            timeout_ms,
        }
    }

    /// Create an invalid filter error
    pub fn invalid_filter(script: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::InvalidFilter {
            script: script.into(),
            pattern: pattern.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_error_display() {
        let err = ScriptError::InvalidEventName("bogus".to_string());
        assert_eq!(err.to_string(), "Invalid event name: bogus");

        let err = ScriptError::compilation(PathBuf::from("test.js"), "syntax error");
        assert_eq!(
            err.to_string(),
            "Failed to compile script test.js: syntax error"
        );

        let err = ScriptError::timeout("test.js", 5000);
        assert_eq!(err.to_string(), "Script timeout after 5000ms in test.js");
    }

    #[test]
    fn test_error_helpers() {
        let err = ScriptError::execution("test.js", "undefined is not a function");
        match err {
            ScriptError::ExecutionError { script, message } => {
                assert_eq!(script, "test.js");
                assert_eq!(message, "undefined is not a function");
            }
            _ => panic!("Expected ExecutionError"),
        }
    }
}
