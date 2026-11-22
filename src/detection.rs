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
        // A gap between a bordered label and its instructions can be much
        // smaller than the whitespace around the whole document. Text line
        // spacing is normally far below this threshold at print resolution.
        let minimum = ((end - start) as f32 * 0.025).ceil().max(12.) as u32;
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
    // A separate short instruction block should not be fitted with the label.
    // Only select automatically when one plausible label region dominates;
    // two similarly sized labels or ambiguous groups remain full-page.
    let likely_label = choose_label(image, &candidates);
    // A section with its own barcode is part of the label, never separate instructions:
    // for example the USPS "Deliver to" half of a FedEx Ground Economy label.
    let label = likely_label.map(|index| keep_barcode_sections(image, &candidates, index));
    if let Some(crop) = label
        && candidates.iter().all(|c| contains(crop, *c))
    {
        // Every section belongs to the label: nothing is excluded, so this is the same as a
        // plain whitespace trim.
        return Detection {
            proposed: whole,
            candidates,
            message: "Whitespace trimmed with a safety buffer. Check all edges and barcode quiet zones before printing.".into(),
            needs_review: false,
        };
    }
    let needs_review = !candidates.is_empty();
    Detection {
        proposed: if let Some(crop) = label {
            crop
        } else if needs_review {
            Crop::FULL
        } else {
            whole
        },
        candidates,
        message: if likely_label.is_some() {
            "Likely shipping label selected; separate instructions are excluded from the processed preview and print. Check every edge and barcode quiet zone before confirming.".into()
        } else if needs_review {
            "Separate content groups found. Full page preserved. Select a candidate and confirm that all shipping information and barcode quiet zones remain.".into()
        } else {
            "Whitespace trimmed with a safety buffer. Check all edges and barcode quiet zones before printing.".into()
        },
        needs_review,
    }
}

/// The chosen label, extended to take in every other section that carries a barcode.
fn keep_barcode_sections(image: &RgbImage, candidates: &[Crop], label: usize) -> Crop {
    candidates
        .iter()
        .enumerate()
        .filter(|&(i, c)| i != label && has_stable_bars(image, *c))
        .fold(candidates[label], |crop, (_, c)| union(crop, *c))
}
fn union(a: Crop, b: Crop) -> Crop {
    let (x, y) = (a.x.min(b.x), a.y.min(b.y));
    Crop {
        x,
        y,
        w: (a.x + a.w).max(b.x + b.w) - x,
        h: (a.y + a.h).max(b.y + b.h) - y,
    }
}
fn contains(outer: Crop, inner: Crop) -> bool {
    const SLACK: f32 = 1e-4;
    inner.x >= outer.x - SLACK
        && inner.y >= outer.y - SLACK
        && inner.x + inner.w <= outer.x + outer.w + SLACK
        && inner.y + inner.h <= outer.y + outer.h + SLACK
}
/// Whether a region holds a linear barcode: many rows in a row cross the same bars, with
/// every bar edge at the same place. Text lines are too short and their letter edges move
/// from row to row, so instructions do not qualify.
fn has_stable_bars(image: &RgbImage, crop: Crop) -> bool {
    let (x, y, w, h) = crop.pixels(image.width(), image.height());
    // Bars must stand at least this tall: taller than a line of text.
    let tall = (image.height() / 80).max(12);
    let edges = |row: u32| -> Vec<u32> {
        let mut previous = false;
        let mut found = vec![];
        for xx in x..x + w {
            let black = ink(image.get_pixel(xx, row));
            if xx > x && black != previous {
                found.push(xx);
            }
            previous = black;
        }
        found
    };
    let same = |a: &[u32], b: &[u32]| {
        a.len() == b.len() && a.iter().zip(b).all(|(p, q)| p.abs_diff(*q) <= 1)
    };
    let mut run_start = y;
    let mut previous: Vec<u32> = vec![];
    for row in y..y + h {
        let current = edges(row);
        if current.len() >= 24 && same(&current, &previous) {
            if row - run_start >= tall {
                return true;
            }
        } else {
            run_start = row;
        }
        previous = current;
    }
    false
}

fn choose_label(image: &RgbImage, candidates: &[Crop]) -> Option<usize> {
    if candidates.len() < 2 {
        return None;
    }
    let mut ranked: Vec<(usize, f32, bool, bool)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let (_, _, w, h) = c.pixels(image.width(), image.height());
            let ratio = w as f32 / h as f32;
            let plausible = (0.45..=2.3).contains(&ratio);
            let area = w as f32 * h as f32;
            (i, area, plausible, has_linear_barcode_pattern(image, *c))
        })
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let best = ranked.iter().find(|item| item.2)?;
    let largest_other = ranked
        .iter()
        .filter(|item| item.0 != best.0)
        .map(|item| item.1)
        .fold(0., f32::max);
    let other_barcode = ranked.iter().any(|item| item.0 != best.0 && item.3);
    let dominant = best.1 >= largest_other * 1.8;
    let barcode_evidence = best.3 && !other_barcode && best.1 >= largest_other * 0.7;
    // Do not choose one of several overlapping proposals as if they were
    // distinct label/instruction sections.
    let disjoint = ranked.iter().filter(|item| item.0 != best.0).all(|item| {
        let a = candidates[best.0];
        let b = candidates[item.0];
        let x = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let y = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        (x.max(0.) * y.max(0.)) < (a.w * a.h).min(b.w * b.h) * 0.15
    });
    if disjoint && (dominant || barcode_evidence) {
        Some(best.0)
    } else {
        None
    }
}

fn has_linear_barcode_pattern(image: &RgbImage, crop: Crop) -> bool {
    let (x, y, w, h) = crop.pixels(image.width(), image.height());
    let stride = (h / 160).max(1);
    let mut consecutive = 0;
    for yy in (y..y + h).step_by(stride as usize) {
        let mut previous = false;
        let mut transitions = 0;
        for xx in x..x + w {
            let black = ink(image.get_pixel(xx, yy));
            if xx > x && black != previous {
                transitions += 1;
            }
            previous = black;
        }
        if transitions >= 24 {
            consecutive += 1;
        } else {
            consecutive = 0;
        }
        if consecutive >= 10 {
            return true;
        }
    }
    false
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
    fn separate_instructions_are_removed_before_layout() {
        let mut im = RgbImage::from_pixel(800, 1000, image::Rgb([255; 3]));
        // Representative label body, with a smaller, wide instruction block
        // well outside its bottom boundary.
        block(&mut im, 120, 55, 520, 620);
        block(&mut im, 100, 820, 600, 70);
        let d = detect(&im);
        assert!(d.needs_review);
        assert_eq!(d.candidates.len(), 2);
        let (x, y, w, h) = d.proposed.pixels(800, 1000);
        assert!(x < 120 && y < 55 && x + w > 640 && y + h > 675);
        assert!(y + h < 820, "instruction block entered processed crop");
    }
    fn bars(im: &mut RgbImage, x: u32, y: u32, w: u32, h: u32) {
        for xx in (x..x + w).filter(|xx| (xx * 7 / 5) % 3 == 0) {
            for yy in y..y + h {
                im.put_pixel(xx, yy, image::Rgb([0; 3]));
            }
        }
    }
    /// Lines of letter-like marks whose edges move from row to row, like printed text.
    fn text(im: &mut RgbImage, x: u32, y: u32, w: u32, lines: u32) {
        for line in 0..lines {
            let top = y + line * 26;
            for row in 0..18 {
                let mut xx = x;
                let mut glyph = 0u32;
                while xx + 12 < x + w {
                    // Each glyph's stroke shifts with the row, as curves do.
                    let shift = (row * (glyph % 5 + 1) / 4) % 6;
                    for dx in 0..3 {
                        im.put_pixel(xx + shift + dx, top + row, image::Rgb([0; 3]));
                    }
                    xx += 9 + glyph % 4;
                    glyph += 1;
                }
            }
        }
    }
    #[test]
    fn a_lower_section_with_its_own_barcode_stays_with_the_label() {
        // A FedEx Ground Economy label: the FedEx half on top and, after a gap, a smaller
        // USPS section with its own barcode. Nothing is separate instructions.
        let mut im = RgbImage::from_pixel(1200, 1800, image::Rgb([255; 3]));
        block(&mut im, 20, 10, 1160, 60);
        bars(&mut im, 60, 90, 1000, 300);
        text(&mut im, 40, 410, 1100, 4);
        bars(&mut im, 120, 540, 960, 220);
        block(&mut im, 20, 780, 1160, 8);
        // The USPS section, after a wide gap.
        text(&mut im, 40, 1300, 700, 4);
        bars(&mut im, 100, 1420, 1000, 280);
        let d = detect(&im);
        assert_eq!(d.candidates.len(), 2, "{:?}", d.candidates);
        let (_, y, _, h) = d.proposed.pixels(1200, 1800);
        assert!(
            y <= 10 && y + h >= 1700,
            "lower barcode cut off: {:?}",
            (y, h)
        );
        assert!(!d.needs_review);
    }
    #[test]
    fn instructions_with_text_are_still_removed_next_to_a_barcode_label() {
        let mut im = RgbImage::from_pixel(1200, 1800, image::Rgb([255; 3]));
        block(&mut im, 100, 60, 1000, 40);
        bars(&mut im, 150, 120, 900, 400);
        text(&mut im, 120, 540, 960, 8);
        bars(&mut im, 150, 770, 900, 200);
        // Printed instructions below the label: text only, no barcode.
        text(&mut im, 100, 1450, 1000, 10);
        let d = detect(&im);
        assert!(d.needs_review);
        let (_, y, _, h) = d.proposed.pixels(1200, 1800);
        assert!(y + h < 1450, "instructions entered the crop");
        assert!(y + h >= 970, "label barcode cut off");
    }
    #[test]
    fn text_is_not_mistaken_for_a_barcode() {
        let mut im = RgbImage::from_pixel(1200, 1800, image::Rgb([255; 3]));
        text(&mut im, 50, 50, 1100, 20);
        assert!(!has_stable_bars(&im, Crop::FULL));
        bars(&mut im, 100, 900, 900, 150);
        assert!(has_stable_bars(&im, Crop::FULL));
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
