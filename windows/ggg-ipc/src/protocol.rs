//! IPC protocol messages exchanged between ggg (TUI) and its IPC clients.
//!
//! Wire format: each message is a single JSON line terminated by `\n`.
use serde::{Deserialize, Serialize};

/// Default Named Pipe name
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\ggg-dnd";

/// Prefix for fallback pipe names (appended with `-{pid}`)
pub const PIPE_NAME_PREFIX: &str = r"\\.\pipe\ggg-dnd-";

/// Request sent from a client (ggg-dnd, ggg-bridge) to ggg.
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

/// Response sent from ggg back to a client.
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
