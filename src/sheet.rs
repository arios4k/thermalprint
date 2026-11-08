//! Several labels on one A4 or Letter sheet, for office printers and sheets of sticky labels.
use crate::layout::Output;
use anyhow::{Result, bail};
use image::{Rgb, RgbImage};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    A4,
    Letter,
}

impl Page {
    pub fn size_mm(self) -> (f32, f32) {
        match self {
            Page::A4 => (210., 297.),
            Page::Letter => (215.9, 279.4),
        }
    }
    /// The media name printers use for this paper.
    pub fn media(self) -> &'static str {
        match self {
            Page::A4 => "A4",
            Page::Letter => "Letter",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetSpec {
    pub page: Page,
    pub columns: u32,
    pub rows: u32,
    /// Space between the page edge and the labels, in millimeters.
    pub margin_mm: f32,
    /// Space between neighbouring labels, in millimeters.
    pub gap_mm: f32,
}

impl SheetSpec {
    pub fn cells(&self) -> usize {
        (self.columns * self.rows) as usize
    }

    /// The most columns and rows of this label size that fit the page.
    pub fn fitting(page: Page, label_w: f32, label_h: f32, margin_mm: f32, gap_mm: f32) -> Self {
        let (pw, ph) = page.size_mm();
        let fit = |page_len: f32, label: f32| {
            if label <= 0. || page_len - 2. * margin_mm < label {
                0
            } else {
                (((page_len - 2. * margin_mm + gap_mm) / (label + gap_mm)).floor() as u32).max(1)
            }
        };
        Self {
            page,
            columns: fit(pw, label_w),
            rows: fit(ph, label_h),
            margin_mm,
            gap_mm,
        }
    }

    /// Top-left corner of every cell in millimeters, row by row. The grid is centered on the page.
    pub fn positions(&self, label_w: f32, label_h: f32) -> Result<Vec<(f32, f32)>> {
        let ok = |v: f32| v.is_finite() && v >= 0.;
        if !ok(self.margin_mm) || !ok(self.gap_mm) || !label_w.is_finite() || !label_h.is_finite() {
            bail!("The sheet settings are not valid numbers");
        }
        if !(1..=50).contains(&self.columns) || !(1..=50).contains(&self.rows) {
            bail!("Use 1 to 50 columns and rows");
        }
        let (pw, ph) = self.page.size_mm();
        let used_w = self.columns as f32 * label_w + (self.columns - 1) as f32 * self.gap_mm;
        let used_h = self.rows as f32 * label_h + (self.rows - 1) as f32 * self.gap_mm;
        if used_w > pw - 2. * self.margin_mm + 0.01 || used_h > ph - 2. * self.margin_mm + 0.01 {
            bail!(
                "{}×{} labels of {:.1} × {:.1} mm do not fit on {} with these margins",
                self.columns,
                self.rows,
                label_w,
                label_h,
                self.page.media()
            );
        }
        let (x0, y0) = ((pw - used_w) / 2., (ph - used_h) / 2.);
        let mut out = Vec::new();
        for r in 0..self.rows {
            for c in 0..self.columns {
                out.push((
                    x0 + c as f32 * (label_w + self.gap_mm),
                    y0 + r as f32 * (label_h + self.gap_mm),
                ));
            }
        }
        Ok(out)
    }
}

/// How many sheets `labels` labels need.
pub fn sheets_needed(labels: usize, spec: &SheetSpec) -> usize {
    labels.div_ceil(spec.cells().max(1)).max(1)
}

/// Place the labels on pages, filling cells row by row; a new page starts when one is full.
/// All labels must be the same size and resolution.
pub fn compose(labels: &[Output], spec: &SheetSpec) -> Result<Vec<Output>> {
    let Some(first) = labels.first() else {
        bail!("There are no labels to place");
    };
    let dpi = first.dpi;
    if labels.iter().any(|l| {
        l.dpi != dpi
            || (l.width_mm - first.width_mm).abs() > 0.01
            || (l.height_mm - first.height_mm).abs() > 0.01
    }) {
        bail!("Every label on a sheet must have the same size");
    }
    let cells = spec.positions(first.width_mm, first.height_mm)?;
    let (pw, ph) = spec.page.size_mm();
    let dots = |mm: f32| (mm / 25.4 * dpi as f32).round() as u32;
    let (w, h) = (dots(pw), dots(ph));
    if w as u64 * h as u64 > 36_000_000 {
        bail!("The sheet is too large at {dpi} DPI");
    }
    let mut pages = Vec::new();
    for chunk in labels.chunks(cells.len()) {
        let mut image = RgbImage::from_pixel(w, h, Rgb([255; 3]));
        let mut warnings = Vec::new();
        for (label, (x, y)) in chunk.iter().zip(&cells) {
            image::imageops::replace(&mut image, &label.image, dots(*x) as i64, dots(*y) as i64);
            warnings.extend(label.warnings.iter().cloned());
        }
        pages.push(Output {
            image,
            warnings,
            clipped: chunk.iter().any(|l| l.clipped),
            width_mm: pw,
            height_mm: ph,
            dpi,
        });
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(w: f32, h: f32, shade: u8) -> Output {
        let dpi = 100;
        let dots = |mm: f32| (mm / 25.4 * dpi as f32).round() as u32;
        Output {
            image: RgbImage::from_pixel(dots(w), dots(h), Rgb([shade; 3])),
            warnings: vec![],
            clipped: false,
            width_mm: w,
            height_mm: h,
            dpi,
        }
    }

    #[test]
    fn the_most_labels_that_fit_are_found() {
        let spec = SheetSpec::fitting(Page::A4, 100., 50., 5., 0.);
        assert_eq!((spec.columns, spec.rows), (2, 5));
        let tiny = SheetSpec::fitting(Page::Letter, 38.1, 21.2, 10., 2.);
        assert_eq!((tiny.columns, tiny.rows), (4, 11));
        // Five across would need 198.5 mm of the 195.9 mm available.
        assert!(
            SheetSpec { columns: 5, ..tiny }
                .positions(38.1, 21.2)
                .is_err()
        );
        let huge = SheetSpec::fitting(Page::A4, 300., 50., 5., 0.);
        assert_eq!(huge.columns, 0, "does not fit at all");
        assert!(huge.positions(300., 50.).is_err());
    }

    #[test]
    fn cells_are_centered_and_ordered_row_by_row() {
        let spec = SheetSpec {
            page: Page::A4,
            columns: 2,
            rows: 2,
            margin_mm: 5.,
            gap_mm: 10.,
        };
        let cells = spec.positions(80., 100.).unwrap();
        assert_eq!(cells.len(), 4);
        // 2 labels of 80 mm and a 10 mm gap use 170 mm, centered on 210 mm.
        assert!(
            (cells[0].0 - 20.).abs() < 0.01 && (cells[1].0 - 110.).abs() < 0.01,
            "{cells:?}"
        );
        assert!(cells[1].0 > cells[0].0 && cells[1].1 == cells[0].1);
        assert!(cells[2].1 > cells[0].1 && cells[2].0 == cells[0].0);
        let too_many = SheetSpec { columns: 3, ..spec };
        assert!(
            too_many
                .positions(80., 100.)
                .unwrap_err()
                .to_string()
                .contains("do not fit")
        );
        let bad = SheetSpec {
            gap_mm: f32::NAN,
            ..spec
        };
        assert!(bad.positions(80., 100.).is_err());
        assert!(SheetSpec { rows: 0, ..spec }.positions(80., 100.).is_err());
    }

    #[test]
    fn labels_fill_pages_and_start_new_ones() {
        let spec = SheetSpec {
            page: Page::A4,
            columns: 2,
            rows: 1,
            margin_mm: 5.,
            gap_mm: 5.,
        };
        let labels: Vec<Output> = (0..3).map(|i| label(90., 50., 10 + i)).collect();
        let pages = compose(&labels, &spec).unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(sheets_needed(3, &spec), 2);
        let page = &pages[0];
        assert_eq!((page.width_mm, page.height_mm), (210., 297.));
        // Two labels on the first page, one on the second, rest stays white.
        let dark = |p: &Output| p.image.pixels().filter(|px| px.0[0] < 100).count();
        let one = dark(&pages[1]);
        assert!(dark(page) > one * 19 / 10, "{} vs {one}", dark(page));
        assert!(
            page.image.pixels().next().unwrap().0 == [255; 3],
            "margins stay white"
        );
    }

    #[test]
    fn mixed_label_sizes_are_refused() {
        let spec = SheetSpec::fitting(Page::A4, 50., 30., 5., 0.);
        assert!(compose(&[label(50., 30., 0), label(50., 31., 0)], &spec).is_err());
        assert!(compose(&[], &spec).is_err());
    }
}
