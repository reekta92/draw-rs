use crate::state::{DrawData, DrawItemId, Viewport};

#[derive(Debug, PartialEq, Eq)]
pub enum DrawEventAction {
    Continue,
    CopyText(String),
    Quit,
    OpenHelp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawInteraction {
    Move {
        id: DrawItemId,
        start_world: (f64, f64),
        original_translation: (f64, f64),
        preview_translation: (f64, f64),
    },
    Rotate {
        id: DrawItemId,
        pivot_world: (f64, f64),
        original_degrees: f64,
        preview_degrees: f64,
        start_angle: Option<f64>,
    },
    Scale {
        id: DrawItemId,
        pivot_world: (f64, f64),
        original_scale: f64,
        preview_scale: f64,
        start_distance: Option<f64>,
    },
    Paste {
        item: crate::state::DrawItem,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawMenuTarget {
    NonText(DrawItemId),
    Text(DrawItemId),
    Empty { x: f64, y: f64 },
}

impl DrawMenuTarget {
    #[must_use]
    pub fn item_id(&self) -> Option<&DrawItemId> {
        match self {
            Self::NonText(id) | Self::Text(id) => Some(id),
            Self::Empty { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawMenuKind {
    Actions,
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawMenuItem {
    Rotate,
    Scale,
    Color,
    Copy,
    Erase,
    EditText,
    Paste,
}

impl DrawMenuItem {
    const fn label(self) -> &'static str {
        match self {
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
            Self::Color => "Color...",
            Self::Copy => "Copy",
            Self::Erase => "Erase",
            Self::EditText => "Edit Text",
            Self::Paste => "Paste",
        }
    }

    const fn shortcut(self) -> char {
        match self {
            Self::Rotate => 'r',
            Self::Scale => 's',
            Self::Color => 'o',
            Self::Copy => 'c',
            Self::Erase => 'e',
            Self::EditText => 't',
            Self::Paste => 'v',
        }
    }
}

#[must_use]
pub fn draw_menu_items(
    target: &DrawMenuTarget,
    clipboard_available: bool,
) -> &'static [DrawMenuItem] {
    const NON_TEXT: &[DrawMenuItem] = &[
        DrawMenuItem::Rotate,
        DrawMenuItem::Scale,
        DrawMenuItem::Color,
        DrawMenuItem::Copy,
        DrawMenuItem::Erase,
    ];
    const TEXT: &[DrawMenuItem] = &[
        DrawMenuItem::EditText,
        DrawMenuItem::Color,
        DrawMenuItem::Copy,
        DrawMenuItem::Erase,
    ];
    const PASTE: &[DrawMenuItem] = &[DrawMenuItem::Paste];

    match target {
        DrawMenuTarget::NonText(_) => NON_TEXT,
        DrawMenuTarget::Text(_) => TEXT,
        DrawMenuTarget::Empty { .. } if clipboard_available => PASTE,
        DrawMenuTarget::Empty { .. } => &[],
    }
}

pub(crate) fn draw_menu_shortcut_index(
    target: &DrawMenuTarget,
    clipboard_available: bool,
    shortcut: char,
) -> Option<usize> {
    let shortcut = shortcut.to_ascii_lowercase();
    draw_menu_items(target, clipboard_available)
        .iter()
        .position(|item| item.shortcut().to_ascii_lowercase() == shortcut)
}

#[must_use]
pub fn draw_menu_specs(
    target: &DrawMenuTarget,
    clipboard_available: bool,
) -> Vec<crate::ui::CanvasMenuItemSpec> {
    draw_menu_items(target, clipboard_available)
        .iter()
        .map(|item| crate::ui::CanvasMenuItemSpec::new(item.label()).shortcut(item.shortcut()))
        .collect()
}

#[must_use]
pub fn draw_color_menu_specs() -> Vec<crate::ui::CanvasMenuItemSpec> {
    crate::COLOR_PICKER_PALETTE
        .iter()
        .map(|(label, _, color)| {
            let shortcut = match *label {
                "Red" => 'r',
                "Orange" => 'o',
                "Yellow" => 'y',
                "Green" => 'g',
                "Cyan" => 'c',
                "Purple" => 'p',
                "Blue" => 'b',
                "Magenta" => 'm',
                "White" => 'w',
                _ => unreachable!("fixed color palette has known names"),
            };
            crate::ui::CanvasMenuItemSpec::new(label)
                .shortcut(shortcut)
                .color(*color)
        })
        .collect()
}

use ratatui::layout::Rect;
use ratatui_textarea::TextArea;

pub struct DrawState {
    pub data: DrawData,
    pub viewport: Viewport,
    path: Option<std::path::PathBuf>,

    pub active_tool: crate::state::DrawTool,
    pub selection: crate::ui::CanvasSelection<DrawItemId>,
    pub hovered: Option<DrawItemId>,
    pub interaction: Option<DrawInteraction>,
    pub current_stroke: Option<crate::state::Stroke>,
    pub last_area: Rect,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub mouse_pos: Option<(u16, u16)>,
    pub last_click: Option<(u16, u16, std::time::Instant)>,
    pub right_mouse: crate::ui::MarqueeDragState,
    pub right_mouse_screen: Option<(u16, u16)>,
    pub right_mouse_target: Option<DrawItemId>,
    pub context_menu: Option<crate::ui::CanvasContextMenu>,
    pub menu_target: Option<DrawMenuTarget>,
    pub menu_kind: Option<DrawMenuKind>,
    pub text_editor: Option<(DrawItemId, TextArea<'static>)>,
    pub text_editor_rect: Option<Rect>,
    pub(crate) mouse_selection: crate::ui::MouseTextSelection,
    pub active_shape_type: crate::state::DrawShapeType,
    pub show_shape_selector: bool,
    pub show_color_selector: bool,
    pub active_color: (u8, u8, u8),
    pub creation_origin: Option<(f64, f64)>,
    pub preview_element: Option<crate::state::DrawElement>,
    pub(crate) erase_start_data: Option<DrawData>,
    pub grid: bool,
    pub is_panning: bool,
    status_notice: Option<&'static str>,
    pub undo_stack: Vec<DrawData>,
    pub redo_stack: Vec<DrawData>,
}

impl DrawState {
    pub fn input_available(&self) -> bool {
        self.text_editor.is_none()
            && self.context_menu.is_none()
            && !self.show_shape_selector
            && !self.show_color_selector
            && self.interaction.is_none()
            && self.current_stroke.is_none()
            && self.creation_origin.is_none()
            && self.preview_element.is_none()
            && self.erase_start_data.is_none()
            && !self.is_panning
            && self.right_mouse_screen.is_none()
            && !self.mouse_selection.active
    }
    pub fn new() -> Self {
        Self {
            data: DrawData::default(),
            viewport: Viewport::default(),
            path: None,

            active_tool: crate::state::DrawTool::Cursor,
            selection: crate::ui::CanvasSelection::new(),
            hovered: None,
            interaction: None,
            current_stroke: None,
            last_area: Rect::default(),
            mouse_pos: None,
            last_mouse_pos: None,
            last_click: None,
            right_mouse: crate::ui::MarqueeDragState::new(3),
            right_mouse_screen: None,
            right_mouse_target: None,
            context_menu: None,
            menu_target: None,
            menu_kind: None,
            text_editor: None,
            text_editor_rect: None,
            mouse_selection: crate::ui::MouseTextSelection::default(),
            active_shape_type: crate::state::DrawShapeType::Rect,
            show_shape_selector: false,
            show_color_selector: false,
            active_color: (255, 255, 255),
            creation_origin: None,
            preview_element: None,
            erase_start_data: None,
            grid: true,
            is_panning: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            status_notice: None,
        }
    }

    pub fn set_active_tool(&mut self, tool: crate::state::DrawTool) {
        self.clear_transient_interaction();
        self.active_tool = tool;
    }
    const TRANSIENT_MODE_MESSAGES: [&str; 3] = [
        "ROTATE MODE: Drag pointer to rotate, Left-click to begin",
        "SCALE MODE: Drag pointer to scale, Left-click to begin",
        "PASTE MODE: Move pointer, Left-click to place",
    ];

    #[must_use]
    pub fn active_mode_message(&self) -> Option<&'static str> {
        match self.interaction.as_ref() {
            Some(DrawInteraction::Rotate { .. }) => Some(Self::TRANSIENT_MODE_MESSAGES[0]),
            Some(DrawInteraction::Scale { .. }) => Some(Self::TRANSIENT_MODE_MESSAGES[1]),
            Some(DrawInteraction::Paste { .. }) => Some(Self::TRANSIENT_MODE_MESSAGES[2]),
            Some(DrawInteraction::Move { .. }) | None => None,
        }
    }

    pub fn notify(&mut self, message: &'static str) {
        self.status_notice = Some(message);
    }

    pub fn take_notice(&mut self) -> Option<&'static str> {
        self.status_notice.take()
    }

    pub fn text_editor_mut(&mut self) -> Option<&mut TextArea<'static>> {
        self.text_editor.as_mut().map(|(_, editor)| editor)
    }

    pub fn insert_text(&mut self, text: &str) -> bool {
        if let Some(editor) = self.text_editor_mut() {
            editor.insert_str(text);
            true
        } else {
            false
        }
    }

    pub fn set_canvas_area(&mut self, area: Rect) {
        self.last_area = area;
        self.text_editor_rect = self
            .text_editor
            .as_ref()
            .map(|_| ratatui::widgets::Block::bordered().inner(crate::ui::editor_rect(area)));
    }

    #[must_use]
    pub fn topmost_hit(&self, world_point: (f64, f64)) -> Option<DrawItemId> {
        let hit = |item: &crate::state::DrawItem| {
            crate::geometry::hit_test_item(item, world_point, 5.0, &self.viewport)
        };
        self.data
            .elements
            .iter()
            .rev()
            .find(|item| matches!(&item.element, crate::state::DrawElement::Text(_)) && hit(item))
            .or_else(|| {
                self.data.elements.iter().rev().find(|item| {
                    !matches!(&item.element, crate::state::DrawElement::Text(_)) && hit(item)
                })
            })
            .map(|item| item.id.clone())
    }

    pub fn begin_text_editor(&mut self, id: DrawItemId) -> bool {
        let Some(crate::state::DrawElement::Text(text)) =
            self.data.item(&id).map(|item| &item.element)
        else {
            return false;
        };
        self.text_editor = Some((id, TextArea::new(vec![text.content.clone()])));
        true
    }

    pub fn open_context_menu(
        &mut self,
        x: u16,
        y: u16,
        target: DrawMenuTarget,
        clipboard_available: bool,
    ) {
        let specs = draw_menu_specs(&target, clipboard_available);
        if specs.is_empty() {
            return;
        }
        self.context_menu = Some(crate::ui::CanvasContextMenu::new(x, y, specs));
        self.menu_target = Some(target);
        self.menu_kind = Some(DrawMenuKind::Actions);
    }

    pub fn open_color_menu(&mut self, x: u16, y: u16) {
        self.context_menu = Some(crate::ui::CanvasContextMenu::new(
            x,
            y,
            draw_color_menu_specs(),
        ));
        self.menu_kind = Some(DrawMenuKind::Color);
    }

    pub fn record_undo_state(&mut self, previous: DrawData) {
        Self::push_history(&mut self.undo_stack, previous);
        self.redo_stack.clear();
    }

    pub fn commit_data_change(&mut self, previous: DrawData) -> anyhow::Result<bool> {
        if self.data == previous {
            return Ok(false);
        }
        self.record_undo_state(previous);
        self.save()?;
        Ok(true)
    }

    pub fn undo(&mut self) -> anyhow::Result<bool> {
        let Some(previous) = self.undo_stack.pop() else {
            return Ok(false);
        };
        let current = std::mem::replace(&mut self.data, previous);
        Self::push_history(&mut self.redo_stack, current);
        self.clear_transient_interaction();
        self.save()?;
        Ok(true)
    }

    pub fn redo(&mut self) -> anyhow::Result<bool> {
        let Some(next) = self.redo_stack.pop() else {
            return Ok(false);
        };
        let current = std::mem::replace(&mut self.data, next);
        Self::push_history(&mut self.undo_stack, current);
        self.clear_transient_interaction();
        self.save()?;
        Ok(true)
    }

    pub fn clear_transient_interaction(&mut self) {
        self.current_stroke = None;
        self.creation_origin = None;
        self.preview_element = None;
        self.erase_start_data = None;
        self.text_editor = None;
        self.text_editor_rect = None;
        self.mouse_selection = crate::ui::MouseTextSelection::default();
        self.show_shape_selector = false;
        self.show_color_selector = false;
        self.is_panning = false;
        self.last_mouse_pos = None;
        self.hovered = None;
        self.selection.clear();
        self.interaction = None;
        self.last_click = None;
        self.right_mouse.clear();
        self.right_mouse_screen = None;
        self.right_mouse_target = None;
        self.context_menu = None;
        self.menu_target = None;
        self.menu_kind = None;
    }

    fn push_history(stack: &mut Vec<DrawData>, data: DrawData) {
        const MAX_HISTORY: usize = 20;
        if stack.len() == MAX_HISTORY {
            stack.remove(0);
        }
        stack.push(data);
    }

    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// Load an existing regular file without changing its bytes.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        use std::io::Read;
        let path = path.canonicalize()?;
        anyhow::ensure!(path.metadata()?.is_file(), "Drawing is not a regular file");
        let file = std::fs::File::open(&path)?;
        let metadata = file.metadata()?;
        anyhow::ensure!(metadata.is_file(), "Drawing is not a regular file");
        anyhow::ensure!(
            metadata.len() <= crate::MAX_DOCUMENT_BYTES as u64,
            "Drawing exceeds 16 MiB"
        );
        let mut content = String::new();
        file.take(crate::MAX_DOCUMENT_BYTES as u64 + 1)
            .read_to_string(&mut content)?;
        anyhow::ensure!(
            content.len() <= crate::MAX_DOCUMENT_BYTES,
            "Drawing exceeds 16 MiB"
        );
        Ok(Self {
            data: serde_json::from_str(&content)?,
            path: Some(path),
            ..Self::new()
        })
    }

    /// Atomically create a new drawing without replacing an existing path.
    pub fn create(path: &std::path::Path) -> anyhow::Result<Self> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let path = parent.canonicalize()?.join(
            path.file_name()
                .ok_or_else(|| anyhow::anyhow!("Drawing requires a filename"))?,
        );
        let state = Self {
            path: Some(path),
            ..Self::new()
        };
        crate::fsutil::atomic_create(
            state.path().expect("creation has a path"),
            &state.serialized()?,
        )?;
        Ok(state)
    }

    fn serialized(&self) -> anyhow::Result<Vec<u8>> {
        // Limit during serialization as well, so successful saves are always reopenable.
        struct Bounded(Vec<u8>);
        impl std::io::Write for Bounded {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if bytes.len() > crate::MAX_DOCUMENT_BYTES.saturating_sub(self.0.len()) {
                    return Err(std::io::Error::other("Drawing exceeds 16 MiB"));
                }
                self.0.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut output = Bounded(Vec::new());
        serde_json::to_writer(&mut output, &self.data)?;
        Ok(output.0)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(path) = self.path() {
            crate::fsutil::atomic_write(path, &self.serialized()?)?;
        } else {
            anyhow::ensure!(
                self.data == DrawData::default(),
                "Drawing has changes but no save target"
            );
        }
        Ok(())
    }
}

impl Default for DrawState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> (tempfile::TempDir, DrawState) {
        let temp = tempfile::tempdir().unwrap();
        let state = DrawState::create(&temp.path().join("history.draw")).unwrap();
        (temp, state)
    }

    #[test]
    fn history_caps_invalidates_redo_and_saves_restores() {
        let (_temp, mut state) = test_state();
        let unchanged = state.data.clone();
        assert!(!state.commit_data_change(unchanged).unwrap());
        assert_eq!(state.undo_stack.len(), 0);

        for index in 1..=21 {
            let previous = state.data.clone();
            state.data.width = 1000.0 + index as f64;
            assert!(state.commit_data_change(previous).unwrap());
        }
        assert_eq!(state.undo_stack.len(), 20);

        let selected = DrawItemId::new();
        state.selection.select_only(selected.clone());
        state.hovered = Some(selected);
        state.current_stroke = Some(crate::state::Stroke {
            points: vec![(0.0, 0.0)],
            color: (0, 0, 0),
        });
        assert!(state.undo().unwrap());
        assert_eq!(state.redo_stack.len(), 1);
        let empty = state.selection.is_empty();
        assert!(empty);
        assert!(state.hovered.is_none());
        assert!(state.current_stroke.is_none());

        let previous = state.data.clone();
        state.data.height = 777.0;
        assert!(state.commit_data_change(previous).unwrap());
        assert_eq!(state.redo_stack.len(), 0);

        let saved = std::fs::read_to_string(state.path().unwrap()).unwrap();
        assert_eq!(
            serde_json::from_str::<DrawData>(&saved).unwrap(),
            state.data
        );
    }

    #[test]
    fn topmost_hit_prefers_text_then_latest_vector() {
        let (_temp, mut state) = test_state();
        let text =
            crate::state::DrawItem::new(crate::state::DrawElement::Text(crate::state::Text {
                content: "text".to_string(),
                x: 0.0,
                y: 0.0,
                color: (255, 255, 255),
            }));
        let text_id = text.id.clone();
        state.data.elements.push(text);
        state.data.elements.push(crate::state::DrawItem::new(
            crate::state::DrawElement::Shape(crate::state::Shape::Line {
                x1: 0.0,
                y1: 1.0,
                x2: 10.0,
                y2: 1.0,
                color: (255, 255, 255),
            }),
        ));
        assert_eq!(state.topmost_hit((1.0, 1.0)), Some(text_id));

        state.data.elements.remove(0);
        let vector = crate::state::DrawItem::new(crate::state::DrawElement::Shape(
            crate::state::Shape::Line {
                x1: 0.0,
                y1: 1.0,
                x2: 10.0,
                y2: 1.0,
                color: (255, 255, 255),
            },
        ));
        let vector_id = vector.id.clone();
        state.data.elements.push(vector);
        assert_eq!(state.topmost_hit((1.0, 1.0)), Some(vector_id));
    }

    #[test]
    fn active_mode_message_only_tracks_transient_interactions() {
        let (_temp, mut state) = test_state();
        assert_eq!(state.active_mode_message(), None);

        state.set_active_tool(crate::state::DrawTool::Text);
        assert_eq!(state.active_mode_message(), None);

        state.interaction = Some(DrawInteraction::Paste {
            item: crate::state::DrawItem::new(crate::state::DrawElement::Stroke(
                crate::state::Stroke {
                    points: vec![(0.0, 0.0)],
                    color: (255, 255, 255),
                },
            )),
        });
        assert_eq!(
            state.active_mode_message(),
            Some("PASTE MODE: Move pointer, Left-click to place")
        );
    }

    #[test]
    fn context_menu_specs_match_draw_item_scope() {
        let id = DrawItemId::new();
        let labels = |target: DrawMenuTarget, clipboard_available| {
            draw_menu_specs(&target, clipboard_available)
                .into_iter()
                .map(|spec| (spec.label, spec.shortcut))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            labels(DrawMenuTarget::NonText(id.clone()), false),
            vec![
                ("Rotate", Some('r')),
                ("Scale", Some('s')),
                ("Color...", Some('o')),
                ("Copy", Some('c')),
                ("Erase", Some('e')),
            ]
        );
        assert_eq!(
            labels(DrawMenuTarget::Text(id), false),
            vec![
                ("Edit Text", Some('t')),
                ("Color...", Some('o')),
                ("Copy", Some('c')),
                ("Erase", Some('e')),
            ]
        );
        assert_eq!(
            draw_menu_specs(&DrawMenuTarget::Empty { x: 0.0, y: 0.0 }, false).len(),
            0
        );
        assert_eq!(
            labels(DrawMenuTarget::Empty { x: 0.0, y: 0.0 }, true),
            vec![("Paste", Some('v'))]
        );

        let id = DrawItemId::new();
        assert_eq!(
            draw_menu_shortcut_index(&DrawMenuTarget::NonText(id.clone()), false, 'E'),
            Some(4)
        );
        assert_eq!(
            draw_menu_shortcut_index(&DrawMenuTarget::Text(id), false, 'T'),
            Some(0)
        );
    }
}
