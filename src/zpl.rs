//! ZPL II raster backend. It deliberately emits the exact validated design
//! raster, so unsupported object types cannot silently disappear.
use crate::{design::LabelDocument, design_render, layout::Output};
use anyhow::{Result, bail};
use std::fmt::Write as _;
pub fn encode_output(output: &Output) -> Result<Vec<u8>> {
    if output.clipped {
        bail!("Resolve clipping before ZPL output");
    }
    let w = output.image.width();
    let h = output.image.height();
    let row_bytes = w.div_ceil(8);
    let total = row_bytes * h;
    let mut zpl = format!("^XA\n^PW{w}\n^LL{h}\n^FO0,0\n^GFA,{total},{total},{row_bytes},");
    for y in 0..h {
        for byte in 0..row_bytes {
            let mut packed = 0u8;
            for bit in 0..8 {
                let x = byte * 8 + bit;
                if x < w {
                    let p = output.image.get_pixel(x, y);
                    if (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114)
                        / 1000
                        < 128
                    {
                        packed |= 1 << (7 - bit);
                    }
                }
            }
            write!(&mut zpl, "{packed:02X}")?;
        }
    }
    zpl.push_str("\n^FS\n^XZ\n");
    Ok(zpl.into_bytes())
}
pub fn encode_design(doc: &LabelDocument, dpi: u32) -> Result<Vec<u8>> {
    encode_output(&design_render::render_for_print(doc, dpi)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{Element, LabelDocument};
    #[test]
    fn raster_zpl_has_exact_width_and_length() {
        let mut d = LabelDocument::blank(25.4, 25.4);
        d.add(
            Element::Qr {
                data: "TEST".into(),
            },
            3.,
            3.,
            16.,
            16.,
        );
        let text = String::from_utf8(encode_design(&d, 203).unwrap()).unwrap();
        assert!(text.starts_with("^XA\n^PW203\n^LL203"));
        assert!(text.contains("^GFA,"));
        assert!(text.ends_with("^XZ\n"));
    }
}
