//! Conservative whitespace/layout analysis. This is a proposal, never a semantic
//! claim that a carrier label or barcode has been recognized.
use image::RgbImage;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Crop {
    pub const FULL: Self = Self {
        x: 0.,
        y: 0.,
        w: 1.,
        h: 1.,
    };
    pub fn bounded(self) -> Self {
        if ![self.x, self.y, self.w, self.h]
            .iter()
            .all(|n| n.is_finite())
        {
            return Self::FULL;
        }
        let x = self.x.clamp(0., 0.99);
        let y = self.y.clamp(0., 0.99);
        Self {
            x,
            y,
            w: self.w.clamp(0.01, 1. - x),
            h: self.h.clamp(0.01, 1. - y),
        }
    }
    pub fn pixels(self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let c = self.bounded();
        let x = (c.x * width as f32).floor() as u32;
        let y = (c.y * height as f32).floor() as u32;
        let right = ((c.x + c.w) * width as f32).ceil().min(width as f32) as u32;
        let bottom = ((c.y + c.h) * height as f32).ceil().min(height as f32) as u32;
        (x, y, (right - x).max(1), (bottom - y).max(1))
    }
}
#[derive(Clone, Debug)]
pub struct Detection {
    pub proposed: Crop,
    pub candidates: Vec<Crop>,
    pub message: String,
    pub needs_review: bool,
}
fn ink(p: &image::Rgb<u8>) -> bool {
    p.0.iter().copied().min().unwrap_or(255) < 220
}
pub fn detect(image: &RgbImage) -> Detection {
    let (w, h) = image.dimensions();
    let mut rows = vec![false; h as usize];
    let mut cols = vec![false; w as usize];
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in image.enumerate_pixels() {
        if ink(p) {
            rows[y as usize] = true;
            cols[x as usize] = true;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return Detection {
            proposed: Crop::FULL,
            candidates: vec![],
            message: "No clear content found. Full page preserved; choose a crop manually.".into(),
            needs_review: true,
        };
    }
    let padded = |a: u32, b: u32, c: u32, d: u32| {
        // Keep a generous quiet-zone buffer. Never tighten around individual bars.
        let pad = ((w.min(h) as f32 * 0.025).ceil() as u32).max(8);
        let left = a.saturating_sub(pad);
        let top = b.saturating_sub(pad);
        Crop {
            x: left as f32 / w as f32,
            y: top as f32 / h as f32,
            w: (c.saturating_add(pad).min(w) - left) as f32 / w as f32,
            h: (d.saturating_add(pad).min(h) - top) as f32 / h as f32,
        }
    };
    let whole = padded(x0, y0, x1, y1);
    let mut candidates = vec![];
    for (axis, start, end, horizontal) in [(&rows, y0, y1, true), (&cols, x0, x1, false)] {
        let minimum = ((end - start) as f32 * 0.055).ceil() as u32;
        let mut sections = vec![];
        let mut section = start;
        let mut i = start;
        while i < end {
            if !axis[i as usize] {
                let gap_start = i;
                while i < end && !axis[i as usize] {
                    i += 1;
                }
                if i - gap_start >= minimum && gap_start > section {
                    sections.push((section, gap_start));
                    section = i;
                }
            } else {
                i += 1;
            }
        }
        if section < end {
            sections.push((section, end));
        }
        if sections.len() > 1 && sections.len() <= 8 {
            for (a, b) in sections {
                let (mut l, mut t, mut r, mut bot) = (w, h, 0, 0);
                let mut count = 0u64;
                for (x, y, p) in image.enumerate_pixels() {
                    let v = if horizontal { y } else { x };
                    if v >= a && v < b && ink(p) {
                        l = l.min(x);
                        t = t.min(y);
                        r = r.max(x + 1);
                        bot = bot.max(y + 1);
                        count += 1;
                    }
                }
                if count > 100
                    && r > l
                    && bot > t
                    && (r - l) as f32 * (bot - t) as f32 > (w as f32 * h as f32) * 0.025
                {
                    let c = padded(l, t, r, bot);
                    if !candidates.contains(&c) {
                        candidates.push(c);
                    }
                }
            }
        }
    }
    let needs_review = !candidates.is_empty();
    Detection {
        proposed: if needs_review { Crop::FULL } else { whole },
        candidates,
        message: if needs_review {
            "Separate content groups found. Full page preserved. Select a candidate and confirm that all shipping information and barcode quiet zones remain.".into()
        } else {
            "Whitespace trimmed with a safety buffer. Check all edges and barcode quiet zones before printing.".into()
        },
        needs_review,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn block(im: &mut RgbImage, x: u32, y: u32, w: u32, h: u32) {
        for yy in y..y + h {
            for xx in x..x + w {
                im.put_pixel(xx, yy, image::Rgb([0; 3]));
            }
        }
    }
    #[test]
    fn off_center_preserves_content_and_buffer() {
        let mut im = RgbImage::from_pixel(800, 1000, image::Rgb([255; 3]));
        block(&mut im, 450, 300, 200, 400);
        let d = detect(&im);
        assert!(!d.needs_review);
        let (x, y, w, h) = d.proposed.pixels(800, 1000);
        assert!(x < 450 && y < 300 && x + w > 650 && y + h > 700);
    }
    #[test]
    fn multiple_groups_require_review() {
        let mut im = RgbImage::from_pixel(800, 1000, image::Rgb([255; 3]));
        block(&mut im, 100, 50, 600, 350);
        block(&mut im, 100, 600, 600, 350);
        let d = detect(&im);
        assert!(d.needs_review);
        assert_eq!(d.proposed, Crop::FULL);
        assert_eq!(d.candidates.len(), 2);
    }
    #[test]
    fn blank_is_not_discarded() {
        let d = detect(&RgbImage::from_pixel(100, 100, image::Rgb([255; 3])));
        assert_eq!(d.proposed, Crop::FULL);
        assert!(d.needs_review);
    }
    #[test]
    fn invalid_crop_is_safe() {
        assert_eq!(
            Crop {
                x: f32::NAN,
                ..Crop::FULL
            }
            .bounded(),
            Crop::FULL
        );
        assert_eq!(Crop::FULL.pixels(300, 400), (0, 0, 300, 400));
    }
}
