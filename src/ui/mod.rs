mod canvas_grid;
mod canvas_menu;
mod canvas_overlay;
mod canvas_selection;
use crate::ThemeColors;
pub(crate) use canvas_grid::*;
pub use canvas_menu::*;
pub use canvas_overlay::*;
pub use canvas_selection::*;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Clear, Paragraph},
};
use ratatui_textarea::{CursorMove, TextArea};
#[derive(Default)]
pub(crate) struct MouseTextSelection {
    pub active: bool,
    pub dragged: bool,
}

pub(crate) fn move_text_cursor(text: &mut TextArea, area: Rect, col: u16, row: u16) {
    let (scroll_row, scroll_col) = text.scroll_offset();
    move_textarea_cursor_to_mouse(
        text,
        area,
        col.clamp(area.x, area.right().saturating_sub(1).max(area.x)),
        row.clamp(area.y, area.bottom().saturating_sub(1).max(area.y)),
        scroll_row as usize,
        scroll_col as usize,
    );
}
fn move_textarea_cursor_to_mouse(
    textarea: &mut TextArea,
    body_inner: Rect,
    mouse_col: u16,
    mouse_row: u16,
    scroll_row: usize,
    scroll_col: usize,
) {
    if textarea.lines().is_empty() || body_inner.width == 0 || body_inner.height == 0 {
        return;
    }

    let target_row = (mouse_row.saturating_sub(body_inner.y) as usize).saturating_add(scroll_row);
    let target_col = (mouse_col.saturating_sub(body_inner.x) as usize).saturating_add(scroll_col);

    let cur = textarea.screen_cursor();
    // vertical: move by screen lines
    let drow = target_row as i64 - cur.row as i64;
    let vmove = if drow >= 0 {
        CursorMove::Down
    } else {
        CursorMove::Up
    };
    for _ in 0..drow.unsigned_abs() {
        textarea.move_cursor(vmove);
    }
    // horizontal: move by screen cells, clamped to the current screen line
    let cur = textarea.screen_cursor();
    let dcol = target_col as i64 - cur.col as i64;
    let hmove = if dcol >= 0 {
        CursorMove::Forward
    } else {
        CursorMove::Back
    };
    for _ in 0..dcol.unsigned_abs() {
        let before = textarea.screen_cursor().row;
        textarea.move_cursor(hmove);
        if textarea.screen_cursor().row != before {
            // stepped across a wrapped screen-line boundary: revert + stop
            textarea.move_cursor(if matches!(hmove, CursorMove::Forward) {
                CursorMove::Back
            } else {
                CursorMove::Forward
            });
            break;
        }
    }
}

pub(crate) fn selector_rect(area: Rect, width: u16, count: usize) -> Rect {
    let width = width.min(area.width);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y,
        width,
        (count as u16).min(area.height),
    )
}
pub(crate) fn editor_rect(area: Rect) -> Rect {
    let width = 60.min(area.width);
    let height = 5.min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}
pub(crate) fn draw_selector(
    frame: &mut Frame,
    rect: Rect,
    items: &[(&str, bool, Option<Color>)],
    theme: &ThemeColors,
    mouse: Option<(u16, u16)>,
) {
    frame.render_widget(Clear, rect);
    for (i, (name, selected, color)) in items.iter().take(rect.height as usize).enumerate() {
        let row = Rect::new(rect.x, rect.y + i as u16, rect.width, 1);
        let style = if *selected {
            theme.selected
        } else if mouse.is_some_and(|(x, y)| row.contains((x, y).into())) {
            theme.hover
        } else {
            theme.preview_bg_style()
        };
        let label = if color.is_some() {
            format!(" ■ {name}")
        } else {
            format!("  {name}")
        };
        frame.render_widget(
            Paragraph::new(label).style(color.map_or(style, |c| style.fg(c))),
            row,
        );
    }
}
pub(crate) fn draw_hints(
    frame: &mut Frame,
    area: Rect,
    confirm: &str,
    cancel: &str,
    theme: &ThemeColors,
) {
    if !area.is_empty() {
        let row = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        frame.render_widget(
            Paragraph::new(format!("{confirm}: confirm  {cancel}: cancel"))
                .style(Style::default().fg(theme.muted)),
            row,
        );
    }
}
