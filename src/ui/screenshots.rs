//! Deterministic, opt-in documentation captures using only synthetic demo data.
use super::*;
use crate::design::{Element, LabelDocument, ShapeKind, TextAlign};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const SCREENS: &[&str] = &[
    "home",
    "designer",
    "import",
    "print-center",
    "printing-terms",
    "about",
    "rotation",
    "save-changes",
];

/// Extra screens for the user manual. They are produced only when named in
/// `THERMALPRINT_SCREENSHOTS_ONLY` (comma separated), so the README set stays small.
const MANUAL_SCREENS: &[&str] = &[
    "designer-text",
    "import-watch",
    "print-center-full",
    "batch",
    "barcode-types",
    "alignment-test",
];

/// The screens to capture: the README set, or the named ones.
fn scenes() -> Vec<&'static str> {
    let Ok(only) = std::env::var("THERMALPRINT_SCREENSHOTS_ONLY") else {
        return SCREENS.to_vec();
    };
    let wanted: Vec<&str> = only.split(',').map(str::trim).collect();
    // "save-changes" leaves a prompt open, so it must come last.
    let mut names: Vec<&'static str> = SCREENS
        .iter()
        .chain(MANUAL_SCREENS)
        .copied()
        .filter(|name| *name != "save-changes" && wanted.contains(name))
        .collect();
    if wanted.contains(&"save-changes") {
        names.push("save-changes");
    }
    names
}

pub(super) struct Capture {
    directory: PathBuf,
    style: String,
    scenes: Vec<&'static str>,
    index: usize,
    prepared: bool,
    requested: bool,
    saved: Arc<AtomicBool>,
    since: Instant,
}

impl Capture {
    pub(super) fn new(directory: PathBuf, style: String) -> Self {
        Self {
            directory,
            style,
            scenes: scenes(),
            index: 0,
            prepared: false,
            requested: false,
            saved: Arc::new(AtomicBool::new(false)),
            since: Instant::now(),
        }
    }

    pub(super) fn tick(&mut self, app: &mut ThermalPrint, ctx: &egui::Context) {
        ctx.request_repaint_after(Duration::from_millis(80));
        if self.saved.swap(false, Ordering::SeqCst) {
            self.index += 1;
            self.prepared = false;
            self.requested = false;
        }
        if self.index >= self.scenes.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let scene = self.scenes[self.index];
        // Reference images are drawn directly, not screenshotted.
        if matches!(scene, "barcode-types" | "alignment-test") {
            if self.style == "light" {
                save_reference(scene, &self.directory);
            }
            self.index += 1;
            return;
        }
        if !self.prepared {
            app.area = match scene {
                "designer" | "rotation" | "save-changes" | "designer-text" | "batch" => {
                    Area::Designer
                }
                "import" | "import-watch" => Area::Import,
                "print-center" | "print-center-full" => Area::PrintCenter,
                _ => Area::Home,
            };
            app.terms_open = scene == "printing-terms";
            app.about_open = scene == "about";
            match scene {
                "rotation" => {
                    if let Some(id) = app
                        .designer
                        .doc
                        .objects
                        .iter()
                        .find(|object| matches!(object.element, Element::Qr { .. }))
                        .map(|object| object.id)
                    {
                        app.designer.select_object(id);
                        app.designer.rotate_selected_to(23.5);
                    }
                }
                "save-changes" => app.designer.request_new(),
                "designer-text" => {
                    if let Some(id) = app
                        .designer
                        .doc
                        .objects
                        .iter()
                        .find(|object| matches!(&object.element, Element::Text { text, .. } if text.starts_with("Alex")))
                        .map(|object| object.id)
                    {
                        app.designer.select_object(id);
                    }
                }
                "import-watch" => {
                    app.settings.watch_folder = Some(PathBuf::from("/home/demo/Downloads"));
                    app.settings.watch_enabled = true;
                }
                "print-center-full" => app.history = demo_history(),
                "batch" => {
                    app.designer.doc = batch_demo_design();
                    app.designer.demo_batch(DEMO_CSV.as_bytes());
                }
                _ => {}
            }
            self.since = Instant::now();
            self.prepared = true;
        }
        if !self.requested && self.since.elapsed() > Duration::from_millis(1200) && !app.busy {
            std::fs::create_dir_all(&self.directory).expect("create screenshot output directory");
            let path = self.directory.join(format!("{}-{}.png", scene, self.style));
            let saved = self.saved.clone();
            let ctx = ctx.clone();
            ctx.clone().request_screenshot(move |image| {
                let pixels: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                image::save_buffer(
                    &path,
                    &pixels,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .expect("save documentation screenshot");
                println!("Saved {}", path.display());
                saved.store(true, Ordering::SeqCst);
                ctx.request_repaint();
            });
            self.requested = true;
        }
    }
}

pub(super) fn seed(app: &mut ThermalPrint, ctx: &egui::Context) {
    app.settings.printer = "Demo thermal printer".into();
    app.settings.media = "4x6".into();
    app.printers = vec![Printer {
        name: app.settings.printer.clone(),
        is_default: true,
        description: "Demo thermal printer · example queue".into(),
        media: vec!["4x6".into(), "4x4".into()],
        media_labels: [
            ("4x6".into(), "4 × 6 in".into()),
            ("4x4".into(), "4 × 4 in".into()),
        ]
        .into(),
        media_sizes: [
            ("4x6".into(), (101.6, 152.4)),
            ("4x4".into(), (101.6, 101.6)),
        ]
        .into(),
        media_margins: Default::default(),
        resolutions: vec!["203dpi".into(), "300dpi".into()],
        resolution_option: "Resolution".into(),
        custom_media: true,
    }];
    let mut doc = LabelDocument::blank(101.6, 152.4);
    doc.title = "Northline · shipping label".into();
    let mut text = |content: &str, font_pt, bold, x, y, w, h| {
        doc.add(
            Element::Text {
                text: content.into(),
                font_pt,
                bold,
                align: TextAlign::Left,
            },
            x,
            y,
            w,
            h,
        );
    };
    text("NORTHLINE STUDIO", 17., true, 7., 7., 88., 10.);
    text(
        "SMALL BATCH GOODS / ORDER TP-1048",
        8.,
        false,
        7.,
        18.,
        88.,
        6.,
    );
    text(
        "FROM\nNorthline Studio\n42 Demo Lane\nExample City, CA 00000",
        10.,
        false,
        7.,
        31.,
        57.,
        28.,
    );
    text("SHIP TO", 9., true, 7., 69., 80., 7.);
    text(
        "Alex Morgan\n123 Example Street\nSample City, CA 00000",
        15.,
        true,
        7.,
        79.,
        88.,
        29.,
    );
    text("DEMO LABEL / NOT POSTAGE", 8., false, 7., 140., 88., 6.);
    doc.add(
        Element::Shape {
            kind: ShapeKind::DividerHorizontal,
            stroke_mm: 0.4,
            filled: false,
        },
        7.,
        63.,
        88.,
        1.,
    );
    doc.add(
        Element::Barcode {
            kind: crate::design::BarcodeKind::Code128,
            data: "TP-1048-DEMO".into(),
            show_text: true,
        },
        7.,
        115.,
        88.,
        20.,
    );
    doc.add(
        Element::Qr {
            data: "https://example.com/order/TP-1048".into(),
        },
        70.,
        31.,
        24.,
        24.,
    );
    let output = crate::design_render::render_label(&doc, 203).expect("render demo label");
    let barcode_id = doc
        .objects
        .iter()
        .find(|object| matches!(object.element, Element::Barcode { .. }))
        .map(|object| object.id);
    app.designer.doc = doc;
    if let Some(id) = barcode_id {
        app.designer.select_object(id);
    }
    app.designer.grid = true;
    app.designer.status = "Sample design · editable text, barcode, and QR code".into();
    let mut page_image = image::RgbImage::from_pixel(1678, 2373, image::Rgb([255; 3]));
    image::imageops::replace(&mut page_image, &output.image, 100, 120);
    app.crop = Crop {
        x: 100. / 1678.,
        y: 120. / 2373.,
        w: output.image.width() as f32 / 1678.,
        h: output.image.height() as f32 / 2373.,
    };
    let page = Arc::new(Page {
        image: page_image,
        width_mm: 210.,
        height_mm: 297.,
        index: 0,
        count: 1,
        path: PathBuf::from("demo-shipping-label.pdf"),
    });
    app.detection = Some(detection::detect(&page.image));
    app.original_texture = Some(ThermalPrint::texture(ctx, "demo-original", &page.image));
    app.output_texture = Some(ThermalPrint::texture(ctx, "demo-output", &output.image));
    app.page = Some(page);
    app.output = Some(Arc::new(output));
    app.reviewed = true;
    app.settings.mode = Mode::Manual;
    app.selected = Some(0);
    app.queue = (1..=3)
        .map(|i| QueueItem {
            path: PathBuf::from(format!("demo-order-{}.pdf", 1047 + i)),
            page: 0,
            crop: app.crop,
            reviewed: true,
            mode: Mode::Manual,
        })
        .collect();
    app.status = "Demo workspace · All sample labels are fictional".into();
}

const DEMO_CSV: &str = "Name,City,SKU\nAda Lovelace,London,SKU-1001\nGrace Hopper,New York,SKU-1002\nAlan Turing,Manchester,SKU-1003\n";

fn batch_demo_design() -> LabelDocument {
    let mut doc = LabelDocument::blank(101.6, 50.8);
    doc.title = "Customer label".into();
    let text = |doc: &mut LabelDocument, content: &str, pt, bold, y, h| {
        doc.add(
            Element::Text {
                text: content.into(),
                font_pt: pt,
                bold,
                align: TextAlign::Left,
            },
            5.,
            y,
            90.,
            h,
        );
    };
    text(&mut doc, "Ship to: ${Name}", 16., true, 5., 10.);
    text(&mut doc, "${City}   #${serial}", 12., false, 17., 8.);
    doc.add(
        Element::Barcode {
            kind: crate::design::BarcodeKind::Code128,
            data: "${SKU}".into(),
            show_text: true,
        },
        5.,
        28.,
        60.,
        16.,
    );
    doc.add(
        Element::Qr {
            data: "https://example.com/${SKU}".into(),
        },
        72.,
        26.,
        20.,
        20.,
    );
    doc
}

fn demo_history() -> Vec<crate::history::Entry> {
    use crate::history::{Entry, Kind};
    let entry = |time: &str, title: &str, copies, kind| Entry {
        time: time.into(),
        printer: "Demo thermal printer".into(),
        title: title.into(),
        copies,
        kind,
        ..Default::default()
    };
    vec![
        entry(
            "2026-11-07 09:14",
            "Customer label · batch of 3",
            1,
            Kind::Design,
        ),
        entry("2026-11-07 09:02", "demo-order-1048.pdf", 1, Kind::Import),
        entry("2026-11-06 16:40", "Shelf tag", 2, Kind::Design),
    ]
}

/// Images that are drawn straight from the renderer: a sheet of every barcode type, and
/// the alignment test label.
fn save_reference(scene: &str, directory: &std::path::Path) {
    std::fs::create_dir_all(directory).expect("create output directory");
    let image = match scene {
        "alignment-test" => crate::calibration::alignment_test(101.6, 152.4, 203).image,
        _ => {
            crate::design_render::render_label(&barcode_sheet(), 300)
                .expect("render barcode sheet")
                .image
        }
    };
    let path = directory.join(format!("{scene}.png"));
    image.save(&path).expect("save reference image");
    println!("Saved {}", path.display());
}

fn barcode_sheet() -> LabelDocument {
    use crate::design::BarcodeKind::{
        Codabar, Code39, Code93, Code128, DataMatrix, Ean8, Ean13, Itf, UpcA,
    };
    let rows = [
        (Code128, "Code 128", "SKU-48213"),
        (Code39, "Code 39", "THERMAL"),
        (Code93, "Code 93", "TP-93"),
        (Ean13, "EAN-13", "5901234123457"),
        (Ean8, "EAN-8", "55123457"),
        (UpcA, "UPC-A", "036000291452"),
        (Itf, "ITF (Interleaved 2 of 5)", "12345670"),
        (Codabar, "Codabar", "A40156B"),
        (DataMatrix, "Data Matrix", "TP-1048-DEMO"),
    ];
    let mut doc = LabelDocument::blank(100., 150.);
    doc.title = "Barcode types".into();
    for (index, (kind, name, data)) in rows.into_iter().enumerate() {
        let y = 4. + index as f32 * 15.5;
        doc.add(
            Element::Text {
                text: name.into(),
                font_pt: 8.,
                bold: true,
                align: TextAlign::Left,
            },
            4.,
            y,
            60.,
            5.,
        );
        let (x, width, height) = if kind == DataMatrix {
            (4., 12., 12.)
        } else {
            (4., 70., 9.)
        };
        doc.add(
            Element::Barcode {
                kind,
                data: data.into(),
                show_text: false,
            },
            x,
            y + 5.,
            width,
            height,
        );
    }
    doc
}
