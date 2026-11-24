//! A ruler-style test label for correcting a printer's alignment offset.
use crate::layout::Output;
use anyhow::{Result, ensure};
use image::{Rgb, RgbImage};

/// A 1 mm inset border, a center cross, and ruler ticks every 5 mm (longer every 10 mm).
/// Compare where the border lands on the stock with the label edges, then enter the
/// difference as the printer's horizontal and vertical offset.
pub fn alignment_test(width_mm: f32, height_mm: f32, dpi: u32) -> Output {
    alignment_test_in_area(width_mm, height_mm, dpi, (0., 0., 0., 0.))
        .expect("valid alignment-test dimensions")
}

pub fn alignment_test_in_area(
    width_mm: f32,
    height_mm: f32,
    dpi: u32,
    margins_mm: (f32, f32, f32, f32),
) -> Result<Output> {
    ensure!(
        width_mm.is_finite()
            && height_mm.is_finite()
            && width_mm > 0.
            && height_mm > 0.
            && dpi > 0
            && [margins_mm.0, margins_mm.1, margins_mm.2, margins_mm.3]
                .iter()
                .all(|margin| margin.is_finite() && *margin >= 0.),
        "Invalid alignment-test geometry"
    );
    let dots = |mm: f32| Output::mm_to_pixels(mm, dpi);
    let (width, height) = (dots(width_mm).max(8) as u32, dots(height_mm).max(8) as u32);
    ensure!(
        width as u64 * height as u64 <= crate::layout::MAX_OUTPUT_PIXELS,
        "The alignment test is too large at {dpi} DPI. Choose a lower resolution"
    );
    let mut image = RgbImage::from_pixel(width, height, Rgb([255; 3]));
    let thickness = (dpi / 100).max(2) as i64;
    let mut fill = |x: i64, y: i64, w: i64, h: i64| {
        for yy in y.max(0)..(y + h).min(height as i64) {
            for xx in x.max(0)..(x + w).min(width as i64) {
                image.put_pixel(xx as u32, yy as u32, Rgb([0; 3]));
            }
        }
    };
    let (w, h) = (width as i64, height as i64);
    let inset = |margin: f32| dots(margin.max(0.5) + 0.5);
    let (mut left, mut top, mut right, mut bottom) = (
        inset(margins_mm.0),
        inset(margins_mm.1),
        inset(margins_mm.2),
        inset(margins_mm.3),
    );
    if margins_mm == (0., 0., 0., 0.) {
        left = left.min((w - thickness - 1).max(0) / 2);
        right = right.min((w - thickness - 1).max(0) / 2);
        top = top.min((h - thickness - 1).max(0) / 2);
        bottom = bottom.min((h - thickness - 1).max(0) / 2);
    }
    ensure!(
        left + right + thickness < w && top + bottom + thickness < h,
        "The printer's unprintable margins leave no room for an alignment pattern"
    );
    let (cx, cy, arm) = (w / 2, h / 2, dots(5.));
    if margins_mm != (0., 0., 0., 0.) {
        ensure!(
            cx - arm >= left && cx + arm <= w - right && cy - arm >= top && cy + arm <= h - bottom,
            "The printer's unprintable margins leave no room for the alignment cross"
        );
    }
    fill(left, top, w - left - right, thickness);
    fill(left, h - bottom - thickness, w - left - right, thickness);
    fill(left, top, thickness, h - top - bottom);
    fill(w - right - thickness, top, thickness, h - top - bottom);
    fill(cx - arm, cy - thickness / 2, 2 * arm, thickness);
    fill(cx - thickness / 2, cy - arm, thickness, 2 * arm);
    let mut mm = 5;
    while left + dots(mm as f32) < w - right {
        let long = mm % 10 == 0;
        let tick = dots(if long { 4. } else { 2. });
        let at = left + dots(mm as f32);
        fill(at, top + thickness, thickness, tick);
        fill(at, h - bottom - thickness - tick, thickness, tick);
        mm += 5;
    }
    let mut mm = 5;
    while top + dots(mm as f32) < h - bottom {
        let long = mm % 10 == 0;
        let tick = dots(if long { 4. } else { 2. });
        let at = top + dots(mm as f32);
        fill(left + thickness, at, tick, thickness);
        fill(w - right - thickness - tick, at, tick, thickness);
        mm += 5;
    }
    Ok(Output {
        image,
        warnings: vec![],
        clipped: false,
        width_mm,
        height_mm,
        dpi,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_has_the_requested_size_a_border_and_a_center_cross() {
        let output = alignment_test(101.6, 152.4, 203);
        assert_eq!(output.image.dimensions(), (812, 1218));
        let inset = (1. / 25.4 * 203.0_f32).round() as u32;
        assert_eq!(output.image.get_pixel(400, inset).0, [0; 3], "top border");
        assert_eq!(output.image.get_pixel(406, 609).0, [0; 3], "center cross");
        assert_eq!(
            output.image.get_pixel(0, 0).0,
            [255; 3],
            "outside the border stays white"
        );
    }

    #[test]
    fn tiny_labels_do_not_panic() {
        let output = alignment_test(1., 1., 203);
        assert!(output.image.width() >= 8);
    }

    #[test]
    fn alignment_marks_stay_inside_simulated_driver_margins_at_preset_sizes() {
        for &(width, height) in &[
            (101.6, 152.4),
            (101.6, 101.6),
            (50.8, 25.4),
            (100., 150.),
            (50., 30.),
        ] {
            for dpi in [203, 300] {
                let margins = (2.54, 2.0, 3.0, 2.54);
                let output = alignment_test_in_area(width, height, dpi, margins).unwrap();
                let (left, top, right, bottom) = (
                    Output::mm_to_pixels(margins.0, dpi) as u32,
                    Output::mm_to_pixels(margins.1, dpi) as u32,
                    Output::mm_to_pixels(margins.2, dpi) as u32,
                    Output::mm_to_pixels(margins.3, dpi) as u32,
                );
                let image_width = output.image.width();
                let image_height = output.image.height();
                assert!(output.image.enumerate_pixels().all(|(x, y, pixel)| {
                    pixel.0 == [255; 3]
                        || (x >= left
                            && x < image_width - right
                            && y >= top
                            && y < image_height - bottom)
                }));
            }
        }
    }
}
