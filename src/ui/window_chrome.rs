//! Client-side title bar with native window-manager move/resize commands.
use eframe::egui::{self, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Height of the client-side title bar, matching a libadwaita header bar.
pub(super) const HEADER_HEIGHT: f32 = 46.;

/// Makes `rect` move the window when dragged and maximize it when double-clicked.
/// Widgets placed on top of it afterwards keep their own clicks.
pub(super) fn drag_area(ui: &mut egui::Ui, rect: Rect) {
    let response = ui.interact(
        rect,
        ui.scope_id().with("title-drag"),
        Sense::click_and_drag(),
    );
    if response.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    } else if response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}

/// A bold-looking title centered on `center`. The interface font has no bold face,
/// so the text is drawn twice, half a pixel apart.
pub(super) fn title_text(ui: &egui::Ui, center: Pos2, text: &str) {
    let font = egui::FontId::proportional(15.);
    let color = ui.visuals().strong_text_color();
    for dx in [0., 0.5] {
        ui.painter().text(
            center + Vec2::new(dx, 0.),
            egui::Align2::CENTER_CENTER,
            text,
            font.clone(),
            color,
        );
    }
}

/// A frameless header button (the main menu or help) that shows a soft rounded
/// background on hover, like a flat libadwaita header button.
pub(super) fn flat_button(ui: &mut egui::Ui, center: Pos2, label: &str) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(34.));
    let response = ui.put(rect, egui::Button::new("").frame(false));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    if response.hovered() || response.has_focus() || response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, 8., ui.visuals().widgets.hovered.weak_bg_fill);
    }
    response
}

/// The minimize, maximize and close buttons as small round buttons at the right end of
/// `bar`, the way GNOME draws them. Returns the left edge of the group.
pub(super) fn controls(ui: &mut egui::Ui, bar: Rect) -> f32 {
    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    let mut center = Pos2::new(bar.right() - 23., bar.center().y);
    for (kind, label, command) in [
        ("close", "Close window", egui::ViewportCommand::Close),
        (
            if maximized { "restore" } else { "maximize" },
            if maximized {
                "Restore window"
            } else {
                "Maximize window"
            },
            egui::ViewportCommand::Maximized(!maximized),
        ),
        (
            "minimize",
            "Minimize window",
            egui::ViewportCommand::Minimized(true),
        ),
    ] {
        let hit = Rect::from_center_size(center, Vec2::splat(34.));
        let response = ui
            .put(hit, egui::Button::new("").frame(false))
            .on_hover_text(label);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
        let widgets = &ui.visuals().widgets;
        let fill = if response.is_pointer_button_down_on() {
            widgets.active.weak_bg_fill
        } else if response.hovered() || response.has_focus() {
            widgets.hovered.weak_bg_fill
        } else {
            widgets.inactive.weak_bg_fill
        };
        ui.painter().circle_filled(center, 12., fill);
        if response.has_focus() {
            ui.painter()
                .circle_stroke(center, 13.5, ui.visuals().selection.stroke);
        }
        let color = ui.visuals().strong_text_color();
        let stroke = Stroke::new(1.5, color);
        let r = Rect::from_center_size(center, Vec2::splat(8.));
        match kind {
            "close" => {
                ui.painter()
                    .line_segment([r.left_top(), r.right_bottom()], stroke);
                ui.painter()
                    .line_segment([r.right_top(), r.left_bottom()], stroke);
            }
            "minimize" => {
                ui.painter()
                    .line_segment([r.left_center(), r.right_center()], stroke);
            }
            "restore" => {
                let back = r.translate(Vec2::new(1.5, -1.5)).shrink(1.);
                let front = r.translate(Vec2::new(-1.5, 1.5)).shrink(1.);
                ui.painter()
                    .rect_stroke(back, 1., stroke, StrokeKind::Inside);
                ui.painter().rect_filled(front, 1., fill);
                ui.painter()
                    .rect_stroke(front, 1., stroke, StrokeKind::Inside);
            }
            _ => {
                ui.painter().rect_stroke(r, 1., stroke, StrokeKind::Inside);
            }
        }
        if response.clicked() {
            ui.ctx().send_viewport_cmd(command);
        }
        center.x -= 36.;
    }
    center.x + 36. - 17.
}

fn resize_direction(rect: Rect, pos: Pos2) -> Option<egui::ResizeDirection> {
    if !rect.contains(pos) {
        return None;
    }
    let left = pos.x < rect.left() + 6.;
    let right = pos.x > rect.right() - 6.;
    let top = pos.y < rect.top() + 6.;
    let bottom = pos.y > rect.bottom() - 6.;
    use egui::ResizeDirection::*;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(NorthWest),
        (_, true, true, _) => Some(NorthEast),
        (true, _, _, true) => Some(SouthWest),
        (_, true, _, true) => Some(SouthEast),
        (true, _, _, _) => Some(West),
        (_, true, _, _) => Some(East),
        (_, _, true, _) => Some(North),
        (_, _, _, true) => Some(South),
        _ => None,
    }
}

pub(super) fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    let Some(pos) = ctx.pointer_hover_pos() else {
        return;
    };
    let Some(direction) = resize_direction(ctx.content_rect(), pos) else {
        return;
    };
    use egui::ResizeDirection::*;
    ctx.set_cursor_icon(match direction {
        North | South => egui::CursorIcon::ResizeVertical,
        East | West => egui::CursorIcon::ResizeHorizontal,
        NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
        NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
    });
    if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_edges_resize_in_all_eight_directions_without_claiming_the_content() {
        use egui::ResizeDirection::*;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(760., 560.));
        for (x, y, direction) in [
            (1., 1., NorthWest),
            (759., 1., NorthEast),
            (1., 559., SouthWest),
            (759., 559., SouthEast),
            (1., 280., West),
            (759., 280., East),
            (380., 1., North),
            (380., 559., South),
        ] {
            assert_eq!(resize_direction(rect, Pos2::new(x, y)), Some(direction));
        }
        assert_eq!(resize_direction(rect, rect.center()), None);
        assert_eq!(resize_direction(rect, Pos2::new(-1., 20.)), None);
        assert_eq!(resize_direction(rect, Pos2::new(20., 40.)), None);
    }
}
