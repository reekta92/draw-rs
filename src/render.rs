use crate::app::{DrawInteraction, DrawState};
use crate::state::{DrawElement, DrawItem, DrawShapeType, DrawTransform, Shape, Stroke};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::canvas::{Canvas, Context, Line, Rectangle};
use ratatui::widgets::{Block, Paragraph};

pub fn draw_view(
    frame: &mut Frame,
    app: &mut DrawState,
    area: Rect,
    theme: &ThemeColors,
    hints: &DrawHints,
    mouse_pos: Option<(u16, u16)>,
) {
    app.set_canvas_area(area);
    let canvas_area = area;
    let projection = crate::geometry::CanvasProjection::new(area, &app.viewport);
    let x_bounds = projection.x_bounds;
    let y_bounds = projection.y_bounds;
    let cols_per_world_x = projection.cols_per_world_x;
    let rows_per_world_y = projection.rows_per_world_y;
    let canvas = Canvas::default()
        .block(Block::default().style(Style::default().bg(theme.bg.unwrap_or(Color::Reset))))
        .background_color(theme.bg.unwrap_or(Color::Reset))
        .marker(Marker::Braille)
        .x_bounds(x_bounds)
        .y_bounds(y_bounds)
        .paint(|ctx| {
            for item in app.data.elements.iter().filter(|item| {
                !matches!(&item.element, DrawElement::Text(_))
                    && !is_interaction_item(app, &item.id)
            }) {
                draw_item(ctx, item);
            }

            if let Some(stroke) = &app.current_stroke {
                draw_smoothed_stroke(ctx, stroke);
            }

            if let Some(DrawElement::Shape(shape)) = &app.preview_element {
                draw_shape(ctx, shape, crate::geometry::DrawAffine::identity());
            }
            draw_interaction_preview(ctx, app, false);

            for item in app.data.elements.iter().filter(|item| {
                matches!(&item.element, DrawElement::Text(_)) && !is_interaction_item(app, &item.id)
            }) {
                draw_item(ctx, item);
            }
            draw_interaction_preview(ctx, app, true);
            draw_selection_and_hover(ctx, app, theme);
        });

    frame.render_widget(canvas, canvas_area);
    crate::ui::draw_canvas_grid(
        frame,
        canvas_area,
        app.grid,
        crate::ui::CanvasGridProjection {
            world_left: x_bounds[0],
            world_right: x_bounds[1],
            world_top: y_bounds[0],
            world_bottom: y_bounds[1],
            origin_col: canvas_area.left() as f64 - x_bounds[0] * cols_per_world_x,
            origin_row: canvas_area.top() as f64 - y_bounds[1] * rows_per_world_y,
            cols_per_world_x,
            rows_per_world_y,
        },
        theme.muted,
        app.viewport.zoom,
    );

    if let Some(menu) = &app.context_menu {
        crate::ui::render_canvas_context_menu(frame, canvas_area, menu, theme, mouse_pos);
    }

    if app.show_shape_selector {
        let shapes = [
            (DrawShapeType::Rect, "Rect"),
            (DrawShapeType::Ellipse, "Ellipse"),
            (DrawShapeType::Diamond, "Diamond"),
            (DrawShapeType::Line, "Line"),
            (DrawShapeType::Arrow, "Arrow"),
        ];
        let items: Vec<_> = shapes
            .iter()
            .map(|(shape, name)| (*name, app.active_shape_type == *shape, None))
            .collect();
        crate::ui::draw_selector(
            frame,
            crate::ui::selector_rect(area, 11, items.len()),
            &items,
            theme,
            mouse_pos,
        );
        let selector = crate::ui::selector_rect(area, 11, shapes.len());
        crate::ui::draw_hints(
            frame,
            Rect::new(
                selector.x,
                selector.y,
                selector.width,
                selector.height.saturating_add(1).min(area.height),
            ),
            &hints.shape_confirm,
            &hints.shape_cancel,
            theme,
        );
    }
    if app.show_color_selector {
        let items: Vec<_> = crate::COLOR_PICKER_PALETTE
            .iter()
            .map(|(name, _, color)| {
                (
                    *name,
                    *color
                        == Color::Rgb(app.active_color.0, app.active_color.1, app.active_color.2),
                    Some(*color),
                )
            })
            .collect();
        crate::ui::draw_selector(
            frame,
            crate::ui::selector_rect(area, 14, items.len()),
            &items,
            theme,
            mouse_pos,
        );
        let selector = crate::ui::selector_rect(area, 14, items.len());
        crate::ui::draw_hints(
            frame,
            Rect::new(
                selector.x,
                selector.y,
                selector.width,
                selector.height.saturating_add(1).min(area.height),
            ),
            &hints.color_confirm,
            &hints.color_cancel,
            theme,
        );
    }
    app.text_editor_rect = None;
    if let Some((_, textarea)) = &app.text_editor {
        let content = crate::ui::editor_rect(area);
        let mut themed_textarea = textarea.clone();
        themed_textarea.set_block(
            Block::bordered()
                .style(theme.bg_style())
                .border_style(Style::default().fg(theme.heading)),
        );
        themed_textarea.set_style(theme.bg_style());
        if content.y > area.y {
            let title = " EDIT TEXT ";
            let width = ratatui::text::Line::from(title)
                .width()
                .min(content.width as usize) as u16;
            frame.render_widget(
                Paragraph::new(title).style(
                    Style::default()
                        .fg(theme.selected.fg.unwrap_or(theme.text))
                        .bg(theme.heading),
                ),
                Rect::new(
                    content.x + (content.width - width) / 2,
                    content.y - 1,
                    width,
                    1,
                ),
            );
        }
        frame.render_widget(ratatui::widgets::Clear, content);
        frame.render_widget(&themed_textarea, content);
        app.text_editor_rect = Some(Block::bordered().inner(content));
        crate::ui::draw_hints(
            frame,
            Rect::new(
                content.x,
                content.y,
                content.width,
                content.height.saturating_add(1).min(area.height),
            ),
            &hints.text_confirm,
            &hints.text_cancel,
            theme,
        );
    }
}

fn is_interaction_item(app: &DrawState, item_id: &crate::state::DrawItemId) -> bool {
    match &app.interaction {
        Some(DrawInteraction::Move { id, .. })
        | Some(DrawInteraction::Rotate { id, .. })
        | Some(DrawInteraction::Scale { id, .. }) => id == item_id,
        Some(DrawInteraction::Paste { .. }) | None => false,
    }
}

fn interaction_transform_for_item(
    item: &DrawItem,
    interaction: Option<&DrawInteraction>,
) -> DrawTransform {
    let mut transform = item.transform;
    match interaction {
        Some(DrawInteraction::Move {
            id,
            preview_translation,
            ..
        }) if id == &item.id => {
            transform.translate_x = preview_translation.0;
            transform.translate_y = preview_translation.1;
        }
        Some(DrawInteraction::Rotate {
            id,
            preview_degrees,
            ..
        }) if id == &item.id => {
            transform.rotation_degrees = *preview_degrees;
        }
        Some(DrawInteraction::Scale {
            id, preview_scale, ..
        }) if id == &item.id => {
            transform.scale = *preview_scale;
        }
        Some(DrawInteraction::Paste { .. }) | None => {}
        _ => {}
    }
    transform
}

fn draw_interaction_preview(ctx: &mut Context, app: &DrawState, text: bool) {
    let Some(interaction) = &app.interaction else {
        return;
    };
    match interaction {
        DrawInteraction::Paste { item } => {
            if matches!(&item.element, DrawElement::Text(_)) == text {
                draw_item(ctx, item);
            }
        }
        DrawInteraction::Move { id, .. }
        | DrawInteraction::Rotate { id, .. }
        | DrawInteraction::Scale { id, .. } => {
            let Some(item) = app.data.item(id) else {
                return;
            };
            if matches!(&item.element, DrawElement::Text(_)) == text {
                draw_item_with_transform(
                    ctx,
                    item,
                    interaction_transform_for_item(item, app.interaction.as_ref()),
                );
            }
        }
    }
}

fn draw_selection_and_hover(ctx: &mut Context, app: &DrawState, theme: &ThemeColors) {
    if let Some(id) = &app.selection.primary
        && let Some(item) = app.data.item(id)
    {
        let transform = interaction_transform_for_item(item, app.interaction.as_ref());
        draw_item_with_transform_and_color(
            ctx,
            item,
            transform,
            blended_item_color(item, theme.accent, 45),
        );
        draw_selection_bounds(ctx, item, transform, app, theme);
    } else if let Some(id) = &app.hovered
        && let Some(item) = app.data.item(id)
    {
        let transform = interaction_transform_for_item(item, app.interaction.as_ref());
        draw_item_with_transform_and_color(
            ctx,
            item,
            transform,
            blended_item_color(item, theme.accent, 30),
        );
    }
}

fn draw_selection_bounds(
    ctx: &mut Context,
    item: &DrawItem,
    transform: DrawTransform,
    app: &DrawState,
    theme: &ThemeColors,
) {
    let Some(bounds) = crate::geometry::transformed_bounds_with_transform(item, &transform) else {
        return;
    };
    let style = Style::default().fg(theme.accent);
    ctx.print(
        bounds.min_x,
        bounds.max_y,
        ratatui::text::Line::from("┌").style(style),
    );
    ctx.print(
        bounds.max_x,
        bounds.max_y,
        ratatui::text::Line::from("┐").style(style),
    );
    ctx.print(
        bounds.min_x,
        bounds.min_y,
        ratatui::text::Line::from("└").style(style),
    );
    ctx.print(
        bounds.max_x,
        bounds.min_y,
        ratatui::text::Line::from("┘").style(style),
    );
    let active_handle = match app.interaction.as_ref() {
        Some(DrawInteraction::Rotate { id, .. }) if id == &item.id => {
            crate::geometry::selection_handle_points(item, &transform, &app.viewport)
                .map(|(rotation, _)| (rotation, "○"))
        }
        Some(DrawInteraction::Scale { id, .. }) if id == &item.id => {
            crate::geometry::selection_handle_points(item, &transform, &app.viewport)
                .map(|(_, scale)| (scale, "◢"))
        }
        _ => None,
    };
    if let Some((point, glyph)) = active_handle {
        ctx.print(
            point.0,
            point.1,
            ratatui::text::Line::from(glyph).style(style),
        );
    }
}

pub(crate) fn draw_item(ctx: &mut Context, item: &DrawItem) {
    draw_item_with_transform(ctx, item, item.transform);
}

fn draw_item_with_transform(ctx: &mut Context, item: &DrawItem, transform: DrawTransform) {
    draw_item_with_transform_and_color(ctx, item, transform, item_color(item));
}

fn draw_item_with_transform_and_color(
    ctx: &mut Context,
    item: &DrawItem,
    transform: DrawTransform,
    color: Color,
) {
    match &item.element {
        DrawElement::Stroke(stroke) => {
            draw_stroke_colored(
                ctx,
                stroke,
                crate::geometry::DrawAffine::new(&transform),
                color,
            );
        }
        DrawElement::Shape(shape) => {
            draw_shape_colored(
                ctx,
                shape,
                crate::geometry::DrawAffine::new(&transform),
                color,
            );
        }
        DrawElement::Text(text) => {
            ctx.print(
                text.x + transform.translate_x,
                text.y + transform.translate_y,
                ratatui::text::Line::from(text.content.clone()).style(Style::default().fg(color)),
            );
        }
    }
}

fn item_color(item: &DrawItem) -> Color {
    match &item.element {
        DrawElement::Stroke(stroke) => Color::Rgb(stroke.color.0, stroke.color.1, stroke.color.2),
        DrawElement::Shape(
            Shape::Rect { color, .. }
            | Shape::Ellipse { color, .. }
            | Shape::Diamond { color, .. }
            | Shape::Line { color, .. }
            | Shape::Arrow { color, .. },
        ) => Color::Rgb(color.0, color.1, color.2),
        DrawElement::Text(text) => Color::Rgb(text.color.0, text.color.1, text.color.2),
    }
}

fn blended_item_color(item: &DrawItem, accent: Color, amount: u8) -> Color {
    let Color::Rgb(red, green, blue) = item_color(item) else {
        return accent;
    };
    let Some((accent_red, accent_green, accent_blue)) = color_rgb(accent) else {
        return accent;
    };
    let amount = u16::from(amount);
    let inverse = 100 - amount;
    Color::Rgb(
        ((u16::from(red) * inverse + u16::from(accent_red) * amount) / 100) as u8,
        ((u16::from(green) * inverse + u16::from(accent_green) * amount) / 100) as u8,
        ((u16::from(blue) * inverse + u16::from(accent_blue) * amount) / 100) as u8,
    )
}

fn color_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Black => Some((0, 0, 0)),
        Color::Red => Some((128, 0, 0)),
        Color::Green => Some((0, 128, 0)),
        Color::Yellow => Some((128, 128, 0)),
        Color::Blue => Some((0, 0, 128)),
        Color::Magenta => Some((128, 0, 128)),
        Color::Cyan => Some((0, 128, 128)),
        Color::Gray => Some((192, 192, 192)),
        Color::DarkGray => Some((128, 128, 128)),
        Color::LightRed => Some((255, 0, 0)),
        Color::LightGreen => Some((0, 255, 0)),
        Color::LightYellow => Some((255, 255, 0)),
        Color::LightBlue => Some((0, 0, 255)),
        Color::LightMagenta => Some((255, 0, 255)),
        Color::LightCyan => Some((0, 255, 255)),
        Color::White => Some((255, 255, 255)),
        Color::Rgb(red, green, blue) => Some((red, green, blue)),
        Color::Reset | Color::Indexed(_) => None,
    }
}

fn draw_stroke(ctx: &mut Context, stroke: &Stroke, transform: crate::geometry::DrawAffine) {
    draw_stroke_colored(
        ctx,
        stroke,
        transform,
        Color::Rgb(stroke.color.0, stroke.color.1, stroke.color.2),
    );
}

fn draw_stroke_colored(
    ctx: &mut Context,
    stroke: &Stroke,
    transform: crate::geometry::DrawAffine,
    color: Color,
) {
    for window in stroke.points.windows(2) {
        if let [start, end] = window {
            draw_transformed_line(ctx, *start, *end, color, transform);
        }
    }
}

fn draw_canvas_line(ctx: &mut Context, start: (f64, f64), end: (f64, f64), color: Color) {
    ctx.draw(&Line {
        x1: start.0,
        y1: start.1,
        x2: end.0,
        y2: end.1,
        color,
    });
}

fn draw_transformed_line(
    ctx: &mut Context,
    start: (f64, f64),
    end: (f64, f64),
    color: Color,
    transform: crate::geometry::DrawAffine,
) {
    draw_canvas_line(
        ctx,
        transform.transform_point(start),
        transform.transform_point(end),
        color,
    );
}

fn draw_closed_polygon(
    ctx: &mut Context,
    points: &[(f64, f64)],
    color: Color,
    transform: crate::geometry::DrawAffine,
) {
    let Some((&first, rest)) = points.split_first() else {
        return;
    };
    let mut previous = transform.transform_point(first);
    for &point in rest {
        let current = transform.transform_point(point);
        draw_canvas_line(ctx, previous, current, color);
        previous = current;
    }
    draw_canvas_line(ctx, previous, transform.transform_point(first), color);
}

/// Binomial filter smoothing (discrete Gaussian blur).
/// Applies a 3-point moving average with weights [0.25, 0.5, 0.25]
/// for 10 iterations. Acts as a powerful low-pass filter that
/// eliminates stair-step quantization noise.
pub fn smooth_points(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let n = points.len();
    let mut xs: Vec<f64> = points.iter().map(|point| point.0).collect();
    let mut ys: Vec<f64> = points.iter().map(|point| point.1).collect();
    for _ in 0..10 {
        let previous_xs = xs.clone();
        let previous_ys = ys.clone();
        xs[0] = previous_xs[0];
        ys[0] = previous_ys[0];
        for index in 1..n - 1 {
            xs[index] = 0.25 * previous_xs[index - 1]
                + 0.5 * previous_xs[index]
                + 0.25 * previous_xs[index + 1];
            ys[index] = 0.25 * previous_ys[index - 1]
                + 0.5 * previous_ys[index]
                + 0.25 * previous_ys[index + 1];
        }
        xs[n - 1] = previous_xs[n - 1];
        ys[n - 1] = previous_ys[n - 1];
    }
    xs.into_iter().zip(ys).collect()
}

fn draw_smoothed_stroke(ctx: &mut Context, stroke: &Stroke) {
    let smoothed = smooth_points(&stroke.points);
    draw_stroke(
        ctx,
        &Stroke {
            points: smoothed,
            color: stroke.color,
        },
        crate::geometry::DrawAffine::identity(),
    );
}

fn draw_shape(ctx: &mut Context, shape: &Shape, transform: crate::geometry::DrawAffine) {
    draw_shape_colored(ctx, shape, transform, shape_color(shape));
}

fn shape_color(shape: &Shape) -> Color {
    match shape {
        Shape::Rect { color, .. }
        | Shape::Ellipse { color, .. }
        | Shape::Diamond { color, .. }
        | Shape::Line { color, .. }
        | Shape::Arrow { color, .. } => Color::Rgb(color.0, color.1, color.2),
    }
}

fn draw_shape_colored(
    ctx: &mut Context,
    shape: &Shape,
    transform: crate::geometry::DrawAffine,
    draw_color: Color,
) {
    match shape {
        Shape::Rect {
            x,
            y,
            width,
            height,
            ..
        } if transform.is_identity() => {
            ctx.draw(&Rectangle {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                color: draw_color,
            });
        }
        Shape::Rect {
            x,
            y,
            width,
            height,
            ..
        } => {
            let points = [
                (*x, *y),
                (*x + *width, *y),
                (*x + *width, *y + *height),
                (*x, *y + *height),
            ];
            draw_closed_polygon(ctx, &points, draw_color, transform);
        }
        Shape::Ellipse {
            x,
            y,
            width,
            height,
            ..
        } => {
            let radius_x = width / 2.0;
            let radius_y = height / 2.0;
            let center_x = x + radius_x;
            let center_y = y + radius_y;
            const SEGMENTS: usize = 32;
            for index in 0..SEGMENTS {
                let start_angle = (index as f64 / SEGMENTS as f64) * std::f64::consts::TAU;
                let end_angle = ((index + 1) as f64 / SEGMENTS as f64) * std::f64::consts::TAU;
                draw_transformed_line(
                    ctx,
                    (
                        center_x + radius_x * start_angle.cos(),
                        center_y + radius_y * start_angle.sin(),
                    ),
                    (
                        center_x + radius_x * end_angle.cos(),
                        center_y + radius_y * end_angle.sin(),
                    ),
                    draw_color,
                    transform,
                );
            }
        }
        Shape::Diamond {
            x,
            y,
            width,
            height,
            ..
        } => {
            let points = [
                (*x + *width / 2.0, *y),
                (*x + *width, *y + *height / 2.0),
                (*x + *width / 2.0, *y + *height),
                (*x, *y + *height / 2.0),
            ];
            draw_closed_polygon(ctx, &points, draw_color, transform);
        }
        Shape::Line { x1, y1, x2, y2, .. } => {
            draw_transformed_line(ctx, (*x1, *y1), (*x2, *y2), draw_color, transform);
        }
        Shape::Arrow { x1, y1, x2, y2, .. } => {
            let start = (*x1, *y1);
            let end = (*x2, *y2);
            draw_transformed_line(ctx, start, end, draw_color, transform);

            let angle = (y2 - y1).atan2(x2 - x1);
            let head_length = 5.0;
            let head_angle = std::f64::consts::PI / 6.0;
            let left = (
                x2 - head_length * (angle - head_angle).cos(),
                y2 - head_length * (angle - head_angle).sin(),
            );
            let right = (
                x2 - head_length * (angle + head_angle).cos(),
                y2 - head_length * (angle + head_angle).sin(),
            );
            draw_transformed_line(ctx, end, left, draw_color, transform);
            draw_transformed_line(ctx, end, right, draw_color, transform);
        }
    }
}

/// Colors supplied by the embedding host; `None` preserves terminal transparency.
#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
    pub bg: Option<Color>,
    pub accent: Color,
    pub heading: Color,
    pub muted: Color,
    pub text: Color,
    pub popup_bg: Option<Color>,
    pub selected: Style,
    pub hover: Style,
}
impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            bg: None,
            accent: Color::Cyan,
            heading: Color::Yellow,
            muted: Color::DarkGray,
            text: Color::White,
            popup_bg: None,
            selected: Style::default().fg(Color::Black).bg(Color::Cyan),
            hover: Style::default().bg(Color::DarkGray),
        }
    }
}
impl ThemeColors {
    pub(crate) fn bg_style(&self) -> Style {
        Style::default()
            .fg(self.text)
            .bg(self.bg.unwrap_or(Color::Reset))
    }
    pub(crate) fn preview_bg_style(&self) -> Style {
        Style::default()
            .fg(self.text)
            .bg(self.popup_bg.or(self.bg).unwrap_or(Color::Reset))
    }
}

#[derive(Clone, Debug)]
pub struct DrawHints {
    pub shape_confirm: String,
    pub shape_cancel: String,
    pub color_confirm: String,
    pub color_cancel: String,
    pub text_confirm: String,
    pub text_cancel: String,
}
impl Default for DrawHints {
    fn default() -> Self {
        Self {
            shape_confirm: "Enter".into(),
            shape_cancel: "Esc".into(),
            color_confirm: "Enter".into(),
            color_cancel: "Esc".into(),
            text_confirm: "Enter".into(),
            text_cancel: "Esc".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_and_selection_blend_item_color_toward_accent() {
        let item = DrawItem::new(DrawElement::Stroke(Stroke {
            points: vec![(0.0, 0.0)],
            color: (100, 150, 200),
        }));
        let accent = Color::Rgb(200, 100, 0);

        assert_eq!(
            blended_item_color(&item, accent, 30),
            Color::Rgb(130, 135, 140)
        );
        assert_eq!(
            blended_item_color(&item, accent, 45),
            Color::Rgb(145, 127, 110)
        );
    }
}
