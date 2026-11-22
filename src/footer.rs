//! An optional footer for labels made in Label Designer: a short message such as
//! "Printed with ThermalPrint" and, only together with that message, a small QR code just
//! before it. The whole footer is aligned to the left, center, or right of the label.
//!
//! The footer is only ever added to designs, never to imported shipping labels. It never
//! covers design content and never changes the label's size or printable area: it goes into
//! blank space at the bottom, inside the printable area, or it is left out with a note.
//! When the footer is off, nothing is reserved and output is unchanged.
use crate::layout::Output;
use anyhow::Result;
use image::{Rgb, RgbImage};
use qrcode::{Color, EcLevel, QrCode};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MESSAGE: &str = "Printed with ThermalPrint";
/// The ThermalPrint user manual.
pub const DEFAULT_QR_TEXT: &str = "https://arios4k.github.io/thermalprint/manual/";
pub const MAX_MESSAGE_CHARS: usize = 80;
pub const MAX_QR_CHARS: usize = 200;

/// Height of the message text: small print, about 6 pt.
const TEXT_MM: f32 = 2.0;
/// The smallest text size tried before a long message is shortened.
const MIN_TEXT_MM: f32 = 1.7;
/// Space between the design content and the footer.
const GAP_MM: f32 = 0.8;
/// Extra space between the QR code's white border and the message.
const TEXT_GAP_MM: f32 = 0.5;
/// The QR code is left out when it would take more than this share of the label height,
/// so the footer stays small next to the label content.
const MAX_QR_SHARE: f32 = 0.12;
/// A QR module is at least this wide, so phone cameras can read it from a thermal print.
const QR_MODULE_MM: f32 = 0.25;
/// White modules kept around the QR code: the full quiet zone the QR standard requires.
const QR_QUIET: u32 = 4;
/// Designs keep at least this much distance between the footer and the label edge.
const DESIGN_EDGE_MM: f32 = 1.5;
/// A message without a QR code keeps this much white between it and the label's side and
/// bottom edges, counting any margin already there. (A QR code's white border does this
/// job when it is present.)
const TEXT_EDGE_MM: f32 = 1.5;

/// Where the whole footer, QR code included, sits along the bottom of the label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FooterAlign {
    #[default]
    Left,
    Center,
    Right,
}
impl FooterAlign {
    pub const ALL: [FooterAlign; 3] = [FooterAlign::Left, FooterAlign::Center, FooterAlign::Right];
    pub fn name(self) -> &'static str {
        match self {
            FooterAlign::Left => "Left",
            FooterAlign::Center => "Center",
            FooterAlign::Right => "Right",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Footer {
    /// Print the message along the bottom of each designed label.
    pub message_enabled: bool,
    pub message: String,
    /// Add a QR code before the message. It prints only while the message does.
    pub qr_enabled: bool,
    pub qr_text: String,
    /// Where the footer sits along the bottom edge.
    pub align: FooterAlign,
}
impl Default for Footer {
    fn default() -> Self {
        Self {
            message_enabled: false,
            message: DEFAULT_MESSAGE.into(),
            qr_enabled: false,
            qr_text: DEFAULT_QR_TEXT.into(),
            align: FooterAlign::Left,
        }
    }
}

fn dots(mm: f32, dpi: u32) -> u32 {
    (mm / 25.4 * dpi as f32).round() as u32
}

/// Where a footer goes on a label: its horizontal extent and what it holds.
struct Placement {
    /// The first column of the footer, QR code border included.
    left: u32,
    /// Its width in dots, QR code border included.
    width: u32,
    qr: Option<QrCode>,
    /// Width of the QR code with its white border, and the gap after it.
    qr_width: u32,
    gap: u32,
    text: Option<FittedText>,
    /// The band the QR code (or the message alone) is centered in.
    band_top: u32,
    band: u32,
}

impl Footer {
    /// Whether anything prints. A QR code alone never does.
    pub fn is_active(&self) -> bool {
        self.message_enabled && !self.message.trim().is_empty()
    }
    pub fn shows_qr(&self) -> bool {
        self.is_active() && self.qr_enabled && !self.qr_text.trim().is_empty()
    }
    /// What to try printing, in order: the footer as set, then, if it has a QR code, the
    /// message alone, which needs less space.
    pub fn choices(&self) -> Vec<Footer> {
        let mut choices = vec![self.clone()];
        if self.shows_qr() {
            choices.push(Footer {
                qr_enabled: false,
                ..self.clone()
            });
        }
        choices
    }
    /// The warning for printing `chosen` instead of this footer, if any.
    pub fn fallback_note(&self, chosen: &Footer, dpi: u32, label_h: u32) -> Option<&'static str> {
        if self.qr_left_out(dpi, label_h) {
            Some(QR_LEFT_OUT)
        } else if self.shows_qr() && !chosen.shows_qr() {
            Some(QR_NO_SPACE)
        } else {
            None
        }
    }
    /// Keep values loaded from a settings file within bounds.
    pub fn clean(&mut self) {
        self.message = self
            .message
            .replace(['\n', '\r', '\t'], " ")
            .chars()
            .take(MAX_MESSAGE_CHARS)
            .collect();
        self.qr_text = self.qr_text.chars().take(MAX_QR_CHARS).collect();
    }
    fn encode(&self) -> Option<QrCode> {
        if !self.shows_qr() {
            return None;
        }
        QrCode::with_error_correction_level(self.qr_text.trim().as_bytes(), EcLevel::L).ok()
    }
    /// The QR code, if it is on and small enough for a label `label_h` dots tall.
    fn qr(&self, dpi: u32, label_h: u32) -> Option<QrCode> {
        self.encode()
            .filter(|code| Self::qr_side(code, dpi) as f32 <= label_h as f32 * MAX_QR_SHARE)
    }
    /// Whether the QR code is on but left out because the label is too short for it.
    pub fn qr_left_out(&self, dpi: u32, label_h: u32) -> bool {
        self.encode().is_some() && self.qr(dpi, label_h).is_none()
    }
    fn module(dpi: u32) -> u32 {
        dots(QR_MODULE_MM, dpi).max(2)
    }
    fn qr_side(code: &QrCode, dpi: u32) -> u32 {
        (code.width() as u32 + 2 * QR_QUIET) * Self::module(dpi)
    }
    /// The height of the message line.
    fn text_dots(dpi: u32) -> u32 {
        dots(TEXT_MM * 1.25, dpi)
    }
    /// The space the footer takes from the bottom of a label `label_h` dots tall, including
    /// the gap that keeps it apart from the label content; 0 when the footer is off. `below`
    /// is the white margin under the footer.
    ///
    /// With a QR code, its white border doubles as that gap, and the border below it overlaps
    /// the white margin, so the QR code adds as little height as the standard allows.
    pub fn reserved_dots(&self, dpi: u32, label_h: u32, below: u32) -> u32 {
        if !self.is_active() {
            return 0;
        }
        match self.qr(dpi, label_h) {
            Some(code) => {
                let quiet = QR_QUIET * Self::module(dpi);
                code.width() as u32 * Self::module(dpi) + quiet + quiet.saturating_sub(below)
            }
            None => {
                Self::text_dots(dpi)
                    + dots(GAP_MM, dpi)
                    + dots(TEXT_EDGE_MM, dpi).saturating_sub(below)
            }
        }
    }
    /// A short problem with the QR text, for the footer controls.
    pub fn qr_problem(&self) -> Option<&'static str> {
        (self.shows_qr() && self.encode().is_none())
            .then_some("The QR text is too long to encode. Shorten it.")
    }

    /// Lay the footer out inside `x0..x1` of an image `image_w` × `label_h` dots, above row
    /// `bottom` with `below` white rows under it.
    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        dpi: u32,
        image_w: u32,
        label_h: u32,
        x0: u32,
        x1: u32,
        bottom: u32,
        below: u32,
    ) -> Option<Placement> {
        let reserved = self.reserved_dots(dpi, label_h, below);
        if reserved == 0 || x1 <= x0 || bottom < reserved {
            return None;
        }
        let (band_top, band) = match self.qr(dpi, label_h) {
            Some(code) => (
                bottom - reserved + QR_QUIET * Self::module(dpi),
                code.width() as u32 * Self::module(dpi),
            ),
            None => (
                bottom - dots(TEXT_EDGE_MM, dpi).saturating_sub(below) - Self::text_dots(dpi),
                Self::text_dots(dpi),
            ),
        };
        let room = x1 - x0;
        let gap = dots(TEXT_GAP_MM, dpi);
        let qr = self
            .qr(dpi, label_h)
            .filter(|code| Self::qr_side(code, dpi) + gap < room);
        let qr_width = qr.as_ref().map_or(0, |code| Self::qr_side(code, dpi));
        let gap = if qr.is_some() { gap } else { 0 };
        // A message alone is kept off the label's side edges.
        let (inset_left, inset_right) = if qr.is_none() {
            let edge = dots(TEXT_EDGE_MM, dpi);
            (
                edge.saturating_sub(x0).min(room / 4),
                edge.saturating_sub(image_w.saturating_sub(x1))
                    .min(room / 4),
            )
        } else {
            (0, 0)
        };
        let room = room - inset_left - inset_right;
        let text = fit_text(self.message.trim(), dpi, room - qr_width - gap);
        let width = qr_width + gap + text.as_ref().map_or(0, |t| t.width);
        let start = x0 + inset_left;
        let left = match self.align {
            FooterAlign::Left => start,
            FooterAlign::Center => start + (room - width) / 2,
            FooterAlign::Right => start + room - width,
        };
        Some(Placement {
            left,
            width,
            qr,
            qr_width,
            gap,
            text,
            band_top,
            band,
        })
    }

    /// Draw the footer inside `x0..x1`, in the space [`Self::reserved_dots`] gives it above
    /// row `bottom`, with `below` white rows under it. The QR code comes first and the
    /// message follows it, vertically centered on it; the pair is aligned as set.
    pub fn draw(&self, image: &mut RgbImage, dpi: u32, x0: u32, x1: u32, bottom: u32, below: u32) {
        let (w, h) = image.dimensions();
        let Some(place) = self.place(dpi, w, h, x0, x1, bottom, below) else {
            return;
        };
        if let Some(code) = &place.qr {
            let module = Self::module(dpi);
            let origin_x = place.left + QR_QUIET * module;
            let origin_y = place.band_top;
            let width = code.width() as u32;
            for y in 0..width {
                for x in 0..width {
                    if code[(x as usize, y as usize)] == Color::Dark {
                        for py in 0..module {
                            for px in 0..module {
                                image.put_pixel(
                                    origin_x + x * module + px,
                                    origin_y + y * module + py,
                                    Rgb([0; 3]),
                                );
                            }
                        }
                    }
                }
            }
        }
        if let Some(text) = &place.text {
            draw_text(
                image,
                text,
                place.left + place.qr_width + place.gap,
                place.band_top,
                place.band,
            );
        }
    }
}

/// A message sized to fit its room.
struct FittedText {
    text: String,
    px: f32,
    width: u32,
}

/// Fit the message into `room` dots: shrink it a little first, then shorten it with an
/// ellipsis.
fn fit_text(message: &str, dpi: u32, room: u32) -> Option<FittedText> {
    let font = crate::design_render::default_font(false);
    let room = room as f32;
    let measure = |text: &str, px: f32| -> f32 {
        text.chars()
            .map(|c| font.metrics(c, px).advance_width)
            .sum()
    };
    let mut px = dots(TEXT_MM, dpi) as f32;
    let min_px = dots(MIN_TEXT_MM, dpi) as f32;
    while px > min_px && measure(message, px) > room {
        px -= 1.;
    }
    let mut text: String = message.into();
    if measure(&text, px) > room {
        let mut chars: Vec<char> = message.chars().collect();
        loop {
            chars.pop();
            if chars.is_empty() {
                return None;
            }
            text = chars.iter().collect::<String>().trim_end().to_owned() + "…";
            if measure(&text, px) <= room {
                break;
            }
        }
    }
    let width = (measure(&text, px).ceil() as u32).min(room as u32);
    Some(FittedText { text, px, width })
}

/// Draw fitted text starting at `left`, its capital letters centered in the strip so it
/// lines up with the middle of the QR code.
fn draw_text(image: &mut RgbImage, fitted: &FittedText, left: u32, top: u32, strip: u32) {
    let font = crate::design_render::default_font(false);
    let px = fitted.px;
    let cap = font.metrics('H', px).height as f32;
    let baseline = top as f32 + (strip as f32 + cap) / 2.;
    let right = left + fitted.width;
    let mut pen = left as f32;
    for c in fitted.text.chars() {
        let (metrics, bitmap) = font.rasterize(c, px);
        let gx = pen.round() as i64 + metrics.xmin as i64;
        let gy = baseline.round() as i64 - metrics.height as i64 - metrics.ymin as i64;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let coverage = bitmap[row * metrics.width + col];
                let (x, y) = (gx + col as i64, gy + row as i64);
                if coverage == 0
                    || x < left as i64
                    || x > right as i64
                    || x >= image.width() as i64
                    || y < 0
                    || y >= image.height() as i64
                {
                    continue;
                }
                let pixel = image.get_pixel_mut(x as u32, y as u32);
                for channel in pixel.0.iter_mut() {
                    *channel = (*channel).min(255 - coverage);
                }
            }
        }
        pen += metrics.advance_width;
    }
}

/// Add the footer to a rendered design, inside the printable area: `edge_mm` is the
/// distance kept from the label's sides and bottom (the larger of the safe margin and the
/// printer's unprintable border). The footer goes only where the design is blank, at the
/// chosen alignment; otherwise the design is left untouched and a warning explains what was
/// left out and why. The label's size never changes.
pub fn add_to_design(output: &mut Output, footer: &Footer, edge_mm: f32) -> Result<()> {
    let dpi = output.dpi;
    let (w, h) = output.image.dimensions();
    let edge = dots(edge_mm.max(DESIGN_EDGE_MM), dpi);
    if !footer.is_active() {
        return Ok(());
    }
    if edge * 2 >= w {
        output.warnings.push(FOOTER_NO_ROOM.into());
        return Ok(());
    }
    let (x0, x1, bottom) = (edge, w - edge, h - edge);
    let pad = dots(GAP_MM, dpi);
    // The footer's box, a little clearance at its sides, and the margin under it, which the
    // QR code's border may use, must all be blank. If the QR code does not fit, the message
    // alone may.
    let image = &output.image;
    let fits = |choice: &Footer| -> Option<()> {
        let reserved = choice.reserved_dots(dpi, h, edge);
        if reserved == 0 || reserved + edge >= h {
            return None;
        }
        let place = choice.place(dpi, w, h, x0, x1, bottom, edge)?;
        place.text.as_ref()?;
        let from = place.left.saturating_sub(pad);
        let to = (place.left + place.width + pad).min(w);
        (bottom - reserved..h)
            .all(|y| (from..to).all(|x| image.get_pixel(x, y).0.iter().all(|&v| v > 245)))
            .then_some(())
    };
    let Some(chosen) = footer
        .choices()
        .into_iter()
        .find(|choice| fits(choice).is_some())
    else {
        let too_small = footer
            .choices()
            .iter()
            .all(|c| c.reserved_dots(dpi, h, edge) + edge >= h);
        output.warnings.push(
            if too_small {
                FOOTER_NO_ROOM
            } else {
                FOOTER_BLOCKED
            }
            .into(),
        );
        return Ok(());
    };
    if let Some(note) = footer.fallback_note(&chosen, dpi, h) {
        output.warnings.push(note.into());
    }
    chosen.draw(&mut output.image, dpi, x0, x1, bottom, edge);
    Ok(())
}

pub const FOOTER_BLOCKED: &str = "The footer was left out: part of the design is in the blank space it needs at the bottom of the label. Move objects up, choose another alignment, or turn the footer off. The label size is unchanged.";
pub const QR_NO_SPACE: &str = "The footer QR code was left out because there is not enough blank space at the bottom of this label for it. The message still prints.";
pub const QR_LEFT_OUT: &str = "The footer QR code was left out because it would be large on this short label. The message still prints.";
pub const FOOTER_NO_ROOM: &str =
    "The footer was left out: the label is too small for it inside the printable area.";

#[cfg(test)]
mod tests {
    use super::*;

    fn on(qr: bool) -> Footer {
        Footer {
            message_enabled: true,
            qr_enabled: qr,
            ..Footer::default()
        }
    }
    fn ink(image: &RgbImage, rows: std::ops::Range<u32>) -> usize {
        rows.flat_map(|y| (0..image.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| image.get_pixel(x, y).0[0] < 128)
            .count()
    }

    #[test]
    fn off_by_default_and_qr_needs_the_message() {
        let footer = Footer::default();
        assert!(!footer.is_active());
        assert_eq!(footer.reserved_dots(203, 1218, 16), 0);
        let qr_only = Footer {
            qr_enabled: true,
            ..Footer::default()
        };
        assert!(!qr_only.is_active() && !qr_only.shows_qr());
        assert_eq!(qr_only.reserved_dots(203, 1218, 16), 0);
        let blank_message = Footer {
            message: "   ".into(),
            ..on(true)
        };
        assert!(!blank_message.is_active());
        assert!(on(true).shows_qr());
        assert!(!on(false).shows_qr());
    }

    #[test]
    fn the_footer_stays_small_next_to_the_label() {
        for dpi in [203, 300, 600] {
            let label_h = dots(152.4, dpi);
            let margin = dots(2., dpi);
            let text = on(false).reserved_dots(dpi, label_h, margin);
            let qr = on(true).reserved_dots(dpi, label_h, margin);
            assert!(qr > text, "{dpi}");
            assert!(qr as f32 <= label_h as f32 * 0.1, "{dpi}");
            // The QR code's border shares the white margin under it.
            assert!(qr < on(true).reserved_dots(dpi, label_h, 0), "{dpi}");
            assert!(Footer::module(dpi) as f32 / dpi as f32 * 25.4 >= 0.24);
        }
        // On a short label the QR code is left out; the message remains.
        let short = dots(30., 203);
        assert!(on(true).qr_left_out(203, short));
        assert_eq!(
            on(true).reserved_dots(203, short, 16),
            on(false).reserved_dots(203, short, 16)
        );
    }

    /// The columns holding ink inside rows `rows`.
    fn ink_columns(image: &RgbImage, rows: std::ops::Range<u32>) -> Vec<u32> {
        (0..image.width())
            .filter(|&x| rows.clone().any(|y| image.get_pixel(x, y).0[0] < 128))
            .collect()
    }

    #[test]
    fn the_qr_code_comes_first_with_a_gap_before_the_centered_message() {
        let dpi = 203;
        let mut image = RgbImage::from_pixel(800, 1200, Rgb([255; 3]));
        let footer = on(true);
        footer.draw(&mut image, dpi, 10, 790, 1190, 10);
        let reserved = footer.reserved_dots(dpi, 1200, 10);
        assert!(
            reserved > on(false).reserved_dots(dpi, 1200, 10),
            "QR code included"
        );
        let rows = 1190 - reserved..1200;
        // The QR code's border is white: the top of the reserved space stays blank.
        let quiet = QR_QUIET * Footer::module(dpi);
        assert_eq!(ink(&image, 0..rows.start + quiet), 0);
        let columns = ink_columns(&image, rows.clone());
        // QR code first, then a run of white columns, then the message.
        let qr_code = footer.qr(dpi, 1200).unwrap();
        let qr_end = columns[0] + qr_code.width() as u32 * Footer::module(dpi);
        let text_start = *columns.iter().find(|&&x| x >= qr_end).unwrap();
        let gap_mm = (text_start - qr_end) as f32 / dpi as f32 * 25.4;
        assert!((1.0..=2.5).contains(&gap_mm), "gap {gap_mm} mm");
        // The message's capitals are centered on the QR code's middle row.
        let text_rows: Vec<u32> = rows
            .clone()
            .filter(|&y| (text_start..text_start + 8).any(|x| image.get_pixel(x, y).0[0] < 128))
            .collect();
        let text_middle = (text_rows[0] + text_rows[text_rows.len() - 1]) / 2;
        let qr_middle = rows.start + quiet + qr_code.width() as u32 * Footer::module(dpi) / 2;
        assert!(
            text_middle.abs_diff(qr_middle) <= 2,
            "{text_middle} vs {qr_middle}"
        );
        // At the left, the QR code's white border against the left edge, and the right side
        // of the label stays clear.
        assert_eq!(columns[0], 10 + quiet);
        assert!(*columns.last().unwrap() < 600);
    }

    #[test]
    fn a_message_alone_also_sits_at_the_left() {
        let mut image = RgbImage::from_pixel(800, 1200, Rgb([255; 3]));
        on(false).draw(&mut image, 203, 10, 790, 1190, 10);
        assert!(ink_columns(&image, 1100..1190)[0] <= 16);
    }

    #[test]
    fn the_whole_footer_follows_each_alignment() {
        for qr in [true, false] {
            let mut spans = vec![];
            for align in FooterAlign::ALL {
                let mut image = RgbImage::from_pixel(800, 1200, Rgb([255; 3]));
                Footer { align, ..on(qr) }.draw(&mut image, 203, 10, 790, 1190, 10);
                let columns = ink_columns(&image, 1000..1200);
                spans.push((columns[0], *columns.last().unwrap()));
            }
            let [(l0, l1), (c0, c1), (r0, r1)] = spans[..] else {
                unreachable!()
            };
            // The same footer, QR code included, just moved along the bottom.
            assert!((l1 - l0).abs_diff(c1 - c0) <= 1, "{qr}: {spans:?}");
            assert!((l1 - l0).abs_diff(r1 - r0) <= 1, "{qr}: {spans:?}");
            assert!(l0 < 40, "left {qr}: {spans:?}");
            assert!(
                ((c0 + c1) / 2).abs_diff(400) <= 12,
                "center {qr}: {spans:?}"
            );
            assert!(r1 > 770 && r1 < 790, "right {qr}: {spans:?}");
            // With a QR code, it still comes first at every alignment.
            if qr {
                let mut image = RgbImage::from_pixel(800, 1200, Rgb([255; 3]));
                let footer = Footer {
                    align: FooterAlign::Right,
                    ..on(true)
                };
                footer.draw(&mut image, 203, 10, 790, 1190, 10);
                assert_eq!(decode(&image).as_deref(), Some(DEFAULT_QR_TEXT));
            }
        }
    }

    #[test]
    fn designs_place_the_footer_where_the_chosen_alignment_is_blank() {
        let busy = || {
            let mut output = Output {
                image: RgbImage::from_pixel(812, 1218, Rgb([255; 3])),
                warnings: vec![],
                clipped: false,
                width_mm: 101.6,
                height_mm: 152.4,
                dpi: 203,
            };
            // Design content in the bottom-left corner only.
            for y in 1150..1200 {
                for x in 20..200 {
                    output.image.put_pixel(x, y, Rgb([0; 3]));
                }
            }
            output
        };
        let mut left = busy();
        let before = left.image.clone();
        add_to_design(&mut left, &on(true), 0.).unwrap();
        assert_eq!(left.image, before, "never drawn over the design");
        assert_eq!(left.warnings, vec![FOOTER_BLOCKED.to_string()]);
        for align in [FooterAlign::Center, FooterAlign::Right] {
            let mut output = busy();
            add_to_design(&mut output, &Footer { align, ..on(true) }, 0.).unwrap();
            assert!(output.warnings.is_empty(), "{align:?}");
            assert_eq!(decode(&output.image).as_deref(), Some(DEFAULT_QR_TEXT));
            // The label keeps its size.
            assert_eq!(output.image.dimensions(), (812, 1218));
        }
    }

    #[test]
    fn the_footer_stays_inside_the_printable_area() {
        let mut output = Output {
            image: RgbImage::from_pixel(812, 1218, Rgb([255; 3])),
            warnings: vec![],
            clipped: false,
            width_mm: 101.6,
            height_mm: 152.4,
            dpi: 203,
        };
        let edge = dots(5., 203);
        add_to_design(
            &mut output,
            &Footer {
                align: FooterAlign::Right,
                ..on(true)
            },
            5.,
        )
        .unwrap();
        let columns = ink_columns(&output.image, 0..1218);
        assert!(columns[0] >= edge && *columns.last().unwrap() < 812 - edge);
        assert_eq!(ink(&output.image, 1218 - edge..1218), 0);
    }

    #[test]
    fn a_message_alone_keeps_clear_of_the_label_edges_without_a_margin() {
        // No margin: the message still stays about 1.5 mm from the left and bottom edges.
        let dpi = 203;
        let edge = dots(TEXT_EDGE_MM, dpi);
        let mut image = RgbImage::from_pixel(812, 1218, Rgb([255; 3]));
        on(false).draw(&mut image, dpi, 0, 812, 1218, 0);
        let columns = ink_columns(&image, 1100..1218);
        assert!(columns[0] >= edge, "left {}", columns[0]);
        let last_row = (1100..1218)
            .rev()
            .find(|&y| (0..812).any(|x| image.get_pixel(x, y).0[0] < 128))
            .unwrap();
        assert!(1218 - 1 - last_row >= edge - 1, "bottom {last_row}");
        // The space reserved includes that padding.
        assert!(on(false).reserved_dots(dpi, 1218, 0) > on(false).reserved_dots(dpi, 1218, 16));
    }

    fn decode(image: &RgbImage) -> Option<String> {
        let (w, h) = (image.width() as usize, image.height() as usize);
        let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
            image.get_pixel(x as u32, y as u32).0[0]
        });
        let grids = prepared.detect_grids();
        let (_, content) = grids.first()?.decode().ok()?;
        Some(content)
    }

    #[test]
    fn the_qr_code_scans_at_every_resolution() {
        for dpi in [203, 300, 600] {
            let (w, h) = (dots(101.6, dpi), dots(152.4, dpi));
            let mut image = RgbImage::from_pixel(w, h, Rgb([255; 3]));
            let footer = Footer {
                qr_text: "https://example.com/shop?id=42".into(),
                ..on(true)
            };
            footer.draw(&mut image, dpi, 0, w, h, 0);
            assert_eq!(
                decode(&image).as_deref(),
                Some("https://example.com/shop?id=42"),
                "{dpi}"
            );
        }
        // The default link scans too.
        let mut image = RgbImage::from_pixel(812, 1218, Rgb([255; 3]));
        on(true).draw(&mut image, 203, 0, 812, 1218, 0);
        assert_eq!(decode(&image).as_deref(), Some(DEFAULT_QR_TEXT));
    }

    #[test]
    fn long_messages_are_shortened_to_fit() {
        let footer = Footer {
            message: "W".repeat(80),
            ..on(false)
        };
        let mut image = RgbImage::from_pixel(200, 100, Rgb([255; 3]));
        footer.draw(&mut image, 203, 20, 180, 95, 5);
        for y in 0..100 {
            for x in (0..20).chain(180..200) {
                assert_eq!(image.get_pixel(x, y).0, [255; 3]);
            }
        }
    }

    #[test]
    fn designs_get_the_footer_only_when_the_bottom_is_blank() {
        let blank = || Output {
            image: RgbImage::from_pixel(812, 1218, Rgb([255; 3])),
            warnings: vec![],
            clipped: false,
            width_mm: 101.6,
            height_mm: 152.4,
            dpi: 203,
        };
        let mut output = blank();
        add_to_design(&mut output, &on(true), 0.).unwrap();
        assert!(output.warnings.is_empty());
        assert!(ink(&output.image, 1100..1218) > 0);

        let mut busy = blank();
        for x in 100..700 {
            busy.image.put_pixel(x, 1200, Rgb([0; 3]));
        }
        let before = busy.image.clone();
        add_to_design(&mut busy, &on(true), 0.).unwrap();
        assert_eq!(busy.image, before);
        assert_eq!(busy.warnings, vec![FOOTER_BLOCKED.to_string()]);

        let mut untouched = blank();
        add_to_design(&mut untouched, &Footer::default(), 0.).unwrap();
        assert_eq!(untouched.image, blank().image);
        assert!(untouched.warnings.is_empty());
    }

    #[test]
    fn designs_fall_back_to_the_message_alone_when_the_qr_code_does_not_fit() {
        let mut output = Output {
            image: RgbImage::from_pixel(812, 1218, Rgb([255; 3])),
            warnings: vec![],
            clipped: false,
            width_mm: 101.6,
            height_mm: 152.4,
            dpi: 203,
        };
        // A line of the design 50 rows above the footer's bottom: too close for the QR code,
        // far enough for the message.
        let edge = dots(DESIGN_EDGE_MM, 203);
        let line = 1218 - edge - 50;
        for x in 100..700 {
            output.image.put_pixel(x, line, Rgb([0; 3]));
        }
        add_to_design(&mut output, &on(true), 0.).unwrap();
        assert_eq!(output.warnings, vec![QR_NO_SPACE.to_string()]);
        assert!(ink(&output.image, line + 1..1218) > 0, "message printed");
        assert!(decode(&output.image).is_none(), "QR code printed");
    }

    #[test]
    fn cleaning_bounds_loaded_text() {
        let mut footer = Footer {
            message: format!("a\nb{}", "x".repeat(200)),
            qr_text: "y".repeat(500),
            ..Footer::default()
        };
        footer.clean();
        assert!(!footer.message.contains('\n'));
        assert_eq!(footer.message.chars().count(), MAX_MESSAGE_CHARS);
        assert_eq!(footer.qr_text.chars().count(), MAX_QR_CHARS);
    }
}
