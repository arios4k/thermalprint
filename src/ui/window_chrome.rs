//! Client-side title bar with native window-manager move/resize commands.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

pub(super) fn title(ui: &mut egui::Ui, icon: &egui::TextureHandle) {
    let controls_width = 4. * 38. + 5. * ui.spacing().item_spacing.x + 10.;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new((ui.available_width() - controls_width).max(120.), 42.),
        Sense::click_and_drag(),
    );
    egui::Image::new(icon).paint_at(
        ui,
        Rect::from_center_size(
            Pos2::new(rect.left() + 18., rect.center().y),
            Vec2::splat(32.),
        ),
    );
    ui.painter().text(
        Pos2::new(rect.left() + 44., rect.center().y),
        egui::Align2::LEFT_CENTER,
        "ThermalPrint",
        egui::FontId::proportional(21.),
        ui.visuals().text_color(),
    );
    if rect.width() > 430. {
        ui.painter().text(
            Pos2::new(rect.left() + 188., rect.center().y),
            egui::Align2::LEFT_CENTER,
            "Universal shipping labels",
            egui::FontId::proportional(12.),
            ui.visuals().weak_text_color(),
        );
    }
    if response.double_clicked() {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    } else if response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}

pub(super) fn controls(ui: &mut egui::Ui) {
    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
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
        let response = ui
            .add(
                egui::Button::new("")
                    .frame(false)
                    .min_size(Vec2::splat(36.)),
            )
            .on_hover_text(label);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
        let hovered = response.hovered() || response.has_focus();
        let color = if hovered && kind == "close" {
            Color32::WHITE
        } else {
            ui.style().interact(&response).fg_stroke.color
        };
        if hovered {
            ui.painter().rect_filled(
                response.rect,
                8.,
                if kind == "close" {
                    Color32::from_rgb(180, 45, 55)
                } else {
                    ui.visuals().widgets.hovered.bg_fill
                },
            );
        }
        let r = Rect::from_center_size(response.rect.center(), Vec2::splat(14.));
        let stroke = Stroke::new(1.6, color);
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
                ui.painter().rect_stroke(
                    r.translate(Vec2::new(2., -2.)).shrink(2.),
                    1.,
                    stroke,
                    StrokeKind::Inside,
                );
                ui.painter().rect_filled(
                    r.translate(Vec2::new(-2., 2.)).shrink(2.),
                    1.,
                    ui.visuals().panel_fill,
                );
                ui.painter().rect_stroke(
                    r.translate(Vec2::new(-2., 2.)).shrink(2.),
                    1.,
                    stroke,
                    StrokeKind::Inside,
                );
            }
            _ => {
                ui.painter().rect_stroke(r, 1., stroke, StrokeKind::Inside);
            }
        }
        if response.clicked() {
            ui.ctx().send_viewport_cmd(command);
        }
    }
    ui.separator();
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
