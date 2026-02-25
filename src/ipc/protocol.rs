/// IPC protocol messages exchanged between ggg (TUI) and ggg-dnd (GUI).
///
/// Wire format: each message is a single JSON line terminated by `\n`.
use serde::{Deserialize, Serialize};

/// Default Named Pipe name
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\ggg-dnd";

/// Prefix for fallback pipe names (appended with `-{pid}`)
pub const PIPE_NAME_PREFIX: &str = r"\\.\pipe\ggg-dnd-";

/// Request sent from GUI (ggg-dnd) to TUI (ggg)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    /// Add a URL to the current folder's download queue
    #[serde(rename = "add_url")]
    AddUrl { url: String },

    /// Connection health check
    #[serde(rename = "ping")]
    Ping,
}

/// Response sent from TUI (ggg) to GUI (ggg-dnd)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    /// URL was accepted and queued
    #[serde(rename = "ok")]
    Ok { message: String },

    /// Request was rejected or an error occurred
    #[serde(rename = "error")]
    Error { message: String },

    /// Pong reply to a ping request
    #[serde(rename = "pong")]
    Pong,
}
