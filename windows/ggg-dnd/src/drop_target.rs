/// OLE IDropTarget implementation for receiving browser URL drag & drop.
///
/// Browsers send dragged URLs as CF_UNICODETEXT through OLE D&D.
/// This module implements the COM IDropTarget interface to extract the URL
/// and forward it to the TUI via Named Pipe.
use crate::SharedState;
use std::sync::Mutex;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;

/// Implements IDropTarget for the main window.
#[implement(IDropTarget)]
pub struct DropTarget {
    state: SharedState,
    hwnd: Mutex<HWND>,
}

impl DropTarget {
    pub fn new(state: SharedState) -> Self {
        Self {
            state,
            hwnd: Mutex::new(HWND::default()),
        }
    }

    pub fn set_hwnd(&self, hwnd: HWND) {
        *self.hwnd.lock().unwrap() = hwnd;
    }

    /// Extract URL text from OLE IDataObject.
    /// Attempts CF_UNICODETEXT, which is the standard format
    /// browsers use when dragging URLs.
    pub fn extract_url(data_object: &IDataObject) -> Option<String> {
        let format = FORMATETC {
            cfFormat: CF_UNICODETEXT.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };

        unsafe {
            let medium = data_object.GetData(&format).ok()?;
            if medium.tymed != TYMED_HGLOBAL.0 as u32 {
                ReleaseStgMedium(&medium as *const _ as *mut _);
                return None;
            }

            let hglobal = medium.u.hGlobal;
            let ptr = GlobalLock(hglobal) as *const u16;
            if ptr.is_null() {
                ReleaseStgMedium(&medium as *const _ as *mut _);
                return None;
            }

            // Find the null terminator and build a string
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            let text = String::from_utf16_lossy(slice).trim().to_string();

            let _ = GlobalUnlock(hglobal);
            ReleaseStgMedium(&medium as *const _ as *mut _);

            // Validate as URL
            if text.starts_with("http://") || text.starts_with("https://") {
                Some(text)
            } else {
                None
            }
        }
    }
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> Result<()> {
        let connected = {
            let s = self.state.lock().unwrap();
            s.connected
        };

        unsafe {
            let has_url = pdataobj.ok()
                .map(|obj| DropTarget::extract_url(obj).is_some())
                .unwrap_or(false);
            if connected && has_url {
                *pdweffect = DROPEFFECT_COPY;
            } else {
                *pdweffect = DROPEFFECT_NONE;
            }
        }
        Ok(())
    }

    fn DragOver(
        &self,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> Result<()> {
        let connected = {
            let s = self.state.lock().unwrap();
            s.connected
        };

        unsafe {
            *pdweffect = if connected {
                DROPEFFECT_COPY
            } else {
                DROPEFFECT_NONE
            };
        }
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> Result<()> {
        unsafe {
            *pdweffect = DROPEFFECT_NONE;
        }

        let data_obj = match pdataobj.ok() {
            Ok(obj) => obj,
            Err(_) => return Ok(()),
        };
        let url = match DropTarget::extract_url(data_obj) {
            Some(url) => url,
            None => return Ok(()),
        };

        // Send URL to TUI via Named Pipe
        match crate::ipc_client::send_url(&self.state, &url) {
            Ok(_msg) => {
                let mut s = self.state.lock().unwrap();
                s.last_url = Some(url);
                s.status_message = "🎉".to_string();
            }
            Err(e) => {
                let mut s = self.state.lock().unwrap();
                s.status_message = format!("🔥 {}", e);
            }
        }

        // Request repaint
        unsafe {
            let hwnd = *self.hwnd.lock().unwrap();
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, true);
        }

        Ok(())
    }
}
