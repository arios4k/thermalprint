//! Compact branded About content, matching the workspace palette.
use eframe::egui::{self, Color32, RichText, Vec2};

/// Icons for the author's other apps, shown in the "More from the author" card.
pub(super) struct AppIcons {
    pub voltkit: egui::TextureHandle,
    pub battery_health_center: egui::TextureHandle,
}

impl AppIcons {
    pub(super) fn load(ctx: &egui::Context) -> Self {
        // A bold family for the About title; the default UI font has no heavy weight.
        ctx.add_font(egui::epaint::text::FontInsert::new(
            "about-bold",
            egui::FontData::from_static(include_bytes!("../../assets/fonts/DejaVuSans-Bold.ttf")),
            vec![egui::epaint::text::InsertFontFamily {
                family: bold_family(),
                priority: egui::epaint::text::FontPriority::Highest,
            }],
        ));
        let texture = |name: &str, bytes: &[u8]| {
            let image = image::load_from_memory(bytes)
                .expect("bundled app icon must decode")
                .to_rgba8();
            ctx.load_texture(
                name,
                egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            )
        };
        Self {
            voltkit: texture(
                "voltkit-icon",
                include_bytes!("../../assets/apps/voltkit.png"),
            ),
            battery_health_center: texture(
                "battery-health-center-icon",
                include_bytes!("../../assets/apps/battery-health-center.png"),
            ),
        }
    }
}

fn bold_family() -> egui::FontFamily {
    egui::FontFamily::Name("about-bold".into())
}

pub(super) fn show(
    ctx: &egui::Context,
    icon: &egui::TextureHandle,
    apps: &AppIcons,
) -> (bool, bool, egui::Rect) {
    let compact = ctx.content_rect().height() < 720.;
    let mut license = false;
    let response = egui::Modal::new(egui::Id::unique("thermalprint-about"))
        .frame(
            egui::Frame::window(&ctx.global_style())
                .inner_margin(16)
                .corner_radius(16),
        )
        .show(ctx, |ui| {
            ui.set_width(
                (ctx.content_rect().width() - 64.).clamp(260., if compact { 620. } else { 500. }),
            );
            ui.spacing_mut().item_spacing.y = 6.;
            ui.horizontal(|ui| {
                ui.label(RichText::new("About ThermalPrint").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        ui.close();
                    }
                });
            });
            ui.add_space(4.);
            license = content(ui, icon, apps, compact);
        });
    (response.should_close(), license, response.response.rect)
}

/// The accent color at a given opacity (unmultiplied, so it stays blue instead of darkening).
fn tinted(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.) as u8)
}

/// Brand blue for the About tints; the theme's selection color is translucent, so its
/// channels cannot be reused as a tint base.
const ACCENT: Color32 = Color32::from_rgb(53, 132, 228);

fn accent_text(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(140, 185, 255)
    } else {
        Color32::from_rgb(28, 90, 190)
    }
}

fn hero_details(ui: &mut egui::Ui) {
    let dark = ui.visuals().dark_mode;
    ui.label(
        RichText::new("ThermalPrint")
            .font(egui::FontId::new(32., bold_family()))
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(RichText::new("Design and prepare thermal labels for Linux.").weak());
    let copied = ui
        .data_mut(|data| data.get_temp::<f64>(egui::Id::unique("about-version-copied")))
        .is_some_and(|time| ui.input(|input| input.time) - time < 2.);
    let label = if copied {
        "Version copied".to_owned()
    } else {
        format!("Version {}", env!("CARGO_PKG_VERSION"))
    };
    // A flat, tinted pill with a copy icon, in the accent color.
    let color = accent_text(dark);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.clone(), font, color);
    let size = Vec2::new(galley.size().x + 56., 30.);
    let response = ui
        .allocate_response(size, egui::Sense::click())
        .on_hover_text("Copy version information")
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            true,
            "Copy ThermalPrint version information",
        )
    });
    let tint = tinted(ACCENT, if dark { 0.30 } else { 0.14 });
    let tint = if response.hovered() {
        tinted(ACCENT, if dark { 0.45 } else { 0.22 })
    } else {
        tint
    };
    ui.painter().rect_filled(response.rect, 15., tint);
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            15.,
            egui::Stroke::new(2., color),
            egui::StrokeKind::Outside,
        );
    }
    crate::designer_ui::paint_tool_icon(
        ui.painter(),
        egui::pos2(response.rect.left() + 22., response.rect.center().y),
        color,
        "Duplicate",
    );
    ui.painter().galley(
        egui::pos2(
            response.rect.left() + 38.,
            response.rect.center().y - galley.size().y / 2.,
        ),
        galley,
        color,
    );
    if response.clicked() {
        ui.ctx()
            .copy_text(concat!("ThermalPrint ", env!("CARGO_PKG_VERSION")).into());
        let time = ui.input(|input| input.time);
        ui.data_mut(|data| data.insert_temp(egui::Id::unique("about-version-copied"), time));
    }
    if copied {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(2));
    }
}

/// A rounded card shape whose tint fades from top to bottom.
fn gradient_card(
    rect: egui::Rect,
    radius: f32,
    base: Color32,
    top: f32,
    bottom: f32,
) -> egui::Shape {
    let strips = rect.height().ceil() as usize;
    let mut shapes = Vec::with_capacity(strips);
    for i in 0..strips {
        let y = i as f32;
        let t = (y + 0.5) / rect.height();
        let inset = |distance: f32| {
            if distance >= radius {
                0.
            } else {
                radius
                    - (radius * radius - (radius - distance).powi(2))
                        .max(0.)
                        .sqrt()
            }
        };
        let edge = inset(y + 0.5).max(inset(rect.height() - y - 0.5));
        let color = tinted(base, top + (bottom - top) * t);
        shapes.push(egui::Shape::rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.left() + edge, rect.top() + y),
                egui::pos2(
                    rect.right() - edge,
                    (rect.top() + y + 1.).min(rect.bottom()),
                ),
            ),
            0.,
            color,
        ));
    }
    egui::Shape::Vec(shapes)
}

fn content(ui: &mut egui::Ui, icon: &egui::TextureHandle, apps: &AppIcons, compact: bool) -> bool {
    let dark = ui.visuals().dark_mode;
    let backdrop = ui.painter().add(egui::Shape::Noop);
    let hero = egui::Frame::new()
        .corner_radius(14)
        .inner_margin(14)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if compact {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(icon).fit_to_exact_size(Vec2::splat(64.)));
                    ui.add_space(10.);
                    ui.vertical(hero_details);
                });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add(egui::Image::new(icon).fit_to_exact_size(Vec2::splat(56.)));
                    hero_details(ui);
                });
            }
        });
    let (top, bottom) = if dark { (0.30, 0.04) } else { (0.16, 0.02) };
    let backdrop_shape = gradient_card(hero.response.rect, 14., ACCENT, top, bottom);
    ui.painter().set(backdrop, backdrop_shape);
    tags(ui, dark);
    let mut license = false;
    if compact {
        ui.columns(2, |columns| {
            project(&mut columns[0]);
            license = credits(&mut columns[1]);
        });
    } else {
        project(ui);
        license = credits(ui);
    }
    more_apps(ui, apps);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(concat!("© 2026 ", env!("CARGO_PKG_AUTHORS")))
                .small()
                .weak(),
        );
    });
    license
}

/// Equal-width tag chips with icons under the version, centered when they fit one row.
fn tags(ui: &mut egui::Ui, dark: bool) {
    const TAGS: [(&str, &str); 3] = [
        ("Print device", "Thermal labels"),
        ("System", "Linux app"),
        ("External link", "Open source"),
    ];
    const ICON: f32 = 24.;
    const PADDING: f32 = 12.;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text_width = TAGS
        .iter()
        .map(|(_, text)| {
            ui.painter()
                .layout_no_wrap((*text).into(), font.clone(), Color32::WHITE)
                .size()
                .x
        })
        .fold(0., f32::max);
    let chip = Vec2::new(ICON + 4. + text_width + PADDING * 2., 36.);
    let spacing = ui.spacing().item_spacing.x;
    let total = chip.x * TAGS.len() as f32 + spacing * (TAGS.len() - 1) as f32;
    let lead = ((ui.available_width() - total) / 2.).max(0.);
    ui.horizontal_wrapped(|ui| {
        // Narrow windows wrap to several rows, where the lead would unbalance them.
        if lead > 0. {
            ui.add_space(lead);
        }
        for (icon, text) in TAGS {
            let (rect, response) = ui.allocate_exact_size(chip, egui::Sense::hover());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, text));
            ui.painter().rect_filled(
                rect,
                8.,
                if dark {
                    Color32::from_gray(48)
                } else {
                    Color32::from_rgb(232, 235, 240)
                },
            );
            let galley =
                ui.painter()
                    .layout_no_wrap(text.into(), font.clone(), ui.visuals().text_color());
            let content = ICON + 4. + galley.size().x;
            let left = rect.center().x - content / 2.;
            crate::designer_ui::paint_tool_icon(
                ui.painter(),
                egui::pos2(left + ICON / 2., rect.center().y),
                ui.visuals().text_color(),
                icon,
            );
            ui.painter().galley(
                egui::pos2(left + ICON + 4., rect.center().y - galley.size().y / 2.),
                galley,
                ui.visuals().text_color(),
            );
        }
    });
}

fn project(ui: &mut egui::Ui) {
    ui.heading(RichText::new("Explore ThermalPrint").size(16.).strong());
    card(ui, |ui| {
        if row(
            ui,
            "Project & source code",
            "Documentation, releases, and source",
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
}

fn more_apps(ui: &mut egui::Ui, apps: &AppIcons) {
    ui.heading(RichText::new("More from the author").size(16.).strong());
    card(ui, |ui| {
        ui.columns(2, |columns| {
            if app_link(
                &mut columns[0],
                &apps.voltkit,
                "Voltkit",
                "Power tools for your desktop",
            ) {
                columns[0]
                    .ctx()
                    .open_url(egui::OpenUrl::new_tab("https://github.com/arios4k/voltkit"));
            }
            if app_link(
                &mut columns[1],
                &apps.battery_health_center,
                "Battery Health Center",
                "Battery health for GNOME",
            ) {
                columns[1].ctx().open_url(egui::OpenUrl::new_tab(
                    "https://github.com/arios4k/battery-health-center",
                ));
            }
        });
    });
}

/// An icon with a title and subtitle that opens the app's project page.
fn app_link(ui: &mut egui::Ui, icon: &egui::TextureHandle, title: &str, subtitle: &str) -> bool {
    const ICON: f32 = 36.;
    let width = ui.available_width();
    let text_width = (width - ICON - 34.).max(60.);
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
    let height = (title_text.size().y + subtitle_text.size().y + 16.).max(ICON + 16.);
    // A real button keeps focus, keyboard activation, and accessibility actions.
    let response = ui
        .add_sized([width, height], egui::Button::new("").frame(false))
        .accessible_name(format!("{title}. {subtitle}. Opens in your browser"));
    let rect = response.rect;
    ui.painter().image(
        icon.id(),
        egui::Rect::from_min_size(
            egui::pos2(rect.left() + 4., rect.center().y - ICON / 2.),
            Vec2::splat(ICON),
        ),
        egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(1., 1.)),
        Color32::WHITE,
    );
    let text_x = rect.left() + ICON + 12.;
    let text_height = title_text.size().y + subtitle_text.size().y + 2.;
    let text_y = rect.center().y - text_height / 2.;
    let subtitle_y = text_y + title_text.size().y + 2.;
    ui.painter().galley(
        egui::pos2(text_x, text_y),
        title_text,
        ui.visuals().text_color(),
    );
    ui.painter().galley(
        egui::pos2(text_x, subtitle_y),
        subtitle_text,
        ui.visuals().weak_text_color(),
    );
    crate::designer_ui::paint_tool_icon(
        ui.painter(),
        egui::pos2(rect.right() - 12., rect.center().y),
        ui.visuals().text_color(),
        "Export document",
    );
    if response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            rect,
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

fn credits(ui: &mut egui::Ui) -> bool {
    ui.heading(RichText::new("Credits and license").size(16.).strong());
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
    license
}

fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(12)
        .inner_margin(10)
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
    let height = (title_text.size().y + subtitle_text.size().y + 16.).max(44.);
    // Use a real button for focus, keyboard activation, and accessibility actions.
    let response = ui
        .add_sized([width, height], egui::Button::new("").frame(false))
        .accessible_name(format!("{title}. {subtitle}"));
    let text_origin = response.rect.left_top() + Vec2::new(4., 6.);
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
    fn about_fits_supported_window_sizes_without_scrolling() {
        for size in [
            Vec2::new(760., 560.),
            Vec2::new(1024., 768.),
            Vec2::new(1240., 840.),
        ] {
            let ctx = egui::Context::default();
            ctx.all_styles_mut(|style| {
                style.spacing.item_spacing = Vec2::new(10., 8.);
                style.spacing.button_padding = Vec2::new(12., 6.);
                style.spacing.interact_size.y = 32.;
                for (kind, size) in [
                    (egui::TextStyle::Body, 14.),
                    (egui::TextStyle::Button, 14.),
                    (egui::TextStyle::Small, 12.),
                ] {
                    style
                        .text_styles
                        .insert(kind, egui::FontId::proportional(size));
                }
            });
            let icon = ctx.load_texture(
                "about-test-icon",
                egui::ColorImage::from_rgba_unmultiplied([1, 1], &[255; 4]),
                Default::default(),
            );
            let apps = AppIcons::load(&ctx);
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let mut bounds = egui::Rect::NOTHING;
            for _ in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        ..Default::default()
                    },
                    |_| {
                        bounds = show(&ctx, &icon, &apps).2;
                    },
                );
                output.textures_delta.clear();
            }
            assert!(
                screen.contains_rect(bounds),
                "About {bounds:?} exceeds window {size:?}"
            );
        }
    }

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
