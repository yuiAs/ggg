//! Script request sender utilities
//!
//! This module provides helper functions for sending script requests and waiting for responses.
//! It abstracts the common pattern of request/response communication between the download
//! manager and the script executor.

use std::sync::mpsc;

use super::message::ScriptRequest;
use super::error::ScriptResult;

/// Send a script request with context modification and wait for response
///
/// This function handles the common pattern of:
/// 1. Creating a response channel
/// 2. Sending a script request with the response sender
/// 3. Waiting for the response in a blocking task
/// 4. Returning the modified context and script result
///
/// # Type Parameters
///
/// * `C` - The context type that will be modified by the script
///
/// # Arguments
///
/// * `sender` - The sync channel sender for script requests
/// * `request_builder` - A closure that builds the ScriptRequest given a response channel
///
/// # Returns
///
/// Returns the modified context and script result, or an error string if communication fails
pub async fn send_script_request_with_context<C>(
    sender: &mpsc::Sender<ScriptRequest>,
    request_builder: impl FnOnce(mpsc::Sender<(C, ScriptResult<()>)>) -> ScriptRequest + Send + 'static,
) -> Result<(C, ScriptResult<()>), String>
where
    C: Send + 'static,
{
    let (response_tx, response_rx) = mpsc::channel();
    let sender_clone = sender.clone();

    tokio::task::spawn_blocking(move || {
        let request = request_builder(response_tx);
        sender_clone.send(request).map_err(|e| format!("Send error: {:?}", e))?;
        response_rx.recv().map_err(|e| format!("Recv error: {:?}", e))
    }).await
    .map_err(|e| format!("Blocking task error: {}", e))?
}

/// Send a script request without context modification and wait for response
///
/// This is a simpler version of `send_script_request_with_context` for cases where
/// the script doesn't need to modify any context (e.g., error hooks, completion hooks).
///
/// # Arguments
///
/// * `sender` - The sync channel sender for script requests
/// * `request_builder` - A closure that builds the ScriptRequest given a response channel
///
/// # Returns
///
/// Returns only the script result, or an error string if communication fails
pub async fn send_script_request_no_context(
    sender: &mpsc::Sender<ScriptRequest>,
    request_builder: impl FnOnce(mpsc::Sender<ScriptResult<()>>) -> ScriptRequest + Send + 'static,
) -> Result<ScriptResult<()>, String>
{
    let (response_tx, response_rx) = mpsc::channel();
    let sender_clone = sender.clone();

    tokio::task::spawn_blocking(move || {
        let request = request_builder(response_tx);
        sender_clone.send(request).map_err(|e| format!("Send error: {:?}", e))?;
        response_rx.recv().map_err(|e| format!("Recv error: {:?}", e))
    }).await
    .map_err(|e| format!("Blocking task error: {}", e))?
}
