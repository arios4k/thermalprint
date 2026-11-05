//! Minimal image-only PDF writer: paper MediaBox is expressed in PDF points,
//! while the embedded RGB image retains its independently rendered pixel grid.
use crate::layout::Output;
use anyhow::{Result, bail};
use flate2::{Compression, write::ZlibEncoder};
use std::{io::Write, path::Path};
pub fn pdf_bytes(output: &Output) -> Result<Vec<u8>> {
    if output.clipped {
        bail!("Resolve clipping before exporting");
    }
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(output.image.as_raw())?;
    let data = z.finish()?;
    let w = output.width_mm as f64 / 25.4 * 72.;
    let h = output.height_mm as f64 / 25.4 * 72.;
    let content = format!("q\n{w:.5} 0 0 {h:.5} 0 0 cm\n/Label Do\nQ\n");
    let objects=[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.5} {h:.5}] /Resources << /XObject << /Label 4 0 R >> >> /Contents 5 0 R >>").into_bytes(),
        stream(&format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate false /Filter /FlateDecode",output.image.width(),output.image.height()),&data),
        stream("",content.as_bytes()),
    ];
    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0];
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        writeln!(pdf, "{} 0 obj", i + 1)?;
        pdf.extend(obj);
        pdf.extend(b"\nendobj\n");
    }
    let start = pdf.len();
    write!(pdf, "xref\n0 {}\n0000000000 65535 f \n", offsets.len())?;
    for offset in offsets.iter().skip(1) {
        writeln!(pdf, "{offset:010} 00000 n ")?;
    }
    write!(
        pdf,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n",
        offsets.len()
    )?;
    Ok(pdf)
}
fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut b = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    b.extend(data);
    b.extend(b"\nendstream");
    b
}
pub fn save(output: &Output, path: &Path) -> Result<()> {
    if output.clipped {
        bail!("Resolve clipping before exporting");
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("pdf") => file.write_all(&pdf_bytes(output)?)?,
        Some("png") => {
            let mut encoder = png::Encoder::new(
                file.as_file_mut(),
                output.image.width(),
                output.image.height(),
            );
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let ppm = (output.dpi as f64 / 0.0254).round() as u32;
            encoder.set_pixel_dims(Some(png::PixelDimensions {
                xppu: ppm,
                yppu: ppm,
                unit: png::Unit::Meter,
            }));
            encoder
                .write_header()?
                .write_image_data(output.image.as_raw())?;
        }
        _ => bail!("Choose a .pdf or .png filename"),
    }
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
