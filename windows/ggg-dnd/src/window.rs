/// Win32 window creation and message loop for ggg-dnd GUI.
///
/// Creates a small, always-on-top window that displays connection status
/// and last sent URL. The entire window area accepts drag & drop.
use crate::drop_target::DropTarget;
use crate::SharedState;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Ole::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const WINDOW_WIDTH: i32 = 360;
const WINDOW_HEIGHT: i32 = 160;
const WINDOW_CLASS: PCWSTR = w!("GggDndWindow");
const WINDOW_TITLE: PCWSTR = w!("ggg-dnd");

/// Stores the main HWND for cross-thread access (IPC monitor thread).
/// Stored as isize (pointer cast) for AtomicIsize compatibility.
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

/// Cached UI font family name, resolved once on first paint.
/// Prefers "Segoe UI Variable" (Windows 11+), falls back to "Segoe UI".
static UI_FONT_FAMILY: OnceLock<&str> = OnceLock::new();

/// Get the main window handle (called from IPC monitor thread)
pub fn get_main_hwnd() -> isize {
    MAIN_HWND.load(Ordering::Relaxed)
}

/// Run the Win32 GUI. Blocks until the window is closed.
pub fn run(state: SharedState) -> Result<()> {
    unsafe {
        // Initialize OLE (includes COM) — required for RegisterDragDrop
        OleInitialize(None)?;

        let hinstance = GetModuleHandleW(None)?;

        // Register window class
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        // Calculate centered position on primary monitor
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - WINDOW_WIDTH) / 2;
        let y = (screen_h - WINDOW_HEIGHT) / 2;

        // Store state pointer in window user data
        let state_ptr = Box::into_raw(Box::new(state.clone()));

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_ACCEPTFILES,
            WINDOW_CLASS,
            WINDOW_TITLE,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            None,
            None,
            Some(hinstance.into()),
            Some(state_ptr as *const _),
        )?;

        MAIN_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

        // Register OLE drop target on the window
        let drop_target = DropTarget::new(state.clone());
        drop_target.set_hwnd(hwnd);
        let drop_target_interface: IDropTarget = drop_target.into();
        RegisterDragDrop(hwnd, &drop_target_interface)?;

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        // Message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        RevokeDragDrop(hwnd)?;
        OleUninitialize();

        // Clean up the leaked state pointer
        let _ = Box::from_raw(state_ptr);

        Ok(())
    }
}

/// Window procedure
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            // Store the SharedState pointer in window user data
            let create_struct = &*(lparam.0 as *const CREATESTRUCTW);
            if !create_struct.lpCreateParams.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create_struct.lpCreateParams as isize);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            // Ctrl+V: read clipboard and send URLs via IPC
            let vk = (wparam.0 & 0xFFFF) as u16;
            let ctrl_held = GetKeyState(VK_CONTROL.0 as i32) < 0;
            if ctrl_held && vk == VK_V.0 {
                handle_paste(hwnd);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Paint the window contents: connection status, last URL, drop hint.
unsafe fn paint(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);

    // Fill background
    let bg_brush = CreateSolidBrush(COLORREF(0x00FFFFFF)); // White
    FillRect(hdc, &rect, bg_brush);
    let _ = DeleteObject(bg_brush.into());

    // Get state from window user data
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SharedState;
    if state_ptr.is_null() {
        let _ = EndPaint(hwnd, &ps);
        return;
    }
    let state = &*state_ptr;
    let s = state.lock().unwrap();

    // Set text properties
    SetBkMode(hdc, TRANSPARENT);

    // Select UI font (Segoe UI Variable on Win11+, Segoe UI fallback)
    let font_name = resolve_ui_font(hdc);
    let hfont = create_ui_font(font_name, -14);
    let old_font = SelectObject(hdc, hfont.into());

    // Line 1: Connection status
    let (status_text, status_color) = if s.connected {
        ("✅ Connected", COLORREF(0x0000AA00)) // Green
    } else {
        ("⛔️ Disconnected", COLORREF(0x000000CC)) // Red
    };
    SetTextColor(hdc, status_color);
    let status_wide = to_wide(status_text);
    let mut status_rect = RECT {
        left: rect.left + 12,
        top: rect.top + 12,
        right: rect.right - 12,
        bottom: rect.top + 12 + 14 + 4,
    };
    DrawTextW(hdc, &mut status_wide.clone(), &mut status_rect, DT_LEFT | DT_SINGLELINE);

    // Line 2: Pipe name
    SetTextColor(hdc, COLORREF(0x00808080)); // Gray
    let pipe_label = format!("⛓️‍💥 {}", s.pipe_name);
    let mut pipe_wide = to_wide(&pipe_label);
    let mut pipe_rect = RECT {
        left: rect.left + 12,
        top: status_rect.bottom,
        right: rect.right - 12,
        bottom: status_rect.bottom + 14 + 4,
    };
    DrawTextW(hdc, &mut pipe_wide, &mut pipe_rect, DT_LEFT | DT_SINGLELINE);

    // Line 3: Last URL (or drop hint)
    SetTextColor(hdc, COLORREF(0x00333333)); // Dark gray
    let url_text = match &s.last_url {
        Some(url) => format!("{}", truncate_display(url, 45)),
        None => "💡 Drop a URL here from your browser".to_string(),
    };
    let mut url_wide = to_wide(&url_text);
    let mut url_rect = RECT {
        left: rect.left + 12,
        top: pipe_rect.bottom,
        right: rect.right - 12,
        bottom: pipe_rect.bottom + 14 + 4,
    };
    DrawTextW(hdc, &mut url_wide, &mut url_rect, DT_LEFT | DT_SINGLELINE);

    // Line 4: Status message
    SetTextColor(hdc, COLORREF(0x00666666)); // Medium gray
    let mut msg_wide = to_wide(&s.status_message);
    let mut msg_rect = RECT {
        left: rect.left + 12,
        top: url_rect.bottom,
        right: rect.right - 12,
        bottom: url_rect.bottom + 14 + 4,
    };
    DrawTextW(hdc, &mut msg_wide, &mut msg_rect, DT_LEFT | DT_SINGLELINE);

    // Restore original font and clean up
    SelectObject(hdc, old_font);
    let _ = DeleteObject(hfont.into());

    drop(s); // Release lock before EndPaint
    let _ = EndPaint(hwnd, &ps);
}

/// Handle Ctrl+V paste: read clipboard text and send URLs via IPC.
unsafe fn handle_paste(hwnd: HWND) {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SharedState;
    if state_ptr.is_null() {
        return;
    }
    let state = &*state_ptr;

    // Check connection before opening clipboard
    {
        let s = state.lock().unwrap();
        if !s.connected {
            return;
        }
    }

    // Read CF_UNICODETEXT from clipboard
    let text = match read_clipboard_text(hwnd) {
        Some(t) if !t.is_empty() => t,
        _ => return,
    };

    // Parse lines: each line that starts with http(s):// is a URL candidate
    let urls: Vec<&str> = text
        .lines()
        .map(|line| line.trim())
        .filter(|line| line.starts_with("http://") || line.starts_with("https://"))
        .collect();

    if urls.is_empty() {
        let mut s = state.lock().unwrap();
        s.status_message = "⚠️ No URLs in clipboard".to_string();
        let _ = InvalidateRect(Some(hwnd), None, true);
        return;
    }

    let mut success_count = 0;
    let mut last_err: Option<String> = None;

    for url in &urls {
        match crate::ipc_client::send_url(state, url) {
            Ok(_) => success_count += 1,
            Err(e) => last_err = Some(e),
        }
    }

    // Update state with result
    {
        let mut s = state.lock().unwrap();
        if success_count > 0 {
            s.last_url = Some(urls.last().unwrap().to_string());
        }
        if let Some(err) = last_err {
            s.status_message = format!("📋 ✅ {} ⚠️ {}", success_count, err);
        } else if success_count == 1 {
            s.status_message = "📋 ✅".to_string();
        } else {
            s.status_message = format!("📋 ✅ {}", success_count);
        }
    }

    let _ = InvalidateRect(Some(hwnd), None, true);
}

/// Read text from the clipboard (CF_UNICODETEXT).
unsafe fn read_clipboard_text(hwnd: HWND) -> Option<String> {
    if !OpenClipboard(Some(hwnd)).is_ok() {
        return None;
    }

    let result = (|| {
        let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
        let ptr = GlobalLock(HGLOBAL(handle.0)) as *const u16;
        if ptr.is_null() {
            return None;
        }
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let text = String::from_utf16_lossy(slice);
        let _ = GlobalUnlock(HGLOBAL(handle.0));
        Some(text)
    })();

    let _ = CloseClipboard();
    result
}

/// Resolve the preferred UI font family.
/// Checks for "Segoe UI Variable" (Windows 11+) via font enumeration,
/// falls back to "Segoe UI" (Windows Vista+).
unsafe fn resolve_ui_font(hdc: HDC) -> &'static str {
    UI_FONT_FAMILY.get_or_init(|| {
        if font_family_exists(hdc, "Segoe UI Variable") {
            "Segoe UI Variable"
        } else {
            "Segoe UI"
        }
    })
}

/// Check if a font family is installed on the system.
unsafe fn font_family_exists(hdc: HDC, family: &str) -> bool {
    unsafe extern "system" fn callback(
        _lf: *const LOGFONTW,
        _tm: *const TEXTMETRICW,
        _font_type: u32,
        lparam: LPARAM,
    ) -> i32 {
        *(lparam.0 as *mut bool) = true;
        0 // Stop enumeration after first match
    }

    let mut lf = LOGFONTW::default();
    let wide: Vec<u16> = family.encode_utf16().collect();
    let len = wide.len().min(31);
    lf.lfFaceName[..len].copy_from_slice(&wide[..len]);

    let mut found = false;
    EnumFontFamiliesExW(
        hdc,
        &lf,
        Some(callback),
        LPARAM(&mut found as *mut bool as isize),
        0,
    );
    found
}

/// Create a font handle with the specified family and pixel height.
unsafe fn create_ui_font(family: &str, height: i32) -> HFONT {
    let mut lf = LOGFONTW::default();
    lf.lfHeight = height;
    lf.lfWeight = 400; // FW_NORMAL
    lf.lfQuality = CLEARTYPE_QUALITY;
    let wide: Vec<u16> = family.encode_utf16().collect();
    let len = wide.len().min(31);
    lf.lfFaceName[..len].copy_from_slice(&wide[..len]);
    CreateFontIndirectW(&lf)
}

/// Convert a Rust string to a null-terminated wide string buffer.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Truncate a string for display purposes, adding "..." if truncated.
fn truncate_display(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
