//! Short explanations shared by contextual help and the printing glossary.
use eframe::egui;

pub const TERMS: &[(&str, &str)] = &[
    (
        "CUPS",
        "Common UNIX Printing System: Linux’s printing service. It manages printers, drivers, and queued jobs. Accepted by CUPS means the job was submitted, not that paper has printed.",
    ),
    (
        "DPI",
        "Dots per inch: the resolution used to render or print a label. Match your printer’s supported resolution for sharp text and barcodes.",
    ),
    (
        "PDF",
        "Portable Document Format: a document format that preserves page size and layout, useful for sharing and printing labels.",
    ),
    (
        "PNG",
        "Portable Network Graphics: a lossless image format that preserves sharp text and barcode edges.",
    ),
    (
        "SVG",
        "Scalable Vector Graphics: an image format that keeps shapes and text sharp when resized.",
    ),
    (
        "ZPL",
        "Zebra Programming Language: printer commands for compatible label printers. Raw ZPL bypasses normal driver processing and requires a compatible printer and queue.",
    ),
    (
        "QR",
        "Quick Response code: a square barcode that can store a web address or text and be scanned with a phone or compatible scanner.",
    ),
    (
        "EAN",
        "European Article Number: a retail product barcode standard. EAN-13 uses 13 digits, including a check digit.",
    ),
    (
        "UPC",
        "Universal Product Code: a retail product barcode standard. UPC-A uses 12 digits, including a check digit.",
    ),
    (
        "SKU",
        "Stock keeping unit: your own identifier for a product or inventory item.",
    ),
    (
        "JPEG",
        "A compressed photo format. Compression can blur barcode edges; prefer PDF or PNG when available.",
    ),
    (
        "TIFF",
        "Tagged Image File Format: an image format often used for scanned documents.",
    ),
    (
        "BMP",
        "Bitmap: a raster image format that stores an image as pixels.",
    ),
    (
        "WebP",
        "An image format that supports compressed pictures. Lossy compression can soften fine barcode detail.",
    ),
];

pub fn explanation(term: &str) -> &'static str {
    TERMS
        .iter()
        .find(|(key, _)| *key == term)
        .map(|(_, text)| *text)
        .unwrap_or("")
}

pub fn tooltip(text: &str) -> String {
    let mut result = text.to_owned();
    for &(term, explanation) in TERMS {
        if text
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| word == term)
        {
            result.push_str("\n\n");
            result.push_str(explanation);
        }
    }
    result
}

/// Both hover and click expose the explanation, including for keyboard users.
pub fn hint(ui: &mut egui::Ui, term: &str) {
    let response = ui.small_button("?").on_hover_text(explanation(term));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, format!("About {term}"))
    });
    egui::Popup::menu(&response).show(|ui| {
        ui.set_max_width(300.);
        ui.label(egui::RichText::new(term).strong());
        ui.label(explanation(term));
    });
}

pub fn notice(ui: &mut egui::Ui, title: &str, message: &str, warning: bool) {
    let color = match (warning, ui.visuals().dark_mode) {
        (true, true) => egui::Color32::from_rgb(245, 193, 99),
        (true, false) => egui::Color32::from_rgb(132, 83, 10),
        (false, true) => egui::Color32::from_rgb(88, 199, 173),
        (false, false) => egui::Color32::from_rgb(28, 111, 100),
    };
    egui::Frame::group(ui.style())
        .corner_radius(8)
        .inner_margin(10)
        .stroke(egui::Stroke::new(1., color.gamma_multiply(0.5)))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(title).strong().color(color));
            ui.label(message);
        });
}

/// A dedicated, searchable reference with the same icon style as the workspace.
pub fn glossary(ui: &mut egui::Ui, search: &mut String) {
    let accent = if ui.visuals().dark_mode {
        egui::Color32::from_rgb(88, 199, 173)
    } else {
        egui::Color32::from_rgb(28, 111, 100)
    };
    egui::Frame::new().inner_margin(14).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (badge, _) = ui.allocate_exact_size(egui::Vec2::splat(48.), egui::Sense::hover());
            ui.painter()
                .rect_filled(badge, 14., accent.gamma_multiply(0.12));
            crate::designer_ui::paint_tool_icon(
                ui.painter(),
                badge.center(),
                accent,
                "New document",
            );
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("A little printing clarity")
                        .size(23.)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new("Plain-language explanations, all in one place.").weak(),
                );
            });
        });
        ui.add_space(14.);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(search)
                    .hint_text("Search terms or explanations…")
                    .desired_width((ui.available_width() - 70.).max(120.)),
            );
            if ui
                .button("Clear")
                .on_hover_text("Show all printing terms")
                .clicked()
            {
                search.clear();
            }
        });
        ui.add_space(12.);
        let query = search.trim().to_lowercase();
        let height = ui
            .available_height()
            .min((ui.ctx().content_rect().height() - 240.).max(160.))
            .max(160.);
        egui::ScrollArea::vertical()
            .id_salt("printing-terms-list")
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.;
                let mut found = false;
                for (group, terms) in [
                    ("Printers & quality", &["CUPS", "DPI", "ZPL"][..]),
                    (
                        "Documents & images",
                        &["PDF", "PNG", "SVG", "JPEG", "TIFF", "BMP", "WebP"][..],
                    ),
                    ("Barcodes & inventory", &["QR", "EAN", "UPC", "SKU"][..]),
                ] {
                    let matches: Vec<_> = terms
                        .iter()
                        .copied()
                        .filter(|term| {
                            query.is_empty()
                                || format!("{term} {}", explanation(term))
                                    .to_lowercase()
                                    .contains(&query)
                        })
                        .collect();
                    if matches.is_empty() {
                        continue;
                    }
                    found = true;
                    ui.add_space(4.);
                    ui.label(egui::RichText::new(group).strong().color(accent));
                    for term in matches {
                        let icon = match term {
                            "CUPS" => "Print device",
                            "DPI" => "Grid",
                            "PDF" => "New document",
                            "SVG" => "Design label",
                            "ZPL" => "Export document",
                            "QR" => "Add QR code",
                            "EAN" | "UPC" => "Add Code 128",
                            "SKU" => "Price tag",
                            _ => "Add image / logo...",
                        };
                        egui::Frame::group(ui.style())
                            .corner_radius(12)
                            .inner_margin(14)
                            .show(ui, |ui| {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 14.;
                                    let (badge, _) = ui.allocate_exact_size(
                                        egui::Vec2::splat(34.),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        badge,
                                        9.,
                                        accent.gamma_multiply(0.12),
                                    );
                                    crate::designer_ui::paint_tool_icon(
                                        ui.painter(),
                                        badge.center(),
                                        accent,
                                        icon,
                                    );
                                    let text_width = ui.available_width();
                                    ui.vertical(|ui| {
                                        ui.set_width(text_width);
                                        ui.spacing_mut().item_spacing.y = 6.;
                                        ui.label(egui::RichText::new(term).size(17.).strong());
                                        ui.add(egui::Label::new(explanation(term)).wrap());
                                    });
                                });
                            });
                    }
                }
                if !found {
                    ui.label(egui::RichText::new("No matching terms").strong());
                    ui.label("Try a word such as printer, image, or barcode, or clear the search.");
                }
            });
    });
}
