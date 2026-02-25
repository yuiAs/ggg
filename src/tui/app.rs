use super::events::TuiEvent;
use super::state::{DetailsPosition, FocusPane, TuiState, UiMode};
use crate::app::keybindings::KeyAction;
use crate::app::state::AppState;
use crate::download::manager::DownloadManager;
use crate::download::task::DownloadStatus;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::event::{EnableBracketedPaste, DisableBracketedPaste};
use crossterm::ExecutableCommand;
use futures_util::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

/// Maximum input buffer length to prevent overflow
/// URLs can be up to 2048 chars (common browser limit)
const MAX_INPUT_LENGTH: usize = 2048;

/// Main TUI application
pub struct TuiApp {
    pub state: TuiState,
    pub manager: DownloadManager,
    pub should_quit: bool,
    last_update_time: std::time::Instant,
    /// Pending input buffer for URL detection in Normal mode
    /// NOTE: This is a workaround for crossterm not firing Event::Paste on Windows Terminal
    /// See: https://github.com/crossterm-rs/crossterm/issues/737
    ///      https://github.com/helix-editor/helix/discussions/9243
    pending_url_input: String,
    /// Last character input time for detecting paste-like rapid input
    last_char_input_time: std::time::Instant,
}

impl TuiApp {
    pub fn new(
        app_state: AppState,
        manager: DownloadManager,
        keybindings: &crate::app::keybindings::KeybindingsConfig,
    ) -> Self {
        Self {
            state: TuiState::new(app_state, keybindings),
            manager,
            should_quit: false,
            last_update_time: std::time::Instant::now(),
            pending_url_input: String::new(),
            last_char_input_time: std::time::Instant::now(),
        }
    }

    /// Handle a TUI event
    pub async fn handle_event(&mut self, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Tick => {
                // Debounce UI updates: only update every 250ms to reduce CPU usage
                let now = std::time::Instant::now();
                if now.duration_since(self.last_update_time) >= Duration::from_millis(250) {
                    self.state.update_downloads(&self.manager).await;
                    self.last_update_time = now;
                    self.state.mark_dirty();  // Mark for redraw after data update
                }

                // Check for pending URL input (drag & drop detection)
                // NOTE: This is a workaround for crossterm not firing Event::Paste on Windows Terminal
                // If input has stopped for 300ms, check if it's a valid URL
                if !self.pending_url_input.is_empty()
                    && now.duration_since(self.last_char_input_time) >= Duration::from_millis(300)
                    && self.state.ui_mode == UiMode::Normal
                {
                    let pending = self.pending_url_input.clone();
                    self.pending_url_input.clear();

                    if Self::is_valid_download_url(&pending) {
                        tracing::info!("Auto-detected URL from rapid input (D&D): {}", pending);
                        if let Err(e) = self.add_download_from_paste(&pending).await {
                            tracing::error!("Failed to add download from auto-detected URL: {}", e);
                        }
                        self.state.mark_dirty();  // Mark for redraw after adding download
                    } else {
                        tracing::debug!("Ignored non-URL rapid input: {}", pending);
                    }
                }
            }
            TuiEvent::Input(input) => {
                self.handle_input(input).await?;
                // Force update after user input for immediate feedback
                self.state.update_downloads(&self.manager).await;
                self.last_update_time = std::time::Instant::now();
                self.state.mark_dirty();  // Mark for redraw after input handling
            }
            #[cfg(windows)]
            TuiEvent::IpcUrl(url) => {
                tracing::info!("IPC URL received from ggg-dnd: {}", url);
                if let Err(e) = self.add_download_from_paste(&url).await {
                    tracing::error!("Failed to add download from IPC: {}", e);
                }
                self.state.update_downloads(&self.manager).await;
                self.state.mark_dirty();
            }
        }
        Ok(())
    }

    /// Handle keyboard input
    async fn handle_input(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(KeyEvent { code, modifiers, kind, .. }) => {
                // Only process key press events, ignore release and repeat
                if kind != KeyEventKind::Press {
                    return Ok(());
                }

                match self.state.ui_mode {
                    UiMode::Normal => self.handle_normal_mode(code, modifiers).await?,
                    UiMode::AddDownload | UiMode::EditingField => self.handle_input_mode(code, modifiers).await?,
                    UiMode::DownloadPreview => self.handle_download_preview_mode(code).await?,
                    UiMode::Search => self.handle_search_mode(code).await?,
                    UiMode::Help => self.handle_help_mode(code),
                    UiMode::Settings => self.handle_settings_mode(code).await?,
                    UiMode::FolderEdit => self.handle_folder_edit_mode(code, modifiers).await?,
                    UiMode::ChangeFolder => self.handle_change_folder_mode(code, modifiers).await?,
                    UiMode::SwitchFolder => self.handle_switch_folder_mode(code).await?,
                    UiMode::ConfirmDelete => self.handle_confirm_delete_mode(code).await?,
                    UiMode::ContextMenu => self.handle_context_menu_mode(code).await?,
                    UiMode::FolderContextMenu => self.handle_folder_context_menu_mode(code).await?,
                }
            }
            Event::Paste(text) => {
                // Handle paste events based on current mode
                let trimmed = text.trim();
                tracing::debug!("Paste event received in mode {:?}: {} chars", self.state.ui_mode, trimmed.len());

                match self.state.ui_mode {
                    // AddDownload mode: always add to input buffer
                    UiMode::AddDownload => {
                        // Prevent buffer overflow by limiting total length
                        let available_space = MAX_INPUT_LENGTH.saturating_sub(self.state.input_buffer.len());
                        if available_space > 0 {
                            // Use char-based slicing to avoid breaking UTF-8 sequences
                            let text_to_add: String = text.chars().take(available_space).collect();
                            self.state.input_buffer.push_str(&text_to_add);
                            self.state.mark_dirty();  // Mark for redraw after paste
                        }
                    }

                    // Settings screens: ignore paste for now
                    // Future: may add specific handling for settings input fields
                    UiMode::Settings | UiMode::FolderEdit => {}

                    // All other modes (except settings): try to add as download if valid URL
                    _ => {
                        if Self::is_valid_download_url(trimmed) {
                            tracing::info!("Valid download URL detected in mode {:?}, adding to queue", self.state.ui_mode);
                            if let Err(e) = self.add_download_from_paste(trimmed).await {
                                tracing::error!("Failed to add download from paste: {}", e);
                            }
                        } else {
                            tracing::debug!("Paste ignored in mode {:?}: not a valid download URL", self.state.ui_mode);
                        }
                        // Future: may add to input buffer for specific modes, e.g.:
                        // UiMode::Search | UiMode::ChangeFolder => {
                        //     self.state.input_buffer.push_str(&text_to_add);
                        // }
                    }
                }
            }
            Event::Resize(_width, _height) => {
                // Handle terminal resize events
                // Adjust selection to ensure it's within valid bounds
                let filtered_count = self.state.filtered_downloads().len();
                if filtered_count > 0 && self.state.selected_index >= filtered_count {
                    self.state.selected_index = filtered_count - 1;
                    self.state.table_state_mut().select(Some(self.state.selected_index));
                }
                // Adjust scroll offset if needed
                if self.state.scroll_offset >= filtered_count {
                    self.state.scroll_offset = filtered_count.saturating_sub(1);
                }
                tracing::debug!("Terminal resized to {}x{}", _width, _height);
            }
            Event::Mouse(mouse_event) => {
                self.handle_mouse_event(mouse_event).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle mouse events
    async fn handle_mouse_event(&mut self, event: MouseEvent) -> Result<()> {
        let MouseEvent { kind, column, row, .. } = event;

        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_left_click(column, row).await?;
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.handle_right_click(column, row).await?;
            }
            MouseEventKind::ScrollUp => {
                self.handle_scroll(-3);
            }
            MouseEventKind::ScrollDown => {
                self.handle_scroll(3);
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle left click at position
    async fn handle_left_click(&mut self, x: u16, y: u16) -> Result<()> {
        // Only handle clicks in Normal mode or when clicking context menu items
        match self.state.ui_mode {
            UiMode::Normal => {
                self.handle_normal_mode_left_click(x, y).await?;
            }
            UiMode::ContextMenu => {
                // Check for menu item click
                if let Some(action_idx) = self.hit_test_context_menu_item(x, y) {
                    self.state.context_menu_index = action_idx;
                    if let Some(action) = self.state.get_selected_menu_action() {
                        self.execute_menu_action(action).await?;
                    }
                } else {
                    // Click outside menu cancels it
                    self.state.ui_mode = UiMode::Normal;
                    self.state.reset_context_menu();
                }
            }
            UiMode::FolderContextMenu => {
                // Check for menu item click
                if let Some(action_idx) = self.hit_test_folder_context_menu_item(x, y) {
                    self.state.folder_context_menu_index = action_idx;
                    let is_completed = self.state.is_viewing_completed_node();
                    if let Some(action) = self.state.get_selected_folder_menu_action(is_completed) {
                        self.execute_folder_menu_action(action).await?;
                    }
                } else {
                    // Click outside menu cancels it
                    self.state.ui_mode = UiMode::Normal;
                    self.state.reset_folder_context_menu();
                }
            }
            UiMode::ConfirmDelete => {
                // Handle dialog button clicks
                self.handle_confirm_delete_click(x, y).await?;
            }
            UiMode::DownloadPreview => {
                // Handle preview dialog button clicks
                self.handle_download_preview_click(x, y).await?;
            }
            UiMode::Settings | UiMode::FolderEdit => {
                // Handle settings screen clicks
                self.handle_settings_click(x, y).await?;
            }
            _ => {}
        }

        self.state.mark_dirty();
        Ok(())
    }

    /// Handle left click in normal mode
    async fn handle_normal_mode_left_click(&mut self, x: u16, y: u16) -> Result<()> {
        // Clone regions data to avoid borrow issues
        let (folder_items, download_rows, folder_tree, download_list, details_panel) = {
            let regions = self.state.click_regions.borrow();
            (
                regions.folder_items.clone(),
                regions.download_rows.clone(),
                regions.folder_tree,
                regions.download_list,
                regions.details_panel,
            )
        };

        // Check for folder item click first (more specific)
        for (idx, rect) in &folder_items {
            if Self::point_in_rect(x, y, rect) {
                self.state.focus_pane = FocusPane::FolderTree;
                self.state.tree_selected_index = *idx;
                // Sync current_folder_id with tree selection
                self.state.sync_current_folder_from_tree();
                // Refresh downloads for the new folder
                self.state.update_downloads(&self.manager).await;
                return Ok(());
            }
        }

        // Check for download row click
        for (idx, rect) in &download_rows {
            if Self::point_in_rect(x, y, rect) {
                self.state.focus_pane = FocusPane::DownloadList;
                self.state.selected_index = *idx;
                self.state.table_state_mut().select(Some(*idx));
                return Ok(());
            }
        }

        // Check for pane click (less specific, for focus only)
        if let Some(ref rect) = folder_tree {
            if Self::point_in_rect(x, y, rect) {
                self.state.focus_pane = FocusPane::FolderTree;
                return Ok(());
            }
        }

        if let Some(ref rect) = download_list {
            if Self::point_in_rect(x, y, rect) {
                self.state.focus_pane = FocusPane::DownloadList;
                return Ok(());
            }
        }

        if let Some(ref rect) = details_panel {
            if Self::point_in_rect(x, y, rect) {
                self.state.focus_pane = FocusPane::DetailsPanel;
                return Ok(());
            }
        }

        Ok(())
    }

    /// Check if a point is inside a rectangle
    fn point_in_rect(x: u16, y: u16, rect: &ratatui::layout::Rect) -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    }

    /// Hit test for context menu items
    fn hit_test_context_menu_item(&self, x: u16, y: u16) -> Option<usize> {
        let regions = self.state.click_regions.borrow();
        for (idx, rect) in regions.context_menu_items.iter().enumerate() {
            if Self::point_in_rect(x, y, rect) {
                return Some(idx);
            }
        }
        None
    }

    /// Hit test for folder context menu items
    fn hit_test_folder_context_menu_item(&self, x: u16, y: u16) -> Option<usize> {
        let regions = self.state.click_regions.borrow();
        for (idx, rect) in regions.context_menu_items.iter().enumerate() {
            if Self::point_in_rect(x, y, rect) {
                return Some(idx);
            }
        }
        None
    }

    /// Handle confirm delete dialog click
    async fn handle_confirm_delete_click(&mut self, x: u16, y: u16) -> Result<()> {
        let regions = self.state.click_regions.borrow();
        for (label, rect) in &regions.dialog_buttons {
            if Self::point_in_rect(x, y, rect) {
                let label = label.clone();
                drop(regions);
                if label == "yes" {
                    self.delete_download().await?;
                }
                self.state.ui_mode = UiMode::Normal;
                return Ok(());
            }
        }
        Ok(())
    }

    /// Handle download preview dialog click
    async fn handle_download_preview_click(&mut self, x: u16, y: u16) -> Result<()> {
        let regions = self.state.click_regions.borrow();
        for (label, rect) in &regions.dialog_buttons {
            if Self::point_in_rect(x, y, rect) {
                let label = label.clone();
                drop(regions);
                if label == "confirm" && !self.state.input_buffer.is_empty() {
                    // Confirm and add download (same logic as Enter key)
                    let url = self.state.input_buffer.clone();
                    let config = self.state.app_state.config.read().await;
                    let task = crate::download::task::DownloadTask::new_with_folder(
                        url,
                        self.state.current_folder_id.clone(),
                        &config,
                    );
                    drop(config);
                    self.add_download_with_auto_start(task).await?;
                    self.state.ui_mode = UiMode::Normal;
                    self.state.input_buffer.clear();
                    self.state.preview_info = None;
                } else {
                    // Cancel - return to add download mode
                    self.state.ui_mode = UiMode::AddDownload;
                    self.state.preview_info = None;
                }
                return Ok(());
            }
        }
        Ok(())
    }

    /// Handle settings screen click
    async fn handle_settings_click(&mut self, x: u16, y: u16) -> Result<()> {
        use super::state::SettingsSection;

        // Clone regions data to avoid borrow issues
        let (settings_tabs, settings_folder_items) = {
            let regions = self.state.click_regions.borrow();
            (
                regions.settings_tabs.clone(),
                regions.settings_folder_items.clone(),
            )
        };

        // Check for tab click
        for (idx, rect) in &settings_tabs {
            if Self::point_in_rect(x, y, rect) {
                match idx {
                    0 => self.state.settings_section = SettingsSection::Application,
                    1 => self.state.settings_section = SettingsSection::Folder,
                    _ => {}
                }
                return Ok(());
            }
        }

        // Check for folder item click (only in Folder section)
        if self.state.settings_section == SettingsSection::Folder {
            for (idx, rect) in &settings_folder_items {
                if Self::point_in_rect(x, y, rect) {
                    self.state.settings_folder_index = *idx;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Handle right click at position
    async fn handle_right_click(&mut self, x: u16, y: u16) -> Result<()> {
        // Only handle right clicks in Normal mode
        if self.state.ui_mode != UiMode::Normal {
            return Ok(());
        }

        // Clone regions data to avoid borrow issues
        let (folder_items, download_rows) = {
            let regions = self.state.click_regions.borrow();
            (regions.folder_items.clone(), regions.download_rows.clone())
        };

        // Check for folder item right-click
        for (idx, rect) in &folder_items {
            if Self::point_in_rect(x, y, rect) {
                self.state.focus_pane = FocusPane::FolderTree;
                self.state.tree_selected_index = *idx;
                // Sync current_folder_id with tree selection
                self.state.sync_current_folder_from_tree();
                self.state.reset_folder_context_menu();
                self.state.ui_mode = UiMode::FolderContextMenu;
                self.state.mark_dirty();
                return Ok(());
            }
        }

        // Check for download row right-click
        for (idx, rect) in &download_rows {
            if Self::point_in_rect(x, y, rect) {
                self.state.focus_pane = FocusPane::DownloadList;
                self.state.selected_index = *idx;
                self.state.table_state_mut().select(Some(*idx));
                self.state.reset_context_menu();
                self.state.ui_mode = UiMode::ContextMenu;
                self.state.mark_dirty();
                return Ok(());
            }
        }

        Ok(())
    }

    /// Handle scroll wheel (delta is positive for down, negative for up)
    fn handle_scroll(&mut self, delta: i32) {
        // Only handle scroll in Normal mode
        if self.state.ui_mode != UiMode::Normal {
            return;
        }

        let steps = delta.unsigned_abs() as usize;

        match self.state.focus_pane {
            FocusPane::FolderTree => {
                if delta > 0 {
                    // Scroll down
                    for _ in 0..steps {
                        self.state.move_tree_selection_down();
                    }
                } else {
                    // Scroll up
                    for _ in 0..steps {
                        self.state.move_tree_selection_up();
                    }
                }
            }
            FocusPane::DownloadList => {
                if delta > 0 {
                    // Scroll down
                    for _ in 0..steps {
                        self.state.move_selection_down();
                    }
                } else {
                    // Scroll up
                    for _ in 0..steps {
                        self.state.move_selection_up();
                    }
                }
            }
            FocusPane::DetailsPanel => {
                // Details panel doesn't have scrollable selection
                // Could be used for scrolling logs in the future
            }
        }

        self.state.mark_dirty();
    }

    /// Handle normal mode keys
    /// Uses configurable keybindings from config
    async fn handle_normal_mode(&mut self, key: KeyCode, mods: KeyModifiers) -> Result<()> {
        // Resolve key to action using configurable keybindings
        let action = self.state.keybinding_resolver.resolve(key, mods);

        // Handle actions from the keybinding resolver
        if let Some(action) = action {
            match action {
                // Quit
                KeyAction::Quit => {
                    self.should_quit = true;
                    return Ok(());
                }

                // Undo
                KeyAction::Undo => {
                    self.undo_delete().await?;
                    return Ok(());
                }

                // Navigation
                KeyAction::MoveUp => {
                    match self.state.focus_pane {
                        FocusPane::FolderTree => self.state.move_tree_selection_up(),
                        FocusPane::DownloadList | FocusPane::DetailsPanel => {
                            self.state.move_selection_up()
                        }
                    }
                    return Ok(());
                }
                KeyAction::MoveDown => {
                    match self.state.focus_pane {
                        FocusPane::FolderTree => self.state.move_tree_selection_down(),
                        FocusPane::DownloadList | FocusPane::DetailsPanel => {
                            self.state.move_selection_down()
                        }
                    }
                    return Ok(());
                }
                KeyAction::MoveToTop => {
                    self.state.move_to_top();
                    return Ok(());
                }
                KeyAction::MoveToBottom => {
                    self.state.move_to_bottom();
                    return Ok(());
                }
                KeyAction::PageUp => {
                    for _ in 0..10 {
                        self.state.move_selection_up();
                    }
                    return Ok(());
                }
                KeyAction::PageDown => {
                    for _ in 0..10 {
                        self.state.move_selection_down();
                    }
                    return Ok(());
                }
                KeyAction::FocusNextPane => {
                    self.state.focus_next_pane();
                    return Ok(());
                }
                KeyAction::FocusPrevPane => {
                    self.state.focus_prev_pane();
                    return Ok(());
                }
                KeyAction::FocusLeft => {
                    match self.state.focus_pane {
                        FocusPane::DownloadList | FocusPane::DetailsPanel => {
                            self.state.set_focus(FocusPane::FolderTree);
                        }
                        FocusPane::FolderTree => {}
                    }
                    return Ok(());
                }
                KeyAction::FocusRight => {
                    match self.state.focus_pane {
                        FocusPane::FolderTree => {
                            self.state.set_focus(FocusPane::DownloadList);
                        }
                        FocusPane::DownloadList => {
                            if self.state.details_position != DetailsPosition::Hidden {
                                self.state.set_focus(FocusPane::DetailsPanel);
                            }
                        }
                        FocusPane::DetailsPanel => {}
                    }
                    return Ok(());
                }

                // Selection
                KeyAction::SelectItem => {
                    match self.state.focus_pane {
                        FocusPane::FolderTree => {
                            // Enter on FolderTree = confirm folder selection
                            self.state.sync_current_folder_from_tree();
                        }
                        _ => {
                            // Enter on other panes = view details
                            self.state.show_details = !self.state.show_details;
                        }
                    }
                    return Ok(());
                }
                KeyAction::ToggleSelection => {
                    self.state.toggle_selection();
                    return Ok(());
                }
                KeyAction::SelectAll => {
                    self.state.select_all();
                    return Ok(());
                }
                KeyAction::DeselectAll => {
                    self.state.clear_search();
                    self.state.clear_selections();
                    return Ok(());
                }

                // Actions
                KeyAction::AddDownload => {
                    self.state.ui_mode = UiMode::AddDownload;
                    self.state.input_buffer.clear();
                    return Ok(());
                }
                KeyAction::DeleteDownload => {
                    if !self.state.selected_downloads.is_empty()
                        || self.state.get_selected_download().is_some()
                    {
                        self.state.ui_mode = UiMode::ConfirmDelete;
                    }
                    return Ok(());
                }
                KeyAction::ToggleDownload => {
                    self.toggle_download().await?;
                    return Ok(());
                }
                KeyAction::RetryDownload => {
                    self.retry_download().await?;
                    return Ok(());
                }
                KeyAction::ResumeAll => {
                    let resumed = self
                        .manager
                        .resume_all(
                            self.state.app_state.script_sender.clone(),
                            self.state.app_state.config.clone(),
                        )
                        .await;
                    if resumed > 0 {
                        tracing::info!("Resumed {} downloads", resumed);
                    }
                    return Ok(());
                }
                KeyAction::PauseAll => {
                    let paused = self.manager.pause_all().await;
                    if paused > 0 {
                        tracing::info!("Paused {} downloads", paused);
                    }
                    return Ok(());
                }
                KeyAction::OpenContextMenu => {
                    self.state.reset_context_menu();
                    self.state.ui_mode = UiMode::ContextMenu;
                    return Ok(());
                }
                KeyAction::EditItem => {
                    self.state.ui_mode = UiMode::ChangeFolder;
                    self.state.input_buffer.clear();
                    return Ok(());
                }

                // View
                KeyAction::ToggleDetails => {
                    self.state.show_details = !self.state.show_details;
                    return Ok(());
                }
                KeyAction::OpenSearch => {
                    // Search is only available in the History view
                    if self.state.is_viewing_completed_node() {
                        self.state.ui_mode = UiMode::Search;
                        self.state.input_buffer.clear();
                    }
                    return Ok(());
                }
                KeyAction::OpenHelp => {
                    self.state.ui_mode = UiMode::Help;
                    return Ok(());
                }
                KeyAction::OpenSettings => {
                    self.state.ui_mode = UiMode::Settings;
                    return Ok(());
                }
                KeyAction::SwitchFolder => {
                    self.state.ui_mode = UiMode::SwitchFolder;
                    self.state.folder_picker_index = 0;
                    return Ok(());
                }

                // System
                KeyAction::Refresh => {
                    // Refresh - already happens on tick
                    return Ok(());
                }
            }
        }

        // Handle keys not covered by keybinding resolver
        // (e.g., special behaviors like D for details position toggle)
        match key {
            // Toggle details position (D key cycles: Bottom -> Right -> Hidden)
            KeyCode::Char('D') => {
                self.state.toggle_details_position();
            }

            // URL input detection for drag & drop
            // NOTE: This is a workaround for crossterm not firing Event::Paste on Windows Terminal
            // When paste events work correctly, this code path won't be triggered
            KeyCode::Char(c) => {
                let now = std::time::Instant::now();

                // If this character comes quickly after the last one (< 50ms), treat as paste-like input
                if now.duration_since(self.last_char_input_time) < Duration::from_millis(50) {
                    self.pending_url_input.push(c);
                } else {
                    // New input sequence starts
                    self.pending_url_input.clear();
                    self.pending_url_input.push(c);
                }

                self.last_char_input_time = now;
            }

            _ => {}
        }
        Ok(())
    }

    /// Handle input mode (for Add Download dialog)
    async fn handle_input_mode(&mut self, key: KeyCode, mods: KeyModifiers) -> Result<()> {
        // Handle Ctrl+u first (before Char match)
        if matches!(key, KeyCode::Char('u')) && mods.contains(KeyModifiers::CONTROL) {
            self.state.input_buffer.clear();
            return Ok(());
        }

        match key {
            KeyCode::Char(c) => {
                // Prevent buffer overflow
                if self.state.input_buffer.len() < MAX_INPUT_LENGTH {
                    self.state.input_buffer.push(c);
                }
                // Clear validation error on new input
                self.state.validation_error = None;
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Enter => {
                // Check if editing application setting
                if self.state.is_editing_app_setting {
                    self.save_app_setting_value().await?;
                    self.state.is_editing_app_setting = false;
                } else if !self.state.input_buffer.is_empty() {
                    let url = self.state.input_buffer.clone();

                    // Shift+Enter: Expand URL patterns like [1-10] or [001-010]
                    // Normal Enter: Add URL as-is ([] is valid in URLs)
                    let expand_patterns = mods.contains(KeyModifiers::SHIFT);

                    let urls_to_add = if expand_patterns {
                        let expanded = crate::util::url_expansion::expand_url(&url);
                        if expanded.is_empty() {
                            self.state.validation_error =
                                Some("Invalid URL range pattern".to_string());
                            return Ok(());
                        }
                        expanded
                    } else {
                        vec![url]
                    };

                    // Check if preview should be skipped
                    let skip_preview = {
                        let config = self.state.app_state.config.read().await;
                        config.general.skip_download_preview
                    };

                    // For multiple URLs, always skip individual previews
                    let is_batch = urls_to_add.len() > 1;

                    if skip_preview || is_batch {
                        // Add downloads directly without preview
                        // Create all tasks first while holding the config lock
                        let tasks: Vec<_> = {
                            let config = self.state.app_state.config.read().await;
                            let folder_id = self.state.current_folder_id.clone();
                            urls_to_add
                                .iter()
                                .map(|url| {
                                    crate::download::task::DownloadTask::new_with_folder(
                                        url.clone(),
                                        folder_id.clone(),
                                        &config,
                                    )
                                })
                                .collect()
                        };

                        // Now add all tasks (config lock is released)
                        for task in tasks {
                            self.add_download_with_auto_start(task).await?;
                        }

                        if is_batch {
                            tracing::info!("Added {} downloads from URL pattern", urls_to_add.len());
                        }

                        self.state.ui_mode = UiMode::Normal;
                        self.state.input_buffer.clear();
                    } else {
                        // Single URL with preview
                        let single_url = urls_to_add.into_iter().next().unwrap();
                        match self.fetch_download_info(&single_url).await {
                            Ok(info) => {
                                self.state.preview_info = Some(info);
                                self.state.ui_mode = UiMode::DownloadPreview;
                                // Keep input_buffer for preview dialog
                            }
                            Err(e) => {
                                tracing::error!("Failed to fetch download info: {}", e);
                                // Still show preview with error info
                                self.state.preview_info = None;
                                self.state.ui_mode = UiMode::DownloadPreview;
                            }
                        }
                    }
                } else {
                    self.state.ui_mode = UiMode::Normal;
                    self.state.input_buffer.clear();
                }
            }
            KeyCode::Esc => {
                if self.state.is_editing_app_setting {
                    self.state.is_editing_app_setting = false;
                    self.state.ui_mode = UiMode::Settings;
                } else {
                    self.state.ui_mode = UiMode::Normal;
                }
                self.state.input_buffer.clear();
                // Clear validation error on cancel
                self.state.validation_error = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle search mode
    async fn handle_search_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char(c) => {
                tracing::trace!("Search mode: char '{}' added to buffer", c);
                // Prevent buffer overflow
                if self.state.input_buffer.len() < MAX_INPUT_LENGTH {
                    self.state.input_buffer.push(c);
                    self.state.set_search_query(self.state.input_buffer.clone());
                }
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
                self.state.set_search_query(self.state.input_buffer.clone());
            }
            KeyCode::Enter => {
                // Check if search query is actually a URL
                let query = self.state.input_buffer.trim().to_string();
                if Self::is_valid_download_url(&query) {
                    tracing::info!("Search input detected as URL, adding to download queue: {}", query);
                    if let Err(e) = self.add_download_from_paste(&query).await {
                        tracing::error!("Failed to add download from search: {}", e);
                    }
                    // Clear search and return to normal mode
                    self.state.input_buffer.clear();
                    self.state.clear_search();
                }
                self.state.ui_mode = UiMode::Normal;
            }
            KeyCode::Esc => {
                self.state.ui_mode = UiMode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle help mode
    fn handle_help_mode(&mut self, key: KeyCode) {
        // Only close on Esc or q, not on ? to avoid toggle issues with Shift+/
        if matches!(key, KeyCode::Esc | KeyCode::Char('q')) {
            self.state.ui_mode = UiMode::Normal;
        }
    }

    /// Handle settings mode
    async fn handle_settings_mode(&mut self, key: KeyCode) -> Result<()> {
        use super::state::{ApplicationSettingsField, SettingsSection};

        // Close settings screen
        if matches!(key, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('x')) {
            self.state.reset_settings_state();
            self.state.ui_mode = UiMode::Normal;
            return Ok(());
        }

        // Reload configuration from disk (Shift+R)
        if matches!(key, KeyCode::Char('R')) {
            use crate::ui::commands::{Command, handle_command};

            let command = Command::ReloadConfig;
            let response = handle_command(
                command,
                self.state.app_state.clone(),
                self.manager.clone(),
            )
            .await;

            match response {
                crate::ui::commands::CommandResponse::Success { .. } => {
                    tracing::info!("Configuration reloaded successfully");
                }
                crate::ui::commands::CommandResponse::Error { error } => {
                    tracing::error!("Failed to reload config: {}", error);
                }
            }
            return Ok(());
        }

        // Switch between Application and Folder sections
        if matches!(key, KeyCode::Tab) {
            self.state.settings_section = match self.state.settings_section {
                SettingsSection::Application => SettingsSection::Folder,
                SettingsSection::Folder => SettingsSection::Application,
            };
            return Ok(());
        } else if matches!(key, KeyCode::BackTab) {
            self.state.settings_section = match self.state.settings_section {
                SettingsSection::Application => SettingsSection::Folder,
                SettingsSection::Folder => SettingsSection::Application,
            };
            return Ok(());
        }

        match self.state.settings_section {
            SettingsSection::Application => {
                match key {
                    // Toggle scripts section expand/collapse
                    KeyCode::Char('s') => {
                        self.state.app_scripts_expanded = !self.state.app_scripts_expanded;
                        if !self.state.app_scripts_expanded {
                            self.state.script_files_index = 0;
                        }
                    }

                    // Navigation and actions depend on whether scripts section is expanded
                    KeyCode::Char('j') | KeyCode::Down => {
                        if self.state.app_scripts_expanded {
                            // Navigate script files
                            let config = self.state.app_state.config.read().await;
                            let script_dir = config.scripts.directory.clone();
                            drop(config);

                            let script_count = match std::fs::read_dir(&script_dir) {
                                Ok(entries) => entries
                                    .filter_map(|e| e.ok())
                                    .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("js"))
                                    .count(),
                                Err(_) => 0,
                            };

                            if script_count > 0 {
                                self.state.script_files_index =
                                    (self.state.script_files_index + 1) % script_count;
                            }
                        } else {
                            // Navigate application settings fields
                            let field_count = ApplicationSettingsField::all().len();
                            if field_count > 0 {
                                self.state.app_settings_field_index =
                                    (self.state.app_settings_field_index + 1) % field_count;
                            }
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if self.state.app_scripts_expanded {
                            // Navigate script files
                            let config = self.state.app_state.config.read().await;
                            let script_dir = config.scripts.directory.clone();
                            drop(config);

                            let script_count = match std::fs::read_dir(&script_dir) {
                                Ok(entries) => entries
                                    .filter_map(|e| e.ok())
                                    .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("js"))
                                    .count(),
                                Err(_) => 0,
                            };

                            if script_count > 0 {
                                self.state.script_files_index = if self.state.script_files_index == 0 {
                                    script_count - 1
                                } else {
                                    self.state.script_files_index - 1
                                };
                            }
                        } else {
                            // Navigate application settings fields
                            let field_count = ApplicationSettingsField::all().len();
                            if field_count > 0 {
                                self.state.app_settings_field_index = if self.state.app_settings_field_index == 0 {
                                    field_count - 1
                                } else {
                                    self.state.app_settings_field_index - 1
                                };
                            }
                        }
                    }

                    // Enter or Space
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.state.app_scripts_expanded {
                            // Toggle script file
                            let config = self.state.app_state.config.read().await;
                            let script_dir = config.scripts.directory.clone();
                            drop(config);

                            let script_files = match std::fs::read_dir(&script_dir) {
                                Ok(entries) => {
                                    let mut files: Vec<String> = entries
                                        .filter_map(|e| e.ok())
                                        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("js"))
                                        .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                                        .collect();
                                    files.sort();
                                    files
                                }
                                Err(_) => Vec::new(),
                            };

                            if self.state.script_files_index < script_files.len() {
                                let filename = script_files[self.state.script_files_index].clone();
                                use crate::ui::commands::{Command, handle_command};

                                let command = Command::ToggleScriptFile { filename };
                                handle_command(
                                    command,
                                    self.state.app_state.clone(),
                                    self.manager.clone(),
                                ).await;
                            }
                        } else {
                            // Edit selected application setting
                            self.start_app_settings_edit().await?;
                        }
                    }

                    // Reload scripts
                    KeyCode::Char('r') => {
                        if self.state.app_scripts_expanded {
                            use crate::ui::commands::{Command, handle_command};

                            let command = Command::ReloadScripts;
                            handle_command(
                                command,
                                self.state.app_state.clone(),
                                self.manager.clone(),
                            ).await;

                            tracing::info!("Script reload requested");
                        }
                    }

                    _ => {}
                }
            }

            SettingsSection::Folder => {
                // Get folder list
                let config = self.state.app_state.config.read().await;
                let mut folder_ids: Vec<String> = config.folders.keys().cloned().collect();
                folder_ids.sort();
                let folder_count = folder_ids.len();
                drop(config);

                match key {
                    // Navigate folder list
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.state.move_folder_selection_down(folder_count);
                        if folder_count > 0 {
                            self.state.selected_folder_id =
                                Some(folder_ids[self.state.settings_folder_index].clone());
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.state.move_folder_selection_up();
                        if folder_count > 0 {
                            self.state.selected_folder_id =
                                Some(folder_ids[self.state.settings_folder_index].clone());
                        }
                    }

                    // Create new folder
                    KeyCode::Char('n') => {
                        self.create_new_folder().await?;
                    }

                    // Delete selected folder
                    KeyCode::Char('d') => {
                        self.delete_selected_folder().await?;
                    }

                    // Save configuration
                    KeyCode::Char('s') => {
                        self.save_config().await?;
                    }

                    // Enter folder edit mode
                    KeyCode::Enter => {
                        if self.state.selected_folder_id.is_some() {
                            self.state.settings_field_index = 0;
                            self.state.ui_mode = UiMode::FolderEdit;
                        }
                    }

                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Handle folder edit mode
    async fn handle_folder_edit_mode(&mut self, key: KeyCode, mods: KeyModifiers) -> Result<()> {
        // If currently editing a field, handle text input
        if self.state.settings_edit_field.is_some() {
            return self.handle_field_text_input(key, mods).await;
        }

        // Return to settings mode
        if matches!(key, KeyCode::Esc | KeyCode::Char('q')) {
            self.state.ui_mode = UiMode::Settings;
            self.state.input_buffer.clear();
            self.state.folder_scripts_expanded = false;
            self.state.script_files_index = 0;
            return Ok(());
        }

        match key {
            // Toggle scripts section expand/collapse
            KeyCode::Char('s') => {
                self.state.folder_scripts_expanded = !self.state.folder_scripts_expanded;
                if !self.state.folder_scripts_expanded {
                    self.state.script_files_index = 0;
                }
            }

            // Navigation depends on whether scripts section is expanded
            KeyCode::Char('j') | KeyCode::Down => {
                if self.state.folder_scripts_expanded {
                    // Navigate script files
                    let config = self.state.app_state.config.read().await;
                    let script_dir = config.scripts.directory.clone();
                    drop(config);

                    let script_count = match std::fs::read_dir(&script_dir) {
                        Ok(entries) => entries
                            .filter_map(|e| e.ok())
                            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("js"))
                            .count(),
                        Err(_) => 0,
                    };

                    if script_count > 0 {
                        self.state.script_files_index =
                            (self.state.script_files_index + 1) % script_count;
                    }
                } else {
                    // Navigate fields
                    let field_count = 6; // save_path, auto_date, scripts, max_concurrent, user_agent, headers
                    self.state.move_field_selection_down(field_count);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.state.folder_scripts_expanded {
                    // Navigate script files
                    let config = self.state.app_state.config.read().await;
                    let script_dir = config.scripts.directory.clone();
                    drop(config);

                    let script_count = match std::fs::read_dir(&script_dir) {
                        Ok(entries) => entries
                            .filter_map(|e| e.ok())
                            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("js"))
                            .count(),
                        Err(_) => 0,
                    };

                    if script_count > 0 {
                        self.state.script_files_index = if self.state.script_files_index == 0 {
                            script_count - 1
                        } else {
                            self.state.script_files_index - 1
                        };
                    }
                } else {
                    // Navigate fields
                    self.state.move_field_selection_up();
                }
            }

            // Enter or Space
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.state.folder_scripts_expanded {
                    // Toggle folder script file
                    if let Some(ref folder_id) = self.state.selected_folder_id {
                        let config = self.state.app_state.config.read().await;
                        let script_dir = config.scripts.directory.clone();
                        drop(config);

                        let script_files = match std::fs::read_dir(&script_dir) {
                            Ok(entries) => {
                                let mut files: Vec<String> = entries
                                    .filter_map(|e| e.ok())
                                    .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("js"))
                                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                                    .collect();
                                files.sort();
                                files
                            }
                            Err(_) => Vec::new(),
                        };

                        if self.state.script_files_index < script_files.len() {
                            let filename = script_files[self.state.script_files_index].clone();
                            use crate::ui::commands::{Command, handle_command};

                            let command = Command::ToggleFolderScriptFile {
                                folder_id: folder_id.clone(),
                                filename,
                            };
                            handle_command(
                                command,
                                self.state.app_state.clone(),
                                self.manager.clone(),
                            ).await;
                        }
                    }
                } else {
                    // Edit selected field
                    self.start_field_edit().await?;
                }
            }

            // Reload scripts
            KeyCode::Char('r') => {
                if self.state.folder_scripts_expanded {
                    use crate::ui::commands::{Command, handle_command};

                    let command = Command::ReloadScripts;
                    handle_command(
                        command,
                        self.state.app_state.clone(),
                        self.manager.clone(),
                    ).await;

                    tracing::info!("Script reload requested");
                }
            }

            _ => {}
        }

        Ok(())
    }

    /// Handle text input when editing a field
    async fn handle_field_text_input(&mut self, key: KeyCode, mods: KeyModifiers) -> Result<()> {
        // Handle Ctrl+u to clear buffer
        if matches!(key, KeyCode::Char('u')) && mods.contains(KeyModifiers::CONTROL) {
            self.state.input_buffer.clear();
            return Ok(());
        }

        match key {
            KeyCode::Char(c) => {
                // Prevent buffer overflow
                if self.state.input_buffer.len() < MAX_INPUT_LENGTH {
                    self.state.input_buffer.push(c);
                }
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Enter => {
                // Save the edited value
                self.save_field_edit().await?;
                self.state.settings_edit_field = None;
                self.state.input_buffer.clear();
            }
            KeyCode::Esc => {
                // Cancel editing
                self.state.settings_edit_field = None;
                self.state.input_buffer.clear();
            }
            _ => {}
        }

        Ok(())
    }

    /// Save the edited field value
    async fn save_field_edit(&mut self) -> Result<()> {
        use super::state::SettingsField;

        if let Some(ref folder_id) = self.state.selected_folder_id {
            if let Some(field) = self.state.settings_edit_field {
                let mut config = self.state.app_state.config.write().await;
                if let Some(folder) = config.folders.get_mut(folder_id) {
                    match field {
                        SettingsField::FolderSavePath => {
                            folder.save_path = PathBuf::from(&self.state.input_buffer);
                            tracing::info!("Updated save_path to '{}' for folder '{}'", self.state.input_buffer, folder_id);
                        }
                        SettingsField::FolderMaxConcurrent => {
                            if self.state.input_buffer.is_empty() {
                                folder.max_concurrent = None;
                                tracing::info!("Cleared max_concurrent for folder '{}'", folder_id);
                            } else if let Ok(value) = self.state.input_buffer.parse::<usize>() {
                                folder.max_concurrent = Some(value);
                                tracing::info!("Updated max_concurrent to {} for folder '{}'", value, folder_id);
                            } else {
                                self.state.validation_error = Some(format!(
                                    "Invalid number: '{}'. Expected a positive integer or leave empty to inherit.",
                                    self.state.input_buffer
                                ));
                                tracing::warn!("Invalid number: '{}'", self.state.input_buffer);
                            }
                        }
                        SettingsField::FolderUserAgent => {
                            if self.state.input_buffer.is_empty() {
                                folder.user_agent = None;
                                tracing::info!("Cleared user_agent for folder '{}'", folder_id);
                            } else {
                                folder.user_agent = Some(self.state.input_buffer.clone());
                                tracing::info!("Updated user_agent to '{}' for folder '{}'", self.state.input_buffer, folder_id);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    /// Start editing the selected field
    async fn start_field_edit(&mut self) -> Result<()> {
        use super::state::SettingsField;

        if self.state.selected_folder_id.is_none() {
            return Ok(());
        }

        // Determine which field is selected
        let selected_field = match self.state.settings_field_index {
            0 => SettingsField::FolderSavePath,
            1 => SettingsField::FolderAutoDate,
            2 => SettingsField::FolderAutoStart,
            3 => SettingsField::FolderScripts,
            4 => SettingsField::FolderMaxConcurrent,
            5 => SettingsField::FolderUserAgent,
            6 => SettingsField::FolderHeaders,
            _ => return Ok(()),
        };

        self.state.settings_edit_field = Some(selected_field);

        match selected_field {
            SettingsField::FolderAutoDate => {
                // Toggle boolean directly
                self.toggle_auto_date().await?;
                self.state.settings_edit_field = None;
            }
            SettingsField::FolderAutoStart => {
                // Toggle boolean directly
                self.toggle_auto_start().await?;
                self.state.settings_edit_field = None;
            }
            SettingsField::FolderScripts => {
                // Cycle through None -> Some(false) -> Some(true) -> None
                self.cycle_scripts_enabled().await?;
                self.state.settings_edit_field = None;
            }
            SettingsField::FolderSavePath
            | SettingsField::FolderMaxConcurrent
            | SettingsField::FolderUserAgent => {
                // Text/number input - populate input buffer with current value
                self.populate_input_buffer_for_field(selected_field).await;
                // Keep settings_edit_field set to show input dialog
            }
            SettingsField::FolderHeaders => {
                // Complex field - for now, just show a message
                tracing::info!("Headers editing not yet implemented - edit config.toml manually");
                self.state.settings_edit_field = None;
            }
        }

        Ok(())
    }

    /// Toggle auto_date_directory for selected folder
    async fn toggle_auto_date(&mut self) -> Result<()> {
        if let Some(ref folder_id) = self.state.selected_folder_id {
            let mut config = self.state.app_state.config.write().await;
            if let Some(folder) = config.folders.get_mut(folder_id) {
                folder.auto_date_directory = !folder.auto_date_directory;
                tracing::info!(
                    "Toggled auto_date_directory to {} for folder '{}'",
                    folder.auto_date_directory,
                    folder_id
                );
            }
        }
        Ok(())
    }

    /// Toggle auto_start_downloads for selected folder
    async fn toggle_auto_start(&mut self) -> Result<()> {
        if let Some(ref folder_id) = self.state.selected_folder_id {
            let mut config = self.state.app_state.config.write().await;
            if let Some(folder) = config.folders.get_mut(folder_id) {
                folder.auto_start_downloads = !folder.auto_start_downloads;
                tracing::info!(
                    "Toggled auto_start_downloads to {} for folder '{}'",
                    folder.auto_start_downloads,
                    folder_id
                );
            }
        }
        Ok(())
    }

    /// Cycle scripts_enabled through None -> Some(false) -> Some(true) -> None
    async fn cycle_scripts_enabled(&mut self) -> Result<()> {
        if let Some(ref folder_id) = self.state.selected_folder_id {
            let mut config = self.state.app_state.config.write().await;
            if let Some(folder) = config.folders.get_mut(folder_id) {
                folder.scripts_enabled = match folder.scripts_enabled {
                    None => Some(false),
                    Some(false) => Some(true),
                    Some(true) => None,
                };
                tracing::info!(
                    "Cycled scripts_enabled to {:?} for folder '{}'",
                    folder.scripts_enabled,
                    folder_id
                );
            }
        }
        Ok(())
    }

    /// Toggle scripts enabled at application level
    async fn toggle_app_scripts_enabled(&mut self) -> Result<()> {
        use crate::ui::commands::{Command, handle_command};

        let config = self.state.app_state.config.write().await;
        let new_value = !config.scripts.enabled;
        drop(config);

        let command = Command::UpdateScriptsEnabled { value: new_value };
        handle_command(
            command,
            self.state.app_state.clone(),
            self.manager.clone(),
        )
        .await;

        tracing::info!("Toggled scripts enabled to {}", new_value);
        Ok(())
    }

    /// Toggle skip download preview at application level
    async fn toggle_app_skip_download_preview(&mut self) -> Result<()> {
        use crate::ui::commands::{Command, handle_command};

        let config = self.state.app_state.config.write().await;
        let new_value = !config.general.skip_download_preview;
        drop(config);

        let command = Command::UpdateSkipDownloadPreview { value: new_value };
        handle_command(
            command,
            self.state.app_state.clone(),
            self.manager.clone(),
        )
        .await;

        tracing::info!("Toggled skip download preview to {}", new_value);
        Ok(())
    }

    /// Toggle auto-launch ggg-dnd at application level
    async fn toggle_app_auto_launch_dnd(&mut self) -> Result<()> {
        use crate::ui::commands::{Command, handle_command};

        let config = self.state.app_state.config.write().await;
        let new_value = !config.general.auto_launch_dnd;
        drop(config);

        let command = Command::UpdateAutoLaunchDnd { value: new_value };
        handle_command(
            command,
            self.state.app_state.clone(),
            self.manager.clone(),
        )
        .await;

        tracing::info!("Toggled auto launch dnd to {}", new_value);
        Ok(())
    }

    /// Populate input buffer with current value for the field
    async fn populate_input_buffer_for_field(&mut self, field: super::state::SettingsField) {
        use super::state::SettingsField;

        if let Some(ref folder_id) = self.state.selected_folder_id {
            let config = self.state.app_state.config.read().await;
            if let Some(folder) = config.folders.get(folder_id) {
                self.state.input_buffer = match field {
                    SettingsField::FolderSavePath => {
                        folder.save_path.to_string_lossy().to_string()
                    }
                    SettingsField::FolderMaxConcurrent => {
                        folder.max_concurrent.map(|v| v.to_string()).unwrap_or_default()
                    }
                    SettingsField::FolderUserAgent => {
                        folder.user_agent.clone().unwrap_or_default()
                    }
                    _ => String::new(),
                };
            }
        }
    }

    /// Handle change folder mode
    async fn handle_change_folder_mode(&mut self, key: KeyCode, mods: KeyModifiers) -> Result<()> {
        // Handle Ctrl+u first (before Char match)
        if matches!(key, KeyCode::Char('u')) && mods.contains(KeyModifiers::CONTROL) {
            self.state.input_buffer.clear();
            return Ok(());
        }

        match key {
            KeyCode::Char(c) => {
                // Prevent buffer overflow
                if self.state.input_buffer.len() < MAX_INPUT_LENGTH {
                    self.state.input_buffer.push(c);
                }
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Enter => {
                // Submit new path
                if !self.state.input_buffer.is_empty() {
                    if let Some(task) = self.state.get_selected_download() {
                        let new_path = std::path::PathBuf::from(&self.state.input_buffer);

                        // Change the save path
                        if let Err(e) = self.manager.change_save_path(task.id, new_path).await {
                            // Store error message for display (future enhancement)
                            tracing::warn!("Failed to change path: {}", e);
                        } else {
                            self.save_queue().await?;
                        }
                    }
                }
                self.state.ui_mode = UiMode::Normal;
                self.state.input_buffer.clear();
            }
            KeyCode::Esc => {
                self.state.ui_mode = UiMode::Normal;
                self.state.input_buffer.clear();
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle switch folder mode (folder picker dialog)
    async fn handle_switch_folder_mode(&mut self, key: KeyCode) -> Result<()> {
        // Get folder list
        let config = self.state.app_state.config.read().await;
        let mut folder_ids: Vec<String> = config.folders.keys().cloned().collect();
        folder_ids.sort();
        let folder_count = folder_ids.len();
        drop(config);

        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if folder_count > 0 {
                    self.state.folder_picker_index = (self.state.folder_picker_index + 1) % folder_count;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if folder_count > 0 {
                    self.state.folder_picker_index = if self.state.folder_picker_index == 0 {
                        folder_count - 1
                    } else {
                        self.state.folder_picker_index - 1
                    };
                }
            }
            KeyCode::Enter => {
                // Select folder
                if folder_count > 0 && self.state.folder_picker_index < folder_count {
                    self.state.current_folder_id = folder_ids[self.state.folder_picker_index].clone();
                    tracing::info!("Switched current folder to: {}", self.state.current_folder_id);
                }
                self.state.ui_mode = UiMode::Normal;
            }
            KeyCode::Esc => {
                self.state.ui_mode = UiMode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle confirm delete mode
    async fn handle_confirm_delete_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // Confirm - delete the download
                self.delete_download().await?;
                self.state.ui_mode = UiMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                // Cancel - return to normal mode
                self.state.ui_mode = UiMode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle context menu mode
    async fn handle_context_menu_mode(&mut self, key: KeyCode) -> Result<()> {
        use super::state::ContextMenuAction;

        match key {
            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                self.state.context_menu_move_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.state.context_menu_move_up();
            }

            // Execute selected action
            KeyCode::Enter => {
                if let Some(action) = self.state.get_selected_menu_action() {
                    self.execute_menu_action(action).await?;
                }
            }

            // Direct keybindings for quick actions
            KeyCode::Char(' ') => {
                self.execute_menu_action(ContextMenuAction::StartPause).await?;
            }
            KeyCode::Char('r') => {
                self.execute_menu_action(ContextMenuAction::Retry).await?;
            }
            KeyCode::Char('d') => {
                self.execute_menu_action(ContextMenuAction::Delete).await?;
            }
            KeyCode::Char('f') => {
                self.execute_menu_action(ContextMenuAction::ChangeFolder).await?;
            }
            KeyCode::Char('p') => {
                self.execute_menu_action(ContextMenuAction::ChangeSavePath).await?;
            }
            KeyCode::Char('c') => {
                self.execute_menu_action(ContextMenuAction::CopyUrl).await?;
            }
            KeyCode::Char('o') => {
                self.execute_menu_action(ContextMenuAction::OpenFolder).await?;
            }

            // Cancel menu
            KeyCode::Esc => {
                self.state.ui_mode = UiMode::Normal;
            }

            _ => {}
        }

        Ok(())
    }

    /// Execute a context menu action
    async fn execute_menu_action(&mut self, action: super::state::ContextMenuAction) -> Result<()> {
        use super::state::ContextMenuAction;

        match action {
            ContextMenuAction::StartPause => {
                self.state.ui_mode = UiMode::Normal;
                self.toggle_download().await?;
            }
            ContextMenuAction::Retry => {
                self.state.ui_mode = UiMode::Normal;
                self.retry_download().await?;
            }
            ContextMenuAction::Delete => {
                // Go to confirm delete mode
                self.state.ui_mode = UiMode::ConfirmDelete;
            }
            ContextMenuAction::ChangeFolder => {
                self.state.ui_mode = UiMode::ChangeFolder;
                self.state.input_buffer.clear();
            }
            ContextMenuAction::ChangeSavePath => {
                self.state.ui_mode = UiMode::ChangeFolder;
                self.state.input_buffer.clear();
            }
            ContextMenuAction::CopyUrl => {
                // Copy URL to clipboard
                // TODO: Implement clipboard integration (requires clipboard crate)
                if let Some(task) = self.state.get_selected_download() {
                    tracing::info!("Copy URL feature: {}", task.url);
                    // For now, just log the URL - clipboard integration can be added later
                }
                self.state.ui_mode = UiMode::Normal;
            }
            ContextMenuAction::OpenFolder => {
                // Open download folder in file explorer
                if let Some(task) = self.state.get_selected_download() {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new("explorer")
                            .arg(task.save_path.to_string_lossy().to_string())
                            .spawn();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("open")
                            .arg(task.save_path.to_string_lossy().to_string())
                            .spawn();
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(task.save_path.to_string_lossy().to_string())
                            .spawn();
                    }
                    tracing::info!("Opening folder: {}", task.save_path.display());
                }
                self.state.ui_mode = UiMode::Normal;
            }
            ContextMenuAction::Cancel => {
                self.state.ui_mode = UiMode::Normal;
            }
        }

        Ok(())
    }

    /// Handle folder context menu mode keys
    async fn handle_folder_context_menu_mode(&mut self, key: KeyCode) -> Result<()> {
        use super::state::FolderContextMenuAction;

        let is_completed = self.state.is_viewing_completed_node();

        match key {
            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                self.state.folder_context_menu_move_down(is_completed);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.state.folder_context_menu_move_up();
            }

            // Execute selected action
            KeyCode::Enter => {
                if let Some(action) = self.state.get_selected_folder_menu_action(is_completed) {
                    self.execute_folder_menu_action(action).await?;
                }
            }

            // Direct keybindings
            KeyCode::Char('s') if !is_completed => {
                self.execute_folder_menu_action(FolderContextMenuAction::StartAll)
                    .await?;
            }
            KeyCode::Char('p') if !is_completed => {
                self.execute_folder_menu_action(FolderContextMenuAction::StopAll)
                    .await?;
            }
            KeyCode::Char('d') if !is_completed => {
                self.execute_folder_menu_action(FolderContextMenuAction::DeleteAll)
                    .await?;
            }
            KeyCode::Char('c') if is_completed => {
                self.execute_folder_menu_action(FolderContextMenuAction::ClearHistory)
                    .await?;
            }

            // Cancel menu
            KeyCode::Esc => {
                self.state.ui_mode = UiMode::Normal;
                self.state.reset_folder_context_menu();
            }

            _ => {}
        }

        self.state.mark_dirty();
        Ok(())
    }

    /// Execute a folder context menu action
    async fn execute_folder_menu_action(
        &mut self,
        action: super::state::FolderContextMenuAction,
    ) -> Result<()> {
        use super::state::FolderContextMenuAction;

        match action {
            FolderContextMenuAction::StartAll => {
                // Start all pending downloads in the current folder
                if let Some(folder_id) = self.state.selected_folder_id_from_tree() {
                    self.manager
                        .start_folder_tasks(
                            folder_id,
                            self.state.app_state.script_sender.clone(),
                            self.state.app_state.config.clone(),
                        )
                        .await;
                }
                self.state.ui_mode = UiMode::Normal;
            }
            FolderContextMenuAction::StopAll => {
                // Stop all downloading tasks in the current folder
                if let Some(folder_id) = self.state.selected_folder_id_from_tree() {
                    self.manager.stop_folder_tasks(folder_id).await;
                }
                self.state.ui_mode = UiMode::Normal;
            }
            FolderContextMenuAction::DeleteAll => {
                // Delete all downloads in the current folder
                if let Some(folder_id) = self.state.selected_folder_id_from_tree() {
                    if let Some(tasks) = self.state.folder_downloads.get(folder_id) {
                        let ids: Vec<_> = tasks.iter().map(|t| t.id).collect();
                        for id in ids {
                            self.manager.remove_download(id).await;
                        }
                    }
                }
                self.state.ui_mode = UiMode::Normal;
            }
            FolderContextMenuAction::ClearHistory => {
                // Clear all history items
                self.manager.clear_history().await;
                self.state.ui_mode = UiMode::Normal;
            }
            FolderContextMenuAction::Cancel => {
                self.state.ui_mode = UiMode::Normal;
            }
        }

        self.state.reset_folder_context_menu();
        Ok(())
    }

    /// Toggle download (start/pause) - supports multi-selection
    async fn toggle_download(&mut self) -> Result<()> {
        // If there are selected downloads, toggle all of them
        if !self.state.selected_downloads.is_empty() {
            let selected_ids = self.state.get_selected_download_ids();
            for id in selected_ids {
                // Find the task to check its status
                if let Some(task) = self.manager.get_by_id(id).await {
                    match task.status {
                        DownloadStatus::Downloading => {
                            self.manager.pause_download(id).await?;
                        }
                        DownloadStatus::Pending | DownloadStatus::Paused | DownloadStatus::Error => {
                            self.manager.start_download(id, self.state.app_state.script_sender.clone(), self.state.app_state.config.clone()).await?;
                        }
                        _ => {}
                    }
                }
            }
            self.save_queue().await?;
        } else if let Some(task) = self.state.get_selected_download() {
            // No multi-selection, toggle current item
            match task.status {
                DownloadStatus::Downloading => {
                    self.manager.pause_download(task.id).await?;
                }
                DownloadStatus::Pending | DownloadStatus::Paused | DownloadStatus::Error => {
                    self.manager.start_download(task.id, self.state.app_state.script_sender.clone(), self.state.app_state.config.clone()).await?;
                }
                _ => {}
            }
            self.save_queue().await?;
        }
        Ok(())
    }

    /// Delete selected download(s) - supports multi-selection
    async fn delete_download(&mut self) -> Result<()> {
        const MAX_UNDO_HISTORY: usize = 10;

        // If there are selected downloads, delete all of them
        if !self.state.selected_downloads.is_empty() {
            let ids_to_delete = self.state.get_selected_download_ids();
            for id in ids_to_delete {
                // Save to undo history before deleting
                if let Some(mut task) = self.manager.get_by_id(id).await {
                    // Mark as deleted and add to history
                    task.status = DownloadStatus::Deleted;
                    self.manager.add_to_history(task.clone()).await;
                    self.state.delete_history.push(task);
                }
                self.manager.remove_download(id).await;
            }
            self.state.clear_selections();
            self.save_queue().await?;
            self.state.adjust_selection_after_delete();
        } else if let Some(task) = self.state.get_selected_download() {
            // Get ID first to avoid borrow issues
            let task_id = task.id;
            let mut task_clone = task.clone();

            // Mark as deleted and add to history
            task_clone.status = DownloadStatus::Deleted;
            self.manager.add_to_history(task_clone.clone()).await;

            // Save to undo history before deleting
            self.state.delete_history.push(task_clone);

            // No multi-selection, delete current item
            self.manager.remove_download(task_id).await;
            self.save_queue().await?;
            self.state.adjust_selection_after_delete();
        }

        // Limit history size to prevent excessive memory usage
        if self.state.delete_history.len() > MAX_UNDO_HISTORY {
            self.state.delete_history.drain(0..self.state.delete_history.len() - MAX_UNDO_HISTORY);
        }

        Ok(())
    }

    /// Undo last delete operation
    async fn undo_delete(&mut self) -> Result<()> {
        if let Some(task) = self.state.delete_history.pop() {
            self.add_download_with_auto_start(task).await?;
            tracing::info!("Undid delete operation");
        }
        Ok(())
    }

    /// Retry failed download
    async fn retry_download(&mut self) -> Result<()> {
        if let Some(task) = self.state.get_selected_download() {
            if task.status == DownloadStatus::Error {
                self.manager.start_download(task.id, self.state.app_state.script_sender.clone(), self.state.app_state.config.clone()).await?;
                self.save_queue().await?;
            }
        }
        Ok(())
    }

    /// Save queue to folder-based files
    pub async fn save_queue(&self) -> Result<()> {
        self.manager.save_queue_to_folders().await
    }

    /// Fetch download information from URL
    async fn fetch_download_info(&self, url: &str) -> Result<crate::download::http_client::DownloadInfo> {
        use crate::download::http_client::HttpClient;

        let config = self.state.app_state.config.read().await;
        let user_agent = config.download.user_agent.clone();
        drop(config);

        let client = HttpClient::with_user_agent(&user_agent)?;
        let headers = HttpClient::build_headers(Some(&user_agent), None, &std::collections::HashMap::new())?;

        client.get_info(url, &headers).await
    }

    /// Handle download preview mode
    async fn handle_download_preview_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Enter => {
                // Confirm and add download
                if !self.state.input_buffer.is_empty() {
                    let url = self.state.input_buffer.clone();
                    let config = self.state.app_state.config.read().await;

                    // Use new_with_folder to apply folder defaults
                    let task = crate::download::task::DownloadTask::new_with_folder(
                        url,
                        self.state.current_folder_id.clone(),
                        &config,
                    );
                    drop(config); // Release read lock before async operations

                    self.add_download_with_auto_start(task).await?;
                }

                // Clean up and return to normal mode
                self.state.ui_mode = UiMode::Normal;
                self.state.input_buffer.clear();
                self.state.preview_info = None;
            }
            KeyCode::Esc => {
                // Cancel and return to add download mode
                self.state.ui_mode = UiMode::AddDownload;
                self.state.preview_info = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Create a new folder in settings
    async fn create_new_folder(&mut self) -> Result<()> {
        let mut config = self.state.app_state.config.write().await;

        // Find unique folder name
        let mut counter = 1;
        let mut new_folder_id = format!("folder_{}", counter);
        while config.folders.contains_key(&new_folder_id) {
            counter += 1;
            new_folder_id = format!("folder_{}", counter);
        }

        // Create new folder with default settings
        let new_folder = crate::app::config::FolderConfig {
            save_path: config.download.default_directory.clone(),
            auto_date_directory: false,
            auto_start_downloads: false,
            scripts_enabled: None,
            script_files: None,
            max_concurrent: None,
            user_agent: None,
            default_headers: std::collections::HashMap::new(),
        };

        config.folders.insert(new_folder_id.clone(), new_folder);
        drop(config);

        // Select the newly created folder
        self.state.selected_folder_id = Some(new_folder_id);

        Ok(())
    }

    /// Delete selected folder in settings
    async fn delete_selected_folder(&mut self) -> Result<()> {
        if let Some(ref folder_id) = self.state.selected_folder_id {
            // Don't allow deleting the "default" folder
            if folder_id == "default" {
                tracing::warn!("Cannot delete the default folder");
                return Ok(());
            }

            let folder_id_owned = folder_id.clone();

            let mut config = self.state.app_state.config.write().await;
            config.folders.remove(&folder_id_owned);

            // Save config to persist the removal
            if let Err(e) = config.save() {
                tracing::error!("Failed to save config after folder deletion: {}", e);
            }
            drop(config);

            // Delete folder config directory from filesystem
            if let Ok(config_dir) = crate::util::paths::find_config_directory() {
                let folder_dir = config_dir.join(&folder_id_owned);
                if folder_dir.exists() && folder_dir.is_dir() {
                    if let Err(e) = std::fs::remove_dir_all(&folder_dir) {
                        tracing::error!("Failed to delete folder directory {:?}: {}", folder_dir, e);
                    } else {
                        tracing::info!("Deleted folder config directory: {:?}", folder_dir);
                    }
                }
            }

            // Clear selection
            self.state.selected_folder_id = None;
            if self.state.settings_folder_index > 0 {
                self.state.settings_folder_index -= 1;
            }
        }
        Ok(())
    }

    /// Save configuration to file
    async fn save_config(&self) -> Result<()> {
        let config = self.state.app_state.config.read().await;
        config.save()?;
        tracing::info!("Configuration saved successfully");
        Ok(())
    }

    /// Start editing an application setting
    async fn start_app_settings_edit(&mut self) -> Result<()> {
        use super::state::ApplicationSettingsField;

        let fields = ApplicationSettingsField::all();
        if self.state.app_settings_field_index >= fields.len() {
            return Ok(());
        }

        let field = fields[self.state.app_settings_field_index];

        // Handle toggle fields directly without entering input mode
        match field {
            ApplicationSettingsField::ScriptsEnabled => {
                // Toggle boolean directly
                self.toggle_app_scripts_enabled().await?;
                return Ok(());
            }
            ApplicationSettingsField::SkipDownloadPreview => {
                // Toggle boolean directly
                self.toggle_app_skip_download_preview().await?;
                return Ok(());
            }
            ApplicationSettingsField::AutoLaunchDnd => {
                // Toggle boolean directly
                self.toggle_app_auto_launch_dnd().await?;
                return Ok(());
            }
            _ => {
                // Continue with normal text input for other fields
            }
        }

        let config = self.state.app_state.config.read().await;

        // Pre-fill input buffer with current value
        self.state.input_buffer = match field {
            ApplicationSettingsField::MaxConcurrent => {
                config.download.max_concurrent.to_string()
            }
            ApplicationSettingsField::MaxConcurrentPerFolder => {
                config
                    .download
                    .max_concurrent_per_folder
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }
            ApplicationSettingsField::MaxActiveFolders => {
                config
                    .download
                    .parallel_folder_count
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }
            ApplicationSettingsField::MaxRedirects => {
                config.download.max_redirects.to_string()
            }
            ApplicationSettingsField::RetryCount => {
                config.download.retry_count.to_string()
            }
            ApplicationSettingsField::Language => {
                config.general.language.clone()
            }
            ApplicationSettingsField::ScriptsEnabled | ApplicationSettingsField::SkipDownloadPreview | ApplicationSettingsField::AutoLaunchDnd => {
                // These are handled above as toggles
                unreachable!()
            }
        };

        drop(config);

        // Enter input mode with appropriate title and prompt
        self.state.is_editing_app_setting = true;
        let args = fluent::fluent_args! {
            "label" => field.label(),
        };
        self.state.input_title = self.state.t_with_args("dialog-edit-label", Some(&args));
        self.state.input_prompt = self.state.t("prompt-value");
        self.state.ui_mode = UiMode::EditingField;

        Ok(())
    }

    /// Save application setting value from input buffer
    async fn save_app_setting_value(&mut self) -> Result<()> {
        use super::state::ApplicationSettingsField;
        use crate::ui::commands::{Command, handle_command};

        let fields = ApplicationSettingsField::all();
        if self.state.app_settings_field_index >= fields.len() {
            return Ok(());
        }

        let field = fields[self.state.app_settings_field_index];
        let value_str = self.state.input_buffer.trim();

        // Parse and validate input
        let command = match field {
            ApplicationSettingsField::MaxConcurrent => {
                if let Ok(value) = value_str.parse::<usize>() {
                    Command::UpdateMaxConcurrent { value }
                } else {
                    self.state.validation_error = Some(format!(
                        "Invalid number: '{}'. Expected a positive integer.",
                        value_str
                    ));
                    tracing::error!("Invalid value for MaxConcurrent: {}", value_str);
                    return Ok(());
                }
            }
            ApplicationSettingsField::MaxConcurrentPerFolder => {
                let value = if value_str.is_empty() {
                    None
                } else if let Ok(v) = value_str.parse::<usize>() {
                    Some(v)
                } else {
                    self.state.validation_error = Some(format!(
                        "Invalid number: '{}'. Expected a positive integer or leave empty.",
                        value_str
                    ));
                    tracing::error!("Invalid value for MaxConcurrentPerFolder: {}", value_str);
                    return Ok(());
                };
                Command::UpdateMaxConcurrentPerFolder { value }
            }
            ApplicationSettingsField::MaxActiveFolders => {
                let value = if value_str.is_empty() {
                    None
                } else if let Ok(v) = value_str.parse::<usize>() {
                    Some(v)
                } else {
                    self.state.validation_error = Some(format!(
                        "Invalid number: '{}'. Expected a positive integer or leave empty.",
                        value_str
                    ));
                    tracing::error!("Invalid value for MaxActiveFolders: {}", value_str);
                    return Ok(());
                };
                Command::UpdateMaxActiveFolders { value }
            }
            ApplicationSettingsField::MaxRedirects => {
                if let Ok(value) = value_str.parse::<u32>() {
                    Command::UpdateMaxRedirects { value }
                } else {
                    self.state.validation_error = Some(format!(
                        "Invalid number: '{}'. Expected a positive integer.",
                        value_str
                    ));
                    tracing::error!("Invalid value for MaxRedirects: {}", value_str);
                    return Ok(());
                }
            }
            ApplicationSettingsField::RetryCount => {
                if let Ok(value) = value_str.parse::<u32>() {
                    Command::UpdateRetryCount { value }
                } else {
                    self.state.validation_error = Some(format!(
                        "Invalid number: '{}'. Expected a positive integer.",
                        value_str
                    ));
                    tracing::error!("Invalid value for RetryCount: {}", value_str);
                    return Ok(());
                }
            }
            ApplicationSettingsField::ScriptsEnabled | ApplicationSettingsField::SkipDownloadPreview | ApplicationSettingsField::AutoLaunchDnd => {
                // These are now handled as toggles in start_app_settings_edit()
                // This branch should never be reached
                unreachable!("Toggle fields are handled in start_app_settings_edit()")
            }
            ApplicationSettingsField::Language => {
                let value = value_str.to_lowercase();
                if value != "en" && value != "ja" {
                    self.state.validation_error = Some(format!(
                        "Invalid language: '{}'. Use 'en' or 'ja'.",
                        value_str
                    ));
                    tracing::error!("Invalid language: {}. Use 'en' or 'ja'.", value_str);
                    return Ok(());
                }
                Command::UpdateLanguage { value }
            }
        };

        // Execute command
        let response = handle_command(
            command,
            self.state.app_state.clone(),
            self.manager.clone(),
        )
        .await;

        // Check response
        match response {
            crate::ui::commands::CommandResponse::Success { .. } => {
                self.state.validation_error = None;  // Clear error on success
                tracing::info!("Application setting updated successfully");

                // Log restart reminder for language changes
                if matches!(field, ApplicationSettingsField::Language) {
                    tracing::info!("Language changed. Please restart application for changes to take effect.");
                }
            }
            crate::ui::commands::CommandResponse::Error { error } => {
                self.state.validation_error = Some(error.clone());
                tracing::error!("Failed to update setting: {}", error);
            }
        }

        // Return to settings screen
        self.state.ui_mode = UiMode::Settings;
        self.state.input_buffer.clear();

        Ok(())
    }

    /// Check if text is a valid URL with a scheme that can be downloaded
    /// Uses url crate to validate, accepts schemes that reqwest can handle
    fn is_valid_download_url(text: &str) -> bool {
        match url::Url::parse(text) {
            Ok(parsed) => {
                // Check if scheme is supported by our HTTP client (reqwest)
                matches!(parsed.scheme(), "http" | "https" | "ftp" | "ftps")
            }
            Err(_) => false,
        }
    }

    /// Add download task and auto-start if folder setting enabled
    async fn add_download_with_auto_start(&mut self, task: crate::download::task::DownloadTask) -> Result<()> {
        let folder_id = task.folder_id.clone();
        let task_id = task.id;

        // Add download to queue
        self.manager.add_download(task).await;

        // Check if auto-start is enabled for this folder
        let should_auto_start = {
            let config = self.state.app_state.config.read().await;
            config
                .folders
                .get(&folder_id)
                .map(|f| f.auto_start_downloads)
                .unwrap_or(false)
        };

        // Auto-start if enabled
        if should_auto_start {
            self.manager
                .start_download(
                    task_id,
                    self.state.app_state.script_sender.clone(),
                    self.state.app_state.config.clone(),
                )
                .await?;
            tracing::info!("Auto-started download in folder '{}'", folder_id);
        }

        self.save_queue().await?;
        Ok(())
    }

    /// Add download task from pasted/dropped URL
    /// Does not expand URL patterns ([] is valid in URLs)
    async fn add_download_from_paste(&mut self, url: &str) -> Result<()> {
        let folder_id = self.state.current_folder_id.clone();

        // Create download task (no URL expansion for paste/D&D)
        let task = {
            let config = self.state.app_state.config.read().await;
            crate::download::task::DownloadTask::new_with_folder(
                url.to_string(),
                folder_id.clone(),
                &config,
            )
        };

        self.add_download_with_auto_start(task).await?;

        tracing::info!(
            "Auto-added download from paste/D&D to folder '{}'",
            folder_id
        );

        Ok(())
    }
}

/// Main TUI entry point
pub async fn run_tui(
    app_state: AppState,
    manager: DownloadManager,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;
    stdout.execute(EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Read keybindings from config
    let keybindings = {
        let config = app_state.config.read().await;
        config.keybindings.clone()
    };

    // Create app
    let mut app = TuiApp::new(app_state, manager, &keybindings);

    // Load downloads initially
    app.state.update_downloads(&app.manager).await;

    // Event channel
    let (tx, mut rx) = mpsc::channel(100);

    // Spawn keyboard event reader
    let input_tx = tx.clone();
    tokio::spawn(async move {
        let mut reader = crossterm::event::EventStream::new();
        while let Some(Ok(event)) = reader.next().await {
            if input_tx.send(TuiEvent::Input(event)).await.is_err() {
                break;
            }
        }
    });

    // Spawn tick event generator
    let tick_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            if tick_tx.send(TuiEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // Spawn IPC Named Pipe server (Windows only)
    #[cfg(windows)]
    {
        let ipc_tx = tx.clone();
        let (ipc_event_tx, mut ipc_event_rx) = mpsc::channel(32);
        let (pipe_name, _ipc_handle) =
            crate::ipc::pipe_server::start_pipe_server(ipc_event_tx);
        tracing::info!("IPC pipe server started: {}", pipe_name);
        app.state.ipc_pipe_name = Some(pipe_name.clone());

        // Bridge IPC events into TUI event channel
        tokio::spawn(async move {
            while let Some(ipc_event) = ipc_event_rx.recv().await {
                match ipc_event {
                    crate::ipc::pipe_server::IpcEvent::UrlReceived(url) => {
                        if ipc_tx.send(TuiEvent::IpcUrl(url)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        // Auto-launch ggg-dnd if configured and not already running
        {
            let config = app.state.app_state.config.read().await;
            if config.general.auto_launch_dnd {
                drop(config);
                auto_launch_ggg_dnd(&pipe_name);
            }
        }
    }

    // Track whether mouse capture is currently active
    let mut mouse_captured = true;

    // Main event loop
    while !app.should_quit {
        // Toggle mouse capture based on UI mode:
        // Disable in text-input modes so terminal-native paste (e.g. right-click) works.
        let want_capture = !app.state.ui_mode.is_text_input();
        if want_capture != mouse_captured {
            if want_capture {
                terminal.backend_mut().execute(EnableMouseCapture)?;
            } else {
                terminal.backend_mut().execute(DisableMouseCapture)?;
            }
            mouse_captured = want_capture;
        }

        // Draw UI only if dirty flag is set (optimization)
        if app.state.needs_redraw() {
            terminal.draw(|f| super::ui::render(&app, f))?;
            app.state.clear_dirty();
        }

        // Handle events with timeout
        if let Ok(Some(event)) = tokio::time::timeout(
            Duration::from_millis(100),
            rx.recv()
        ).await {
            app.handle_event(event).await?;
        }
    }

    // Cleanup terminal
    disable_raw_mode()?;
    terminal.backend_mut().execute(DisableMouseCapture)?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.backend_mut().execute(DisableBracketedPaste)?;
    terminal.show_cursor()?;

    // Save queue on exit
    app.save_queue().await?;

    Ok(())
}

/// Auto-launch ggg-dnd.exe if not already running (detected via Named Mutex).
#[cfg(windows)]
fn auto_launch_ggg_dnd(pipe_name: &str) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::OpenMutexW;

    // Check if ggg-dnd is already running via Named Mutex
    let already_running = unsafe {
        match OpenMutexW(
            windows::Win32::System::Threading::MUTEX_ALL_ACCESS,
            false,
            windows::core::w!("Global\\ggg-dnd-running"),
        ) {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                true
            }
            Err(_) => false,
        }
    };

    if already_running {
        tracing::info!("ggg-dnd is already running, skipping auto-launch");
        return;
    }

    // Look for ggg-dnd.exe next to ggg.exe
    let dnd_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("ggg-dnd.exe")));

    match dnd_exe {
        Some(exe_path) if exe_path.exists() => {
            match std::process::Command::new(&exe_path).arg(pipe_name).spawn() {
                Ok(_) => tracing::info!("Auto-launched ggg-dnd: {:?}", exe_path),
                Err(e) => tracing::warn!("Failed to auto-launch ggg-dnd: {}", e),
            }
        }
        _ => {
            tracing::debug!("ggg-dnd.exe not found next to ggg.exe, skipping auto-launch");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_download_url_http() {
        assert!(TuiApp::is_valid_download_url("http://example.com/file.zip"));
        assert!(TuiApp::is_valid_download_url("https://example.com/file.zip"));
    }

    #[test]
    fn test_is_valid_download_url_ftp() {
        assert!(TuiApp::is_valid_download_url("ftp://example.com/file.zip"));
        assert!(TuiApp::is_valid_download_url("ftps://example.com/file.zip"));
    }

    #[test]
    fn test_is_valid_download_url_invalid_scheme() {
        assert!(!TuiApp::is_valid_download_url("javascript:alert('test')"));
        assert!(!TuiApp::is_valid_download_url("data:text/plain,hello"));
        assert!(!TuiApp::is_valid_download_url("file:///etc/passwd"));
        assert!(!TuiApp::is_valid_download_url("mailto:user@example.com"));
    }

    #[test]
    fn test_is_valid_download_url_malformed() {
        assert!(!TuiApp::is_valid_download_url("not a url"));
        assert!(!TuiApp::is_valid_download_url("htp://typo.com"));
        assert!(!TuiApp::is_valid_download_url("://missing-scheme.com"));
        assert!(!TuiApp::is_valid_download_url(""));
    }

    #[test]
    fn test_is_valid_download_url_with_query_and_fragment() {
        assert!(TuiApp::is_valid_download_url(
            "https://example.com/file.zip?download=true#section"
        ));
        assert!(TuiApp::is_valid_download_url(
            "http://example.com/path/to/file?param1=value1&param2=value2"
        ));
    }
}
