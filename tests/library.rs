#![allow(clippy::unwrap_used)]
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use draw::{DrawAction, DrawData, DrawElement, DrawItem, DrawShapeType, DrawState, DrawTool, Text};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

fn mouse(
    state: &mut DrawState,
    kind: MouseEventKind,
    x: u16,
    y: u16,
) -> anyhow::Result<draw::DrawEventAction> {
    draw::handle_draw_mouse(
        state,
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        },
        &mut None,
    )
}

#[test]
fn loads_are_bounded_fallible_and_never_write() {
    let dir = tempfile::tempdir().unwrap();
    assert!(DrawState::load(&dir.path().join("missing.draw")).is_err());
    assert!(DrawState::load(dir.path()).is_err());
    for (name, content) in [
        ("malformed", "invalid".to_string()),
        (
            "future",
            r#"{"version":99,"width":1,"height":1,"elements":[]}"#.into(),
        ),
        ("oversized", " ".repeat(draw::MAX_DOCUMENT_BYTES + 1)),
        ("legacy", include_str!("fixtures/legacy.draw").into()),
        ("v2", include_str!("fixtures/v2.draw").into()),
    ] {
        let path = dir.path().join(format!("{name}.draw"));
        std::fs::write(&path, &content).unwrap();
        let result = DrawState::load(&path);
        assert_eq!(result.is_ok(), matches!(name, "legacy" | "v2"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), content);
    }
}

#[test]
fn creation_never_clobbers_and_failed_publication_cleans_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("art.draw");
    let state = DrawState::create(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(DrawState::create(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "blocker").unwrap();
    assert!(DrawState::create(&blocker.join("art.draw")).is_err());
    assert_eq!(
        DrawState::load(state.path().unwrap()).unwrap().data,
        DrawData::default()
    );
}

#[test]
fn oversized_output_and_fileless_edits_cannot_be_published() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("art.draw");
    let mut state = DrawState::create(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    state
        .data
        .elements
        .push(DrawItem::new(DrawElement::Text(Text {
            content: "x".repeat(draw::MAX_DOCUMENT_BYTES),
            x: 0.0,
            y: 0.0,
            color: (1, 2, 3),
        })));
    assert!(state.save().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let mut fileless = DrawState::new();
    assert!(fileless.save().is_ok());
    fileless.data.width += 1.0;
    assert!(
        fileless
            .save()
            .unwrap_err()
            .to_string()
            .contains("no save target")
    );
}

#[test]
fn nonfinite_output_never_replaces_a_valid_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("finite.draw");
    let mut state = DrawState::create(&path).unwrap();
    state
        .data
        .elements
        .push(DrawItem::new(DrawElement::Text(Text {
            content: "keep me".into(),
            x: 0.0,
            y: 0.0,
            color: (1, 2, 3),
        })));
    state.save().unwrap();
    let original = std::fs::read(&path).unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let DrawElement::Text(text) = &mut state.data.elements[0].element else {
            unreachable!();
        };
        text.x = value;
        assert!(state.save().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(DrawState::load(&path).is_ok());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}

#[test]
fn failed_save_keeps_edit_and_single_history_entry_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("art.draw");
    let mut state = DrawState::create(&path).unwrap();
    let original_permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = original_permissions.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    let previous = state.data.clone();
    state.data.width = 123.0;
    let failed = state.commit_data_change(previous).is_err();
    std::fs::set_permissions(&path, original_permissions.clone()).unwrap();
    assert!(failed);
    assert_eq!(state.data.width, 123.0);
    assert_eq!(state.undo_stack.len(), 1);
    state.save().unwrap();
    assert_eq!(state.undo_stack.len(), 1);
    assert_eq!(DrawState::load(&path).unwrap().data.width, 123.0);
    let mut readonly = original_permissions.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    let undo_failed = state.undo().is_err();
    let undone = state.data.clone();
    let redo_failed = state.redo().is_err();
    std::fs::set_permissions(&path, original_permissions).unwrap();
    assert!(undo_failed && redo_failed);
    assert_eq!(undone.width, 1000.0);
    assert_eq!(state.data.width, 123.0);
    assert_eq!(state.undo_stack.len(), 1);
    state.save().unwrap();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn replacement_preserves_permissions_and_resolves_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("real.draw");
    DrawState::create(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let link = dir.path().join("alias.draw");
    symlink(&path, &link).unwrap();
    let mut state = DrawState::load(&link).unwrap();
    assert_eq!(state.path().unwrap(), path.canonicalize().unwrap());
    state.data.width = 5.0;
    state.save().unwrap();
    assert!(link.is_symlink());
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn all_tools_complete_one_operation_and_captured_release_outside_canvas() {
    let dir = tempfile::tempdir().unwrap();
    for shape in [
        DrawShapeType::Rect,
        DrawShapeType::Ellipse,
        DrawShapeType::Diamond,
        DrawShapeType::Line,
        DrawShapeType::Arrow,
    ] {
        let path = dir.path().join(format!("{shape:?}.draw"));
        let mut state = DrawState::create(&path).unwrap();
        state.active_shape_type = shape;
        state.set_active_tool(DrawTool::Shape);
        state.set_canvas_area(Rect::new(10, 5, 100, 50));
        mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 30, 20).unwrap();
        mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), 60, 35).unwrap();
        assert_eq!(state.data.elements.len(), 0);
        mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 0, 0).unwrap();
        assert_eq!(state.data.elements.len(), 1);
        assert_eq!(state.undo_stack.len(), 1);
        assert!(state.input_available());
        assert_eq!(DrawState::load(&path).unwrap().data, state.data);
    }
    let path = dir.path().join("stroke.draw");
    let mut state = DrawState::create(&path).unwrap();
    state.set_canvas_area(Rect::new(10, 5, 100, 50));
    state.set_active_tool(DrawTool::Draw);
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 0, 0).unwrap();
    assert!(state.current_stroke.is_none());
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 30, 20).unwrap();
    mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), 60, 35).unwrap();
    mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 60, 35).unwrap();
    assert_eq!(state.undo_stack.len(), 1);
    state.set_active_tool(DrawTool::Erase);
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 30, 20).unwrap();
    mouse(&mut state, MouseEventKind::Drag(MouseButton::Left), 60, 35).unwrap();
    mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 60, 35).unwrap();
    assert_eq!(state.data.elements.len(), 0);
    assert_eq!(state.undo_stack.len(), 2);
    state.set_active_tool(DrawTool::Text);
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 30, 20).unwrap();
    let id = state.data.elements[0].id.clone();
    assert!(state.begin_text_editor(id.clone()));
    let editor = state.text_editor_mut().unwrap();
    editor.select_all();
    editor.insert_str("first\nsecond");
    draw::apply_action(&mut state, DrawAction::TextEditorConfirm, &mut None).unwrap();
    assert!(
        matches!(&state.data.item(&id).unwrap().element, DrawElement::Text(t) if t.content == "first")
    );
    let saved = state.data.clone();
    assert!(state.begin_text_editor(id));
    assert!(state.insert_text("draft"));
    draw::apply_action(&mut state, DrawAction::TextEditorCancel, &mut None).unwrap();
    assert_eq!(state.data, saved);
}

#[test]
fn click_strokes_remain_visible_in_canvas_and_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dot.draw");
    let mut state = DrawState::create(&path).unwrap();
    let area = Rect::new(0, 0, 100, 50);
    state.set_canvas_area(area);
    state.grid = false;
    state.set_active_tool(DrawTool::Draw);
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 50, 25).unwrap();
    mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 50, 25).unwrap();
    assert_eq!(state.data.elements.len(), 1);
    assert_eq!(state.undo_stack.len(), 1);
    let reopened = DrawState::load(&path).unwrap();
    assert!(
        matches!(&reopened.data.elements[0].element, DrawElement::Stroke(stroke) if stroke.points.len() == 1)
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 50)).unwrap();
    for snapshot in [false, true] {
        terminal
            .draw(|frame| {
                if snapshot {
                    draw::draw_snapshot(frame, area, &state.data, 1.0, 0.0, 0.0, None);
                } else {
                    draw::draw_view(
                        frame,
                        &mut state,
                        area,
                        &draw::ThemeColors::default(),
                        &draw::DrawHints::default(),
                        None,
                    );
                }
            })
            .unwrap();
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .filter(|cell| !matches!(cell.symbol(), " " | ""))
                .count(),
            1,
            "snapshot={snapshot}"
        );
    }
}

#[test]
fn tool_selector_and_paste_changes_cancel_erasure_without_losing_items() {
    let dir = tempfile::tempdir().unwrap();
    for action in [
        DrawAction::SelectCursorTool,
        DrawAction::SelectDrawTool,
        DrawAction::SelectTextTool,
        DrawAction::SelectEraseTool,
        DrawAction::ToggleShapeSelector,
        DrawAction::ToggleColorSelector,
        DrawAction::Paste,
    ] {
        let path = dir.path().join(format!("{action:?}.draw"));
        let mut state = DrawState::create(&path).unwrap();
        state.set_canvas_area(Rect::new(0, 0, 100, 50));
        state
            .data
            .elements
            .push(DrawItem::new(DrawElement::Stroke(draw::Stroke {
                points: vec![(-20.0, 0.0), (20.0, 0.0)],
                color: (255, 255, 255),
            })));
        state.commit_data_change(DrawData::default()).unwrap();
        let original = state.data.clone();
        state.set_active_tool(DrawTool::Erase);
        mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 50, 25).unwrap();
        assert_eq!(state.data.elements.len(), 0);

        draw::apply_action(&mut state, action, &mut Some(original.elements[0].clone())).unwrap();
        mouse(&mut state, MouseEventKind::Up(MouseButton::Left), 50, 25).unwrap();
        assert_eq!(state.data, original, "{action:?}");
        assert_eq!(state.undo_stack.len(), 1);
        assert_eq!(state.redo_stack.len(), 0);
        state.save().unwrap();
        assert_eq!(DrawState::load(&path).unwrap().data, original);
    }
}

#[test]
fn undo_and_redo_keep_cancelled_erasure_out_of_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("erase-history.draw");
    let mut state = DrawState::create(&path).unwrap();
    state.set_canvas_area(Rect::new(0, 0, 100, 50));
    state
        .data
        .elements
        .push(DrawItem::new(DrawElement::Stroke(draw::Stroke {
            points: vec![(-20.0, 0.0), (20.0, 0.0)],
            color: (255, 255, 255),
        })));
    state.commit_data_change(DrawData::default()).unwrap();
    let original = state.data.clone();
    state.data.width = 800.0;
    state.commit_data_change(original.clone()).unwrap();
    let resized = state.data.clone();
    state.set_active_tool(DrawTool::Erase);
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 50, 25).unwrap();
    assert_eq!(state.data.elements.len(), 0);

    assert!(state.undo().unwrap());
    assert_eq!(state.data, original);
    assert_eq!(state.redo_stack.last(), Some(&resized));
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 50, 25).unwrap();
    assert_eq!(state.data.elements.len(), 0);
    assert!(state.redo().unwrap());
    assert_eq!(state.data, resized);
    assert_eq!(state.undo_stack.last(), Some(&original));
    assert_eq!(DrawState::load(&path).unwrap().data, resized);
}

#[test]
fn nonzero_tiny_and_resized_canvases_keep_popups_inside_and_snapshots_clean() {
    let mut terminal = Terminal::new(TestBackend::new(100, 60)).unwrap();
    let mut state = DrawState::new();
    for area in [
        Rect::new(7, 9, 60, 30),
        Rect::new(7, 9, 1, 1),
        Rect::new(7, 9, 0, 0),
        Rect::new(20, 20, 4, 2),
    ] {
        state.show_color_selector = true;
        terminal
            .draw(|f| {
                draw::draw_view(
                    f,
                    &mut state,
                    area,
                    &draw::ThemeColors::default(),
                    &draw::DrawHints::default(),
                    None,
                )
            })
            .unwrap();
        let point = draw::input::screen_to_canvas(area.x, area.y, &state);
        assert!(point.0.is_finite() && point.1.is_finite());
        mouse(
            &mut state,
            MouseEventKind::Down(MouseButton::Left),
            area.x,
            area.y,
        )
        .unwrap();
        assert!(!state.show_color_selector);
        for bg in [None, Some(Color::Black)] {
            terminal
                .draw(|f| draw::draw_snapshot(f, area, &state.data, 1.0, 0.0, 0.0, bg))
                .unwrap();
        }
    }
    state.show_color_selector = false;
    state.set_canvas_area(Rect::new(10, 5, 100, 50));
    let a = draw::input::screen_to_canvas(59, 29, &state);
    let b = draw::input::screen_to_canvas(60, 30, &state);
    assert!((a.0 + b.0).abs() < 1e-10);
    assert!((a.1 + b.1).abs() < 1e-10);
}

#[test]
fn binary_help_succeeds_without_a_terminal_or_creating_files() {
    let dir = tempfile::tempdir().unwrap();
    for arg in ["-h", "--help"] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_draw"))
            .arg(arg)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, b"Usage: draw <path.draw>\n");
        assert_eq!(result.stderr, b"");
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn binary_load_errors_precede_terminal_setup() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("bad.draw"), "invalid").unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_draw"))
        .arg("bad.draw")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(result.stdout.len(), 0);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("bad.draw")).unwrap(),
        "invalid"
    );
}

#[test]
fn selector_actions_only_change_active_selectors() {
    let mut state = DrawState::new();
    let shape = state.active_shape_type;
    let color = state.active_color;
    for action in [
        DrawAction::ShapeSelectorUp,
        DrawAction::ShapeSelectorDown,
        DrawAction::ShapeSelectorConfirm,
        DrawAction::ColorSelectorUp,
        DrawAction::ColorSelectorDown,
    ] {
        draw::apply_action(&mut state, action, &mut None).unwrap();
    }
    assert_eq!(state.active_shape_type, shape);
    assert_eq!(state.active_color, color);
    assert_eq!(state.active_tool, DrawTool::Cursor);
    state.show_shape_selector = true;
    draw::apply_action(&mut state, DrawAction::ShapeSelectorDown, &mut None).unwrap();
    assert_ne!(state.active_shape_type, shape);
    draw::apply_action(&mut state, DrawAction::ShapeSelectorConfirm, &mut None).unwrap();
    assert_eq!(state.active_tool, DrawTool::Shape);
    assert!(!state.show_shape_selector);
}

#[test]
fn creation_accepts_a_bare_relative_filename_without_changing_working_directory() {
    let cleanup = tempfile::NamedTempFile::new_in(".")
        .unwrap()
        .into_temp_path();
    std::fs::remove_file(&cleanup).unwrap();
    let relative = std::path::Path::new(cleanup.file_name().unwrap());
    let state = DrawState::create(relative).unwrap();
    assert_eq!(
        state.path().unwrap(),
        std::env::current_dir()
            .unwrap()
            .canonicalize()
            .unwrap()
            .join(relative)
    );
    assert_eq!(DrawState::load(relative).unwrap().data, DrawData::default());
}
