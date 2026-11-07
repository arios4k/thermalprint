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

pub(super) struct Capture {
    directory: PathBuf,
    style: String,
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
        if self.index == SCREENS.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if !self.prepared {
            app.area = match self.index {
                1 | 6 | 7 => Area::Designer,
                2 => Area::Import,
                3 => Area::PrintCenter,
                _ => Area::Home,
            };
            app.terms_open = self.index == 4;
            app.about_open = self.index == 5;
            if self.index == 6
                && let Some(id) = app
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
            if self.index == 7 {
                app.designer.request_new();
            }
            self.since = Instant::now();
            self.prepared = true;
        }
        if !self.requested && self.since.elapsed() > Duration::from_millis(1200) && !app.busy {
            std::fs::create_dir_all(&self.directory).expect("create screenshot output directory");
            let path = self
                .directory
                .join(format!("{}-{}.png", SCREENS[self.index], self.style));
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
