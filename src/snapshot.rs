use crate::DrawData;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    symbols::Marker,
    widgets::{Block, canvas::Canvas},
};
#[allow(clippy::too_many_arguments)]
pub fn draw_snapshot(
    frame: &mut Frame,
    area: Rect,
    data: &DrawData,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
    background: Option<Color>,
) {
    if area.is_empty() {
        return;
    }
    let width = area.width;
    let height = area.height;
    let (min_x, min_y, max_x, max_y) = draw_bounds(data);
    let padding = 20.0;
    let cx = (min_x + max_x) / 2.0;
    let cy = (min_y + max_y) / 2.0;
    let hw = ((max_x - min_x) / 2.0 + padding).max(10.0) / scale;
    let hh = ((max_y - min_y) / 2.0 + padding).max(10.0) / scale;
    let ratio_x = (2.0 * hw) / width as f64;
    let ratio_y = (2.0 * hh) / height as f64;
    let cx_shifted = cx - offset_x * ratio_x;
    let cy_shifted = cy + offset_y * ratio_y;
    let x_bounds = [cx_shifted - hw, cx_shifted + hw];
    let y_bounds = [cy_shifted - hh, cy_shifted + hh];

    let bg_color = background.unwrap_or(Color::Reset);
    let canvas = Canvas::default()
        .block(Block::default().style(Style::default().bg(bg_color)))
        .background_color(bg_color)
        .marker(Marker::Braille)
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .paint(|ctx| {
            for item in data
                .elements
                .iter()
                .filter(|item| !matches!(&item.element, crate::state::DrawElement::Text(_)))
            {
                crate::render::draw_item(ctx, item);
            }
            for item in data
                .elements
                .iter()
                .filter(|item| matches!(&item.element, crate::state::DrawElement::Text(_)))
            {
                crate::render::draw_item(ctx, item);
            }
        });
    frame.render_widget(canvas, area);
}
fn draw_bounds(data: &DrawData) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for item in &data.elements {
        if let Some(bounds) = crate::geometry::transformed_bounds(item) {
            min_x = min_x.min(bounds.min_x);
            min_y = min_y.min(bounds.min_y);
            max_x = max_x.max(bounds.max_x);
            max_y = max_y.max(bounds.max_y);
        }
    }
    if min_x == f64::MAX {
        (0.0, 0.0, 100.0, 100.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}
