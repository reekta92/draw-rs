use crate::DrawAction;
use crate::app::{
    DrawEventAction, DrawInteraction, DrawMenuItem, DrawMenuKind, DrawMenuTarget, DrawState,
    draw_menu_items, draw_menu_shortcut_index,
};
use crate::state::{DrawElement, DrawItem, DrawShapeType, DrawTool, Shape, Stroke, Text};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

pub fn handle_draw_mouse(
    app: &mut DrawState,
    event: MouseEvent,
    clipboard: &mut Option<DrawItem>,
) -> anyhow::Result<DrawEventAction> {
    Ok(handle_mouse(event, app, clipboard)?.unwrap_or(DrawEventAction::Continue))
}

pub fn handle_selected_menu_shortcut(
    key: crossterm::event::KeyEvent,
    app: &mut DrawState,
    clipboard: &mut Option<DrawItem>,
) -> anyhow::Result<bool> {
    if app.active_tool != DrawTool::Cursor
        || key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return Ok(false);
    }
    let KeyCode::Char(shortcut) = key.code else {
        return Ok(false);
    };
    let Some(id) = app.selection.primary.clone() else {
        return Ok(false);
    };
    let Some(item) = app.data.item(&id) else {
        return Ok(false);
    };
    let target = if matches!(&item.element, DrawElement::Text(_)) {
        DrawMenuTarget::Text(id)
    } else {
        DrawMenuTarget::NonText(id)
    };
    let Some(index) = draw_menu_shortcut_index(&target, clipboard.is_some(), shortcut) else {
        return Ok(false);
    };
    let (menu_x, menu_y) = app.mouse_pos.unwrap_or((app.last_area.x, app.last_area.y));
    execute_menu_item(
        app,
        clipboard,
        target,
        DrawMenuKind::Actions,
        index,
        menu_x,
        menu_y,
    )?;
    Ok(true)
}

fn cycle_shape_type(app: &mut DrawState, delta: i32) {
    let shapes = [
        DrawShapeType::Rect,
        DrawShapeType::Ellipse,
        DrawShapeType::Diamond,
        DrawShapeType::Line,
        DrawShapeType::Arrow,
    ];
    let current_idx = shapes
        .iter()
        .position(|&s| s == app.active_shape_type)
        .unwrap_or(0) as i32;
    let next_idx = (current_idx + delta).rem_euclid(shapes.len() as i32) as usize;
    app.active_shape_type = shapes[next_idx];
}
fn cycle_color(app: &mut DrawState, delta: i32) {
    let colors = crate::COLOR_PICKER_PALETTE;
    let current_idx = colors
        .iter()
        .position(|&(_, _, c)| {
            if let ratatui::style::Color::Rgb(r, g, b) = c {
                (r, g, b) == app.active_color
            } else {
                false
            }
        })
        .unwrap_or(0) as i32;
    let next_idx = (current_idx + delta).rem_euclid(colors.len() as i32) as usize;
    if let ratatui::style::Color::Rgb(r, g, b) = colors[next_idx].2 {
        app.active_color = (r, g, b);
    }
}

fn handle_text_editor_mouse(
    mouse: MouseEvent,
    app: &mut DrawState,
) -> anyhow::Result<Option<DrawEventAction>> {
    let Some((_, textarea)) = &mut app.text_editor else {
        return Ok(None);
    };
    let Some(area) = app.text_editor_rect else {
        return Ok(None);
    };
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left)
            if area.contains((mouse.column, mouse.row).into()) =>
        {
            crate::ui::move_text_cursor(textarea, area, mouse.column, mouse.row);
            textarea.start_selection();
            app.mouse_selection.active = true;
            app.mouse_selection.dragged = false;
        }
        MouseEventKind::Drag(MouseButton::Left) if app.mouse_selection.active => {
            app.mouse_selection.dragged = true;
            crate::ui::move_text_cursor(textarea, area, mouse.column, mouse.row);
        }
        MouseEventKind::Up(MouseButton::Left) if app.mouse_selection.active => {
            app.mouse_selection.active = false;
            if app.mouse_selection.dragged {
                textarea.copy();
                return Ok(Some(DrawEventAction::CopyText(textarea.yank_text())));
            }
            textarea.cancel_selection();
        }
        _ => {}
    }
    Ok(None)
}

fn handle_mouse(
    ev: MouseEvent,
    app: &mut DrawState,
    clipboard: &mut Option<DrawItem>,
) -> anyhow::Result<Option<DrawEventAction>> {
    app.mouse_pos = Some((ev.column, ev.row));
    if app.text_editor.is_some() {
        return handle_text_editor_mouse(ev, app);
    }
    let area = app.last_area;

    if app.show_shape_selector {
        let shapes = [
            DrawShapeType::Rect,
            DrawShapeType::Ellipse,
            DrawShapeType::Diamond,
            DrawShapeType::Line,
            DrawShapeType::Arrow,
        ];
        let dropdown_rect = crate::ui::selector_rect(area, 11, shapes.len());
        let dropdown_y = dropdown_rect.y;
        let offset = shapes
            .iter()
            .position(|shape| *shape == app.active_shape_type)
            .unwrap_or(0)
            .saturating_sub(dropdown_rect.height.saturating_sub(1) as usize);

        if ev.kind == MouseEventKind::Down(MouseButton::Left) {
            if dropdown_rect.contains((ev.column, ev.row).into()) {
                let row_rel = offset + (ev.row - dropdown_y) as usize;
                if let Some(&shape) = shapes.get(row_rel) {
                    app.active_shape_type = shape;
                    app.set_active_tool(DrawTool::Shape);
                    app.show_shape_selector = false;
                    return Ok(None);
                }
            } else {
                app.show_shape_selector = false;
                return Ok(None);
            }
        }
        return Ok(None);
    }

    if app.show_color_selector {
        let colors = crate::COLOR_PICKER_PALETTE;
        let dropdown_rect = crate::ui::selector_rect(area, 14, colors.len());
        let dropdown_y = dropdown_rect.y;
        let offset = colors
            .iter()
            .position(|(_, _, color)| {
                *color
                    == ratatui::style::Color::Rgb(
                        app.active_color.0,
                        app.active_color.1,
                        app.active_color.2,
                    )
            })
            .unwrap_or(0)
            .saturating_sub(dropdown_rect.height.saturating_sub(1) as usize);

        if ev.kind == MouseEventKind::Down(MouseButton::Left) {
            if dropdown_rect.contains((ev.column, ev.row).into()) {
                let row_rel = offset + (ev.row - dropdown_y) as usize;
                if let Some(&(_, _, c)) = colors.get(row_rel) {
                    if let ratatui::style::Color::Rgb(r, g, b) = c {
                        app.active_color = (r, g, b);
                    }
                    app.show_color_selector = false;
                    return Ok(None);
                }
            } else {
                app.show_color_selector = false;
                return Ok(None);
            }
        }
        return Ok(None);
    }

    if app.context_menu.is_some() {
        return handle_context_menu_mouse(ev, app, clipboard);
    }
    if matches!(
        &app.interaction,
        Some(DrawInteraction::Rotate { .. })
            | Some(DrawInteraction::Scale { .. })
            | Some(DrawInteraction::Paste { .. })
    ) {
        return handle_transform_or_paste_mouse(ev, app);
    }

    if !area.contains((ev.column, ev.row).into())
        && (matches!(
            ev.kind,
            MouseEventKind::Down(_)
                | MouseEventKind::Moved
                | MouseEventKind::ScrollUp
                | MouseEventKind::ScrollDown
        ) || app.input_available())
    {
        return Ok(None);
    }

    match ev.kind {
        MouseEventKind::Moved => {
            if app.active_tool == DrawTool::Cursor && app.interaction.is_none() && !app.is_panning {
                let point = screen_to_canvas(ev.column, ev.row, app);
                app.hovered = app.topmost_hit(point);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let point = screen_to_canvas(ev.column, ev.row, app);
            match app.active_tool {
                DrawTool::Cursor => cursor_left_down(ev, point, app),
                DrawTool::Draw => {
                    app.current_stroke = Some(Stroke {
                        points: vec![point],
                        color: app.active_color,
                    });
                }
                DrawTool::Shape => {
                    app.creation_origin = Some(point);
                }
                DrawTool::Text => {
                    let previous = app.data.clone();
                    app.data
                        .elements
                        .push(DrawItem::new(DrawElement::Text(Text {
                            content: "New Text".to_string(),
                            x: point.0,
                            y: point.1,
                            color: app.active_color,
                        })));
                    app.commit_data_change(previous)?;
                }
                DrawTool::Erase => {
                    app.erase_start_data = Some(app.data.clone());
                    erase_at(point.0, point.1, app);
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            let point = screen_to_canvas(ev.column, ev.row, app);
            app.right_mouse_target = app.topmost_hit(point);
            app.right_mouse_screen = Some((ev.column, ev.row));
            app.right_mouse.on_down(point.0, point.1);
            app.last_mouse_pos = Some((ev.column, ev.row));
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            if let Some((start_x, start_y)) = app.right_mouse_screen
                && app
                    .right_mouse
                    .is_dragging_screen(ev.column, ev.row, start_x, start_y)
            {
                let point = screen_to_canvas(ev.column, ev.row, app);
                app.right_mouse.on_drag(point.0, point.1);
                panning(ev.column, ev.row, app);
            }
        }
        MouseEventKind::Up(MouseButton::Right) => {
            let dragged = app.right_mouse_screen.is_some_and(|(start_x, start_y)| {
                app.right_mouse
                    .is_dragging_screen(ev.column, ev.row, start_x, start_y)
            });
            if !dragged {
                let point = screen_to_canvas(ev.column, ev.row, app);
                let target = app.right_mouse_target.take().map_or(
                    DrawMenuTarget::Empty {
                        x: point.0,
                        y: point.1,
                    },
                    |id| {
                        if app
                            .data
                            .item(&id)
                            .is_some_and(|item| matches!(&item.element, DrawElement::Text(_)))
                        {
                            app.selection.select_only(id.clone());
                            DrawMenuTarget::Text(id)
                        } else {
                            app.selection.select_only(id.clone());
                            DrawMenuTarget::NonText(id)
                        }
                    },
                );
                if matches!(target, DrawMenuTarget::Empty { .. }) {
                    app.selection.clear();
                }
                app.hovered = None;
                app.open_context_menu(ev.column, ev.row, target, clipboard.is_some());
            }
            app.is_panning = false;
            app.last_mouse_pos = None;
            app.right_mouse.clear();
            app.right_mouse_screen = None;
            app.right_mouse_target = None;
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            app.last_mouse_pos = Some((ev.column, ev.row));
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
            panning(ev.column, ev.row, app);
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            app.is_panning = false;
            app.last_mouse_pos = None;
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            let point = screen_to_canvas(ev.column, ev.row, app);
            match app.active_tool {
                DrawTool::Cursor => cursor_left_drag(ev, point, app),
                DrawTool::Text => {}
                DrawTool::Draw => {
                    if let Some(stroke) = &mut app.current_stroke {
                        stroke.points.push(point);
                    }
                }
                DrawTool::Erase => {
                    if app.erase_start_data.is_some() {
                        erase_at(point.0, point.1, app);
                    }
                }
                DrawTool::Shape => {
                    if let Some(origin) = app.creation_origin {
                        app.preview_element = Some(create_shape(
                            origin.0,
                            origin.1,
                            point.0,
                            point.1,
                            app.active_shape_type,
                            app.active_color,
                        ));
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.active_tool == DrawTool::Cursor {
                finish_cursor_move(app)?;
                app.is_panning = false;
                app.last_mouse_pos = None;
                return Ok(None);
            }

            let mut previous = None;
            if let Some(mut stroke) = app.current_stroke.take() {
                stroke.points = crate::render::smooth_points(&stroke.points);
                previous = Some(app.data.clone());
                app.data
                    .elements
                    .push(DrawItem::new(DrawElement::Stroke(stroke)));
            }
            if let Some(element) = app.preview_element.take() {
                if previous.is_none() {
                    previous = Some(app.data.clone());
                }
                app.data.elements.push(DrawItem::new(element));
            }
            if app.active_tool == DrawTool::Erase {
                previous = app.erase_start_data.take();
            }
            app.creation_origin = None;
            if let Some(previous) = previous {
                app.commit_data_change(previous)?;
            }
        }
        MouseEventKind::ScrollUp => {
            app.viewport.zoom *= 1.1;
        }
        MouseEventKind::ScrollDown => {
            app.viewport.zoom /= 1.1;
        }
        _ => {}
    }

    Ok(None)
}

pub fn handle_menu_shortcut(
    key: crossterm::event::KeyEvent,
    app: &mut DrawState,
    clipboard: &mut Option<DrawItem>,
) -> anyhow::Result<bool> {
    let KeyCode::Char(shortcut) = key.code else {
        return Ok(false);
    };
    let Some(index) = app
        .context_menu
        .as_ref()
        .and_then(|m| m.find_shortcut(shortcut))
    else {
        return Ok(false);
    };
    select_menu_item(app, clipboard, index)?;
    Ok(true)
}

fn select_menu_item(
    app: &mut DrawState,
    clipboard: &mut Option<DrawItem>,
    index: usize,
) -> anyhow::Result<()> {
    if let Some(menu) = app.context_menu.take() {
        let target = app.menu_target.take();
        let kind = app.menu_kind.take();
        if let (Some(target), Some(kind)) = (target, kind) {
            execute_menu_item(app, clipboard, target, kind, index, menu.x, menu.y)?;
        }
    }
    Ok(())
}

fn handle_context_menu_mouse(
    ev: MouseEvent,
    app: &mut DrawState,
    clipboard: &mut Option<DrawItem>,
) -> anyhow::Result<Option<DrawEventAction>> {
    if ev.kind != MouseEventKind::Down(MouseButton::Left) {
        return Ok(None);
    }

    let menu_action = app.context_menu.as_ref().and_then(|menu| {
        menu.row_at(menu.rect(app.last_area), ev.column, ev.row)
            .map(|index| (index, menu.x, menu.y))
    });
    let target = app.menu_target.clone();
    let kind = app.menu_kind.take();
    app.context_menu = None;

    if let (Some((index, menu_x, menu_y)), Some(target), Some(kind)) = (menu_action, target, kind) {
        execute_menu_item(app, clipboard, target, kind, index, menu_x, menu_y)?;
    } else {
        app.menu_target = None;
    }
    Ok(None)
}

fn execute_menu_item(
    app: &mut DrawState,
    clipboard: &mut Option<DrawItem>,
    target: DrawMenuTarget,
    kind: DrawMenuKind,
    index: usize,
    menu_x: u16,
    menu_y: u16,
) -> anyhow::Result<()> {
    match kind {
        DrawMenuKind::Actions => {
            let Some(&item) = draw_menu_items(&target, clipboard.is_some()).get(index) else {
                app.menu_target = None;
                return Ok(());
            };
            match item {
                DrawMenuItem::Rotate => {
                    if let Some(id) = target.item_id() {
                        begin_rotate(app, id.clone());
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::Scale => {
                    if let Some(id) = target.item_id() {
                        begin_scale(app, id.clone());
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::Color => {
                    app.menu_target = Some(target);
                    app.open_color_menu(menu_x, menu_y);
                }
                DrawMenuItem::Copy => {
                    if let Some(id) = target.item_id() {
                        copy_item(app, clipboard, id);
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::Erase => {
                    if let Some(id) = target.item_id() {
                        erase_item(app, id)?;
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::EditText => {
                    if let Some(id) = target.item_id() {
                        app.begin_text_editor(id.clone());
                    }
                    app.menu_target = None;
                }
                DrawMenuItem::Paste => {
                    let anchor = match target {
                        DrawMenuTarget::Empty { x, y } => Some((x, y)),
                        DrawMenuTarget::NonText(_) | DrawMenuTarget::Text(_) => None,
                    };
                    begin_paste(app, clipboard.as_ref(), anchor);
                    app.menu_target = None;
                }
            }
        }
        DrawMenuKind::Color => {
            if let Some(id) = target.item_id()
                && let Some((_, _, ratatui::style::Color::Rgb(red, green, blue))) =
                    crate::COLOR_PICKER_PALETTE.get(index)
            {
                set_item_color(app, id, (*red, *green, *blue))?;
            }
            app.menu_target = None;
        }
    }
    Ok(())
}

fn copy_selected(app: &mut DrawState, clipboard: &mut Option<DrawItem>) {
    if let Some(id) = app.selection.primary.clone() {
        copy_item(app, clipboard, &id);
    }
}

fn copy_item(app: &mut DrawState, clipboard: &mut Option<DrawItem>, id: &crate::state::DrawItemId) {
    if let Some(item) = app.data.item(id) {
        *clipboard = Some(item.clone());
        app.notify("Element copied");
    }
}

fn begin_paste(app: &mut DrawState, clipboard: Option<&DrawItem>, anchor: Option<(f64, f64)>) {
    let Some(clipboard) = clipboard else {
        return;
    };
    let point = anchor
        .or_else(|| app.mouse_pos.map(|(x, y)| screen_to_canvas(x, y, app)))
        .unwrap_or((app.viewport.x, app.viewport.y));
    let mut item = crate::state::pasted_item(clipboard);
    place_pasted_item(&mut item, point);
    app.clear_transient_interaction();
    app.interaction = Some(DrawInteraction::Paste { item });
}

fn place_pasted_item(item: &mut DrawItem, point: (f64, f64)) {
    if let Some(bounds) = crate::geometry::transformed_bounds(item) {
        let center = bounds.center();
        item.transform.translate_x += point.0 - center.0;
        item.transform.translate_y += point.1 - center.1;
    }
}

fn begin_rotate(app: &mut DrawState, id: crate::state::DrawItemId) {
    let Some(item) = app.data.item(&id) else {
        return;
    };
    if matches!(&item.element, DrawElement::Text(_)) {
        return;
    }
    app.interaction = Some(DrawInteraction::Rotate {
        id,
        pivot_world: (
            item.transform.pivot_x + item.transform.translate_x,
            item.transform.pivot_y + item.transform.translate_y,
        ),
        original_degrees: item.transform.rotation_degrees,
        preview_degrees: item.transform.rotation_degrees,
        start_angle: None,
    });
}

fn begin_scale(app: &mut DrawState, id: crate::state::DrawItemId) {
    let Some(item) = app.data.item(&id) else {
        return;
    };
    if matches!(&item.element, DrawElement::Text(_)) {
        return;
    }
    app.interaction = Some(DrawInteraction::Scale {
        id,
        pivot_world: (
            item.transform.pivot_x + item.transform.translate_x,
            item.transform.pivot_y + item.transform.translate_y,
        ),
        original_scale: item.transform.scale,
        preview_scale: item.transform.scale,
        start_distance: None,
    });
}

fn erase_item(app: &mut DrawState, id: &crate::state::DrawItemId) -> anyhow::Result<()> {
    if app.data.item(id).is_none() {
        return Ok(());
    }
    let previous = app.data.clone();
    app.data.elements.retain(|item| item.id != *id);
    app.selection.clear();
    app.hovered = None;
    app.commit_data_change(previous)?;
    Ok(())
}

fn set_item_color(
    app: &mut DrawState,
    id: &crate::state::DrawItemId,
    color: (u8, u8, u8),
) -> anyhow::Result<()> {
    let Some(item) = app.data.item(id) else {
        return Ok(());
    };
    let previous_color = element_color(&item.element);
    if previous_color == color {
        return Ok(());
    }
    let previous = app.data.clone();
    if let Some(item) = app.data.item_mut(id) {
        set_element_color(&mut item.element, color);
        app.commit_data_change(previous)?;
    }
    Ok(())
}

fn element_color(element: &DrawElement) -> (u8, u8, u8) {
    match element {
        DrawElement::Stroke(stroke) => stroke.color,
        DrawElement::Shape(
            Shape::Rect { color, .. }
            | Shape::Ellipse { color, .. }
            | Shape::Diamond { color, .. }
            | Shape::Line { color, .. }
            | Shape::Arrow { color, .. },
        ) => *color,
        DrawElement::Text(text) => text.color,
    }
}

fn set_element_color(element: &mut DrawElement, color: (u8, u8, u8)) {
    match element {
        DrawElement::Stroke(stroke) => stroke.color = color,
        DrawElement::Shape(
            Shape::Rect { color: current, .. }
            | Shape::Ellipse { color: current, .. }
            | Shape::Diamond { color: current, .. }
            | Shape::Line { color: current, .. }
            | Shape::Arrow { color: current, .. },
        ) => *current = color,
        DrawElement::Text(text) => text.color = color,
    }
}

fn handle_transform_or_paste_mouse(
    ev: MouseEvent,
    app: &mut DrawState,
) -> anyhow::Result<Option<DrawEventAction>> {
    let point = screen_to_canvas(ev.column, ev.row, app);
    if matches!(&app.interaction, Some(DrawInteraction::Paste { .. })) {
        match ev.kind {
            MouseEventKind::Moved => update_paste_position(app, point),
            MouseEventKind::Down(MouseButton::Left) => {
                update_paste_position(app, point);
                commit_paste(app)?;
            }
            _ => {}
        }
        return Ok(None);
    }

    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => begin_transform_drag(app, point),
        MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved => {
            update_transform_preview(
                app,
                point,
                ev.modifiers.contains(crossterm::event::KeyModifiers::SHIFT),
            );
        }
        MouseEventKind::Up(MouseButton::Left) => finish_transform(app)?,
        _ => {}
    }
    Ok(None)
}

fn update_paste_position(app: &mut DrawState, point: (f64, f64)) {
    if let Some(DrawInteraction::Paste { item }) = &mut app.interaction {
        place_pasted_item(item, point);
    }
}

fn commit_paste(app: &mut DrawState) -> anyhow::Result<()> {
    let Some(DrawInteraction::Paste { item }) = app.interaction.take() else {
        return Ok(());
    };
    let id = item.id.clone();
    let previous = app.data.clone();
    app.data.elements.push(item);
    app.selection.select_only(id);
    app.commit_data_change(previous)?;
    app.notify("Element pasted");
    Ok(())
}

fn begin_transform_drag(app: &mut DrawState, point: (f64, f64)) {
    match &mut app.interaction {
        Some(DrawInteraction::Rotate {
            pivot_world,
            start_angle,
            ..
        }) => {
            *start_angle = Some((point.1 - pivot_world.1).atan2(point.0 - pivot_world.0));
        }
        Some(DrawInteraction::Scale {
            pivot_world,
            start_distance,
            ..
        }) => {
            *start_distance = Some((point.0 - pivot_world.0).hypot(point.1 - pivot_world.1));
        }
        _ => {}
    }
}

fn update_transform_preview(app: &mut DrawState, point: (f64, f64), snap: bool) {
    match &mut app.interaction {
        Some(DrawInteraction::Rotate {
            pivot_world,
            original_degrees,
            preview_degrees,
            start_angle: Some(start_angle),
            ..
        }) => {
            let current = (point.1 - pivot_world.1).atan2(point.0 - pivot_world.0);
            let mut delta = (current - *start_angle).to_degrees();
            if snap {
                delta = (delta / 15.0).round() * 15.0;
            }
            *preview_degrees = (*original_degrees + delta).rem_euclid(360.0);
        }
        Some(DrawInteraction::Scale {
            pivot_world,
            original_scale,
            preview_scale,
            start_distance: Some(start_distance),
            ..
        }) if *start_distance > f64::EPSILON => {
            let current = (point.0 - pivot_world.0).hypot(point.1 - pivot_world.1);
            *preview_scale = (*original_scale * current / *start_distance).clamp(0.1, 10.0);
        }
        _ => {}
    }
}

fn finish_transform(app: &mut DrawState) -> anyhow::Result<()> {
    let Some(interaction) = app.interaction.take() else {
        return Ok(());
    };
    match interaction {
        paused @ (DrawInteraction::Rotate {
            start_angle: None, ..
        }
        | DrawInteraction::Scale {
            start_distance: None,
            ..
        }) => {
            app.interaction = Some(paused);
        }
        DrawInteraction::Rotate {
            id,
            original_degrees,
            preview_degrees,
            ..
        } if original_degrees != preview_degrees => {
            let previous = app.data.clone();
            if let Some(item) = app.data.item_mut(&id) {
                item.transform.rotation_degrees = preview_degrees;
                app.commit_data_change(previous)?;
            }
        }
        DrawInteraction::Scale {
            id,
            original_scale,
            preview_scale,
            ..
        } if original_scale != preview_scale => {
            let previous = app.data.clone();
            if let Some(item) = app.data.item_mut(&id) {
                item.transform.scale = preview_scale;
                app.commit_data_change(previous)?;
            }
        }
        DrawInteraction::Move { .. } | DrawInteraction::Paste { .. } => {}
        DrawInteraction::Rotate { .. } | DrawInteraction::Scale { .. } => {}
    }
    Ok(())
}

fn cursor_left_down(mouse: MouseEvent, point: (f64, f64), app: &mut DrawState) {
    let hit = app.topmost_hit(point);
    let double_click = app.last_click.is_some_and(|(column, row, at)| {
        column == mouse.column && row == mouse.row && at.elapsed().as_millis() < 500
    });

    if let Some(id) = hit {
        let is_text = app
            .data
            .item(&id)
            .is_some_and(|item| matches!(&item.element, DrawElement::Text(_)));
        if double_click && is_text {
            app.selection.select_only(id.clone());
            app.hovered = None;
            app.begin_text_editor(id);
            app.last_click = None;
            return;
        }

        let translation = app.data.item(&id).map_or((0.0, 0.0), |item| {
            (item.transform.translate_x, item.transform.translate_y)
        });
        app.selection.select_only(id.clone());
        app.hovered = None;
        app.interaction = Some(DrawInteraction::Move {
            id,
            start_world: point,
            original_translation: translation,
            preview_translation: translation,
        });
    } else {
        app.selection.clear();
        app.hovered = None;
        app.interaction = None;
        app.last_mouse_pos = Some((mouse.column, mouse.row));
    }
    app.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
}

fn cursor_left_drag(mouse: MouseEvent, point: (f64, f64), app: &mut DrawState) {
    let Some(DrawInteraction::Move {
        start_world,
        original_translation,
        preview_translation,
        ..
    }) = &mut app.interaction
    else {
        if app.last_mouse_pos.is_some() {
            panning(mouse.column, mouse.row, app);
        }
        return;
    };
    preview_translation.0 = original_translation.0 + point.0 - start_world.0;
    preview_translation.1 = original_translation.1 + point.1 - start_world.1;
}

fn finish_cursor_move(app: &mut DrawState) -> anyhow::Result<()> {
    let Some(DrawInteraction::Move {
        id,
        original_translation,
        preview_translation,
        ..
    }) = app.interaction.take()
    else {
        return Ok(());
    };
    if original_translation == preview_translation {
        return Ok(());
    }

    let previous = app.data.clone();
    if let Some(item) = app.data.item_mut(&id) {
        item.transform.translate_x = preview_translation.0;
        item.transform.translate_y = preview_translation.1;
        app.commit_data_change(previous)?;
    }
    Ok(())
}

fn create_shape(
    ox: f64,
    oy: f64,
    cx: f64,
    cy: f64,
    st: DrawShapeType,
    color: (u8, u8, u8),
) -> DrawElement {
    match st {
        DrawShapeType::Rect => DrawElement::Shape(Shape::Rect {
            x: ox.min(cx),
            y: oy.min(cy),
            width: (ox - cx).abs(),
            height: (oy - cy).abs(),
            color,
        }),
        DrawShapeType::Ellipse => DrawElement::Shape(Shape::Ellipse {
            x: ox.min(cx),
            y: oy.min(cy),
            width: (ox - cx).abs(),
            height: (oy - cy).abs(),
            color,
        }),
        DrawShapeType::Diamond => DrawElement::Shape(Shape::Diamond {
            x: ox.min(cx),
            y: oy.min(cy),
            width: (ox - cx).abs(),
            height: (oy - cy).abs(),
            color,
        }),
        DrawShapeType::Line => DrawElement::Shape(Shape::Line {
            x1: ox,
            y1: oy,
            x2: cx,
            y2: cy,
            color,
        }),
        DrawShapeType::Arrow => DrawElement::Shape(Shape::Arrow {
            x1: ox,
            y1: oy,
            x2: cx,
            y2: cy,
            color,
        }),
    }
}

fn erase_at(cx: f64, cy: f64, app: &mut DrawState) {
    app.data
        .elements
        .retain(|item| !crate::geometry::hit_test_item(item, (cx, cy), 5.0, &app.viewport));
}

fn panning(x: u16, y: u16, app: &mut DrawState) {
    app.is_panning = true;
    if let Some((lx, ly)) = app.last_mouse_pos {
        let area = app.last_area;
        if area.width > 0 && area.height > 0 {
            let projection = crate::geometry::CanvasProjection::new(area, &app.viewport);
            let (dx, dy) = projection.pan_delta((lx, ly), (x, y));
            app.viewport.x += dx;
            app.viewport.y += dy;
        }
        app.last_mouse_pos = Some((x, y));
    }
}

pub fn screen_to_canvas(sx: u16, sy: u16, app: &DrawState) -> (f64, f64) {
    crate::geometry::CanvasProjection::new(app.last_area, &app.viewport).world(sx, sy)
}

pub fn apply_action(
    app: &mut DrawState,
    action: DrawAction,
    clipboard: &mut Option<DrawItem>,
) -> anyhow::Result<DrawEventAction> {
    if action == DrawAction::Paste && clipboard.is_some() {
        let anchor = app.menu_target.as_ref().and_then(|t| match t {
            DrawMenuTarget::Empty { x, y } => Some((*x, *y)),
            _ => None,
        });
        app.context_menu = None;
        app.menu_target = None;
        app.menu_kind = None;
        begin_paste(app, clipboard.as_ref(), anchor);
        return Ok(DrawEventAction::Continue);
    }
    if let Some(menu) = &mut app.context_menu {
        match action {
            DrawAction::MenuClose => {
                app.context_menu = None;
                app.menu_target = None;
                app.menu_kind = None;
            }
            DrawAction::MenuUp => menu.move_up(),
            DrawAction::MenuDown => menu.move_down(),
            DrawAction::MenuSelect => {
                let index = menu.selected;
                select_menu_item(app, clipboard, index)?;
            }
            _ => {}
        }
        return Ok(DrawEventAction::Continue);
    }
    match action {
        DrawAction::Quit => {
            return Ok(DrawEventAction::Quit);
        }
        DrawAction::SelectCursorTool => {
            app.set_active_tool(DrawTool::Cursor);
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::SelectDrawTool => {
            app.set_active_tool(DrawTool::Draw);
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::ToggleShapeSelector => {
            if app.show_shape_selector {
                app.show_shape_selector = false;
            } else {
                app.clear_transient_interaction();
                app.show_shape_selector = true;
            }
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::ToggleColorSelector => {
            if app.show_color_selector {
                app.show_color_selector = false;
            } else {
                app.clear_transient_interaction();
                app.show_color_selector = true;
            }
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::SelectTextTool => {
            app.set_active_tool(DrawTool::Text);
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::SelectEraseTool => {
            app.set_active_tool(DrawTool::Erase);
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::Copy => {
            copy_selected(app, clipboard);
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::Paste => {
            begin_paste(app, clipboard.as_ref(), None);
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::Undo => {
            app.undo()?;
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::Redo => {
            app.redo()?;
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::Help => {
            return Ok(DrawEventAction::OpenHelp);
        }
        DrawAction::ToggleGrid => {
            app.grid = !app.grid;
            return Ok(DrawEventAction::Continue);
        }
        DrawAction::MenuClose => {
            app.interaction = None;
            app.selection.clear();
            app.hovered = None;
        }
        DrawAction::MenuUp | DrawAction::MenuDown | DrawAction::MenuSelect => {}
        DrawAction::ShapeSelectorUp if app.show_shape_selector => cycle_shape_type(app, -1),
        DrawAction::ShapeSelectorDown if app.show_shape_selector => cycle_shape_type(app, 1),
        DrawAction::ShapeSelectorConfirm if app.show_shape_selector => {
            app.set_active_tool(DrawTool::Shape)
        }
        DrawAction::ShapeSelectorCancel => app.show_shape_selector = false,
        DrawAction::ColorSelectorUp if app.show_color_selector => cycle_color(app, -1),
        DrawAction::ColorSelectorDown if app.show_color_selector => cycle_color(app, 1),
        DrawAction::ColorSelectorConfirm | DrawAction::ColorSelectorCancel => {
            app.show_color_selector = false
        }
        DrawAction::ShapeSelectorUp
        | DrawAction::ShapeSelectorDown
        | DrawAction::ShapeSelectorConfirm
        | DrawAction::ColorSelectorUp
        | DrawAction::ColorSelectorDown => {}
        DrawAction::TextEditorCancel => {
            app.text_editor = None;
            app.mouse_selection = crate::ui::MouseTextSelection::default();
        }
        DrawAction::TextEditorConfirm => {
            if let Some((id, editor)) = app.text_editor.take() {
                let previous = app.data.clone();
                if let Some(item) = app.data.item_mut(&id)
                    && let DrawElement::Text(text) = &mut item.element
                {
                    text.content = editor.lines()[0].clone();
                }
                app.mouse_selection = crate::ui::MouseTextSelection::default();
                app.commit_data_change(previous)?;
            }
        }
    }
    Ok(DrawEventAction::Continue)
}

pub fn cancel_interaction_or_selection(app: &mut DrawState) -> bool {
    if app.interaction.take().is_some() {
        return true;
    }
    if !app.selection.is_empty() {
        app.selection.clear();
        app.hovered = None;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{Event, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;

    fn test_state() -> (tempfile::TempDir, DrawState) {
        let temp = tempfile::tempdir().unwrap();
        let state = DrawState::create(&temp.path().join("cursor.draw")).unwrap();
        (temp, state)
    }

    fn handle_event(
        ev: crossterm::event::Event,
        state: &mut DrawState,
        clipboard: &mut Option<DrawItem>,
    ) -> anyhow::Result<DrawEventAction> {
        match ev {
            crossterm::event::Event::Mouse(m) => handle_draw_mouse(state, m, clipboard),
            crossterm::event::Event::Key(k) => {
                if state.context_menu.is_some() {
                    handle_menu_shortcut(k, state, clipboard)?;
                    return Ok(DrawEventAction::Continue);
                }
                if handle_selected_menu_shortcut(k, state, clipboard)? {
                    return Ok(DrawEventAction::Continue);
                }
                let action = match k.code {
                    KeyCode::Esc => DrawAction::MenuClose,
                    KeyCode::Char('v') => DrawAction::Paste,
                    KeyCode::Char('a') => DrawAction::SelectCursorTool,
                    KeyCode::Char('G') => DrawAction::ToggleGrid,
                    _ => return Ok(DrawEventAction::Continue),
                };
                apply_action(state, action, clipboard)
            }
            _ => Ok(DrawEventAction::Continue),
        }
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn cursor_drag_commits_one_translated_item() {
        let (_temp, mut state) = test_state();
        state.last_area = Rect::new(0, 0, 100, 100);
        let item = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -2.0,
            y: -2.0,
            width: 4.0,
            height: 4.0,
            color: (255, 255, 255),
        }));
        let id = item.id.clone();
        state.data.elements.push(item);
        let mut clipboard = None;

        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 50),
            &mut state,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Drag(MouseButton::Left), 60, 60),
            &mut state,
            &mut clipboard,
        )
        .unwrap();
        assert_eq!(state.data.item(&id).unwrap().transform.translate_x, 0.0);
        let Some(DrawInteraction::Move {
            id: preview_id,
            start_world,
            original_translation,
            preview_translation,
        }) = &state.interaction
        else {
            panic!("cursor drag must create move preview");
        };
        assert_eq!(preview_id, &id);
        assert!((start_world.0 - 1.0).abs() < 1e-9);
        assert!((start_world.1 + 1.0).abs() < 1e-9);
        assert_eq!(*original_translation, (0.0, 0.0));
        assert!((preview_translation.0 - 20.0).abs() < 1e-9);
        assert!((preview_translation.1 + 20.0).abs() < 1e-9);

        handle_event(
            mouse(MouseEventKind::Up(MouseButton::Left), 60, 60),
            &mut state,
            &mut clipboard,
        )
        .unwrap();
        let item = state.data.item(&id).unwrap();
        assert!((item.transform.translate_x - 20.0).abs() < 1e-9);
        assert!((item.transform.translate_y + 20.0).abs() < 1e-9);
        assert_eq!(state.undo_stack.len(), 1);
        assert!(state.interaction.is_none());
    }

    #[test]
    fn cursor_empty_drag_pans_view() {
        let (_temp, mut state) = test_state();
        state.last_area = Rect::new(0, 0, 100, 100);
        let mut clipboard = None;

        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 20, 20),
            &mut state,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Drag(MouseButton::Left), 30, 25),
            &mut state,
            &mut clipboard,
        )
        .unwrap();

        assert!(state.is_panning);
        assert_eq!(state.viewport.x, -20.0);
        assert_eq!(state.viewport.y, 10.0);
        assert_eq!(state.undo_stack.len(), 0);

        handle_event(
            mouse(MouseEventKind::Up(MouseButton::Left), 30, 25),
            &mut state,
            &mut clipboard,
        )
        .unwrap();

        assert!(!state.is_panning);
        assert!(state.last_mouse_pos.is_none());
    }

    #[test]
    fn cursor_double_click_opens_text_editor() {
        let (_temp, mut state) = test_state();
        state.last_area = Rect::new(0, 0, 100, 100);
        let item = DrawItem::new(DrawElement::Text(Text {
            content: "text".to_string(),
            x: 0.0,
            y: 0.0,
            color: (255, 255, 255),
        }));
        let id = item.id.clone();
        state.data.elements.push(item);
        let mut clipboard = None;

        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 49),
            &mut state,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Up(MouseButton::Left), 50, 49),
            &mut state,
            &mut clipboard,
        )
        .unwrap();
        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 50, 49),
            &mut state,
            &mut clipboard,
        )
        .unwrap();

        assert_eq!(
            state.text_editor.as_ref().map(|(target, _)| target),
            Some(&id)
        );
        assert!(state.interaction.is_none());
    }

    #[test]
    fn context_menu_colors_copies_and_pastes_fresh_item() {
        let (_temp, mut state) = test_state();
        state.last_area = Rect::new(0, 0, 100, 100);
        let source = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -2.0,
            y: -2.0,
            width: 4.0,
            height: 4.0,
            color: (255, 255, 255),
        }));
        let source_id = source.id.clone();
        state.data.elements.push(source);
        let mut clipboard = None;

        for event in [
            mouse(MouseEventKind::Down(MouseButton::Right), 50, 50),
            mouse(MouseEventKind::Up(MouseButton::Right), 50, 50),
        ] {
            handle_event(event, &mut state, &mut clipboard).unwrap();
        }
        assert_eq!(
            state
                .context_menu
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|item| item.label)
                .collect::<Vec<_>>(),
            vec!["Rotate", "Scale", "Color...", "Copy", "Erase"]
        );

        handle_event(key(KeyCode::Char('o')), &mut state, &mut clipboard).unwrap();
        assert_eq!(state.menu_kind, Some(DrawMenuKind::Color));
        handle_event(key(KeyCode::Char('r')), &mut state, &mut clipboard).unwrap();
        assert_eq!(
            element_color(&state.data.item(&source_id).unwrap().element),
            (255, 82, 82)
        );

        for event in [
            mouse(MouseEventKind::Down(MouseButton::Right), 50, 50),
            mouse(MouseEventKind::Up(MouseButton::Right), 50, 50),
            key(KeyCode::Char('c')),
            mouse(MouseEventKind::Down(MouseButton::Right), 70, 50),
            mouse(MouseEventKind::Up(MouseButton::Right), 70, 50),
        ] {
            handle_event(event, &mut state, &mut clipboard).unwrap();
        }
        assert_eq!(
            state
                .context_menu
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|item| item.label)
                .collect::<Vec<_>>(),
            vec!["Paste"]
        );
        assert_eq!(clipboard.as_ref().map(|saved| &saved.id), Some(&source_id));

        handle_event(key(KeyCode::Char('v')), &mut state, &mut clipboard).unwrap();
        handle_event(
            mouse(MouseEventKind::Down(MouseButton::Left), 70, 50),
            &mut state,
            &mut clipboard,
        )
        .unwrap();

        assert_eq!(state.data.elements.len(), 2);
        let pasted = state.data.elements.last().unwrap();
        assert_ne!(pasted.id, source_id);
        let center = crate::geometry::transformed_bounds(pasted)
            .expect("pasted draw item has bounds")
            .center();
        assert!((center.0 - 41.0).abs() < 1e-9);
        assert!((center.1 + 1.0).abs() < 1e-9);
        assert_eq!(state.selection.primary, Some(pasted.id.clone()));
        assert_eq!(state.undo_stack.len(), 2);
    }

    #[test]
    fn paste_key_starts_placement_without_context_menu() {
        let (_temp, mut state) = test_state();
        let source = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -2.0,
            y: -2.0,
            width: 4.0,
            height: 4.0,
            color: (255, 255, 255),
        }));
        let mut clipboard = Some(source.clone());

        handle_event(key(KeyCode::Char('v')), &mut state, &mut clipboard).unwrap();

        assert!(state.context_menu.is_none());
        assert!(matches!(
            state.interaction,
            Some(DrawInteraction::Paste { .. })
        ));
    }

    #[test]
    fn selected_context_shortcut_precedes_cursor_tool_shortcut() {
        let (_temp, mut state) = test_state();
        let item = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -1.0,
            y: -1.0,
            width: 2.0,
            height: 2.0,
            color: (255, 255, 255),
        }));
        let id = item.id.clone();
        state.data.elements.push(item);
        state.selection.select_only(id.clone());
        let mut clipboard = None;

        handle_event(key(KeyCode::Char('e')), &mut state, &mut clipboard).unwrap();

        assert!(state.data.item(&id).is_none());
        let empty = state.selection.is_empty();
        assert!(empty);
        assert_eq!(state.active_tool, DrawTool::Cursor);
        assert_eq!(state.undo_stack.len(), 1);
    }

    #[test]
    fn selected_context_transform_shortcuts_enter_matching_modes() {
        let (_temp, mut state) = test_state();
        let item = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -1.0,
            y: -1.0,
            width: 2.0,
            height: 2.0,
            color: (255, 255, 255),
        }));
        let id = item.id.clone();
        state.data.elements.push(item);
        state.selection.select_only(id.clone());
        let mut clipboard = None;

        handle_event(key(KeyCode::Char('r')), &mut state, &mut clipboard).unwrap();
        assert!(matches!(
            &state.interaction,
            Some(DrawInteraction::Rotate { id: target, .. }) if target == &id
        ));

        state.interaction = None;
        handle_event(key(KeyCode::Char('s')), &mut state, &mut clipboard).unwrap();
        assert!(matches!(
            &state.interaction,
            Some(DrawInteraction::Scale { id: target, .. }) if target == &id
        ));
    }

    #[test]
    fn rotate_and_scale_preview_then_commit_once_each() {
        let (_temp, mut state) = test_state();
        let item = DrawItem::new(DrawElement::Shape(Shape::Rect {
            x: -1.0,
            y: -1.0,
            width: 2.0,
            height: 2.0,
            color: (255, 255, 255),
        }));
        let id = item.id.clone();
        state.data.elements.push(item);
        state.selection.select_only(id.clone());
        let rotation = crate::geometry::selection_handle_points(
            state.data.item(&id).unwrap(),
            &state.data.item(&id).unwrap().transform,
            &state.viewport,
        )
        .unwrap()
        .0;
        cursor_left_down(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            rotation,
            &mut state,
        );
        assert!(!matches!(
            state.interaction,
            Some(DrawInteraction::Rotate { .. } | DrawInteraction::Scale { .. })
        ));

        begin_rotate(&mut state, id.clone());
        begin_transform_drag(&mut state, (1.0, 0.0));
        update_transform_preview(&mut state, (0.0, 1.0), false);
        finish_transform(&mut state).unwrap();
        assert!((state.data.item(&id).unwrap().transform.rotation_degrees - 90.0).abs() < 1e-9);

        begin_scale(&mut state, id.clone());
        begin_transform_drag(&mut state, (1.0, 0.0));
        update_transform_preview(&mut state, (20.0, 0.0), false);
        finish_transform(&mut state).unwrap();
        assert_eq!(state.data.item(&id).unwrap().transform.scale, 10.0);
        assert_eq!(state.undo_stack.len(), 2);
    }

    #[test]
    fn cursor_and_grid_keybinds_update_transient_draw_state() {
        let (_temp, mut state) = test_state();
        let mut clipboard = None;
        state.active_tool = DrawTool::Draw;

        handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
            &mut state,
            &mut clipboard,
        )
        .unwrap();
        assert_eq!(state.active_tool, DrawTool::Cursor);

        handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT)),
            &mut state,
            &mut clipboard,
        )
        .unwrap();
        assert!(!state.grid);
    }
}
