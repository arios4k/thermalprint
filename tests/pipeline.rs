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
#[test]
fn the_footer_gets_its_own_strip_and_never_covers_the_label() {
    let plain = layout::render(&page(), Crop::FULL, &Settings::default()).unwrap();
    let s = Settings {
        footer: Footer {
            message_enabled: true,
            qr_enabled: true,
            ..Footer::default()
        },
        ..Settings::default()
    };
    let with = layout::render(&page(), Crop::FULL, &s).unwrap();
    assert!(!with.clipped);
    assert_eq!(with.image.dimensions(), plain.image.dimensions());
    let height = with.image.height();
    let margin = (s.margin_mm / 25.4 * s.dpi as f32).round() as u32;
    let reserved = s.footer.reserved_dots(s.dpi, height, margin);
    // The label content ends where the footer's space begins, and at least 1 mm of white
    // separates it from the footer.
    let content_bottom = height - margin - reserved;
    let gap = (1. / 25.4 * s.dpi as f32).round() as u32;
    let rows = ink_rows(&with.image);
    assert!(
        rows.iter().any(|&y| y >= content_bottom + gap),
        "footer printed"
    );
    assert!(
        rows.iter()
            .all(|&y| y < content_bottom || y >= content_bottom + gap)
    );
    // The footer is small next to the label.
    assert!(reserved as f32 <= height as f32 * 0.1);
}
#[test]
fn actual_size_labels_that_would_lose_space_to_the_footer_are_blocked_not_covered() {
    let s = Settings {
        scale: Scale::Actual,
        width_mm: 50.8,
        height_mm: 76.2,
        margin_mm: 0.,
        footer: Footer {
            message_enabled: true,
            ..Footer::default()
        },
        ..Settings::default()
    };
    let output = layout::render(&page(), Crop::FULL, &s).unwrap();
    assert!(output.clipped);
    assert!(output.warnings.iter().any(|w| w.contains("footer")));
}
