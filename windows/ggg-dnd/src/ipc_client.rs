/// Named Pipe client for communicating with the ggg TUI application.
///
/// Maintains a persistent connection and supports sending URLs.
/// Runs a connection monitor loop in a background thread.
use crate::SharedState;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum IpcRequest {
    #[serde(rename = "add_url")]
    AddUrl { url: String },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum IpcResponse {
    #[serde(rename = "ok")]
    Ok { message: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "pong")]
    Pong,
}

/// Send a URL to the TUI application via Named Pipe.
///
/// Opens a transient connection, sends the request, reads the response,
/// and closes. This avoids keeping a pipe handle open long-term.
pub fn send_url(state: &SharedState, url: &str) -> Result<String, String> {
    let pipe_name = {
        let s = state.lock().unwrap();
        s.pipe_name.clone()
    };

    let pipe = open_pipe(&pipe_name).map_err(|e| format!("Connection failed: {}", e))?;
    let mut reader = BufReader::new(&pipe);
    let mut writer = &pipe;

    // Send request
    let request = IpcRequest::AddUrl {
        url: url.to_string(),
    };
    let mut json = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    json.push('\n');
    writer
        .write_all(json.as_bytes())
        .map_err(|e| format!("Write failed: {}", e))?;
    writer.flush().map_err(|e| format!("Flush failed: {}", e))?;

    // Read response
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("Read failed: {}", e))?;

    match serde_json::from_str::<IpcResponse>(&line) {
        Ok(IpcResponse::Ok { message }) => Ok(message),
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(IpcResponse::Pong) => Ok("pong".to_string()),
        Err(e) => Err(format!("Invalid response: {}", e)),
    }
}

/// Background thread: periodically ping the TUI to check connection status.
pub fn connection_monitor(state: SharedState) {
    loop {
        let pipe_name = {
            let s = state.lock().unwrap();
            s.pipe_name.clone()
        };

        let connected = match open_pipe(&pipe_name) {
            Ok(pipe) => {
                let mut reader = BufReader::new(&pipe);
                let mut writer = &pipe;
                let request = IpcRequest::Ping;
                let mut json = serde_json::to_string(&request).unwrap_or_default();
                json.push('\n');
                if writer.write_all(json.as_bytes()).is_ok() && writer.flush().is_ok() {
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_ok() {
                        matches!(
                            serde_json::from_str::<IpcResponse>(&line),
                            Ok(IpcResponse::Pong)
                        )
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Err(_) => false,
        };

        // Update shared state
        {
            let mut s = state.lock().unwrap();
            let was_connected = s.connected;
            s.connected = connected;
            if connected && !was_connected {
                s.status_message = "Connected".to_string();
            } else if !connected {
                s.status_message = "Disconnected".to_string();
            }
        }

        // Request window repaint after state change
        let hwnd_val = crate::window::get_main_hwnd();
        if hwnd_val != 0 {
            unsafe {
                let hwnd = windows::Win32::Foundation::HWND(hwnd_val as *mut _);
                let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, true);
            }
        }

        std::thread::sleep(Duration::from_secs(3));
    }
}

/// Open a Named Pipe for reading and writing using standard file I/O.
fn open_pipe(pipe_name: &str) -> std::io::Result<std::fs::File> {
    // Named Pipes appear as files on Windows; open with read+write
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
}
