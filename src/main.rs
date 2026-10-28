#![cfg_attr(test, allow(clippy::unwrap_used))]
use anyhow::{Context, Result, ensure};
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use draw::{DrawAction as A, DrawEventAction, DrawHints, DrawState, DrawTool, ThemeColors};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    widgets::Paragraph,
};
use std::{ffi::OsString, io, path::PathBuf};

fn requested_path(mut args: impl Iterator<Item = OsString>) -> Result<PathBuf> {
    let path = PathBuf::from(args.next().context("Usage: draw <path.draw>")?);
    ensure!(
        args.next().is_none() && path.extension().is_some_and(|ext| ext == "draw"),
        "Usage: draw <path.draw>"
    );
    Ok(path)
}

fn open(path: &std::path::Path) -> Result<DrawState> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => DrawState::load(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => DrawState::create(path),
        Err(e) => Err(e.into()),
    }
}

fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen
    );
}
struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> Result<Self> {
        let guard = Self;
        terminal::enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        Ok(guard)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn action(state: &DrawState, key: KeyEvent) -> Option<A> {
    use KeyCode::{Char, Down, Enter, Esc, Up};
    if key
        .modifiers
        .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return None;
    }
    let plain = key.modifiers.is_empty();
    if state.text_editor.is_some() {
        if !plain {
            return None;
        }
        return match key.code {
            Enter => Some(A::TextEditorConfirm),
            Esc => Some(A::TextEditorCancel),
            _ => None,
        };
    }
    if plain && (state.show_shape_selector || state.show_color_selector) {
        let shape = state.show_shape_selector;
        let selected = match key.code {
            Esc | Char('q') => Some(if shape {
                A::ShapeSelectorCancel
            } else {
                A::ColorSelectorCancel
            }),
            Enter => Some(if shape {
                A::ShapeSelectorConfirm
            } else {
                A::ColorSelectorConfirm
            }),
            Up | Char('k') => Some(if shape {
                A::ShapeSelectorUp
            } else {
                A::ColorSelectorUp
            }),
            Down | Char('j') => Some(if shape {
                A::ShapeSelectorDown
            } else {
                A::ColorSelectorDown
            }),
            _ => None,
        };
        if selected.is_some() {
            return selected;
        }
    }
    if plain && key.code == Char('v') {
        return Some(A::Paste);
    }
    if state.context_menu.is_some() {
        if !plain {
            return None;
        }
        return match key.code {
            Esc => Some(A::MenuClose),
            Enter => Some(A::MenuSelect),
            Up | Char('k') => Some(A::MenuUp),
            Down | Char('j') => Some(A::MenuDown),
            _ => None,
        };
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            Char('Z') => Some(A::Redo),
            Char('z') if key.modifiers.contains(KeyModifiers::SHIFT) => Some(A::Redo),
            Char('z') => Some(A::Undo),
            Char('y') => Some(A::Redo),
            _ => None,
        };
    }
    if !(plain || key.modifiers == KeyModifiers::SHIFT && key.code == Char('G')) {
        return None;
    }
    match key.code {
        Esc => Some(A::MenuClose),
        Char('q') => Some(A::Quit),
        Char('?') => Some(A::Help),
        Char('a') => Some(A::SelectCursorTool),
        Char('d') => Some(A::SelectDrawTool),
        Char('s') => Some(A::ToggleShapeSelector),
        Char('c') => Some(A::ToggleColorSelector),
        Char('t') => Some(A::SelectTextTool),
        Char('e') => Some(A::SelectEraseTool),
        Char('G') => Some(A::ToggleGrid),
        _ => None,
    }
}

fn key_event(
    state: &mut DrawState,
    key: KeyEvent,
    clipboard: &mut Option<draw::DrawItem>,
) -> Result<DrawEventAction> {
    if state.text_editor.is_some() {
        if let Some(action) = action(state, key) {
            return draw::apply_action(state, action, clipboard);
        }
        if let Some(editor) = state.text_editor_mut() {
            editor.input(key);
        }
        return Ok(DrawEventAction::Continue);
    }
    let resolved = action(state, key);
    if (state.show_shape_selector || state.show_color_selector)
        && resolved.is_some_and(|a| {
            matches!(
                a,
                A::ShapeSelectorUp
                    | A::ShapeSelectorDown
                    | A::ShapeSelectorConfirm
                    | A::ShapeSelectorCancel
                    | A::ColorSelectorUp
                    | A::ColorSelectorDown
                    | A::ColorSelectorConfirm
                    | A::ColorSelectorCancel
            )
        })
    {
        return draw::apply_action(state, resolved.expect("selector action"), clipboard);
    }
    if resolved == Some(A::Paste) && clipboard.is_some() {
        return draw::apply_action(state, A::Paste, clipboard);
    }
    if state.context_menu.is_some() {
        if let Some(action) = resolved {
            return draw::apply_action(state, action, clipboard);
        }
        draw::handle_menu_shortcut(key, state, clipboard)?;
        return Ok(DrawEventAction::Continue);
    }
    if key.code == KeyCode::Esc && key.modifiers.is_empty() {
        if draw::cancel_interaction_or_selection(state) {
            return Ok(DrawEventAction::Continue);
        }
        return Ok(DrawEventAction::Quit);
    }
    if draw::handle_selected_menu_shortcut(key, state, clipboard)? {
        return Ok(DrawEventAction::Continue);
    }
    if let Some(action) = resolved {
        return draw::apply_action(state, action, clipboard);
    }
    Ok(DrawEventAction::Continue)
}

const TOOL_TABS: [(&str, A); 5] = [
    ("a: Cursor", A::SelectCursorTool),
    ("d: Draw", A::SelectDrawTool),
    ("s: Shape", A::ToggleShapeSelector),
    ("t: Text", A::SelectTextTool),
    ("e: Erase", A::SelectEraseTool),
];

fn toolbar_action(column: u16, active_tool: DrawTool) -> Option<A> {
    let mut start = 7;
    for (label, action) in TOOL_TABS {
        let end = start + label.len() as u16;
        if (start..end).contains(&column) {
            return Some(
                if action == A::SelectDrawTool && active_tool == DrawTool::Draw {
                    A::ToggleColorSelector
                } else {
                    action
                },
            );
        }
        start = end + 3;
    }
    None
}

fn canvas_area(area: Rect) -> (Rect, Rect, Rect) {
    let [header, canvas, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    (header, canvas, footer)
}

fn run(mut state: DrawState) -> Result<()> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));
    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut clipboard = None;
    let mut status = String::new();
    let toolbar = format!("Draw   {}", TOOL_TABS.map(|(label, _)| label).join("   "));
    let mut help = false;
    loop {
        terminal.draw(|frame| {
            let (header, canvas, footer) = canvas_area(frame.area());
            state.set_canvas_area(canvas);
            frame.render_widget(Paragraph::new(state.active_mode_message().unwrap_or(&toolbar)), header);
            if help {
                frame.render_widget(Paragraph::new("a Cursor   d Draw   s Shape   t Text   e Erase\nc Color   G Grid   Ctrl+z Undo   Ctrl+y Redo\nRight-click: context menu; middle/right drag: pan\nScroll: zoom; v: paste element\nEnter: confirm text/selector; Esc: cancel, clear, leave\nq: save and leave; F1/?: help; F1/Esc: close help"), canvas);
            } else { let mouse = state.mouse_pos; draw::draw_view(frame, &mut state, canvas, &ThemeColors::default(), &DrawHints::default(), mouse); }
            frame.render_widget(Paragraph::new(if status.is_empty() { "q/Esc: back   ?: help" } else { &status }), footer);
        })?;
        let event = event::read()?;
        if help {
            if matches!(
                event,
                Event::Key(KeyEvent {
                    code: KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(1),
                    ..
                })
            ) {
                help = false;
            }
            continue;
        }
        let area = terminal.size()?;
        state.set_canvas_area(canvas_area(Rect::new(0, 0, area.width, area.height)).1);
        let result = match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if key.code == KeyCode::F(1) {
                    Ok(DrawEventAction::OpenHelp)
                } else {
                    key_event(&mut state, key, &mut clipboard)
                }
            }
            Event::Mouse(mouse) => {
                if state.input_available()
                    && mouse.kind == MouseEventKind::Down(MouseButton::Left)
                    && mouse.row == 0
                {
                    let action = toolbar_action(mouse.column, state.active_tool);
                    if let Some(action) = action {
                        draw::apply_action(&mut state, action, &mut clipboard)
                    } else {
                        Ok(DrawEventAction::Continue)
                    }
                } else {
                    draw::handle_draw_mouse(&mut state, mouse, &mut clipboard)
                }
            }
            Event::Paste(text) => {
                state.insert_text(&text);
                Ok(DrawEventAction::Continue)
            }
            _ => Ok(DrawEventAction::Continue),
        };
        match result {
            Ok(DrawEventAction::Quit) => {
                let saved = if state.input_available() {
                    state.save()
                } else {
                    Err(anyhow::anyhow!(
                        "Finish the drawing interaction before leaving"
                    ))
                };
                match saved {
                    Ok(()) => break,
                    Err(error) => status = format!("{error:#}"),
                }
            }
            Ok(DrawEventAction::OpenHelp) => help = true,
            Ok(DrawEventAction::Continue | DrawEventAction::CopyText(_)) => {
                if let Some(notice) = state.take_notice() {
                    status = notice.into();
                }
            }
            Err(error) => status = format!("{error:#}"),
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1).peekable();
    if args.len() == 1
        && args
            .peek()
            .is_some_and(|arg| arg == "-h" || arg == "--help")
    {
        println!("Usage: draw <path.draw>");
        return Ok(());
    }
    let path = requested_path(args)?;
    run(open(&path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_keys_preserve_modifier_rules() {
        let mut state = DrawState::new();
        assert_eq!(
            action(
                &state,
                KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SHIFT)
            ),
            None
        );
        for modifiers in [KeyModifiers::ALT, KeyModifiers::SUPER, KeyModifiers::SHIFT] {
            assert_eq!(
                action(&state, KeyEvent::new(KeyCode::Char('q'), modifiers)),
                None
            );
        }
        assert_eq!(
            action(
                &state,
                KeyEvent::new(
                    KeyCode::Char('z'),
                    KeyModifiers::CONTROL | KeyModifiers::SHIFT
                )
            ),
            Some(A::Redo)
        );
        let id = draw::DrawItemId::new();
        state.text_editor = Some((id, ratatui_textarea::TextArea::default()));
        assert_eq!(
            action(&state, KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)),
            None
        );
        assert_eq!(
            action(&state, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(A::TextEditorConfirm)
        );
    }

    #[test]
    fn right_click_release_allows_departure_after_menu_dismissal() {
        for with_item in [false, true] {
            let mut state = DrawState::new();
            state.set_canvas_area(Rect::new(0, 1, 100, 50));
            if with_item {
                state
                    .data
                    .elements
                    .push(draw::DrawItem::new(draw::DrawElement::Stroke(
                        draw::Stroke {
                            points: vec![(-20.0, 0.0), (20.0, 0.0)],
                            color: (1, 2, 3),
                        },
                    )));
            }
            let mut clipboard = None;
            for kind in [
                MouseEventKind::Down(MouseButton::Right),
                MouseEventKind::Up(MouseButton::Right),
            ] {
                draw::handle_draw_mouse(
                    &mut state,
                    crossterm::event::MouseEvent {
                        kind,
                        column: 50,
                        row: 25,
                        modifiers: KeyModifiers::NONE,
                    },
                    &mut clipboard,
                )
                .unwrap();
            }
            assert_eq!(state.context_menu.is_some(), with_item);
            if with_item {
                key_event(
                    &mut state,
                    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                    &mut clipboard,
                )
                .unwrap();
            }
            assert!(state.input_available());
            assert_eq!(
                key_event(
                    &mut state,
                    KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
                    &mut clipboard
                )
                .unwrap(),
                DrawEventAction::Quit
            );
        }
    }

    #[test]
    fn toolbar_hit_regions_follow_rendered_labels() {
        let mut column = 7;
        for (label, action) in TOOL_TABS {
            assert_eq!(toolbar_action(column, DrawTool::Cursor), Some(action));
            assert_eq!(
                toolbar_action(column + label.len() as u16 - 1, DrawTool::Cursor),
                Some(action)
            );
            column += label.len() as u16;
            assert_eq!(toolbar_action(column, DrawTool::Cursor), None);
            column += 3;
        }
        assert_eq!(
            toolbar_action(19, DrawTool::Draw),
            Some(A::ToggleColorSelector)
        );
    }

    #[test]
    #[ignore = "requires a terminal; exercised through the PTY smoke check"]
    fn terminal_guard_restores_after_panic() {
        let result = std::panic::catch_unwind(|| {
            let _guard = TerminalGuard::enter().expect("terminal setup");
            assert!(terminal::is_raw_mode_enabled().expect("raw mode"));
            panic!("terminal restoration probe");
        });
        assert!(result.is_err());
        assert!(!terminal::is_raw_mode_enabled().expect("restored raw mode"));
    }

    #[test]
    fn validates_arguments_and_loads_before_terminal_setup() {
        for args in [vec![], vec!["a.md"], vec!["a.draw", "b.draw"]] {
            assert!(requested_path(args.into_iter().map(OsString::from)).is_err());
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.draw");
        std::fs::write(&path, "invalid").unwrap();
        assert!(open(&path).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid");
    }
    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("link.draw");
        std::os::unix::fs::symlink(dir.path().join("missing.draw"), &path).unwrap();
        assert!(open(&path).is_err());
        assert!(path.is_symlink());
    }
}
