use image::{Rgb, RgbImage};
use std::path::PathBuf;
use thermalprint::{
    design::{Element, LabelDocument, ShapeKind},
    design_render,
    detection::Crop,
    document::Page,
    export,
    footer::{Footer, FooterAlign},
    layout,
    settings::{Scale, Settings},
};
fn page() -> Page {
    let mut image = RgbImage::from_pixel(600, 900, Rgb([255; 3]));
    for y in 50..850 {
        for x in 50..550 {
            if (x / 3) % 2 == 0 {
                image.put_pixel(x, y, Rgb([0; 3]));
            }
        }
    }
    Page {
        image,
        width_mm: 50.8,
        height_mm: 76.2,
        index: 0,
        count: 1,
        path: PathBuf::from("sample.png"),
    }
}
#[test]
fn output_dimensions_and_pdf_media_box() {
    let s = Settings::default();
    let output = layout::render(&page(), Crop::FULL, &s).unwrap();
    assert_eq!(output.image.dimensions(), (812, 1218));
    assert!(!output.clipped);
    let pdf = export::pdf_bytes(&output).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    let dims = text
        .split("/MediaBox [0 0 ")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    assert!((dims[0] - 288.0).abs() < 0.01);
    assert!((dims[1] - 432.0).abs() < 0.01);
}
#[cfg(windows)]
#[test]
fn pdf_import_and_background_reloads_preserve_label_size() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Étiquette 4 × 6.pdf");
    let output = layout::render(&page(), Crop::FULL, &Settings::default()).unwrap();
    std::fs::write(&path, export::pdf_bytes(&output).unwrap()).unwrap();
    let label = thermalprint::document::load(&path, 0).unwrap();
    assert!((label.width_mm - 101.6).abs() < 0.01);
    assert!((label.height_mm - 152.4).abs() < 0.01);
    assert_eq!(label.count, 1);
    assert!(label.image.pixels().any(|pixel| pixel[0] < 64));

    // Automatic printing reopens the PDF on a worker after the preview loads it.
    std::thread::scope(|scope| {
        let first = scope.spawn(|| thermalprint::document::load(&path, 0).unwrap());
        let second = scope.spawn(|| thermalprint::document::load(&path, 0).unwrap());
        for worker in [first, second] {
            let reloaded = worker.join().unwrap();
            assert_eq!(reloaded.width_mm, label.width_mm);
            assert_eq!(reloaded.height_mm, label.height_mm);
            assert_eq!(reloaded.image, label.image);
        }
    });
}

#[test]
fn actual_size_clipping_is_blocked() {
    let mut p = page();
    p.width_mm = 203.2;
    p.height_mm = 304.8;
    let s = Settings {
        scale: Scale::Actual,
        ..Settings::default()
    };
    let output = layout::render(&p, Crop::FULL, &s).unwrap();
    assert!(output.clipped);
    assert!(export::pdf_bytes(&output).is_err());
}

fn with_footer(qr: bool, align: FooterAlign, base: &Settings) -> Settings {
    Settings {
        footer: Footer {
            message_enabled: true,
            qr_enabled: qr,
            align,
            ..Footer::default()
        },
        ..base.clone()
    }
}
/// The page with its bottom rows cleared, as on a label with white space under its last line.
fn with_blank_bottom(rows: u32) -> Page {
    let mut p = page();
    for y in 900 - rows..900 {
        for x in 0..600 {
            p.image.put_pixel(x, y, Rgb([255; 3]));
        }
    }
    p
}
#[test]
fn imported_labels_are_identical_with_the_designer_footer_on_or_off() {
    // Lots of blank space at the bottom, where a footer could have gone, and none at all;
    // fit and actual size; with and without a margin.
    for label in [with_blank_bottom(250), page()] {
        for (scale, margin_mm) in [(Scale::Fit, 2.), (Scale::Actual, 0.)] {
            let plain = Settings {
                scale,
                margin_mm,
                ..Settings::default()
            };
            let before = layout::render(&label, Crop::FULL, &plain).unwrap();
            // Every alignment, with and without the QR code.
            for (qr, align) in [
                (false, FooterAlign::Left),
                (true, FooterAlign::Center),
                (true, FooterAlign::Right),
            ] {
                let s = with_footer(qr, align, &plain);
                let after = layout::render(&label, Crop::FULL, &s).unwrap();
                assert!(after.image == before.image, "{scale:?} {qr} {align:?}");
                assert_eq!(after.warnings, before.warnings);
                assert_eq!(after.clipped, before.clipped);
                assert_eq!(
                    (after.width_mm, after.height_mm, after.dpi),
                    (before.width_mm, before.height_mm, before.dpi)
                );
                if !(qr && align == FooterAlign::Center) {
                    continue;
                }
                // The file sent to the printer is the same too.
                let dir = tempfile::tempdir().unwrap();
                let (a, b) = (dir.path().join("a.png"), dir.path().join("b.png"));
                export::save(&before, &a).unwrap();
                export::save(&after, &b).unwrap();
                assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
            }
        }
    }
}
fn footer_design() -> LabelDocument {
    let mut doc = LabelDocument::blank(101.6, 152.4);
    doc.add(
        Element::Qr {
            data: "SKU-001".into(),
        },
        10.,
        10.,
        30.,
        30.,
    );
    doc
}
#[test]
fn the_designer_footer_keeps_the_label_size_at_every_alignment() {
    let plain = design_render::render_for_print(&footer_design(), 203).unwrap();
    let mut spans = vec![];
    for align in FooterAlign::ALL {
        let footer = Footer {
            message_enabled: true,
            qr_enabled: true,
            align,
            ..Footer::default()
        };
        let with = design_render::render_with_footer(&footer_design(), 203, &footer, 2.).unwrap();
        assert_eq!(
            with.image.dimensions(),
            plain.image.dimensions(),
            "{align:?}"
        );
        assert_eq!(
            (with.width_mm, with.height_mm),
            (plain.width_mm, plain.height_mm)
        );
        assert!(with.warnings.is_empty(), "{align:?}: {:?}", with.warnings);
        // Only the bottom strip changed.
        let (w, h) = with.image.dimensions();
        let changed: Vec<(u32, u32)> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| with.image.get_pixel(x, y) != plain.image.get_pixel(x, y))
            .collect();
        assert!(!changed.is_empty());
        assert!(changed.iter().all(|&(_, y)| y > h * 9 / 10), "{align:?}");
        let left = changed.iter().map(|p| p.0).min().unwrap();
        let right = changed.iter().map(|p| p.0).max().unwrap();
        // Inside the printable area: at least 2 mm from the sides.
        assert!(left >= 16 && right < w - 16, "{align:?} {left}..{right}");
        spans.push((left, right));
    }
    let w = plain.image.width();
    assert!(spans[0].1 < w / 2, "left {spans:?}");
    assert!(
        ((spans[1].0 + spans[1].1) / 2).abs_diff(w / 2) < 12,
        "center {spans:?}"
    );
    assert!(spans[2].0 > w / 2, "right {spans:?}");
}
#[test]
fn an_overlap_keeps_the_preview_but_blocks_printing() {
    let mut doc = footer_design();
    let shape = doc.add(
        Element::Shape {
            kind: ShapeKind::Rectangle,
            stroke_mm: 1.,
            filled: false,
        },
        30.,
        30.,
        30.,
        30.,
    );
    // The preview still renders, naming the code that overlaps.
    let (preview, overlaps) = design_render::render_preview(&doc, 203).unwrap();
    let code = doc.objects[0].id;
    assert_eq!(overlaps, vec![code]);
    assert_eq!(preview.image.dimensions(), (812, 1218));
    assert!(design_render::overlap_warning(&doc, &overlaps).is_some());
    // Every print path refuses it.
    let footer = Footer::default();
    assert!(design_render::render_label(&doc, 203).is_err());
    assert!(design_render::render_for_print(&doc, 203).is_err());
    assert!(design_render::render_with_footer(&doc, 203, &footer, 0.).is_err());
    assert!(thermalprint::zpl::encode_design_for_print(&doc, 203, &footer, 0.).is_err());
    // Moving the shape away clears it.
    doc.objects.iter_mut().find(|o| o.id == shape).unwrap().x_mm = 60.;
    doc.objects.iter_mut().find(|o| o.id == shape).unwrap().y_mm = 60.;
    let (_, overlaps) = design_render::render_preview(&doc, 203).unwrap();
    assert!(overlaps.is_empty());
    assert!(design_render::render_with_footer(&doc, 203, &footer, 0.).is_ok());
}
