use image::{Rgb, RgbImage};
use std::path::PathBuf;
use thermalprint::{
    detection::Crop,
    document::Page,
    export, layout,
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
