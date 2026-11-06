//! Compact branded About content, matching the workspace palette.
use eframe::egui::{self, Color32, RichText, Vec2};

pub(super) fn content(ui: &mut egui::Ui, icon: &egui::TextureHandle) -> bool {
    let accent = ui.visuals().selection.bg_fill;
    let dark = ui.visuals().dark_mode;
    let hero_fill = if dark {
        Color32::from_rgb(29, 42, 65)
    } else {
        Color32::from_rgb(225, 234, 249)
    };
    egui::Frame::new()
        .fill(hero_fill)
        .corner_radius(16)
        .inner_margin(18)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.add(egui::Image::new(icon).fit_to_exact_size(Vec2::splat(64.)));
                ui.label(RichText::new("ThermalPrint").size(30.).strong());
                ui.label("Design and prepare thermal labels for Linux.");
                ui.add_space(4.);
                let copied = ui
                    .data_mut(|data| data.get_temp::<f64>(egui::Id::unique("about-version-copied")))
                    .is_some_and(|time| ui.input(|input| input.time) - time < 2.);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(if copied {
                                "Version copied".into()
                            } else {
                                format!("Version {} · Copy", env!("CARGO_PKG_VERSION"))
                            })
                            .color(Color32::WHITE),
                        )
                        .fill(accent)
                        .corner_radius(20),
                    )
                    .accessible_name("Copy ThermalPrint version information")
                    .on_hover_text("Copy version information")
                    .clicked()
                {
                    ui.ctx()
                        .copy_text(concat!("ThermalPrint ", env!("CARGO_PKG_VERSION")).into());
                    let time = ui.input(|input| input.time);
                    ui.data_mut(|data| {
                        data.insert_temp(egui::Id::unique("about-version-copied"), time)
                    });
                }
                if copied {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs(2));
                }
            });
        });
    ui.add_space(4.);
    ui.horizontal_wrapped(|ui| {
        for text in ["Thermal labels", "Linux app", "Open source"] {
            egui::Frame::new()
                .fill(if dark {
                    Color32::from_gray(48)
                } else {
                    Color32::from_rgb(232, 235, 240)
                })
                .corner_radius(8)
                .inner_margin(8)
                .show(ui, |ui| {
                    ui.label(text);
                });
        }
    });
    ui.add_space(8.);
    ui.heading("Explore ThermalPrint");
    card(ui, |ui| {
        if row(
            ui,
            "Project & source code",
            "Documentation, releases, and the code behind ThermalPrint",
            "↗",
            true,
        ) {
            ui.ctx()
                .open_url(egui::OpenUrl::new_tab(env!("CARGO_PKG_REPOSITORY")));
        }
        ui.separator();
        if row(
            ui,
            "Report an issue",
            "Share a bug or suggest an improvement",
            "↗",
            true,
        ) {
            ui.ctx().open_url(egui::OpenUrl::new_tab(concat!(
                env!("CARGO_PKG_REPOSITORY"),
                "/issues"
            )));
        }
    });
    ui.add_space(8.);
    ui.heading("Credits and license");
    let mut license = false;
    card(ui, |ui| {
        row(ui, "Created by", env!("CARGO_PKG_AUTHORS"), "", false);
        ui.separator();
        license = row(
            ui,
            "MIT License",
            "Free to use, modify, and share",
            "›",
            true,
        );
    });
    ui.add_space(4.);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(concat!("© 2026 ", env!("CARGO_PKG_AUTHORS")))
                .small()
                .weak(),
        );
    });
    license
}

fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(12)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui);
        });
}

fn row(ui: &mut egui::Ui, title: &str, subtitle: &str, suffix: &str, clickable: bool) -> bool {
    if !clickable {
        ui.label(title);
        ui.label(RichText::new(subtitle).small().weak());
        return false;
    }
    let width = ui.available_width();
    let text_width = (width - 46.).max(80.);
    let title_text = ui.painter().layout(
        title.into(),
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().text_color(),
        text_width,
    );
    let subtitle_text = ui.painter().layout(
        subtitle.into(),
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().weak_text_color(),
        text_width,
    );
    let height = (title_text.size().y + subtitle_text.size().y + 20.).max(60.);
    // Use a real button for focus, keyboard activation, and accessibility actions.
    let response = ui
        .add_sized([width, height], egui::Button::new("").frame(false))
        .accessible_name(format!("{title}. {subtitle}"));
    let text_origin = response.rect.left_top() + Vec2::new(4., 8.);
    let subtitle_origin = text_origin + Vec2::new(0., title_text.size().y + 4.);
    ui.painter()
        .galley(text_origin, title_text, ui.visuals().text_color());
    ui.painter().galley(
        subtitle_origin,
        subtitle_text,
        ui.visuals().weak_text_color(),
    );
    crate::designer_ui::paint_tool_icon(
        ui.painter(),
        egui::pos2(response.rect.right() - 12., response.rect.center().y),
        ui.visuals().text_color(),
        if suffix == "↗" {
            "Export document"
        } else {
            "Next"
        },
    );
    if response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            6.,
            egui::Stroke::new(
                2.,
                if ui.visuals().dark_mode {
                    Color32::from_rgb(140, 185, 255)
                } else {
                    ui.visuals().selection.bg_fill
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_rows_are_keyboard_activatable_and_expose_accessible_names() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let frame = |events: Vec<egui::Event>| {
            let mut clicked = false;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        clicked = row(ui, "MIT License", "Read the license", "›", true);
                    });
                },
            );
            output.textures_delta.clear();
            (clicked, output)
        };
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let (_, output) = frame(vec![]);
        assert!(
            output
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node
                    .label()
                    .is_some_and(|label| label == "MIT License. Read the license"))
        );
        frame(vec![key(egui::Key::Tab)]);
        assert!(ctx.memory(|memory| memory.focused().is_some()));
        assert!(frame(vec![key(egui::Key::Enter)]).0);
        assert!(frame(vec![key(egui::Key::Space)]).0);
    }
}
