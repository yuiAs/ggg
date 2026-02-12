/// Message passing infrastructure for script execution across thread boundaries
///
/// Since rustyscript::Runtime is !Send, we cannot share ScriptManager across
/// threads. Instead, we run a dedicated script executor thread and communicate
/// via channels.

use super::error::ScriptResult;
use super::events::*;
use std::sync::mpsc;

/// Request to execute a script hook
///
/// Sent from download tasks to the script executor thread
pub enum ScriptRequest {
    /// Execute beforeRequest hook
    ///
    /// Modifies context in-place, returns modified context
    BeforeRequest {
        ctx: BeforeRequestContext,
        effective_script_files: std::collections::HashMap<String, bool>,
        response: mpsc::Sender<(BeforeRequestContext, ScriptResult<()>)>,
    },

    /// Execute headersReceived hook
    ///
    /// Read-only inspection of server response
    HeadersReceived {
        ctx: HeadersReceivedContext,
        effective_script_files: std::collections::HashMap<String, bool>,
        response: mpsc::Sender<ScriptResult<()>>,
    },

    /// Execute completed hook
    ///
    /// Modifies context in-place for file operations, returns modified context
    Completed {
        ctx: CompletedContext,
        effective_script_files: std::collections::HashMap<String, bool>,
        response: mpsc::Sender<(CompletedContext, ScriptResult<()>)>,
    },

    /// Execute error hook (fire-and-forget)
    ///
    /// No response expected
    Error {
        ctx: ErrorContext,
        effective_script_files: std::collections::HashMap<String, bool>,
    },

    /// Execute progress hook (fire-and-forget)
    ///
    /// No response expected
    Progress {
        ctx: ProgressContext,
        effective_script_files: std::collections::HashMap<String, bool>,
    },

    /// Execute authRequired hook
    ///
    /// Modifies context in-place for authentication, returns modified context
    AuthRequired {
        ctx: AuthRequiredContext,
        effective_script_files: std::collections::HashMap<String, bool>,
        response: mpsc::Sender<(AuthRequiredContext, ScriptResult<()>)>,
    },

    /// Reload all scripts from disk
    ///
    /// Returns success/failure result
    Reload {
        response: mpsc::Sender<ScriptResult<()>>,
    },
}

impl std::fmt::Debug for ScriptRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeRequest { .. } => write!(f, "ScriptRequest::BeforeRequest"),
            Self::HeadersReceived { .. } => write!(f, "ScriptRequest::HeadersReceived"),
            Self::Completed { .. } => write!(f, "ScriptRequest::Completed"),
            Self::Error { .. } => write!(f, "ScriptRequest::Error"),
            Self::Progress { .. } => write!(f, "ScriptRequest::Progress"),
            Self::AuthRequired { .. } => write!(f, "ScriptRequest::AuthRequired"),
            Self::Reload { .. } => write!(f, "ScriptRequest::Reload"),
        }
    }
}
