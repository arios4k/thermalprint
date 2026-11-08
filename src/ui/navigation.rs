//! Full-width workspace navigation, with quiet inactive rows.
use eframe::egui::{self, Color32, Pos2, Stroke, StrokeKind, Vec2};

pub(super) fn item(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    tooltip: &str,
    selected: bool,
) -> egui::Response {
    let (label, tooltip) = (crate::i18n::tr(label), crate::i18n::tr(tooltip));
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
        // Text starts after the icon and must stay inside the row, whatever the language.
        let available = (response.rect.width() - TEXT_INDENT - TEXT_MARGIN).max(20.);
        let galley = fit_label(ui, label, available);
        ui.painter().galley(
            response.rect.left_center() + Vec2::new(TEXT_INDENT, -galley.size().y / 2.),
            galley,
            ui.visuals().text_color(),
        );
    }
    response
}

const TEXT_INDENT: f32 = 42.;
const TEXT_MARGIN: f32 = 10.;
const MIN_FONT_SIZE: f32 = 11.;

/// Lay out a one-line label no wider than `max_width`: shrink the font a little first,
/// then cut the end with "…" so a long translation never spills out of the sidebar.
pub(super) fn fit_label(
    ui: &egui::Ui,
    label: &str,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let base = egui::TextStyle::Button.resolve(ui.style());
    let layout = |font: egui::FontId, elide: bool| {
        let mut job = egui::text::LayoutJob::simple_singleline(
            label.to_owned(),
            font,
            ui.visuals().text_color(),
        );
        if elide {
            job.wrap = egui::text::TextWrapping {
                max_width,
                max_rows: 1,
                break_anywhere: true,
                overflow_character: Some('…'),
            };
        }
        ui.painter().layout_job(job)
    };
    let natural = layout(base.clone(), false);
    if natural.size().x <= max_width {
        return natural;
    }
    let size = (base.size * max_width / natural.size().x).max(MIN_FONT_SIZE);
    let shrunk = layout(egui::FontId::new(size, base.family.clone()), false);
    if shrunk.size().x <= max_width {
        shrunk
    } else {
        layout(egui::FontId::new(size, base.family), true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_labels_shrink_or_elide_to_fit_and_short_ones_are_untouched() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let width = 150.;
            for label in [
                "Home",
                "Concepteur d'étiquettes",
                "Importieren und Konvertieren",
                "An extremely long label that cannot possibly fit in the sidebar at any size",
            ] {
                let galley = fit_label(ui, label, width);
                assert!(
                    galley.size().x <= width + 0.5,
                    "{label} is {} wide",
                    galley.size().x
                );
            }
            let natural = fit_label(ui, "Home", width);
            assert_eq!(
                natural.job.sections[0].format.font_id.size,
                egui::TextStyle::Button.resolve(ui.style()).size
            );
            let elided = fit_label(
                ui,
                "An extremely long label that cannot possibly fit in the sidebar at any size",
                width,
            );
            assert!(
                elided.elided,
                "text that cannot fit is cut with an ellipsis"
            );
        });
        output.textures_delta.clear();
    }
}
