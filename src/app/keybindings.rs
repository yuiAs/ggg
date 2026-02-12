//! Keyboard shortcut customization support
//!
//! Allows users to customize keyboard shortcuts through configuration.

use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Actions that can be bound to keyboard shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    // Navigation
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    PageUp,
    PageDown,
    FocusNextPane,
    FocusPrevPane,
    FocusLeft,
    FocusRight,

    // Selection
    SelectItem,
    ToggleSelection,
    SelectAll,
    DeselectAll,

    // Actions
    AddDownload,
    DeleteDownload,
    ToggleDownload,
    RetryDownload,
    ResumeAll,
    PauseAll,
    OpenContextMenu,
    EditItem,

    // View
    ToggleDetails,
    OpenSearch,
    OpenHelp,
    OpenSettings,
    SwitchFolder,

    // System
    Quit,
    Undo,
    Refresh,
}

impl KeyAction {
    /// Returns all available actions
    pub fn all() -> Vec<KeyAction> {
        vec![
            KeyAction::MoveUp,
            KeyAction::MoveDown,
            KeyAction::MoveToTop,
            KeyAction::MoveToBottom,
            KeyAction::PageUp,
            KeyAction::PageDown,
            KeyAction::FocusNextPane,
            KeyAction::FocusPrevPane,
            KeyAction::FocusLeft,
            KeyAction::FocusRight,
            KeyAction::SelectItem,
            KeyAction::ToggleSelection,
            KeyAction::SelectAll,
            KeyAction::DeselectAll,
            KeyAction::AddDownload,
            KeyAction::DeleteDownload,
            KeyAction::ToggleDownload,
            KeyAction::RetryDownload,
            KeyAction::ResumeAll,
            KeyAction::PauseAll,
            KeyAction::OpenContextMenu,
            KeyAction::EditItem,
            KeyAction::ToggleDetails,
            KeyAction::OpenSearch,
            KeyAction::OpenHelp,
            KeyAction::OpenSettings,
            KeyAction::SwitchFolder,
            KeyAction::Quit,
            KeyAction::Undo,
            KeyAction::Refresh,
        ]
    }
}

/// A key binding specification (serializable format)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyBindingSpec {
    /// Single key (e.g., "j", "Enter", "Space")
    Single(String),
    /// Multiple keys (e.g., ["j", "Down"])
    Multiple(Vec<String>),
}

impl Default for KeyBindingSpec {
    fn default() -> Self {
        KeyBindingSpec::Single(String::new())
    }
}

/// Parsed key combination
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    /// Parse a key string like "Ctrl+j", "Shift+Enter", "Space", "a"
    pub fn parse(s: &str) -> Option<KeyCombo> {
        let s = s.trim();
        let mut modifiers = KeyModifiers::empty();
        let mut key_part = s;

        // Parse modifiers
        let parts: Vec<&str> = s.split('+').collect();
        if parts.len() > 1 {
            for part in &parts[..parts.len() - 1] {
                match part.to_lowercase().as_str() {
                    "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                    "alt" => modifiers |= KeyModifiers::ALT,
                    "shift" => modifiers |= KeyModifiers::SHIFT,
                    _ => return None, // Unknown modifier
                }
            }
            key_part = parts.last()?;
        }

        // Parse key code
        let code = parse_key_code(key_part)?;

        Some(KeyCombo::new(code, modifiers))
    }

    /// Check if this combo matches the given key event
    pub fn matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        self.code == code && self.modifiers == modifiers
    }
}

/// Parse a key code string
fn parse_key_code(s: &str) -> Option<KeyCode> {
    let s = s.trim();

    // Check for special keys first
    match s.to_lowercase().as_str() {
        "enter" | "return" => return Some(KeyCode::Enter),
        "space" => return Some(KeyCode::Char(' ')),
        "tab" => return Some(KeyCode::Tab),
        "backtab" => return Some(KeyCode::BackTab),
        "backspace" => return Some(KeyCode::Backspace),
        "esc" | "escape" => return Some(KeyCode::Esc),
        "up" => return Some(KeyCode::Up),
        "down" => return Some(KeyCode::Down),
        "left" => return Some(KeyCode::Left),
        "right" => return Some(KeyCode::Right),
        "home" => return Some(KeyCode::Home),
        "end" => return Some(KeyCode::End),
        "pageup" => return Some(KeyCode::PageUp),
        "pagedown" => return Some(KeyCode::PageDown),
        "delete" | "del" => return Some(KeyCode::Delete),
        "insert" | "ins" => return Some(KeyCode::Insert),
        "f1" => return Some(KeyCode::F(1)),
        "f2" => return Some(KeyCode::F(2)),
        "f3" => return Some(KeyCode::F(3)),
        "f4" => return Some(KeyCode::F(4)),
        "f5" => return Some(KeyCode::F(5)),
        "f6" => return Some(KeyCode::F(6)),
        "f7" => return Some(KeyCode::F(7)),
        "f8" => return Some(KeyCode::F(8)),
        "f9" => return Some(KeyCode::F(9)),
        "f10" => return Some(KeyCode::F(10)),
        "f11" => return Some(KeyCode::F(11)),
        "f12" => return Some(KeyCode::F(12)),
        _ => {}
    }

    // Single character
    let mut chars = s.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        return Some(KeyCode::Char(first));
    }

    None
}

/// Keybindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    #[serde(flatten)]
    pub bindings: HashMap<KeyAction, KeyBindingSpec>,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        let mut bindings = HashMap::new();

        // Navigation
        bindings.insert(
            KeyAction::MoveUp,
            KeyBindingSpec::Multiple(vec!["k".into(), "Up".into()]),
        );
        bindings.insert(
            KeyAction::MoveDown,
            KeyBindingSpec::Multiple(vec!["j".into(), "Down".into()]),
        );
        bindings.insert(
            KeyAction::MoveToTop,
            KeyBindingSpec::Multiple(vec!["g".into(), "Home".into()]),
        );
        bindings.insert(
            KeyAction::MoveToBottom,
            KeyBindingSpec::Multiple(vec!["G".into(), "End".into()]),
        );
        bindings.insert(KeyAction::PageUp, KeyBindingSpec::Single("Ctrl+u".into()));
        bindings.insert(KeyAction::PageDown, KeyBindingSpec::Single("Ctrl+d".into()));
        bindings.insert(KeyAction::FocusNextPane, KeyBindingSpec::Single("Tab".into()));
        bindings.insert(
            KeyAction::FocusPrevPane,
            KeyBindingSpec::Single("BackTab".into()),
        );
        bindings.insert(
            KeyAction::FocusLeft,
            KeyBindingSpec::Multiple(vec!["h".into(), "Left".into()]),
        );
        bindings.insert(
            KeyAction::FocusRight,
            KeyBindingSpec::Multiple(vec!["l".into(), "Right".into()]),
        );

        // Selection
        bindings.insert(KeyAction::SelectItem, KeyBindingSpec::Single("Enter".into()));
        bindings.insert(KeyAction::ToggleSelection, KeyBindingSpec::Single("v".into()));
        bindings.insert(KeyAction::SelectAll, KeyBindingSpec::Single("V".into()));
        bindings.insert(
            KeyAction::DeselectAll,
            KeyBindingSpec::Single("Escape".into()),
        );

        // Actions
        bindings.insert(KeyAction::AddDownload, KeyBindingSpec::Single("a".into()));
        bindings.insert(KeyAction::DeleteDownload, KeyBindingSpec::Single("d".into()));
        bindings.insert(KeyAction::ToggleDownload, KeyBindingSpec::Single("Space".into()));
        bindings.insert(KeyAction::RetryDownload, KeyBindingSpec::Single("r".into()));
        bindings.insert(KeyAction::ResumeAll, KeyBindingSpec::Single("S".into()));
        bindings.insert(KeyAction::PauseAll, KeyBindingSpec::Single("P".into()));
        bindings.insert(KeyAction::OpenContextMenu, KeyBindingSpec::Single("m".into()));
        bindings.insert(KeyAction::EditItem, KeyBindingSpec::Single("e".into()));

        // View
        bindings.insert(KeyAction::ToggleDetails, KeyBindingSpec::Single("i".into()));
        bindings.insert(KeyAction::OpenSearch, KeyBindingSpec::Single("/".into()));
        bindings.insert(KeyAction::OpenHelp, KeyBindingSpec::Single("?".into()));
        bindings.insert(KeyAction::OpenSettings, KeyBindingSpec::Single("x".into()));
        bindings.insert(KeyAction::SwitchFolder, KeyBindingSpec::Single("F".into()));

        // System
        bindings.insert(
            KeyAction::Quit,
            KeyBindingSpec::Multiple(vec!["q".into(), "Ctrl+c".into()]),
        );
        bindings.insert(KeyAction::Undo, KeyBindingSpec::Single("Ctrl+z".into()));
        bindings.insert(KeyAction::Refresh, KeyBindingSpec::Single("R".into()));

        Self { bindings }
    }
}

/// Runtime keybinding resolver
#[derive(Debug, Clone)]
pub struct KeybindingResolver {
    /// Maps key combinations to actions
    action_map: HashMap<KeyCombo, KeyAction>,
}

impl KeybindingResolver {
    /// Create a new resolver from configuration
    pub fn from_config(config: &KeybindingsConfig) -> Self {
        let mut action_map = HashMap::new();

        for (action, spec) in &config.bindings {
            let keys = match spec {
                KeyBindingSpec::Single(s) => vec![s.clone()],
                KeyBindingSpec::Multiple(v) => v.clone(),
            };

            for key_str in keys {
                if let Some(combo) = KeyCombo::parse(&key_str) {
                    action_map.insert(combo, *action);
                }
            }
        }

        Self { action_map }
    }

    /// Resolve a key event to an action
    pub fn resolve(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<KeyAction> {
        // Normalize shift modifier for uppercase characters
        let (code, modifiers) = normalize_key_event(code, modifiers);

        let combo = KeyCombo::new(code, modifiers);
        self.action_map.get(&combo).copied()
    }
}

/// Normalize key events to handle shifted characters consistently.
/// Terminals may or may not include SHIFT in modifiers for characters
/// that are inherently shifted (e.g., '?', '!', 'A'-'Z').
/// Strip SHIFT when the character itself already represents the shifted form.
fn normalize_key_event(code: KeyCode, modifiers: KeyModifiers) -> (KeyCode, KeyModifiers) {
    match code {
        KeyCode::Char(c) if !c.is_ascii_lowercase() => {
            (code, modifiers - KeyModifiers::SHIFT)
        }
        _ => (code, modifiers),
    }
}

impl Default for KeybindingResolver {
    fn default() -> Self {
        Self::from_config(&KeybindingsConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_char() {
        let combo = KeyCombo::parse("j").unwrap();
        assert_eq!(combo.code, KeyCode::Char('j'));
        assert!(combo.modifiers.is_empty());
    }

    #[test]
    fn test_parse_uppercase_char() {
        let combo = KeyCombo::parse("G").unwrap();
        assert_eq!(combo.code, KeyCode::Char('G'));
        assert!(combo.modifiers.is_empty());
    }

    #[test]
    fn test_parse_ctrl_key() {
        let combo = KeyCombo::parse("Ctrl+z").unwrap();
        assert_eq!(combo.code, KeyCode::Char('z'));
        assert!(combo.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(KeyCombo::parse("Enter").unwrap().code, KeyCode::Enter);
        assert_eq!(KeyCombo::parse("Space").unwrap().code, KeyCode::Char(' '));
        assert_eq!(KeyCombo::parse("Tab").unwrap().code, KeyCode::Tab);
        assert_eq!(KeyCombo::parse("Escape").unwrap().code, KeyCode::Esc);
        assert_eq!(KeyCombo::parse("Up").unwrap().code, KeyCode::Up);
        assert_eq!(KeyCombo::parse("Down").unwrap().code, KeyCode::Down);
    }

    #[test]
    fn test_default_keybindings() {
        let resolver = KeybindingResolver::default();

        // Test basic navigation
        assert_eq!(
            resolver.resolve(KeyCode::Char('j'), KeyModifiers::empty()),
            Some(KeyAction::MoveDown)
        );
        assert_eq!(
            resolver.resolve(KeyCode::Down, KeyModifiers::empty()),
            Some(KeyAction::MoveDown)
        );
        assert_eq!(
            resolver.resolve(KeyCode::Char('k'), KeyModifiers::empty()),
            Some(KeyAction::MoveUp)
        );

        // Test actions
        assert_eq!(
            resolver.resolve(KeyCode::Char('a'), KeyModifiers::empty()),
            Some(KeyAction::AddDownload)
        );
        assert_eq!(
            resolver.resolve(KeyCode::Char(' '), KeyModifiers::empty()),
            Some(KeyAction::ToggleDownload)
        );

        // Test ctrl combinations
        assert_eq!(
            resolver.resolve(KeyCode::Char('z'), KeyModifiers::CONTROL),
            Some(KeyAction::Undo)
        );
    }

    #[test]
    fn test_uppercase_normalization() {
        let resolver = KeybindingResolver::default();

        // G should work for MoveToBottom
        assert_eq!(
            resolver.resolve(KeyCode::Char('G'), KeyModifiers::empty()),
            Some(KeyAction::MoveToBottom)
        );

        // S should work for ResumeAll
        assert_eq!(
            resolver.resolve(KeyCode::Char('S'), KeyModifiers::empty()),
            Some(KeyAction::ResumeAll)
        );
    }
}
