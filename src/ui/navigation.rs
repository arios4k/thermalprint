//! Full-width workspace navigation, with quiet inactive rows.
use eframe::egui::{self, Color32, Pos2, Stroke, StrokeKind, Vec2};

pub(super) fn item(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    tooltip: &str,
    selected: bool,
) -> egui::Response {
    let response = ui
        .add_sized(
            [ui.available_width(), 44.],
            egui::Button::new("").frame(false).selected(selected),
        )
        .on_hover_text(tooltip);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, tooltip)
    });
    let dark = ui.visuals().dark_mode;
    let accent = if dark {
        Color32::from_rgb(140, 185, 255)
    } else {
        ui.visuals().selection.bg_fill
    };
    if selected {
        ui.painter().rect_filled(
            response.rect,
            9.,
            if dark {
                Color32::from_rgb(29, 47, 77)
            } else {
                Color32::from_rgb(225, 234, 249)
            },
        );
        ui.painter().rect_stroke(
            response.rect,
            9.,
            Stroke::new(1., accent),
            StrokeKind::Inside,
        );
    } else if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(response.rect, 9., ui.visuals().widgets.hovered.bg_fill);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.),
            8.,
            Stroke::new(2., accent),
            StrokeKind::Inside,
        );
    }
    let icon_center = if label.is_empty() {
        response.rect.center()
    } else {
        Pos2::new(response.rect.left() + 20., response.rect.center().y)
    };
    crate::designer_ui::paint_tool_icon(
        ui.painter(),
        icon_center,
        if selected {
            accent
        } else {
            ui.visuals().text_color()
        },
        icon,
    );
    if !label.is_empty() {
        ui.painter().text(
            response.rect.left_center() + Vec2::new(42., 0.),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Button.resolve(ui.style()),
            ui.visuals().text_color(),
        );
    }
    response
}
