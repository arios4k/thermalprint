//! Deterministic rasterization of editable labels at the requested printer DPI.
//! Preview textures are downsampled from this output by the UI.
use crate::{
    design::{BarcodeKind, DesignObject, Element, LabelDocument, ShapeKind, TextAlign},
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
fn expand(text: &str, serial: u64) -> String {
    text.replace(
        "${date}",
        &chrono::Local::now().format("%Y-%m-%d").to_string(),
    )
    .replace("${serial}", &serial.to_string())
}
fn draw_text(
    im: &mut RgbaImage,
    text: &str,
    font_pt: f32,
    bold: bool,
    align: TextAlign,
    dpi: u32,
    serial: u64,
) -> Result<()> {
    let f = font(bold);
    let px = font_pt * dpi as f32 / 72.;
    let line_height = (px * 1.32).ceil() as i32;
    for (row, line_text) in expand(text, serial).lines().enumerate() {
        let advances = line_text
            .chars()
            .map(|c| f.metrics(c, px).advance_width)
            .sum::<f32>();
        let start = match align {
            TextAlign::Left => 0,
            TextAlign::Center => ((im.width() as f32 - advances) / 2.).max(0.) as i32,
            TextAlign::Right => (im.width() as f32 - advances).max(0.) as i32,
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
                        put(im, left + xx as i32, top + yy as i32, Rgba([0, 0, 0, a]));
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
            if !data.bytes().all(|b| b.is_ascii_digit()) || data.len() % 2 != 0 {
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
        BarcodeKind::DataMatrix => bail!("Data Matrix is a 2D symbol"),
    }
}
fn draw_data_matrix(im: &mut RgbaImage, data: &str) -> Result<()> {
    let code = datamatrix::DataMatrix::encode_str(data, datamatrix::SymbolList::default())
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
fn draw_barcode(
    im: &mut RgbaImage,
    kind: BarcodeKind,
    data: &str,
    show_text: bool,
    dpi: u32,
    serial: u64,
) -> Result<()> {
    let data = expand(data, serial);
    if kind == BarcodeKind::DataMatrix {
        if data.is_empty() {
            bail!("Barcode data is empty");
        }
        return draw_data_matrix(im, &data);
    }
    let bits = encoded(kind, &data)?;
    let module = (im.width() / (bits.len() as u32 + 20)).min(12);
    if module < 2 {
        bail!("Barcode is too narrow at {dpi} DPI. Widen it or use a shorter value.");
    }
    let barcode_w = (bits.len() as u32 + 20) * module;
    let left = (im.width() - barcode_w) / 2 + 10 * module;
    let text_h = if show_text {
        dots(4., dpi).min(im.height() / 3)
    } else {
        0
    };
    for (i, b) in bits.iter().enumerate() {
        if *b != 0 {
            fill(
                im,
                (left + i as u32 * module) as i32,
                0,
                (left + (i as u32 + 1) * module) as i32,
                (im.height() - text_h) as i32,
                black(),
            );
        }
    }
    if show_text {
        let text_y = im.height() - text_h;
        let mut layer = RgbaImage::new(im.width(), text_h);
        draw_text(&mut layer, &data, 8., false, TextAlign::Center, dpi, serial)?;
        imageops::overlay(im, &layer, 0, text_y as i64);
    }
    Ok(())
}
fn draw_qr(im: &mut RgbaImage, data: &str, serial: u64) -> Result<()> {
    let code = QrCode::new(expand(data, serial).as_bytes()).context("QR data cannot be encoded")?;
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
fn draw_image(im: &mut RgbaImage, data: &str) -> Result<()> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    if bytes.len() > 9_000_000 {
        bail!("Image exceeds 9 MB");
    }
    let src = image::load_from_memory(&bytes)?.to_rgba8();
    let fit =
        (im.width() as f32 / src.width() as f32).min(im.height() as f32 / src.height() as f32);
    let w = (src.width() as f32 * fit).round().max(1.) as u32;
    let h = (src.height() as f32 * fit).round().max(1.) as u32;
    let scaled = imageops::resize(&src, w, h, imageops::FilterType::Lanczos3);
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
    match &o.element {
        Element::Text {
            text,
            font_pt,
            bold,
            align,
        } => draw_text(&mut im, text, *font_pt, *bold, *align, dpi, serial)?,
        Element::Barcode {
            kind,
            data,
            show_text,
        } => draw_barcode(&mut im, *kind, data, *show_text, dpi, serial)?,
        Element::Qr { data } => draw_qr(&mut im, data, serial)?,
        Element::Shape {
            kind,
            stroke_mm,
            filled,
        } => draw_shape(&mut im, *kind, *stroke_mm, *filled, dpi),
        Element::Image { data_base64 } => draw_image(&mut im, data_base64)?,
    };
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
    let h = dots(doc.height_mm, dpi).max(1);
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
        height_mm: doc.height_mm,
        dpi,
    })
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
        draw_barcode(&mut im, kind, data, false, 203, 1).map(|_| im)
    }
    fn has_ink(im: &RgbaImage) -> bool {
        im.pixels().any(|p| p.0[3] > 0)
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
        assert!(draw_barcode(&mut tiny, BarcodeKind::DataMatrix, "ABC", false, 203, 1).is_err());
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
        )
        .unwrap();
        assert_ne!(a.as_raw(), b.as_raw(), "serial changes the encoded symbol");
    }
}
