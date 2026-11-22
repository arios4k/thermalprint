//! ThermalPrint's own look: a teal accent taken from the app icon, a neutral interface
//! font, one corner radius, and a comfortable or compact density. It deliberately follows
//! no single desktop's design language, so the app looks at home on Windows and on any
//! Linux desktop.
use eframe::egui::{self, Color32, CornerRadius, Stroke, Vec2};

/// The accent on light backgrounds: the teal of the app icon, dark enough for text.
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(21, 116, 104);
/// The accent on dark backgrounds.
pub const ACCENT_DARK: Color32 = Color32::from_rgb(86, 196, 172);
/// Corner radius of buttons and fields.
pub const RADIUS: u8 = 6;
/// Corner radius of cards.
pub const CARD_RADIUS: u8 = 10;

/// The accent color for the current light or dark style.
pub fn accent(visuals: &egui::Visuals) -> Color32 {
    if visuals.dark_mode {
        ACCENT_DARK
    } else {
        ACCENT_LIGHT
    }
}

/// Amber for warnings that do not block anything.
pub fn warning(visuals: &egui::Visuals) -> Color32 {
    if visuals.dark_mode {
        Color32::from_rgb(245, 193, 99)
    } else {
        Color32::from_rgb(132, 83, 10)
    }
}

/// Apply the look to both the light and the dark style. `compact` tightens spacing for
/// small screens and long working days.
pub fn apply(ctx: &egui::Context, compact: bool) {
    ctx.all_styles_mut(|style| {
        let dark = style.visuals.dark_mode;
        if compact {
            style.spacing.item_spacing = Vec2::new(8., 5.);
            style.spacing.button_padding = Vec2::new(9., 4.);
            style.spacing.interact_size.y = 27.;
        } else {
            style.spacing.item_spacing = Vec2::new(10., 8.);
            style.spacing.button_padding = Vec2::new(12., 6.);
            style.spacing.interact_size.y = 32.;
        }
        let body = if compact { 13.5 } else { 14. };
        for (text, size) in [
            (egui::TextStyle::Body, body),
            (egui::TextStyle::Button, body),
            (egui::TextStyle::Small, 12.),
            (egui::TextStyle::Heading, 22.),
        ] {
            style
                .text_styles
                .insert(text, egui::FontId::proportional(size));
        }
        let visuals = &mut style.visuals;
        visuals.weak_text_color = Some(if dark {
            Color32::from_rgb(178, 184, 192)
        } else {
            Color32::from_rgb(86, 92, 102)
        });
        let accent = if dark { ACCENT_DARK } else { ACCENT_LIGHT };
        visuals.selection.bg_fill = if dark {
            Color32::from_rgb(30, 118, 103)
        } else {
            ACCENT_LIGHT
        };
        visuals.selection.stroke = Stroke::new(1., Color32::WHITE);
        visuals.hyperlink_color = accent;
        for widget in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = CornerRadius::same(RADIUS);
        }
        // A thin accent ring shows the hovered control, instead of a heavier fill.
        visuals.widgets.hovered.bg_stroke = Stroke::new(1., accent.gamma_multiply(0.7));
        visuals.window_corner_radius = CornerRadius::same(CARD_RADIUS);
        visuals.menu_corner_radius = CornerRadius::same(CARD_RADIUS);
        if dark {
            visuals.panel_fill = Color32::from_rgb(28, 31, 34);
            visuals.window_fill = Color32::from_rgb(36, 40, 44);
            visuals.extreme_bg_color = Color32::from_rgb(22, 25, 28);
            // egui's default grey text is just under the 4.5:1 contrast WCAG asks for.
            visuals.override_text_color = Some(Color32::from_rgb(222, 226, 230));
        } else {
            visuals.panel_fill = Color32::from_rgb(243, 245, 246);
            visuals.window_fill = Color32::WHITE;
            visuals.override_text_color = Some(Color32::from_rgb(36, 41, 48));
        }
    });
}

/// Card background for the current style.
pub fn card_fill(visuals: &egui::Visuals) -> Color32 {
    if visuals.dark_mode {
        Color32::from_rgb(36, 40, 44)
    } else {
        Color32::WHITE
    }
}

/// A card: a softly outlined, rounded panel for one group of settings.
pub fn card(ui: &egui::Ui) -> egui::Frame {
    let visuals = ui.visuals();
    egui::Frame::new()
        .fill(card_fill(visuals))
        .stroke(Stroke::new(
            1.,
            if visuals.dark_mode {
                Color32::from_rgb(52, 58, 64)
            } else {
                Color32::from_rgb(222, 226, 230)
            },
        ))
        .inner_margin(14)
        .corner_radius(CARD_RADIUS)
}

/// Use a neutral interface font from the computer when one is there: Segoe UI on Windows,
/// Inter or Noto Sans on Linux. The bundled DejaVu Sans is used otherwise. egui's own
/// default font stays behind it for any character the chosen font lacks.
pub fn install_fonts(ctx: &egui::Context) {
    let bytes = crate::fonts::interface_font()
        .unwrap_or_else(|| include_bytes!("../../assets/fonts/DejaVuSans.ttf").to_vec());
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "interface".into(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "interface".into());
    ctx.set_fonts(fonts);
}

/// A row of tabs, each with an icon: the selected one is underlined in the accent color.
/// Returns true when the selection changed.
pub fn tabs<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    current: &mut T,
    items: &[(T, &str, &str)],
) -> bool {
    let before = *current;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.;
        for &(value, label, icon) in items {
            let selected = *current == value;
            let label = crate::i18n::tr(label);
            let font = egui::TextStyle::Button.resolve(ui.style());
            let width = ui
                .painter()
                .layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE)
                .size()
                .x;
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(width + 38., 32.), egui::Sense::click());
            response
                .widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, label));
            // Screen readers announce a tab's state as "selected", not "toggled".
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_selected(selected);
                node.clear_toggled();
            });
            let color = if selected {
                accent(ui.visuals())
            } else if response.hovered() {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().text_color()
            };
            if response.hovered() && !selected {
                ui.painter()
                    .rect_filled(rect, RADIUS, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            crate::designer_ui::paint_tool_icon_scaled(
                ui.painter(),
                egui::Pos2::new(rect.left() + 16., rect.center().y),
                color,
                icon,
                16. / 24.,
            );
            ui.painter().text(
                egui::Pos2::new(rect.left() + 30., rect.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                font,
                color,
            );
            if selected {
                ui.painter().line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(2.5, accent(ui.visuals())),
                );
            }
            crate::designer_ui::focus_ring(ui, &response);
            if response.clicked() {
                *current = value;
            }
        }
    });
    ui.separator();
    *current != before
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance.
    fn luminance(c: Color32) -> f32 {
        let channel = |v: u8| {
            let v = v as f32 / 255.;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }
    fn contrast(a: Color32, b: Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    fn styles() -> Vec<egui::Style> {
        let ctx = egui::Context::default();
        apply(&ctx, false);
        [egui::Theme::Light, egui::Theme::Dark]
            .into_iter()
            .map(|theme| (*ctx.style_of(theme)).clone())
            .collect()
    }

    #[test]
    fn text_and_accent_meet_wcag_aa_contrast_in_both_themes() {
        for style in styles() {
            let v = &style.visuals;
            let backgrounds = [v.panel_fill, v.window_fill, card_fill(v)];
            for background in backgrounds {
                // Normal text needs 4.5:1.
                assert!(
                    contrast(v.text_color(), background) >= 4.5,
                    "text {background:?}"
                );
                assert!(
                    contrast(v.weak_text_color(), background) >= 4.5,
                    "weak {background:?}"
                );
                assert!(
                    contrast(accent(v), background) >= 4.5,
                    "accent {background:?}"
                );
                assert!(
                    contrast(warning(v), background) >= 4.5,
                    "warning {background:?}"
                );
            }
            // White labels on the selection and on primary buttons.
            assert!(contrast(Color32::WHITE, v.selection.bg_fill) >= 4.5);
            assert!(contrast(Color32::WHITE, ACCENT_LIGHT) >= 4.5);
        }
    }

    #[test]
    fn tabs_are_announced_as_tabs_with_their_state_and_work_from_the_keyboard() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut current = 0;
        let frame = |events: Vec<egui::Event>, current: &mut i32| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        tabs(
                            ui,
                            current,
                            &[
                                (0, "Printer", "Print device"),
                                (1, "Recent prints", "Status"),
                            ],
                        );
                    });
                },
            );
            output.textures_delta.clear();
            output
        };
        frame(vec![], &mut current);
        let output = frame(vec![], &mut current);
        let nodes = output.platform_output.accesskit_update.unwrap().nodes;
        let tab = |label: &str| {
            nodes
                .iter()
                .map(|(_, n)| n)
                .find(|n| n.label().is_some_and(|l| l == label))
                .cloned()
                .unwrap_or_else(|| panic!("{label}"))
        };
        assert_eq!(tab("Printer").role(), egui::accesskit::Role::Tab);
        assert_eq!(tab("Printer").is_selected(), Some(true));
        assert_eq!(tab("Recent prints").is_selected(), Some(false));
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        // Tab to the second tab and press Enter.
        frame(vec![key(egui::Key::Tab)], &mut current);
        frame(vec![key(egui::Key::Tab)], &mut current);
        frame(vec![key(egui::Key::Enter)], &mut current);
        assert_eq!(current, 1);
    }

    #[test]
    fn every_tab_label_is_translated() {
        for label in [
            "Object",
            "Label",
            "Footer",
            "Printer",
            "Label stock",
            "Recent prints",
            "Automation",
        ] {
            for language in [
                crate::i18n::Resolved::Spanish,
                crate::i18n::Resolved::German,
                crate::i18n::Resolved::French,
            ] {
                assert_ne!(
                    crate::i18n::translate(language, label),
                    label,
                    "{label} {language:?}"
                );
            }
        }
    }
}
