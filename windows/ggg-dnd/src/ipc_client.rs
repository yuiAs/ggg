/// Named Pipe client for communicating with the ggg TUI application.
///
/// Thin wrapper around `ggg-ipc` that adapts the shared client to ggg-dnd's
/// `SharedState` model and runs a connection monitor loop in a background thread.
use crate::SharedState;
use std::time::Duration;

/// Send a URL to the TUI application via Named Pipe.
pub fn send_url(state: &SharedState, url: &str) -> Result<String, String> {
    let pipe_name = {
        let s = state.lock().unwrap();
        s.pipe_name.clone()
    };
    ggg_ipc::send_url(&pipe_name, url).map_err(|e| e.to_string())
}

/// Background thread: periodically ping the TUI to check connection status.
pub fn connection_monitor(state: SharedState) {
    loop {
        let pipe_name = {
            let s = state.lock().unwrap();
            s.pipe_name.clone()
        };

        let connected = ggg_ipc::ping(&pipe_name).is_ok();

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
