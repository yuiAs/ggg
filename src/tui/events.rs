use crossterm::event::Event as CrosstermEvent;

/// TUI events that can occur
#[derive(Debug, Clone)]
pub enum TuiEvent {
    /// Terminal input event (keyboard, mouse, resize)
    Input(CrosstermEvent),
    /// Tick event for periodic updates
    Tick,
}
