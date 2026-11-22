use image::{Rgb, RgbImage};
use std::path::PathBuf;
use thermalprint::{
    detection::Crop,
    document::Page,
    export,
    footer::Footer,
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

fn ink_rows(image: &RgbImage) -> Vec<u32> {
    (0..image.height())
        .filter(|&y| (0..image.width()).any(|x| image.get_pixel(x, y)[0] < 128))
        .collect()
}
#[test]
fn the_footer_changes_nothing_while_it_is_off() {
    let plain = layout::render(&page(), Crop::FULL, &Settings::default()).unwrap();
    // A QR code alone, or a message switched off, prints nothing and reserves nothing.
    let s = Settings {
        footer: Footer {
            qr_enabled: true,
            message: "Shop at example.com".into(),
            ..Footer::default()
        },
        ..Settings::default()
    };
    let off = layout::render(&page(), Crop::FULL, &s).unwrap();
    assert_eq!(off.image, plain.image);
    assert_eq!(off.warnings, plain.warnings);
}
fn with_footer(qr: bool, base: &Settings) -> Settings {
    Settings {
        footer: Footer {
            message_enabled: true,
            qr_enabled: qr,
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
/// Ink from the page edge to edge, as on a carrier label that fills its stock.
fn full_bleed() -> Page {
    let mut p = page();
    for y in 0..900 {
        for x in 0..600 {
            if (x / 3) % 2 == 0 {
                p.image.put_pixel(x, y, Rgb([0; 3]));
            }
        }
    }
    p
}
/// Rows `from..` painted white, to compare a label without its footer.
fn without_rows_from(image: &RgbImage, from: u32) -> RgbImage {
    let mut copy = image.clone();
    for y in from..copy.height() {
        for x in 0..copy.width() {
            copy.put_pixel(x, y, Rgb([255; 3]));
        }
    }
    copy
}
#[test]
fn the_footer_goes_in_blank_space_and_the_label_prints_exactly_as_before() {
    let label = with_blank_bottom(250);
    for qr in [false, true] {
        let plain = Settings::default();
        let before = layout::render(&label, Crop::FULL, &plain).unwrap();
        let after = layout::render(&label, Crop::FULL, &with_footer(qr, &plain)).unwrap();
        assert!(!after.clipped);
        assert!(!after.warnings.iter().any(|w| w.contains("footer")), "{qr}");
        // Pixel for pixel the same label above its last line: same size, same place.
        let last = *ink_rows(&before.image).last().unwrap();
        assert!(
            without_rows_from(&after.image, last + 1) == without_rows_from(&before.image, last + 1),
            "{qr}: label changed"
        );
        // The footer printed below, with white space between.
        let footer_rows: Vec<u32> = ink_rows(&after.image)
            .into_iter()
            .filter(|&y| y > last)
            .collect();
        assert!(!footer_rows.is_empty(), "{qr}");
        assert!(footer_rows[0] > last + 5, "{qr}");
    }
}
#[test]
fn labels_without_blank_space_are_never_shrunk_for_the_footer() {
    // Fit on 4 × 6 stock, and actual size on stock the label's own size.
    for (scale, width_mm, height_mm) in [(Scale::Fit, 101.6, 152.4), (Scale::Actual, 50.8, 76.2)] {
        let plain = Settings {
            scale,
            width_mm,
            height_mm,
            margin_mm: 0.,
            ..Settings::default()
        };
        let before = layout::render(&full_bleed(), Crop::FULL, &plain).unwrap();
        let after = layout::render(&full_bleed(), Crop::FULL, &with_footer(true, &plain)).unwrap();
        assert_eq!(after.clipped, before.clipped, "{scale:?}");
        assert!(after.image == before.image, "{scale:?} label changed");
        assert!(
            after
                .warnings
                .iter()
                .any(|w| w.contains("footer was left out")),
            "{scale:?}"
        );
    }
}
#[test]
fn a_label_with_room_above_moves_up_whole_to_make_space() {
    // A wide label on slightly taller stock: centered, the spare rows below it are too few
    // for the footer, but together with those above they are enough.
    let s = Settings {
        width_mm: 101.6,
        height_mm: 84.,
        ..Settings::default()
    };
    let wide = Page {
        image: {
            let mut image = RgbImage::from_pixel(900, 600, Rgb([255; 3]));
            for y in 0..600 {
                for x in 0..900 {
                    if (x / 3) % 2 == 0 {
                        image.put_pixel(x, y, Rgb([0; 3]));
                    }
                }
            }
            image
        },
        width_mm: 76.2,
        height_mm: 50.8,
        ..page()
    };
    let before = layout::render(&wide, Crop::FULL, &s).unwrap();
    let after = layout::render(&wide, Crop::FULL, &with_footer(true, &s)).unwrap();
    assert!(!after.warnings.iter().any(|w| w.contains("footer")));
    // Same size: the label's ink spans the same number of rows and columns.
    let (b, a) = (ink_rows(&before.image), ink_rows(&after.image));
    let label_rows = b.last().unwrap() - b[0];
    assert_eq!(a[0] + label_rows, a[a.len() - 1].min(a[0] + label_rows));
    assert!(a[0] < b[0], "label moved up");
    let moved_up = b[0] - a[0];
    let (w, h) = before.image.dimensions();
    let mut shifted = RgbImage::from_pixel(w, h, Rgb([255; 3]));
    image::imageops::overlay(&mut shifted, &before.image, 0, -(moved_up as i64));
    let cut = a[0] + label_rows + 1;
    assert!(
        without_rows_from(&after.image, cut) == without_rows_from(&shifted, cut),
        "label changed beyond moving up"
    );
}
#[test]
fn without_room_for_the_qr_code_only_the_message_prints() {
    // Enough blank space for the message, not for the QR code.
    let mut label = full_bleed();
    for y in 865..900 {
        for x in 0..600 {
            label.image.put_pixel(x, y, Rgb([255; 3]));
        }
    }
    let plain = Settings::default();
    let before = layout::render(&label, Crop::FULL, &plain).unwrap();
    let after = layout::render(&label, Crop::FULL, &with_footer(true, &plain)).unwrap();
    let message_only = layout::render(&label, Crop::FULL, &with_footer(false, &plain)).unwrap();
    assert!(
        after
            .warnings
            .iter()
            .any(|w| w.contains("QR code was left out")),
        "{:?}",
        after.warnings
    );
    // Exactly the message-only footer, and the label itself is untouched.
    assert!(
        after.image == message_only.image,
        "not the message-only footer"
    );
    let last = *ink_rows(&before.image).last().unwrap();
    assert!(
        without_rows_from(&after.image, last + 1) == without_rows_from(&before.image, last + 1),
        "label changed"
    );
    assert!(
        ink_rows(&after.image).iter().any(|&y| y > last + 1),
        "message printed"
    );
}
