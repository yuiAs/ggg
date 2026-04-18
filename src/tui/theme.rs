//! TUI color palette.
//!
//! Centralized color constants for every widget rendered by the TUI. Values are
//! tuned to stay readable on pure-black terminal backgrounds: dark accent
//! values from the original palette were lifted to keep contrast above roughly
//! 4.5:1 against `rgb(0, 0, 0)`.
//!
//! Background constants prefer `Color::Reset` over `Color::Black` so the
//! terminal's own default background shines through. `Color::Black` would
//! otherwise force ANSI color 0 and override the user's terminal theme.
//!
//! External theme loading (e.g. via `config.toml`) is intentionally unsupported
//! at this time; these defaults are the single source of truth.

use ratatui::style::Color;

// --- Borders ---------------------------------------------------------------

/// Border color for the currently focused pane.
pub const BORDER_ACTIVE: Color = Color::Rgb(255, 220, 100);

/// Border color for inactive panes.
pub const BORDER_INACTIVE: Color = Color::Rgb(130, 130, 155);

/// Border accent used by dialog frames.
pub const BORDER_DIALOG: Color = Color::Rgb(140, 175, 215);

// --- Text ------------------------------------------------------------------

/// Primary body text.
pub const TEXT_NORMAL: Color = Color::Rgb(210, 210, 220);

/// Secondary text (hints, paths, labels).
pub const TEXT_MUTED: Color = Color::Rgb(170, 170, 185);

/// Tertiary text (italic help / inline descriptions).
pub const TEXT_DESCRIPTION: Color = Color::Rgb(150, 155, 175);

/// Dimmest text. Reserved for placeholders / disabled-but-inherited items.
pub const TEXT_DIM: Color = Color::Rgb(135, 135, 150);

// --- Accents ---------------------------------------------------------------

/// Selection / active accent (warm yellow).
pub const ACCENT_SELECTED: Color = Color::Rgb(255, 220, 100);

/// Section header accent (cool blue).
pub const ACCENT_SECTION: Color = Color::Rgb(140, 175, 215);

/// Highlight accent for special paths / informational keys.
pub const ACCENT_PURPLE: Color = Color::Rgb(190, 170, 230);

// --- Backgrounds -----------------------------------------------------------

/// Background for main panels. `Reset` defers to the terminal's default
/// background so user terminal themes are respected.
pub const BG_PRIMARY: Color = Color::Reset;

/// Background for the highlighted row in a list or table.
pub const BG_SELECTION: Color = Color::Rgb(50, 60, 95);

/// Background for table header rows.
pub const BG_TABLE_HEADER: Color = Color::Rgb(130, 140, 170);

/// Foreground paired with `BG_TABLE_HEADER`.
pub const FG_ON_TABLE_HEADER: Color = Color::Black;

// --- Semantic status -------------------------------------------------------

pub const STATUS_SUCCESS: Color = Color::Rgb(120, 200, 140);
pub const STATUS_ERROR: Color = Color::Rgb(230, 110, 110);

// --- Download-state column -------------------------------------------------

pub const DL_PENDING: Color = Color::Rgb(255, 200, 100);
pub const DL_DOWNLOADING: Color = Color::Rgb(100, 200, 255);
pub const DL_PAUSED: Color = Color::Rgb(170, 170, 185);
pub const DL_COMPLETED: Color = Color::Rgb(100, 220, 130);
pub const DL_ERROR: Color = Color::Rgb(255, 110, 110);
pub const DL_DELETED: Color = Color::Rgb(155, 155, 170);
