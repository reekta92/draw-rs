#![cfg_attr(test, allow(clippy::unwrap_used))]
#![doc = include_str!("../README.md")]
pub mod geometry;
pub mod state;
pub use state::*;
pub mod app;
mod fsutil;
pub mod input;
pub mod render;
mod snapshot;
mod ui;
pub use app::{DrawEventAction, DrawInteraction, DrawMenuTarget, DrawState};
pub use input::{
    apply_action, cancel_interaction_or_selection, handle_draw_mouse, handle_menu_shortcut,
    handle_selected_menu_shortcut,
};
pub use render::{DrawHints, ThemeColors, draw_view, smooth_points};
pub use snapshot::draw_snapshot;
pub use ui::{CanvasContextMenu, CanvasMenuItemSpec, CanvasSelection, MarqueeDragState};
pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawAction {
    Quit,
    Help,
    SelectCursorTool,
    SelectDrawTool,
    ToggleShapeSelector,
    SelectTextTool,
    SelectEraseTool,
    ShapeSelectorUp,
    ShapeSelectorDown,
    ShapeSelectorConfirm,
    ShapeSelectorCancel,
    ToggleColorSelector,
    ColorSelectorUp,
    ColorSelectorDown,
    ColorSelectorConfirm,
    ColorSelectorCancel,
    TextEditorConfirm,
    TextEditorCancel,
    MenuClose,
    MenuUp,
    MenuDown,
    MenuSelect,
    Copy,
    Paste,
    Undo,
    Redo,
    ToggleGrid,
}
pub const COLOR_PICKER_PALETTE: &[(&str, &str, ratatui::style::Color)] = &[
    ("Red", "#ff5252", ratatui::style::Color::Rgb(255, 82, 82)),
    ("Orange", "#ff9800", ratatui::style::Color::Rgb(255, 152, 0)),
    (
        "Yellow",
        "#ffeb3b",
        ratatui::style::Color::Rgb(255, 235, 59),
    ),
    ("Green", "#4caf50", ratatui::style::Color::Rgb(76, 175, 80)),
    ("Cyan", "#00bcd4", ratatui::style::Color::Rgb(0, 188, 212)),
    (
        "Purple",
        "#9c27b0",
        ratatui::style::Color::Rgb(156, 39, 176),
    ),
    ("Blue", "#2196f3", ratatui::style::Color::Rgb(33, 150, 243)),
    (
        "Magenta",
        "#e91e63",
        ratatui::style::Color::Rgb(233, 30, 99),
    ),
    (
        "White",
        "#ffffff",
        ratatui::style::Color::Rgb(255, 255, 255),
    ),
];
