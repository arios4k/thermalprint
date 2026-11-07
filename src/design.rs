//! Editable, versioned label document. Coordinates and dimensions are millimeters.
//! No UI or printer dependency lives in this module.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const FORMAT_VERSION: u32 = 1;

pub fn is_design_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("thermalprint"))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarcodeKind {
    Code128,
    Code39,
    Ean13,
    UpcA,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
    Line,
    DividerHorizontal,
    DividerVertical,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Element {
    Text {
        text: String,
        font_pt: f32,
        bold: bool,
        align: TextAlign,
    },
    Barcode {
        kind: BarcodeKind,
        data: String,
        show_text: bool,
    },
    Qr {
        data: String,
    },
    Shape {
        kind: ShapeKind,
        stroke_mm: f32,
        filled: bool,
    },
    Image {
        data_base64: String,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignObject {
    pub id: u64,
    pub x_mm: f32,
    pub y_mm: f32,
    pub width_mm: f32,
    pub height_mm: f32,
    /// Clockwise degrees. X/Y locate the rotated bounding box.
    #[serde(serialize_with = "serialize_rotation")]
    pub rotation: f32,
    pub element: Element,
}
// Keep whole-degree values compatible with existing integer-valued documents.
fn serialize_rotation<S: serde::Serializer>(angle: &f32, serializer: S) -> Result<S::Ok, S::Error> {
    if angle.fract() == 0. && (0.0..=65535.0).contains(angle) {
        serializer.serialize_u16(*angle as u16)
    } else {
        serializer.serialize_f32(*angle)
    }
}

pub fn rotated_size(width: f32, height: f32, degrees: f32) -> (f32, f32) {
    let degrees = degrees.rem_euclid(360.);
    if degrees == 0. || degrees == 180. {
        return (width, height);
    }
    if degrees == 90. || degrees == 270. {
        return (height, width);
    }
    let (sin, cos) = degrees.to_radians().sin_cos();
    (
        width * cos.abs() + height * sin.abs(),
        width * sin.abs() + height * cos.abs(),
    )
}

impl DesignObject {
    pub fn rotated_size_mm(&self) -> (f32, f32) {
        rotated_size(self.width_mm, self.height_mm, self.rotation)
    }
    pub fn center_mm(&self) -> (f32, f32) {
        let (width, height) = self.rotated_size_mm();
        (self.x_mm + width / 2., self.y_mm + height / 2.)
    }
    pub fn rotate_to(&mut self, degrees: f32) {
        if !degrees.is_finite() {
            return;
        }
        let (cx, cy) = self.center_mm();
        self.rotation = degrees.rem_euclid(360.);
        let (width, height) = self.rotated_size_mm();
        self.x_mm = cx - width / 2.;
        self.y_mm = cy - height / 2.;
    }
    pub fn label(&self) -> &'static str {
        match &self.element {
            Element::Text { .. } => "Text",
            Element::Barcode { .. } => "Barcode",
            Element::Qr { .. } => "QR code",
            Element::Shape { kind, .. } => match kind {
                ShapeKind::Rectangle => "Rectangle",
                ShapeKind::Ellipse => "Ellipse",
                ShapeKind::Line => "Line",
                ShapeKind::DividerHorizontal => "Horizontal divider",
                ShapeKind::DividerVertical => "Vertical divider",
            },
            Element::Image { .. } => "Image",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelDocument {
    pub version: u32,
    pub title: String,
    pub width_mm: f32,
    pub height_mm: f32,
    pub serial_number: u64,
    pub objects: Vec<DesignObject>,
    pub next_id: u64,
}
impl LabelDocument {
    pub fn blank(width_mm: f32, height_mm: f32) -> Self {
        Self {
            version: FORMAT_VERSION,
            title: "Untitled label".into(),
            width_mm,
            height_mm,
            serial_number: 1,
            objects: vec![],
            next_id: 1,
        }
    }
    pub fn add(
        &mut self,
        element: Element,
        x_mm: f32,
        y_mm: f32,
        width_mm: f32,
        height_mm: f32,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.push(DesignObject {
            id,
            x_mm,
            y_mm,
            width_mm,
            height_mm,
            rotation: 0.,
            element,
        });
        id
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != FORMAT_VERSION {
            bail!("Unsupported .thermalprint version {}", self.version);
        }
        if self.title.len() > 256 || self.objects.len() > 1000 {
            bail!("Design exceeds document limits");
        }
        if ![self.width_mm, self.height_mm]
            .iter()
            .all(|v| v.is_finite() && *v > 0. && *v <= 2000.)
        {
            bail!("Invalid label dimensions");
        }
        let mut seen = std::collections::HashSet::new();
        for o in &self.objects {
            if !seen.insert(o.id)
                || ![o.x_mm, o.y_mm, o.width_mm, o.height_mm]
                    .iter()
                    .all(|v| v.is_finite())
                || o.width_mm <= 0.
                || o.height_mm <= 0.
                || o.width_mm > 2000.
                || o.height_mm > 2000.
                || !o.rotation.is_finite()
            {
                bail!("Invalid design object");
            }
            match &o.element {
                Element::Text { text, font_pt, .. }
                    if text.len() > 20000
                        || !font_pt.is_finite()
                        || !(3.0..=300.0).contains(font_pt) =>
                {
                    bail!("Invalid text object")
                }
                Element::Barcode { data, .. } | Element::Qr { data } if data.len() > 2048 => {
                    bail!("Barcode data is too long")
                }
                Element::Shape { stroke_mm, .. }
                    if !stroke_mm.is_finite() || !(0.05..=20.0).contains(stroke_mm) =>
                {
                    bail!("Invalid line width")
                }
                Element::Image { data_base64 } if data_base64.len() > 12_000_000 => {
                    bail!("Embedded image exceeds the 9 MB limit")
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(file.as_file_mut(), self)?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        if std::fs::metadata(path)?.len() > 16_000_000 {
            bail!("Design file exceeds 16 MB");
        }
        let d: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        d.validate()?;
        Ok(d)
    }
}
#[derive(Clone)]
pub struct Template {
    pub name: &'static str,
    pub description: &'static str,
    pub width_mm: f32,
    pub height_mm: f32,
}
pub const TEMPLATES: &[Template] = &[
    Template {
        name: "Blank custom",
        description: "Start with an empty canvas",
        width_mm: 101.6,
        height_mm: 152.4,
    },
    Template {
        name: "Shipping 4 × 6",
        description: "Custom shipment address layout",
        width_mm: 101.6,
        height_mm: 152.4,
    },
    Template {
        name: "Return address",
        description: "Compact sender details",
        width_mm: 50.8,
        height_mm: 25.4,
    },
    Template {
        name: "Product barcode",
        description: "Product name and Code 128",
        width_mm: 50.8,
        height_mm: 25.4,
    },
    Template {
        name: "Inventory",
        description: "Item and stock code",
        width_mm: 76.2,
        height_mm: 50.8,
    },
    Template {
        name: "QR code",
        description: "Scannable URL or text",
        width_mm: 50.8,
        height_mm: 50.8,
    },
    Template {
        name: "Asset tracking",
        description: "Asset name and code",
        width_mm: 76.2,
        height_mm: 38.1,
    },
    Template {
        name: "Shelf",
        description: "Large location text",
        width_mm: 101.6,
        height_mm: 50.8,
    },
    Template {
        name: "Price tag",
        description: "Product and price",
        width_mm: 50.8,
        height_mm: 25.4,
    },
    Template {
        name: "Name label",
        description: "Large centered name",
        width_mm: 76.2,
        height_mm: 38.1,
    },
    Template {
        name: "Fragile",
        description: "Handling warning",
        width_mm: 101.6,
        height_mm: 50.8,
    },
];
pub fn from_template(index: usize) -> LabelDocument {
    let t = &TEMPLATES[index.min(TEMPLATES.len() - 1)];
    let mut d = LabelDocument::blank(t.width_mm, t.height_mm);
    d.title = t.name.into();
    let width = d.width_mm;
    let text = |s: &str, size: f32, bold: bool, align: TextAlign| Element::Text {
        text: s.into(),
        font_pt: size,
        bold,
        align,
    };
    let code = |s: &str| Element::Barcode {
        kind: BarcodeKind::Code128,
        data: s.into(),
        show_text: true,
    };
    match index {
        1 => {
            d.add(
                text(
                    "CUSTOM SHIPPING LABEL · NOT POSTAGE",
                    8.,
                    true,
                    TextAlign::Center,
                ),
                4.,
                3.,
                width - 8.,
                7.,
            );
            d.add(
                text(
                    "FROM\nYour name\nStreet address\nCity, region ZIP",
                    10.,
                    false,
                    TextAlign::Left,
                ),
                6.,
                15.,
                width - 12.,
                32.,
            );
            d.add(
                Element::Shape {
                    kind: ShapeKind::DividerHorizontal,
                    stroke_mm: 0.4,
                    filled: false,
                },
                6.,
                50.,
                width - 12.,
                1.,
            );
            d.add(
                text(
                    "SHIP TO\nRecipient name\nStreet address\nCity, region ZIP",
                    16.,
                    true,
                    TextAlign::Left,
                ),
                6.,
                57.,
                width - 12.,
                48.,
            );
            d.add(
                text(
                    "Add your own reference or QR code. This label is not carrier postage.",
                    8.,
                    false,
                    TextAlign::Center,
                ),
                6.,
                120.,
                width - 12.,
                18.,
            );
        }
        2 => {
            d.add(
                text(
                    "Your name\nStreet address\nCity, region ZIP",
                    9.,
                    false,
                    TextAlign::Left,
                ),
                3.,
                3.,
                width - 6.,
                19.,
            );
        }
        3 => {
            d.add(
                text("PRODUCT NAME", 12., true, TextAlign::Center),
                3.,
                2.,
                width - 6.,
                6.,
            );
            d.add(code("SKU-001"), 4., 9., width - 8., 13.);
        }
        4 | 6 => {
            d.add(
                text(
                    if index == 4 {
                        "INVENTORY ITEM"
                    } else {
                        "ASSET NAME"
                    },
                    13.,
                    true,
                    TextAlign::Left,
                ),
                4.,
                3.,
                width - 8.,
                10.,
            );
            d.add(code("ASSET-001"), 5., 17., width - 10., 20.);
        }
        5 => {
            d.add(
                Element::Qr {
                    data: "https://example.com".into(),
                },
                8.,
                6.,
                width - 16.,
                width - 16.,
            );
        }
        7 => {
            d.add(
                text("AISLE A · BIN 01", 22., true, TextAlign::Center),
                5.,
                14.,
                width - 10.,
                20.,
            );
        }
        8 => {
            d.add(
                text("PRODUCT NAME", 10., false, TextAlign::Left),
                3.,
                3.,
                width - 6.,
                7.,
            );
            d.add(
                text("$9.99", 22., true, TextAlign::Right),
                3.,
                11.,
                width - 6.,
                11.,
            );
        }
        9 => {
            d.add(
                text("YOUR NAME", 24., true, TextAlign::Center),
                4.,
                9.,
                width - 8.,
                20.,
            );
        }
        10 => {
            d.add(
                text("FRAGILE\nHANDLE WITH CARE", 24., true, TextAlign::Center),
                4.,
                9.,
                width - 8.,
                34.,
            );
        }
        _ => {}
    }
    d
}
#[derive(Clone, Debug)]
pub struct OpenDesign {
    pub document: LabelDocument,
    pub path: Option<PathBuf>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_round_trip() {
        for i in 0..TEMPLATES.len() {
            let d = from_template(i);
            d.validate().unwrap();
            let bytes = serde_json::to_vec(&d).unwrap();
            let read: LabelDocument = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(d, read);
        }
    }
    #[test]
    fn rejects_nonfinite_geometry() {
        let mut d = LabelDocument::blank(50.8, 25.4);
        d.add(Element::Qr { data: "abc".into() }, f32::NAN, 1., 5., 5.);
        assert!(d.validate().is_err());
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;

    #[test]
    fn fractional_rotation_round_trips_and_keeps_the_center_fixed() {
        let mut doc = from_template(0);
        doc.add(
            Element::Shape {
                kind: ShapeKind::Rectangle,
                stroke_mm: 0.4,
                filled: true,
            },
            20.,
            30.,
            40.,
            10.,
        );
        let object = &mut doc.objects[0];
        let center = object.center_mm();
        object.rotate_to(37.25);
        assert_eq!(object.rotation, 37.25);
        let rotated_center = object.center_mm();
        assert!((center.0 - rotated_center.0).abs() < 0.0001);
        assert!((center.1 - rotated_center.1).abs() < 0.0001);
        doc.validate().unwrap();
        let json = serde_json::to_string(&doc).unwrap();
        assert_eq!(serde_json::from_str::<LabelDocument>(&json).unwrap(), doc);
        doc.objects[0].rotate_to(-12.5);
        assert_eq!(doc.objects[0].rotation, 347.5);
    }

    #[test]
    fn whole_degree_serialization_remains_compatible_and_nonfinite_angles_are_rejected() {
        let mut doc = from_template(1);
        doc.objects[0].rotation = 90.;
        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(json["objects"][0]["rotation"].as_u64(), Some(90));
        assert_eq!(
            serde_json::from_value::<LabelDocument>(json)
                .unwrap()
                .objects[0]
                .rotation,
            90.
        );
        doc.objects[0].rotation = f32::NAN;
        assert!(doc.validate().is_err());
        doc.objects[0].rotation = f32::INFINITY;
        assert!(doc.validate().is_err());
    }
}
