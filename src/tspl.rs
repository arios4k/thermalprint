//! TSPL raster backend for the many 4 × 6 shipping printers (Munbyn, iDPRT, TSC and others)
//! that speak TSPL instead of ZPL. Like the ZPL backend it sends the exact validated raster.
//! It sets the label size but leaves the gap or black-mark setting to the printer's own
//! calibration, because a wrong GAP command makes a printer feed labels out of step.
use crate::{design::LabelDocument, design_render, layout::Output};
use anyhow::{Result, bail};

pub fn encode_output(output: &Output) -> Result<Vec<u8>> {
    if output.clipped {
        bail!("Resolve clipping before TSPL output");
    }
    let w = output.image.width();
    let h = output.image.height();
    let row_bytes = w.div_ceil(8);
    let mut tspl = format!(
        "SIZE {:.1} mm,{:.1} mm\r\nCLS\r\nBITMAP 0,0,{row_bytes},{h},0,",
        output.width_mm, output.height_mm
    )
    .into_bytes();
    tspl.reserve((row_bytes * h) as usize + 32);
    for y in 0..h {
        for byte in 0..row_bytes {
            // In TSPL a set bit leaves the dot blank and a clear bit prints it.
            let mut packed = 0xFFu8;
            for bit in 0..8 {
                let x = byte * 8 + bit;
                if x < w {
                    let p = output.image.get_pixel(x, y);
                    if (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114)
                        / 1000
                        < 128
                    {
                        packed &= !(1 << (7 - bit));
                    }
                }
            }
            tspl.push(packed);
        }
    }
    tspl.extend_from_slice(b"\r\nPRINT 1,1\r\n");
    Ok(tspl)
}

/// The design as raw TSPL for printing, with the optional designer footer kept `edge_mm`
/// from the label's sides and bottom.
pub fn encode_design_for_print(
    doc: &LabelDocument,
    dpi: u32,
    footer: &crate::footer::Footer,
    edge_mm: f32,
) -> Result<Vec<u8>> {
    encode_output(&design_render::render_with_footer(
        doc, dpi, footer, edge_mm,
    )?)
}

pub fn encode_design(doc: &LabelDocument, dpi: u32) -> Result<Vec<u8>> {
    encode_output(&design_render::render_for_print(doc, dpi)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{Element, LabelDocument, ShapeKind};

    fn header_and_data(bytes: &[u8]) -> (String, &[u8]) {
        // Skip "BITMAP 0,0," (x and y), then the header ends at ",<mode>,".
        let start = b"BITMAP 0,0,";
        let from = bytes
            .windows(start.len())
            .position(|w| w == start)
            .expect("a BITMAP header")
            + start.len();
        let marker = b",0,";
        let at = from
            + bytes[from..]
                .windows(marker.len())
                .position(|w| w == marker)
                .expect("a BITMAP mode")
            + marker.len();
        (
            String::from_utf8(bytes[..at].to_vec()).unwrap(),
            &bytes[at..],
        )
    }

    #[test]
    fn tspl_sets_the_label_size_and_raster_dimensions() {
        let d = LabelDocument::blank(25.4, 25.4);
        let bytes = encode_design(&d, 203).unwrap();
        let (header, data) = header_and_data(&bytes);
        assert_eq!(
            header,
            "SIZE 25.4 mm,25.4 mm\r\nCLS\r\nBITMAP 0,0,26,203,0,"
        );
        assert_eq!(data.len(), 26 * 203 + b"\r\nPRINT 1,1\r\n".len());
        assert!(bytes.ends_with(b"\r\nPRINT 1,1\r\n"));
        assert!(
            !String::from_utf8_lossy(&bytes).contains("GAP"),
            "the printer keeps its own gap calibration"
        );
    }

    #[test]
    fn white_stays_blank_and_black_prints() {
        let blank = encode_design(&LabelDocument::blank(25.4, 25.4), 203).unwrap();
        let (_, data) = header_and_data(&blank);
        assert!(
            data[..26 * 203].iter().all(|b| *b == 0xFF),
            "white is all set bits"
        );

        let mut filled = LabelDocument::blank(25.4, 25.4);
        filled.add(
            Element::Shape {
                kind: ShapeKind::Rectangle,
                filled: true,
                stroke_mm: 0.5,
            },
            0.,
            0.,
            25.4,
            25.4,
        );
        let bytes = encode_design(&filled, 203).unwrap();
        let (_, data) = header_and_data(&bytes);
        let raster = &data[..26 * 203];
        let middle = raster[100 * 26 + 13];
        assert_eq!(middle, 0x00, "a black area is all clear bits");
    }
}
