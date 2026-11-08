//! A ruler-style test label for correcting a printer's alignment offset.
use crate::layout::Output;
use image::{Rgb, RgbImage};

/// A 1 mm inset border, a center cross, and ruler ticks every 5 mm (longer every 10 mm).
/// Compare where the border lands on the stock with the label edges, then enter the
/// difference as the printer's horizontal and vertical offset.
pub fn alignment_test(width_mm: f32, height_mm: f32, dpi: u32) -> Output {
    let dots = |mm: f32| (mm / 25.4 * dpi as f32).round() as i64;
    let (width, height) = (dots(width_mm).max(8) as u32, dots(height_mm).max(8) as u32);
    let mut image = RgbImage::from_pixel(width, height, Rgb([255; 3]));
    let thickness = (dpi / 100).max(2) as i64;
    let mut fill = |x: i64, y: i64, w: i64, h: i64| {
        for yy in y.max(0)..(y + h).min(height as i64) {
            for xx in x.max(0)..(x + w).min(width as i64) {
                image.put_pixel(xx as u32, yy as u32, Rgb([0; 3]));
            }
        }
    };
    let (w, h, inset) = (width as i64, height as i64, dots(1.));
    fill(inset, inset, w - 2 * inset, thickness);
    fill(inset, h - inset - thickness, w - 2 * inset, thickness);
    fill(inset, inset, thickness, h - 2 * inset);
    fill(w - inset - thickness, inset, thickness, h - 2 * inset);
    let (cx, cy, arm) = (w / 2, h / 2, dots(5.));
    fill(cx - arm, cy - thickness / 2, 2 * arm, thickness);
    fill(cx - thickness / 2, cy - arm, thickness, 2 * arm);
    let mut mm = 5;
    while dots(mm as f32) < w.max(h) {
        let long = mm % 10 == 0;
        let tick = dots(if long { 4. } else { 2. });
        let at = dots(mm as f32);
        if at < w - inset {
            fill(at, inset + thickness, thickness, tick);
            fill(at, h - inset - thickness - tick, thickness, tick);
        }
        if at < h - inset {
            fill(inset + thickness, at, tick, thickness);
            fill(w - inset - thickness - tick, at, tick, thickness);
        }
        mm += 5;
    }
    Output {
        image,
        warnings: vec![],
        clipped: false,
        width_mm,
        height_mm,
        dpi,
    }
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
}
