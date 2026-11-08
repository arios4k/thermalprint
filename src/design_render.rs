//! Deterministic rasterization of editable labels at the requested printer DPI.
//! Preview textures are downsampled from this output by the UI.
use crate::{
    design::{
        BarcodeKind, DesignObject, Element, ImageMode, LabelDocument, ObjectOptions, QrLevel,
        ShapeKind, TextAlign,
    },
    layout::Output,
};
use anyhow::{Context, Result, bail};
use barcoders::sym::{
    codabar::Codabar, code39::Code39, code93::Code93, code128::Code128, ean8::EAN8, ean13::EAN13,
    tf::TF,
};
use base64::Engine as _;
use fontdue::{Font, FontSettings};
use image::{Rgb, RgbImage, Rgba, RgbaImage, imageops};
use qrcode::{Color, QrCode};
fn dots(mm: f32, dpi: u32) -> u32 {
    (mm * dpi as f32 / 25.4).round().max(0.) as u32
}
fn black() -> Rgba<u8> {
    Rgba([0, 0, 0, 255])
}
fn put(im: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < im.width() && (y as u32) < im.height() {
        im.put_pixel(x as u32, y as u32, color);
    }
}
fn fill(im: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    for y in y0.max(0)..y1.min(im.height() as i32) {
        for x in x0.max(0)..x1.min(im.width() as i32) {
            im.put_pixel(x as u32, y as u32, color);
        }
    }
}
fn line(im: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, thick: i32) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for n in 0..=steps {
        let x = x0 + (x1 - x0) * n / steps;
        let y = y0 + (y1 - y0) * n / steps;
        fill(
            im,
            x - thick / 2,
            y - thick / 2,
            x + (thick + 1) / 2,
            y + (thick + 1) / 2,
            black(),
        );
    }
}
fn draw_shape(im: &mut RgbaImage, kind: ShapeKind, stroke_mm: f32, filled: bool, dpi: u32) {
    let w = im.width() as i32;
    let h = im.height() as i32;
    let t = dots(stroke_mm, dpi).max(1) as i32;
    match kind {
        ShapeKind::Rectangle => {
            if filled {
                fill(im, 0, 0, w, h, black());
            } else {
                fill(im, 0, 0, w, t, black());
                fill(im, 0, h - t, w, h, black());
                fill(im, 0, 0, t, h, black());
                fill(im, w - t, 0, w, h, black());
            }
        }
        ShapeKind::Ellipse => {
            let cx = w as f32 / 2.;
            let cy = h as f32 / 2.;
            let rx = cx.max(1.);
            let ry = cy.max(1.);
            let inner_rx = (rx - t as f32).max(0.);
            let inner_ry = (ry - t as f32).max(0.);
            for y in 0..h {
                for x in 0..w {
                    let dx = (x as f32 + 0.5 - cx) / rx;
                    let dy = (y as f32 + 0.5 - cy) / ry;
                    let outer = dx * dx + dy * dy <= 1.;
                    let inner = if inner_rx > 0. && inner_ry > 0. {
                        let dx = (x as f32 + 0.5 - cx) / inner_rx;
                        let dy = (y as f32 + 0.5 - cy) / inner_ry;
                        dx * dx + dy * dy < 1.
                    } else {
                        false
                    };
                    if outer && (filled || !inner) {
                        put(im, x, y, black());
                    }
                }
            }
        }
        ShapeKind::Line => line(im, 0, 0, w - 1, h - 1, t),
        ShapeKind::DividerHorizontal => line(im, 0, h / 2, w - 1, h / 2, t),
        ShapeKind::DividerVertical => line(im, w / 2, 0, w / 2, h - 1, t),
    }
}
fn font(bold: bool) -> &'static Font {
    static REGULAR: std::sync::OnceLock<Font> = std::sync::OnceLock::new();
    static BOLD: std::sync::OnceLock<Font> = std::sync::OnceLock::new();
    if bold {
        BOLD.get_or_init(|| {
            Font::from_bytes(
                include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf") as &[u8],
                FontSettings::default(),
            )
            .expect("bundled bold font")
        })
    } else {
        REGULAR.get_or_init(|| {
            Font::from_bytes(
                include_bytes!("../assets/fonts/DejaVuSans.ttf") as &[u8],
                FontSettings::default(),
            )
            .expect("bundled font")
        })
    }
}
/// Replace `${date}`, `${date:FORMAT}`, `${serial}`, and `${serial:WIDTH}`. A date FORMAT uses
/// strftime codes such as `%d/%m/%Y`; WIDTH zero-pads the serial number. Anything else
/// (including an invalid format) is left as written.
fn expand(text: &str, serial: u64) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        match replacement(&after[..end], serial) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[start..start + 2 + end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
fn replacement(token: &str, serial: u64) -> Option<String> {
    let (name, argument) = match token.split_once(':') {
        Some((name, argument)) => (name, Some(argument)),
        None => (token, None),
    };
    match (name, argument) {
        ("date", None) => Some(chrono::Local::now().format("%Y-%m-%d").to_string()),
        ("date", Some(format)) => format_date(format),
        ("serial", None) => Some(serial.to_string()),
        ("serial", Some(width)) => width
            .parse::<usize>()
            .ok()
            .filter(|width| (1..=12).contains(width))
            .map(|width| format!("{serial:0width$}")),
        _ => None,
    }
}
/// Today's date in a strftime format, or None when the format is invalid or very long.
pub fn format_date(format: &str) -> Option<String> {
    use chrono::format::{Item, StrftimeItems};
    if format.is_empty() || format.len() > 40 {
        return None;
    }
    let items: Vec<Item> = StrftimeItems::new(format).collect();
    if items.iter().any(|item| matches!(item, Item::Error)) {
        return None;
    }
    Some(
        chrono::Local::now()
            .format_with_items(items.into_iter())
            .to_string(),
    )
}
fn advance(font: &Font, text: &str, px: f32) -> f32 {
    text.chars()
        .map(|c| font.metrics(c, px).advance_width)
        .sum()
}
/// Break text into lines. Without `wrap` only explicit line breaks apply; with it, lines are
/// also broken at spaces (and inside a word that is wider than the box).
fn layout_text(text: &str, font: &Font, px: f32, width: f32, wrap: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        if !wrap || advance(font, paragraph, px) <= width {
            lines.push(paragraph.to_owned());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split(' ') {
            let candidate = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{current} {word}")
            };
            if advance(font, &candidate, px) <= width {
                current = candidate;
                continue;
            }
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            if advance(font, word, px) <= width {
                current = word.to_owned();
            } else {
                // A single word wider than the box is broken between letters.
                for c in word.chars() {
                    let mut grown = current.clone();
                    grown.push(c);
                    if !current.is_empty() && advance(font, &grown, px) > width {
                        lines.push(std::mem::take(&mut current));
                    }
                    current.push(c);
                }
            }
        }
        lines.push(current);
    }
    lines
}
fn text_height(lines: usize, px: f32, line_height: i32) -> f32 {
    lines.saturating_sub(1) as f32 * line_height as f32 + px * 1.2
}
#[allow(clippy::too_many_arguments)]
fn draw_text(
    im: &mut RgbaImage,
    text: &str,
    font_pt: f32,
    bold: bool,
    align: TextAlign,
    dpi: u32,
    serial: u64,
    options: &ObjectOptions,
) -> Result<()> {
    let f = crate::fonts::lookup(&options.font).unwrap_or_else(|| font(bold));
    let expanded = expand(text, serial);
    let width = im.width() as f32;
    let spacing = options.line_spacing.clamp(0.5, 3.);
    let measure = |pt: f32| {
        let px = pt * dpi as f32 / 72.;
        let line_height = (px * 1.32 * spacing).ceil() as i32;
        (
            px,
            line_height,
            layout_text(&expanded, f, px, width, options.wrap),
        )
    };
    let (mut px, mut line_height, mut lines) = measure(font_pt);
    if options.fit {
        let mut pt = font_pt;
        loop {
            (px, line_height, lines) = measure(pt);
            let widest = lines.iter().map(|l| advance(f, l, px)).fold(0., f32::max);
            if (widest <= width + 0.5
                && text_height(lines.len(), px, line_height) <= im.height() as f32)
                || pt <= 3.
            {
                break;
            }
            pt = (pt - 0.5).max(3.);
        }
    }
    if options.inverse {
        for pixel in im.pixels_mut() {
            *pixel = black();
        }
    }
    let shear = if options.italic { 0.21 } else { 0. };
    for (row, line_text) in lines.iter().enumerate() {
        let advances = advance(f, line_text, px);
        let start = match align {
            TextAlign::Left => 0,
            TextAlign::Center => ((width - advances) / 2.).max(0.) as i32,
            TextAlign::Right => (width - advances).max(0.) as i32,
        };
        let baseline = (row as i32) * line_height + px.ceil() as i32;
        let mut pen = start as f32;
        for c in line_text.chars() {
            let (m, bits) = f.rasterize(c, px);
            let left = pen.round() as i32 + m.xmin;
            let top = baseline - m.height as i32 - m.ymin;
            for yy in 0..m.height {
                for xx in 0..m.width {
                    let a = bits[yy * m.width + xx];
                    if a > 0 {
                        let y = top + yy as i32;
                        // Italic leans each glyph right above the baseline.
                        let x = left + xx as i32 + ((baseline - y) as f32 * shear).round() as i32;
                        let color = if options.inverse {
                            Rgba([a, a, a, 255])
                        } else {
                            Rgba([0, 0, 0, a])
                        };
                        put(im, x, y, color);
                    }
                }
            }
            pen += m.advance_width;
        }
    }
    Ok(())
}
fn encoded(kind: BarcodeKind, data: &str) -> Result<Vec<u8>> {
    if data.is_empty() {
        bail!("Barcode data is empty");
    }
    match kind {
        BarcodeKind::Code128 => {
            if !data.bytes().all(|b| (32..=126).contains(&b)) {
                bail!("Code 128 currently accepts printable ASCII");
            }
            Ok(Code128::new(format!("Ɓ{data}"))
                .map_err(|e| anyhow::anyhow!("Code 128: {e:?}"))?
                .encode())
        }
        BarcodeKind::Code39 => Ok(Code39::new(data)
            .map_err(|e| anyhow::anyhow!("Code 39: {e:?}"))?
            .encode()),
        BarcodeKind::Ean13 => Ok(EAN13::new(data)
            .map_err(|e| anyhow::anyhow!("EAN-13: {e:?}"))?
            .encode()),
        BarcodeKind::UpcA => {
            if !data.bytes().all(|b| b.is_ascii_digit()) || ![11, 12].contains(&data.len()) {
                bail!("UPC-A requires 11 or 12 digits");
            }
            Ok(EAN13::new(format!("0{data}"))
                .map_err(|e| anyhow::anyhow!("UPC-A: {e:?}"))?
                .encode())
        }
        BarcodeKind::Ean8 => {
            if !data.bytes().all(|b| b.is_ascii_digit()) || ![7, 8].contains(&data.len()) {
                bail!("EAN-8 requires 7 digits, or 8 including the check digit");
            }
            Ok(EAN8::new(data)
                .map_err(|e| anyhow::anyhow!("EAN-8: {e:?}"))?
                .encode())
        }
        BarcodeKind::Itf => {
            if !data.bytes().all(|b| b.is_ascii_digit()) || !data.len().is_multiple_of(2) {
                bail!("ITF requires an even number of digits");
            }
            Ok(TF::interleaved(data)
                .map_err(|e| anyhow::anyhow!("ITF: {e:?}"))?
                .encode())
        }
        BarcodeKind::Codabar => {
            let bytes = data.as_bytes();
            let framed = bytes.len() >= 3
                && b"ABCD".contains(&bytes[0])
                && b"ABCD".contains(&bytes[bytes.len() - 1])
                && bytes[1..bytes.len() - 1]
                    .iter()
                    .all(|b| b.is_ascii_digit() || b"-$:/.+".contains(b));
            if !framed {
                bail!(
                    "Codabar must start and end with A, B, C, or D and use digits or - $ : / . + between"
                );
            }
            Ok(Codabar::new(data)
                .map_err(|e| anyhow::anyhow!("Codabar: {e:?}"))?
                .encode())
        }
        BarcodeKind::Code93 => Ok(Code93::new(data)
            .map_err(|e| anyhow::anyhow!("Code 93: {e:?}"))?
            .encode()),
        BarcodeKind::Gs1128 => {
            let fields = crate::gs1::parse(data)?;
            Ok(Code128::new(crate::gs1::code128_symbol(&fields)?)
                .map_err(|e| anyhow::anyhow!("GS1-128: {e:?}"))?
                .encode())
        }
        BarcodeKind::DataMatrix | BarcodeKind::Gs1DataMatrix => {
            bail!("Data Matrix is a 2D symbol")
        }
    }
}
fn draw_data_matrix(im: &mut RgbaImage, data: &str, gs1: bool) -> Result<()> {
    // Label objects are boxes, so only square symbols are used; the default list
    // would pick a thin rectangle for short text.
    let squares = datamatrix::SymbolList::default().enforce_square();
    let code = if gs1 {
        let fields = crate::gs1::parse(data)?;
        let payload = crate::gs1::data_matrix_payload(&fields)?;
        datamatrix::DataMatrix::encode_gs1(payload.as_bytes(), squares)
    } else {
        datamatrix::DataMatrix::encode_str(data, squares)
    }
    .map_err(|e| anyhow::anyhow!("Data Matrix data cannot be encoded: {e:?}"))?;
    let bitmap = code.bitmap();
    // Two quiet-zone modules on every side keep the symbol scannable.
    let modules = bitmap.width().max(bitmap.height()) as u32 + 4;
    let module = im.width().min(im.height()) / modules;
    if module < 2 {
        bail!("Data Matrix is too small; enlarge the object or use shorter data");
    }
    let left = (im.width() - modules * module) / 2 + 2 * module;
    let top = (im.height() - modules * module) / 2 + 2 * module;
    for (x, y) in bitmap.pixels() {
        fill(
            im,
            (left + x as u32 * module) as i32,
            (top + y as u32 * module) as i32,
            (left + (x as u32 + 1) * module) as i32,
            (top + (y as u32 + 1) * module) as i32,
            black(),
        );
    }
    Ok(())
}
/// The check digit EAN-13, EAN-8, and UPC-A add to a value that has none yet.
pub fn check_digit(kind: BarcodeKind, data: &str) -> Option<char> {
    let (body_len, odd_weight_first) = match kind {
        BarcodeKind::Ean13 => (12, true),
        BarcodeKind::Ean8 => (7, false),
        BarcodeKind::UpcA => (11, false),
        _ => return None,
    };
    if data.len() < body_len || !data.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let sum: u32 = data.as_bytes()[..body_len]
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let weight = if (i % 2 == 0) == odd_weight_first {
                1
            } else {
                3
            };
            (b - b'0') as u32 * weight
        })
        .sum();
    char::from_digit((10 - sum % 10) % 10, 10)
}
/// The value as printed under the bars: retail codes show their check digit.
fn readable_value(kind: BarcodeKind, data: &str) -> String {
    if kind == BarcodeKind::Gs1128
        && let Ok(fields) = crate::gs1::parse(data)
    {
        return crate::gs1::human(&fields);
    }
    let body_len = match kind {
        BarcodeKind::Ean13 => 12,
        BarcodeKind::Ean8 => 7,
        BarcodeKind::UpcA => 11,
        _ => return data.to_owned(),
    };
    match check_digit(kind, data) {
        Some(digit) if data.len() == body_len => format!("{data}{digit}"),
        _ => data.to_owned(),
    }
}
fn draw_barcode(
    im: &mut RgbaImage,
    kind: BarcodeKind,
    data: &str,
    show_text: bool,
    dpi: u32,
    serial: u64,
    options: &ObjectOptions,
) -> Result<()> {
    let data = expand(data, serial);
    if matches!(kind, BarcodeKind::DataMatrix | BarcodeKind::Gs1DataMatrix) {
        if data.is_empty() {
            bail!("Barcode data is empty");
        }
        return draw_data_matrix(im, &data, kind == BarcodeKind::Gs1DataMatrix);
    }
    let bits = encoded(kind, &data)?;
    let module = (im.width() / (bits.len() as u32 + 20)).min(12);
    if module < 2 {
        bail!("Barcode is too narrow at {dpi} DPI. Widen it or use a shorter value.");
    }
    let barcode_w = (bits.len() as u32 + 20) * module;
    let left = (im.width() - barcode_w) / 2 + 10 * module;
    // The readable text is 8 pt in at most 4 mm unless a size is chosen.
    let (text_pt, text_h) = if !show_text {
        (0., 0)
    } else if options.barcode_text_pt > 0. {
        let pt = options.barcode_text_pt;
        (
            pt,
            ((pt * dpi as f32 / 72.) * 1.4)
                .ceil()
                .min((im.height() / 2) as f32) as u32,
        )
    } else {
        (8., dots(4., dpi).min(im.height() / 3))
    };
    let (bars_top, text_y) = if options.text_above {
        (text_h, 0)
    } else {
        (0, im.height() - text_h)
    };
    for (i, b) in bits.iter().enumerate() {
        if *b != 0 {
            fill(
                im,
                (left + i as u32 * module) as i32,
                bars_top as i32,
                (left + (i as u32 + 1) * module) as i32,
                (bars_top + im.height() - text_h) as i32,
                black(),
            );
        }
    }
    if show_text {
        let mut layer = RgbaImage::new(im.width(), text_h);
        draw_text(
            &mut layer,
            &readable_value(kind, &data),
            text_pt,
            false,
            TextAlign::Center,
            dpi,
            serial,
            &ObjectOptions::default(),
        )?;
        imageops::overlay(im, &layer, 0, text_y as i64);
    }
    Ok(())
}
fn draw_qr(im: &mut RgbaImage, data: &str, serial: u64, level: QrLevel) -> Result<()> {
    let level = match level {
        QrLevel::Low => qrcode::EcLevel::L,
        QrLevel::Medium => qrcode::EcLevel::M,
        QrLevel::Quartile => qrcode::EcLevel::Q,
        QrLevel::High => qrcode::EcLevel::H,
    };
    let code = QrCode::with_error_correction_level(expand(data, serial).as_bytes(), level)
        .context("QR data cannot be encoded")?;
    let modules = code.width() as u32 + 8;
    let module = im.width().min(im.height()) / modules;
    if module < 2 {
        bail!("QR code is too small; enlarge the object");
    }
    let left = (im.width() - modules * module) / 2 + 4 * module;
    let top = (im.height() - modules * module) / 2 + 4 * module;
    for y in 0..code.width() {
        for x in 0..code.width() {
            if code[(x, y)] == Color::Dark {
                fill(
                    im,
                    (left + x as u32 * module) as i32,
                    (top + y as u32 * module) as i32,
                    (left + (x as u32 + 1) * module) as i32,
                    (top + (y as u32 + 1) * module) as i32,
                    black(),
                );
            }
        }
    }
    Ok(())
}
/// Reduce an image to black and white for a thermal printer, or adjust its contrast.
/// Fully transparent pixels stay transparent. Returns whether anything was changed.
fn adjust_image(image: &mut RgbaImage, options: &ObjectOptions) {
    let neutral = options.contrast == 0. && !options.invert;
    if options.image_mode == ImageMode::Original && neutral {
        return;
    }
    let contrast = options.contrast.clamp(-1., 1.) * 255.;
    let factor = (259. * (contrast + 255.)) / (255. * (259. - contrast));
    let (width, height) = (image.width() as usize, image.height() as usize);
    // Gray values over a white background, after contrast and inversion.
    let mut gray = vec![255f32; width * height];
    let mut opaque = vec![false; width * height];
    for (index, pixel) in image.pixels().enumerate() {
        let alpha = pixel[3] as f32 / 255.;
        opaque[index] = pixel[3] > 0;
        let luma = 0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32;
        let over_white = luma * alpha + 255. * (1. - alpha);
        let mut value = (factor * (over_white - 128.) + 128.).clamp(0., 255.);
        if options.invert {
            value = 255. - value;
        }
        gray[index] = value;
    }
    match options.image_mode {
        ImageMode::Original => {}
        ImageMode::Threshold => {
            for value in &mut gray {
                *value = if *value < options.threshold as f32 {
                    0.
                } else {
                    255.
                };
            }
        }
        ImageMode::Dither => {
            // Floyd-Steinberg error diffusion.
            for y in 0..height {
                for x in 0..width {
                    let index = y * width + x;
                    let old = gray[index];
                    let new = if old < 128. { 0. } else { 255. };
                    gray[index] = new;
                    let error = old - new;
                    let mut spread = |dx: i32, dy: usize, share: f32| {
                        let nx = x as i32 + dx;
                        if nx >= 0 && (nx as usize) < width && y + dy < height {
                            let target = (y + dy) * width + nx as usize;
                            gray[target] = (gray[target] + error * share).clamp(0., 255.);
                        }
                    };
                    spread(1, 0, 7. / 16.);
                    spread(-1, 1, 3. / 16.);
                    spread(0, 1, 5. / 16.);
                    spread(1, 1, 1. / 16.);
                }
            }
        }
    }
    for (index, pixel) in image.pixels_mut().enumerate() {
        let value = gray[index].round() as u8;
        *pixel = Rgba([value, value, value, if opaque[index] { 255 } else { 0 }]);
    }
}
fn draw_image(im: &mut RgbaImage, data: &str, options: &ObjectOptions) -> Result<()> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    if bytes.len() > 9_000_000 {
        bail!("Image exceeds 9 MB");
    }
    let src = image::load_from_memory(&bytes)?.to_rgba8();
    let fit =
        (im.width() as f32 / src.width() as f32).min(im.height() as f32 / src.height() as f32);
    let w = (src.width() as f32 * fit).round().max(1.) as u32;
    let h = (src.height() as f32 * fit).round().max(1.) as u32;
    let mut scaled = imageops::resize(&src, w, h, imageops::FilterType::Lanczos3);
    // Adjust after scaling so dithering happens at the printed resolution.
    adjust_image(&mut scaled, options);
    imageops::overlay(
        im,
        &scaled,
        ((im.width() - w) / 2) as i64,
        ((im.height() - h) / 2) as i64,
    );
    Ok(())
}
fn render_object(o: &DesignObject, dpi: u32, serial: u64) -> Result<RgbaImage> {
    let w = dots(o.width_mm, dpi).max(1);
    let h = dots(o.height_mm, dpi).max(1);
    if w as u64 * h as u64 > 36_000_000 {
        bail!("Object exceeds render limit");
    }
    let mut im = RgbaImage::new(w, h);
    let options = &o.options;
    match &o.element {
        Element::Text {
            text,
            font_pt,
            bold,
            align,
        } => draw_text(&mut im, text, *font_pt, *bold, *align, dpi, serial, options)?,
        Element::Barcode {
            kind,
            data,
            show_text,
        } => draw_barcode(&mut im, *kind, data, *show_text, dpi, serial, options)?,
        Element::Qr { data } => draw_qr(&mut im, data, serial, options.qr_level)?,
        Element::Shape {
            kind,
            stroke_mm,
            filled,
        } => draw_shape(&mut im, *kind, *stroke_mm, *filled, dpi),
        Element::Image { data_base64 } => draw_image(&mut im, data_base64, options)?,
    };
    if o.flip_x {
        imageops::flip_horizontal_in_place(&mut im);
    }
    if o.flip_y {
        imageops::flip_vertical_in_place(&mut im);
    }
    rotate_layer(im, o.rotation)
}

fn rotate_layer(im: RgbaImage, degrees: f32) -> Result<RgbaImage> {
    let degrees = degrees.rem_euclid(360.);
    match degrees {
        0. => return Ok(im),
        90. => return Ok(imageops::rotate90(&im)),
        180. => return Ok(imageops::rotate180(&im)),
        270. => return Ok(imageops::rotate270(&im)),
        _ => {}
    }
    let (width, height) =
        crate::design::rotated_size(im.width() as f32, im.height() as f32, degrees);
    let (width, height) = (width.ceil() as u32, height.ceil() as u32);
    if width as u64 * height as u64 > 36_000_000 {
        bail!("Rotated object exceeds render limit");
    }
    let mut output = RgbaImage::new(width, height);
    let (sin, cos) = degrees.to_radians().sin_cos();
    for (x, y, target) in output.enumerate_pixels_mut() {
        let dx = x as f32 + 0.5 - width as f32 / 2.;
        let dy = y as f32 + 0.5 - height as f32 / 2.;
        let sx = cos * dx + sin * dy + im.width() as f32 / 2. - 0.5;
        let sy = -sin * dx + cos * dy + im.height() as f32 / 2. - 0.5;
        let (ix, iy) = (sx.floor() as i32, sy.floor() as i32);
        let (fx, fy) = (sx - ix as f32, sy - iy as f32);
        // Interpolate premultiplied colors so transparent image edges do not acquire dark halos.
        let mut alpha = 0.;
        let mut colors = [0.; 3];
        for (xx, yy, weight) in [
            (ix, iy, (1. - fx) * (1. - fy)),
            (ix + 1, iy, fx * (1. - fy)),
            (ix, iy + 1, (1. - fx) * fy),
            (ix + 1, iy + 1, fx * fy),
        ] {
            if xx >= 0 && yy >= 0 && xx < im.width() as i32 && yy < im.height() as i32 {
                let pixel = im.get_pixel(xx as u32, yy as u32);
                let a = pixel[3] as f32 * weight;
                alpha += a;
                for channel in 0..3 {
                    colors[channel] += pixel[channel] as f32 * a;
                }
            }
        }
        if alpha > 0. {
            *target = Rgba([
                (colors[0] / alpha).round() as u8,
                (colors[1] / alpha).round() as u8,
                (colors[2] / alpha).round() as u8,
                alpha.round() as u8,
            ]);
        }
    }
    Ok(output)
}

pub fn render_label(doc: &LabelDocument, dpi: u32) -> Result<Output> {
    doc.validate()?;
    if ![203, 300, 600].contains(&dpi) {
        bail!("Choose 203, 300, or 600 DPI");
    }
    let w = dots(doc.width_mm, dpi).max(1);
    let h = dots(doc.effective_height_mm(), dpi).max(1);
    if w as u64 * h as u64 > 36_000_000 {
        bail!("Label exceeds 36 megapixels at {dpi} DPI");
    }
    let mut image = RgbImage::from_pixel(w, h, Rgb([255; 3]));
    let mut warnings = vec![];
    let mut clipped = false;
    for o in &doc.objects {
        let layer = render_object(o, dpi, doc.serial_number)
            .with_context(|| format!("{} #{}", o.label(), o.id))?;
        let ox = (o.x_mm * dpi as f32 / 25.4).round() as i64;
        let oy = (o.y_mm * dpi as f32 / 25.4).round() as i64;
        if ox < 0
            || oy < 0
            || ox + layer.width() as i64 > w as i64
            || oy + layer.height() as i64 > h as i64
        {
            clipped = true;
            warnings.push(format!("{} #{} extends past paper edge", o.label(), o.id));
        }
        for (x, y, p) in layer.enumerate_pixels() {
            let px = ox + x as i64;
            let py = oy + y as i64;
            if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 {
                continue;
            }
            let dest = image.get_pixel_mut(px as u32, py as u32);
            let a = p[3] as u16;
            for channel in 0..3 {
                dest[channel] =
                    ((p[channel] as u16 * a + dest[channel] as u16 * (255 - a) + 127) / 255) as u8;
            }
        }
    }
    Ok(Output {
        image,
        warnings,
        clipped,
        width_mm: doc.width_mm,
        height_mm: doc.effective_height_mm(),
        dpi,
    })
}
#[cfg(test)]
mod font_option_tests {
    use super::*;
    use crate::design::LabelDocument;

    fn doc_with_font(font: &str) -> LabelDocument {
        let mut doc = LabelDocument::blank(50., 20.);
        let id = doc.add(
            Element::Text {
                text: "Hello".into(),
                font_pt: 14.,
                bold: false,
                align: crate::design::TextAlign::Left,
            },
            2.,
            2.,
            40.,
            10.,
        );
        doc.objects
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .options
            .font = font.into();
        doc
    }

    #[test]
    fn a_font_that_is_not_installed_falls_back_to_the_built_in_one() {
        let plain = render_label(&doc_with_font(""), 203).unwrap();
        let missing = render_label(&doc_with_font("No Such Font 123"), 203).unwrap();
        assert_eq!(plain.image.as_raw(), missing.image.as_raw());
    }

    #[test]
    fn font_names_cannot_point_outside_the_fonts_folder() {
        assert!(doc_with_font("../../etc/passwd").validate().is_err());
        assert!(doc_with_font("Open Sans").validate().is_ok());
        let json = serde_json::to_string(&doc_with_font("")).unwrap();
        assert!(
            !json.contains("\"font\":"),
            "the default adds nothing to a saved file"
        );
    }
}

#[cfg(test)]
mod auto_length_tests {
    use super::*;
    use crate::design::LabelDocument;

    #[test]
    fn auto_length_prints_only_as_long_as_the_content() {
        let mut doc = LabelDocument::blank(50., 200.);
        doc.add(
            Element::Text {
                text: "Hello".into(),
                font_pt: 12.,
                bold: false,
                align: crate::design::TextAlign::Left,
            },
            2.,
            10.,
            40.,
            8.,
        );
        assert_eq!(render_label(&doc, 203).unwrap().height_mm, 200.);
        doc.auto_length = true;
        doc.auto_margin_mm = 3.;
        assert!((doc.effective_height_mm() - 21.).abs() < 0.01);
        let out = render_label(&doc, 203).unwrap();
        assert!((out.height_mm - 21.).abs() < 0.01);
        assert_eq!(out.image.height(), dots(21., 203));
        assert!(!out.clipped);
        doc.objects.clear();
        assert_eq!(
            doc.effective_height_mm(),
            5.,
            "an empty label is not zero-length"
        );
        doc.auto_margin_mm = f32::NAN;
        assert!(doc.validate().is_err());
    }

    #[test]
    fn older_files_without_the_new_fields_still_load() {
        let json = r#"{"version":1,"title":"Old","width_mm":50.0,"height_mm":30.0,"serial_number":1,"objects":[],"next_id":1}"#;
        let doc: LabelDocument = serde_json::from_str(json).unwrap();
        assert!(!doc.auto_length);
        assert_eq!(doc.effective_height_mm(), 30.);
        let saved = serde_json::to_string(&doc).unwrap();
        assert!(!saved.contains("auto_length") && !saved.contains("auto_margin"));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{LabelDocument, from_template};
    #[test]
    fn code128_and_qr_are_real_patterns() {
        assert!(encoded(BarcodeKind::Code128, "SKU-001").unwrap().len() > 50);
        let mut d = LabelDocument::blank(50.8, 25.4);
        d.add(
            Element::Barcode {
                kind: BarcodeKind::Code128,
                data: "SKU-001".into(),
                show_text: false,
            },
            2.,
            3.,
            45.,
            12.,
        );
        let o = render_label(&d, 203).unwrap();
        assert!(o.image.pixels().any(|p| p[0] == 0));
    }
    #[test]
    fn templates_render() {
        for i in 0..crate::design::TEMPLATES.len() {
            let d = from_template(i);
            let result = render_label(&d, 203);
            assert!(result.is_ok(), "{}: {:?}", d.title, result.err());
        }
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;

    #[test]
    fn free_rotation_expands_bounds_and_preserves_alpha_colors() {
        let source = RgbaImage::from_pixel(64, 24, Rgba([255, 0, 0, 255]));
        let rotated = rotate_layer(source.clone(), 37.25).unwrap();
        assert_eq!(rotated.dimensions(), (66, 58));
        assert_eq!(rotated.get_pixel(0, 0)[3], 0);
        assert!(rotated.get_pixel(33, 29)[3] > 250);
        assert!(
            rotated
                .pixels()
                .filter(|pixel| pixel[3] > 0)
                .all(|pixel| pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 0)
        );
        assert_eq!(
            rotate_layer(source.clone(), 90.).unwrap(),
            imageops::rotate90(&source)
        );
        assert_eq!(
            rotate_layer(source.clone(), 270.).unwrap(),
            imageops::rotate270(&source)
        );
    }

    #[test]
    fn rotated_objects_keep_their_center_and_report_left_edge_clipping() {
        let mut doc = LabelDocument::blank(100., 100.);
        doc.add(
            Element::Shape {
                kind: ShapeKind::Rectangle,
                stroke_mm: 0.4,
                filled: true,
            },
            30.,
            35.,
            30.,
            10.,
        );
        doc.objects[0].rotate_to(37.5);
        let result = render_label(&doc, 203).unwrap();
        assert!(!result.clipped);
        assert_eq!(
            result.image.get_pixel(dots(45., 203), dots(40., 203)),
            &Rgb([0, 0, 0])
        );
        doc.objects[0].x_mm = -5.;
        let result = render_label(&doc, 203).unwrap();
        assert!(result.clipped);
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("paper edge"))
        );
    }
}

#[cfg(test)]
mod barcode_kind_tests {
    use super::*;

    fn draws(kind: BarcodeKind, data: &str) -> Result<RgbaImage> {
        let mut im = RgbaImage::new(600, 300);
        draw_barcode(
            &mut im,
            kind,
            data,
            false,
            203,
            1,
            &ObjectOptions::default(),
        )
        .map(|_| im)
    }
    fn has_ink(im: &RgbaImage) -> bool {
        im.pixels().any(|p| p.0[3] > 0)
    }

    #[test]
    fn gs1_barcodes_draw_valid_data_and_refuse_bad_data() {
        let linear = draws(BarcodeKind::Gs1128, "(01)0950600013435(10)LOT7").unwrap();
        assert!(has_ink(&linear));
        // Serial numbers can still be filled in.
        assert!(draws(BarcodeKind::Gs1128, "(00)10614141123456789${serial}").is_err());
        assert!(draws(BarcodeKind::Gs1128, "(21)${serial}").is_ok());
        let square = draws(BarcodeKind::Gs1DataMatrix, "(01)95012345678903(10)LOT7").unwrap();
        assert!(has_ink(&square));
        for (kind, bad) in [
            (BarcodeKind::Gs1128, "09506000134352"),
            (BarcodeKind::Gs1128, "(01)09506000134353"),
            (BarcodeKind::Gs1DataMatrix, "(10)LOT7(01)95012345678903"),
            (BarcodeKind::Gs1DataMatrix, ""),
        ] {
            assert!(draws(kind, bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            readable_value(BarcodeKind::Gs1128, "(01)0950600013435(10)LOT7"),
            "(01)09506000134352(10)LOT7"
        );
    }

    #[test]
    fn new_one_dimensional_symbologies_encode_valid_values() {
        for (kind, data) in [
            (BarcodeKind::Ean8, "5512345"),
            (BarcodeKind::Itf, "12345670"),
            (BarcodeKind::Codabar, "A40156B"),
            (BarcodeKind::Code93, "TEST93"),
        ] {
            assert!(has_ink(&draws(kind, data).unwrap()), "{kind:?}");
        }
    }

    #[test]
    fn invalid_values_explain_what_the_symbology_needs() {
        let error = |kind, data| draws(kind, data).unwrap_err().to_string();
        assert!(error(BarcodeKind::Ean8, "123").contains("7 digits"));
        assert!(error(BarcodeKind::Itf, "123").contains("even"));
        assert!(error(BarcodeKind::Codabar, "12345").contains("A, B, C, or D"));
    }

    #[test]
    fn data_matrix_draws_a_square_symbol_and_rejects_empty_or_tiny_objects() {
        let im = draws(BarcodeKind::DataMatrix, "SKU-1048-DEMO").unwrap();
        assert!(has_ink(&im));
        assert!(
            draws(BarcodeKind::DataMatrix, "")
                .unwrap_err()
                .to_string()
                .contains("empty")
        );
        let mut tiny = RgbaImage::new(12, 12);
        assert!(
            draw_barcode(
                &mut tiny,
                BarcodeKind::DataMatrix,
                "ABC",
                false,
                203,
                1,
                &ObjectOptions::default()
            )
            .is_err()
        );
    }

    #[test]
    fn data_matrix_is_always_square_even_for_short_text() {
        let im = draws(BarcodeKind::DataMatrix, "TP-1048-DEMO").unwrap();
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (u32::MAX, 0, u32::MAX, 0);
        for (x, y, pixel) in im.enumerate_pixels() {
            if pixel.0[3] > 0 {
                (min_x, max_x) = (min_x.min(x), max_x.max(x));
                (min_y, max_y) = (min_y.min(y), max_y.max(y));
            }
        }
        assert_eq!(max_x - min_x, max_y - min_y, "symbol is a square");
    }

    #[test]
    fn data_matrix_variable_expansion_matches_other_symbols() {
        let a = draws(BarcodeKind::DataMatrix, "N-${serial}").unwrap();
        let mut b = RgbaImage::new(600, 300);
        draw_barcode(
            &mut b,
            BarcodeKind::DataMatrix,
            "N-${serial}",
            false,
            203,
            2,
            &ObjectOptions::default(),
        )
        .unwrap();
        assert_ne!(a.as_raw(), b.as_raw(), "serial changes the encoded symbol");
    }
}

#[cfg(test)]
mod option_render_tests {
    use super::*;
    use crate::design::{DesignObject, ImageMode};

    fn text_layer(text: &str, width_mm: f32, height_mm: f32, options: ObjectOptions) -> RgbaImage {
        let mut im = RgbaImage::new(dots(width_mm, 203), dots(height_mm, 203));
        draw_text(&mut im, text, 12., false, TextAlign::Left, 203, 1, &options).unwrap();
        im
    }
    fn ink_rows(im: &RgbaImage) -> usize {
        (0..im.height())
            .filter(|&y| (0..im.width()).any(|x| im.get_pixel(x, y)[3] > 0))
            .count()
    }
    fn ink(im: &RgbaImage) -> usize {
        im.pixels().filter(|p| p[3] > 0).count()
    }
    fn rightmost(im: &RgbaImage) -> u32 {
        (0..im.width())
            .rev()
            .find(|&x| (0..im.height()).any(|y| im.get_pixel(x, y)[3] > 0))
            .unwrap_or(0)
    }

    #[test]
    fn dates_and_serials_accept_formats_and_leave_bad_ones_alone() {
        assert_eq!(expand("n${serial:4}", 7), "n0007");
        assert_eq!(expand("${serial}/${serial:2}", 5), "5/05");
        assert_eq!(
            expand("${serial:0} ${serial:99} ${serial:x}", 5),
            "${serial:0} ${serial:99} ${serial:x}"
        );
        let today = chrono::Local::now();
        assert_eq!(expand("${date}", 1), today.format("%Y-%m-%d").to_string());
        assert_eq!(
            expand("${date:%d/%m/%Y}", 1),
            today.format("%d/%m/%Y").to_string()
        );
        assert_eq!(
            expand("${date:%Q%}", 1),
            "${date:%Q%}",
            "an invalid format is not replaced"
        );
        assert_eq!(expand("${unknown} ${date", 1), "${unknown} ${date");
    }

    #[test]
    fn wrapping_breaks_long_text_into_lines_and_plain_text_stays_on_one() {
        let long = "Fragile glass items packed with care";
        let plain = text_layer(long, 30., 30., ObjectOptions::default());
        let wrapped = text_layer(
            long,
            30.,
            30.,
            ObjectOptions {
                wrap: true,
                ..Default::default()
            },
        );
        assert!(
            ink_rows(&wrapped) > ink_rows(&plain) * 2,
            "wrapped text uses several lines"
        );
        assert!(
            rightmost(&wrapped) < dots(30., 203),
            "wrapped text stays inside the box"
        );
        // A word wider than the box is broken between letters instead of overflowing.
        let word = text_layer(
            "Supercalifragilisticexpialidocious",
            20.,
            40.,
            ObjectOptions {
                wrap: true,
                ..Default::default()
            },
        );
        assert!(ink_rows(&word) > 20 && rightmost(&word) < dots(20., 203));
    }

    #[test]
    fn shrink_to_fit_makes_text_fit_and_spacing_changes_the_height() {
        let text = "ASSET TAG 0000123 WAREHOUSE";
        let natural = advance(font(false), text, 12. * 203. / 72.);
        assert!(
            natural > dots(25., 203) as f32,
            "the text is wider than the box at its normal size"
        );
        let fitted = text_layer(
            text,
            25.,
            8.,
            ObjectOptions {
                fit: true,
                ..Default::default()
            },
        );
        assert!(
            rightmost(&fitted) < dots(25., 203),
            "fitted text ends inside the box"
        );
        assert!(ink(&fitted) > 0);
        let tight = text_layer(
            "a\nb\nc",
            20.,
            40.,
            ObjectOptions {
                line_spacing: 0.8,
                ..Default::default()
            },
        );
        let loose = text_layer(
            "a\nb\nc",
            20.,
            40.,
            ObjectOptions {
                line_spacing: 2.,
                ..Default::default()
            },
        );
        let last_row = |im: &RgbaImage| {
            (0..im.height())
                .rev()
                .find(|&y| (0..im.width()).any(|x| im.get_pixel(x, y)[3] > 0))
                .unwrap()
        };
        assert!(last_row(&loose) > last_row(&tight) + 20);
    }

    #[test]
    fn inverse_text_is_light_on_a_black_box_and_italic_leans_right() {
        let inverse = text_layer(
            "HI",
            20.,
            8.,
            ObjectOptions {
                inverse: true,
                ..Default::default()
            },
        );
        assert_eq!(
            inverse.get_pixel(0, 0).0,
            [0, 0, 0, 255],
            "the box is black"
        );
        assert!(inverse.pixels().any(|p| p[0] > 200), "letters are white");
        let upright = text_layer("l", 10., 8., ObjectOptions::default());
        let leaning = text_layer(
            "l",
            10.,
            8.,
            ObjectOptions {
                italic: true,
                ..Default::default()
            },
        );
        assert!(
            rightmost(&leaning) > rightmost(&upright),
            "italic shifts the top to the right"
        );
    }

    fn photo() -> RgbaImage {
        // A horizontal gradient from black to white.
        RgbaImage::from_fn(64, 8, |x, _| {
            let v = (x * 255 / 63) as u8;
            Rgba([v, v, v, 255])
        })
    }
    fn options(mode: ImageMode) -> ObjectOptions {
        ObjectOptions {
            image_mode: mode,
            ..Default::default()
        }
    }

    #[test]
    fn threshold_dither_contrast_and_invert_adjust_images() {
        let mut original = photo();
        adjust_image(&mut original, &ObjectOptions::default());
        assert_eq!(original, photo(), "defaults leave the picture alone");

        let mut thresholded = photo();
        adjust_image(
            &mut thresholded,
            &ObjectOptions {
                threshold: 200,
                ..options(ImageMode::Threshold)
            },
        );
        assert!(thresholded.pixels().all(|p| p[0] == 0 || p[0] == 255));
        let blacks = thresholded.pixels().filter(|p| p[0] == 0).count();
        assert!(
            blacks > 64 * 8 / 2,
            "a high cut-off makes more pixels black"
        );

        let mut dithered = photo();
        adjust_image(&mut dithered, &options(ImageMode::Dither));
        assert!(dithered.pixels().all(|p| p[0] == 0 || p[0] == 255));
        // The middle of the gradient is mixed black and white, not one solid color.
        let middle: Vec<u8> = (24..40).map(|x| dithered.get_pixel(x, 4)[0]).collect();
        assert!(middle.contains(&0) && middle.contains(&255));

        let mut inverted = photo();
        adjust_image(
            &mut inverted,
            &ObjectOptions {
                invert: true,
                ..Default::default()
            },
        );
        assert_eq!(inverted.get_pixel(0, 0)[0], 255);
        assert_eq!(inverted.get_pixel(63, 0)[0], 0);

        let mut harsh = photo();
        adjust_image(
            &mut harsh,
            &ObjectOptions {
                contrast: 1.,
                ..Default::default()
            },
        );
        assert!(
            harsh.get_pixel(20, 0)[0] < photo().get_pixel(20, 0)[0],
            "contrast darkens the darks"
        );
    }

    #[test]
    fn transparent_pixels_stay_transparent_when_images_are_adjusted() {
        let mut logo = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        logo.put_pixel(1, 1, Rgba([20, 20, 20, 255]));
        adjust_image(&mut logo, &options(ImageMode::Threshold));
        assert_eq!(logo.get_pixel(0, 0)[3], 0);
        assert_eq!(logo.get_pixel(1, 1).0, [0, 0, 0, 255]);
    }

    #[test]
    fn retail_barcodes_compute_and_print_their_check_digit() {
        assert_eq!(check_digit(BarcodeKind::Ean13, "590123412345"), Some('7'));
        assert_eq!(check_digit(BarcodeKind::Ean8, "5512345"), Some('7'));
        assert_eq!(check_digit(BarcodeKind::UpcA, "03600029145"), Some('2'));
        assert_eq!(check_digit(BarcodeKind::Code128, "123"), None);
        assert_eq!(check_digit(BarcodeKind::Ean13, "12"), None);
        assert_eq!(
            readable_value(BarcodeKind::Ean13, "590123412345"),
            "5901234123457"
        );
        assert_eq!(
            readable_value(BarcodeKind::Ean13, "5901234123457"),
            "5901234123457"
        );
        assert_eq!(readable_value(BarcodeKind::Code128, "ABC"), "ABC");
    }

    #[test]
    fn barcode_text_size_and_position_can_be_chosen() {
        let draw = |options: ObjectOptions| {
            let mut im = RgbaImage::new(600, 200);
            draw_barcode(
                &mut im,
                BarcodeKind::Code128,
                "SKU-1",
                true,
                203,
                1,
                &options,
            )
            .unwrap();
            im
        };
        let bars_top = |im: &RgbaImage| {
            (0..im.height())
                .find(|&y| (0..im.width()).any(|x| im.get_pixel(x, y)[3] > 0))
                .unwrap()
        };
        let below = draw(ObjectOptions::default());
        let above = draw(ObjectOptions {
            text_above: true,
            ..Default::default()
        });
        assert_eq!(
            bars_top(&below),
            0,
            "bars start at the top when the text is below"
        );
        assert!(bars_top(&above) < 30, "text sits at the top when above");
        let bar_height = |im: &RgbaImage| {
            (0..im.height())
                .filter(|&y| im.get_pixel(300, y)[3] > 0 || im.get_pixel(310, y)[3] > 0)
                .count()
        };
        let small = draw(ObjectOptions {
            barcode_text_pt: 4.,
            ..Default::default()
        });
        let big = draw(ObjectOptions {
            barcode_text_pt: 20.,
            ..Default::default()
        });
        assert!(
            bar_height(&small) > bar_height(&big),
            "bigger text leaves shorter bars"
        );
    }

    #[test]
    fn qr_error_correction_changes_the_symbol_size() {
        let module_count = |level: QrLevel| {
            let mut im = RgbaImage::new(400, 400);
            draw_qr(
                &mut im,
                "https://example.com/a/fairly/long/address?x=1",
                1,
                level,
            )
            .unwrap();
            ink(&im)
        };
        assert_ne!(module_count(QrLevel::Low), module_count(QrLevel::High));
    }

    #[test]
    fn flips_mirror_an_object_before_it_is_rotated() {
        let mut doc = LabelDocument::blank(30., 12.);
        doc.add(
            Element::Text {
                text: "L".into(),
                font_pt: 14.,
                bold: true,
                align: TextAlign::Left,
            },
            1.,
            1.,
            10.,
            8.,
        );
        let plain = render_label(&doc, 203).unwrap();
        doc.objects[0].flip_x = true;
        let flipped = render_label(&doc, 203).unwrap();
        assert_ne!(plain.image, flipped.image);
        doc.objects[0].flip_x = false;
        doc.objects[0].flip_y = true;
        assert_ne!(plain.image, render_label(&doc, 203).unwrap().image);
        let _ = DesignObject::label;
    }
}
