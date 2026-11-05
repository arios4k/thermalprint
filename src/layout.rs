use crate::{
    detection::Crop,
    document::Page,
    settings::{Scale, Settings},
};
use anyhow::{Result, bail};
use image::{
    RgbImage,
    imageops::{self, FilterType},
};
#[derive(Clone)]
pub struct Output {
    pub image: RgbImage,
    pub warnings: Vec<String>,
    pub clipped: bool,
    pub width_mm: f32,
    pub height_mm: f32,
    pub dpi: u32,
}
pub fn render(page: &Page, crop: Crop, s: &Settings) -> Result<Output> {
    if ![s.width_mm, s.height_mm, s.margin_mm, s.scale_percent]
        .iter()
        .all(|v| v.is_finite())
        || !(10.0..=500.0).contains(&s.width_mm)
        || !(10.0..=500.0).contains(&s.height_mm)
        || ![203, 300, 600].contains(&s.dpi)
        || s.margin_mm < 0.
        || s.scale_percent <= 0.
    {
        bail!("Invalid paper or scaling settings");
    }
    let width = (s.width_mm / 25.4 * s.dpi as f32).round() as u32;
    let height = (s.height_mm / 25.4 * s.dpi as f32).round() as u32;
    if width as u64 * height as u64 > 36_000_000 {
        bail!("Output exceeds the 36 megapixel limit");
    }
    let margin = (s.margin_mm / 25.4 * s.dpi as f32).round() as u32;
    if margin * 2 >= width || margin * 2 >= height {
        bail!("Margins leave no printable area");
    }
    let (x, y, w, h) = crop.pixels(page.image.width(), page.image.height());
    let source = imageops::crop_imm(&page.image, x, y, w, h).to_image();
    let mut mm_w = page.width_mm * w as f32 / page.image.width() as f32;
    let mut mm_h = page.height_mm * h as f32 / page.image.height() as f32;
    let source = match s.rotation {
        90 => {
            std::mem::swap(&mut mm_w, &mut mm_h);
            imageops::rotate90(&source)
        }
        180 => imageops::rotate180(&source),
        270 => {
            std::mem::swap(&mut mm_w, &mut mm_h);
            imageops::rotate270(&source)
        }
        _ => source,
    };
    let available_w = width - margin * 2;
    let available_h = height - margin * 2;
    let fit = (available_w as f32 / source.width() as f32)
        .min(available_h as f32 / source.height() as f32);
    let factor = match s.scale {
        Scale::Fit => fit,
        Scale::Actual => mm_w / 25.4 * s.dpi as f32 / source.width() as f32,
        Scale::Custom => {
            mm_w / 25.4 * s.dpi as f32 / source.width() as f32 * s.scale_percent / 100.
        }
    };
    let target_w = (source.width() as f32 * factor).round().max(1.) as u32;
    let target_h = (source.height() as f32 * factor).round().max(1.) as u32;
    let clipped = target_w > available_w || target_h > available_h;
    let mut warnings = vec![];
    if clipped {
        warnings.push("Content exceeds the printable area. Printing is blocked. Use Fit, rotate, reduce scale, or choose larger paper.".into());
    }
    let physical_scale = (target_w as f32 / s.dpi as f32 * 25.4) / mm_w;
    if physical_scale < 0.8 {
        warnings.push(format!("Content is reduced to {:.0}% of its source size. Small text and barcode modules may become unreadable.",physical_scale*100.));
    }
    if factor > 1.05 {
        warnings.push("Source pixels are enlarged. Increasing resolution cannot recover missing barcode detail.".into());
    }
    if !page
        .path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        warnings.push("Image physical size is estimated at 300 DPI. Verify dimensions when using Actual size.".into());
    }
    let mut canvas = RgbImage::from_pixel(width, height, image::Rgb([255; 3]));
    // Avoid unbounded intermediate allocations for oversized actual/custom layouts.
    if !clipped {
        let mut resized = if source.dimensions() == (target_w, target_h) {
            source
        } else {
            imageops::resize(&source, target_w, target_h, FilterType::Lanczos3)
        };
        if s.enhance {
            for p in resized.pixels_mut() {
                let l =
                    (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000;
                *p = image::Rgb([if l < 160 { 0 } else { 255 }; 3]);
            }
            warnings.push(
                "Black-and-white thresholding is enabled. Inspect fine bars before printing."
                    .into(),
            );
        }
        imageops::overlay(
            &mut canvas,
            &resized,
            ((width - target_w) / 2) as i64,
            ((height - target_h) / 2) as i64,
        );
    }
    let _ = mm_h;
    Ok(Output {
        image: canvas,
        warnings,
        clipped,
        width_mm: s.width_mm,
        height_mm: s.height_mm,
        dpi: s.dpi,
    })
}
