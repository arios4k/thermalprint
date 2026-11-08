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
    Ean8,
    /// Interleaved 2 of 5: digits only, in an even count.
    Itf,
    Codabar,
    Code93,
    /// A compact square 2D symbol (ECC 200) for small parts and labels.
    DataMatrix,
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
    /// A locked object cannot be moved, resized, rotated, or deleted from the canvas.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Mirror the object before it is rotated.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_x: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_y: bool,
    /// Per-type options added after format 1; defaults keep older files unchanged.
    #[serde(default, skip_serializing_if = "ObjectOptions::is_default")]
    pub options: ObjectOptions,
}
fn is_false(value: &bool) -> bool {
    !*value
}

/// How an image is reduced to the black and white a thermal printer prints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageMode {
    /// Draw the picture as it is and let the printer driver convert it.
    #[default]
    Original,
    /// Every pixel becomes black or white around a cut-off.
    Threshold,
    /// Error-diffusion dithering, which keeps shading in photos.
    Dither,
}
/// QR error-correction level: more correction survives more damage but needs a bigger code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QrLevel {
    Low,
    #[default]
    Medium,
    Quartile,
    High,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectOptions {
    /// Text: break long lines at word ends so they fit the box width.
    pub wrap: bool,
    /// Text: shrink the font until the text fits the box.
    pub fit: bool,
    /// Text: multiplier on the normal line height.
    pub line_spacing: f32,
    pub italic: bool,
    /// Text: white letters on a black box.
    pub inverse: bool,
    pub image_mode: ImageMode,
    /// Image: -1 (flat) to 1 (harsh); 0 leaves contrast unchanged.
    pub contrast: f32,
    /// Image: the black and white cut-off for Threshold, 0 to 255.
    pub threshold: u8,
    /// Image: swap black and white.
    pub invert: bool,
    /// Barcode: readable text size in points; 0 chooses a size automatically.
    pub barcode_text_pt: f32,
    /// Barcode: put the readable text above the bars.
    pub text_above: bool,
    pub qr_level: QrLevel,
}
impl Default for ObjectOptions {
    fn default() -> Self {
        Self {
            wrap: false,
            fit: false,
            line_spacing: 1.,
            italic: false,
            inverse: false,
            image_mode: ImageMode::Original,
            contrast: 0.,
            threshold: 128,
            invert: false,
            barcode_text_pt: 0.,
            text_above: false,
            qr_level: QrLevel::Medium,
        }
    }
}
impl ObjectOptions {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn validate(&self) -> Result<()> {
        if !self.line_spacing.is_finite() || !(0.5..=3.).contains(&self.line_spacing) {
            bail!("Invalid line spacing");
        }
        if !self.contrast.is_finite() || !(-1.0..=1.).contains(&self.contrast) {
            bail!("Invalid image contrast");
        }
        if !self.barcode_text_pt.is_finite()
            || !(self.barcode_text_pt == 0. || (2.0..=60.).contains(&self.barcode_text_pt))
        {
            bail!("Invalid barcode text size");
        }
        Ok(())
    }
}

/// Where objects are aligned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignTo {
    /// The combined outline of the selected objects.
    Selection,
    /// The label itself.
    Label,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    CenterHorizontally,
    Right,
    Top,
    CenterVertically,
    Bottom,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

fn outline(object: &DesignObject) -> (f32, f32, f32, f32) {
    let (width, height) = object.rotated_size_mm();
    (
        object.x_mm,
        object.y_mm,
        object.x_mm + width,
        object.y_mm + height,
    )
}

impl LabelDocument {
    /// Align the chosen unlocked objects. Rotated objects are aligned by their outline.
    pub fn align(&mut self, ids: &[u64], alignment: Alignment, to: AlignTo) {
        let chosen: Vec<usize> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| ids.contains(&o.id) && !o.locked)
            .map(|(index, _)| index)
            .collect();
        if chosen.is_empty() || (to == AlignTo::Selection && chosen.len() < 2) {
            return;
        }
        let frame = match to {
            AlignTo::Label => (0., 0., self.width_mm, self.height_mm),
            AlignTo::Selection => chosen
                .iter()
                .map(|&i| outline(&self.objects[i]))
                .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, b| {
                    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
                }),
        };
        for index in chosen {
            let object = &mut self.objects[index];
            let (x0, y0, x1, y1) = outline(object);
            match alignment {
                Alignment::Left => object.x_mm += frame.0 - x0,
                Alignment::Right => object.x_mm += frame.2 - x1,
                Alignment::CenterHorizontally => {
                    object.x_mm += (frame.0 + frame.2) / 2. - (x0 + x1) / 2.
                }
                Alignment::Top => object.y_mm += frame.1 - y0,
                Alignment::Bottom => object.y_mm += frame.3 - y1,
                Alignment::CenterVertically => {
                    object.y_mm += (frame.1 + frame.3) / 2. - (y0 + y1) / 2.
                }
            }
        }
    }

    /// Spread three or more unlocked objects so the gaps between them are equal. The two
    /// outermost objects stay where they are.
    pub fn distribute(&mut self, ids: &[u64], axis: Axis) {
        let mut chosen: Vec<usize> = self
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| ids.contains(&o.id) && !o.locked)
            .map(|(index, _)| index)
            .collect();
        if chosen.len() < 3 {
            return;
        }
        let span = |o: &DesignObject| {
            let (x0, y0, x1, y1) = outline(o);
            match axis {
                Axis::Horizontal => (x0, x1),
                Axis::Vertical => (y0, y1),
            }
        };
        chosen.sort_by(|&a, &b| {
            let (a, b) = (span(&self.objects[a]), span(&self.objects[b]));
            (a.0 + a.1).total_cmp(&(b.0 + b.1))
        });
        let first = span(&self.objects[chosen[0]]).0;
        let last = span(&self.objects[*chosen.last().unwrap()]).1;
        let sizes: f32 = chosen
            .iter()
            .map(|&i| span(&self.objects[i]).1 - span(&self.objects[i]).0)
            .sum();
        let gap = (last - first - sizes) / (chosen.len() - 1) as f32;
        let mut cursor = first;
        for index in chosen {
            let (start, end) = span(&self.objects[index]);
            let object = &mut self.objects[index];
            match axis {
                Axis::Horizontal => object.x_mm += cursor - start,
                Axis::Vertical => object.y_mm += cursor - start,
            }
            cursor += end - start + gap;
        }
    }

    /// Ids of objects whose outline reaches into the printer's unprintable border, given as
    /// (left, top, right, bottom) margins in millimeters.
    pub fn objects_in_margins(&self, margins: (f32, f32, f32, f32)) -> Vec<u64> {
        let (left, top, right, bottom) = margins;
        let tolerance = 0.05;
        self.objects
            .iter()
            .filter(|o| {
                let (x0, y0, x1, y1) = outline(o);
                x0 < left - tolerance
                    || y0 < top - tolerance
                    || x1 > self.width_mm - right + tolerance
                    || y1 > self.height_mm - bottom + tolerance
            })
            .map(|o| o.id)
            .collect()
    }

    /// Move the chosen unlocked objects by a distance in millimeters.
    pub fn nudge(&mut self, ids: &[u64], dx_mm: f32, dy_mm: f32) {
        for object in self
            .objects
            .iter_mut()
            .filter(|o| ids.contains(&o.id) && !o.locked)
        {
            object.x_mm += dx_mm;
            object.y_mm += dy_mm;
        }
    }
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
    /// Spacing of the guide grid in millimeters; snapping moves in steps of a fifth of it.
    #[serde(default = "default_grid")]
    pub grid_mm: f32,
    /// For continuous rolls: print only as long as the content, plus a bottom margin.
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto_length: bool,
    #[serde(
        default = "default_auto_margin",
        skip_serializing_if = "is_default_auto_margin"
    )]
    pub auto_margin_mm: f32,
}
pub const DEFAULT_AUTO_MARGIN_MM: f32 = 2.;
fn default_auto_margin() -> f32 {
    DEFAULT_AUTO_MARGIN_MM
}
fn is_default_auto_margin(value: &f32) -> bool {
    (*value - DEFAULT_AUTO_MARGIN_MM).abs() < f32::EPSILON
}
pub const DEFAULT_GRID_MM: f32 = 5.;
fn default_grid() -> f32 {
    DEFAULT_GRID_MM
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
            grid_mm: DEFAULT_GRID_MM,
            auto_length: false,
            auto_margin_mm: DEFAULT_AUTO_MARGIN_MM,
        }
    }
    /// The length that is printed: the set height, or with auto-length the lowest edge of
    /// any object plus the bottom margin (never less than 5 mm).
    pub fn effective_height_mm(&self) -> f32 {
        if !self.auto_length {
            return self.height_mm;
        }
        let lowest = self.objects.iter().map(|o| outline(o).3).fold(0., f32::max);
        (lowest + self.auto_margin_mm).clamp(5., 2000.)
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
            locked: false,
            flip_x: false,
            flip_y: false,
            options: ObjectOptions::default(),
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
        if !self.grid_mm.is_finite() || !(0.5..=50.).contains(&self.grid_mm) {
            bail!("Invalid grid size");
        }
        if !self.auto_margin_mm.is_finite() || !(0. ..=50.).contains(&self.auto_margin_mm) {
            bail!("Invalid bottom margin");
        }
        let mut seen = std::collections::HashSet::new();
        for o in &self.objects {
            o.options.validate()?;
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

#[cfg(test)]
mod arrangement_tests {
    use super::*;

    fn rect(doc: &mut LabelDocument, x: f32, y: f32, w: f32, h: f32) -> u64 {
        doc.add(
            Element::Shape {
                kind: ShapeKind::Rectangle,
                stroke_mm: 0.4,
                filled: false,
            },
            x,
            y,
            w,
            h,
        )
    }
    fn bounds(doc: &LabelDocument, id: u64) -> (f32, f32, f32, f32) {
        outline(doc.objects.iter().find(|o| o.id == id).unwrap())
    }

    #[test]
    fn objects_align_to_each_edge_and_center_of_the_selection() {
        let mut doc = LabelDocument::blank(100., 100.);
        let (a, b) = (
            rect(&mut doc, 10., 20., 10., 10.),
            rect(&mut doc, 40., 50., 20., 5.),
        );
        let both = [a, b];
        doc.align(&both, Alignment::Left, AlignTo::Selection);
        assert_eq!((bounds(&doc, a).0, bounds(&doc, b).0), (10., 10.));
        doc.align(&both, Alignment::Right, AlignTo::Selection);
        assert_eq!((bounds(&doc, a).2, bounds(&doc, b).2), (30., 30.));
        doc.align(&both, Alignment::Top, AlignTo::Selection);
        assert_eq!((bounds(&doc, a).1, bounds(&doc, b).1), (20., 20.));
        doc.align(&both, Alignment::Bottom, AlignTo::Selection);
        assert_eq!((bounds(&doc, a).3, bounds(&doc, b).3), (30., 30.));
        doc.align(&both, Alignment::CenterVertically, AlignTo::Selection);
        let (ay, by) = (bounds(&doc, a), bounds(&doc, b));
        assert_eq!((ay.1 + ay.3) / 2., (by.1 + by.3) / 2.);
    }

    #[test]
    fn centering_on_the_label_works_for_one_object_and_uses_the_rotated_outline() {
        let mut doc = LabelDocument::blank(100., 60.);
        let id = rect(&mut doc, 0., 0., 20., 10.);
        doc.align(&[id], Alignment::CenterHorizontally, AlignTo::Label);
        doc.align(&[id], Alignment::CenterVertically, AlignTo::Label);
        assert_eq!(bounds(&doc, id), (40., 25., 60., 35.));
        doc.objects[0].rotate_to(90.);
        doc.align(&[id], Alignment::CenterHorizontally, AlignTo::Label);
        doc.align(&[id], Alignment::CenterVertically, AlignTo::Label);
        assert_eq!(bounds(&doc, id), (45., 20., 55., 40.));
        // Aligning a single object to its own selection does nothing.
        doc.align(&[id], Alignment::Left, AlignTo::Selection);
        assert_eq!(bounds(&doc, id), (45., 20., 55., 40.));
    }

    #[test]
    fn distributing_equalizes_the_gaps_and_keeps_the_outer_objects() {
        let mut doc = LabelDocument::blank(200., 50.);
        let a = rect(&mut doc, 0., 0., 10., 10.);
        let b = rect(&mut doc, 20., 0., 30., 10.);
        let c = rect(&mut doc, 70., 0., 10., 10.);
        doc.distribute(&[a, b, c], Axis::Horizontal);
        let (ab, bb, cb) = (bounds(&doc, a), bounds(&doc, b), bounds(&doc, c));
        assert_eq!((ab.0, cb.2), (0., 80.), "outer objects stay put");
        assert!((bb.0 - ab.2 - (cb.0 - bb.2)).abs() < 1e-4, "equal gaps");
        // Two objects cannot be distributed.
        let before = doc.clone();
        doc.distribute(&[a, b], Axis::Horizontal);
        assert_eq!(doc, before);
    }

    #[test]
    fn objects_touching_the_unprintable_border_are_reported() {
        let mut doc = LabelDocument::blank(100., 60.);
        let inside = rect(&mut doc, 5., 5., 10., 10.);
        let left_edge = rect(&mut doc, 1., 20., 10., 10.);
        let bottom_edge = rect(&mut doc, 20., 52., 10., 8.);
        let margins = (2.5, 2.5, 2.5, 2.0);
        assert_eq!(doc.objects_in_margins(margins), [left_edge, bottom_edge]);
        assert!(!doc.objects_in_margins(margins).contains(&inside));
        assert!(doc.objects_in_margins((0., 0., 0., 0.)).is_empty());
    }

    #[test]
    fn locked_objects_are_never_moved() {
        let mut doc = LabelDocument::blank(100., 100.);
        let (a, b) = (
            rect(&mut doc, 10., 10., 10., 10.),
            rect(&mut doc, 50., 50., 10., 10.),
        );
        doc.objects[1].locked = true;
        doc.align(&[a, b], Alignment::Left, AlignTo::Label);
        assert_eq!(bounds(&doc, b).0, 50.);
        assert_eq!(bounds(&doc, a).0, 0.);
        doc.nudge(&[a, b], 1., 2.);
        assert_eq!((bounds(&doc, a).0, bounds(&doc, a).1), (1., 12.));
        assert_eq!((bounds(&doc, b).0, bounds(&doc, b).1), (50., 50.));
    }

    #[test]
    fn older_files_load_and_new_options_survive_a_round_trip() {
        let old = r#"{"version":1,"title":"t","width_mm":50.0,"height_mm":30.0,"serial_number":1,"objects":[{"id":1,"x_mm":1.0,"y_mm":1.0,"width_mm":10.0,"height_mm":5.0,"rotation":0,"element":{"type":"Text","text":"x","font_pt":10.0,"bold":false,"align":"Left"}}],"next_id":2}"#;
        let doc: LabelDocument = serde_json::from_str(old).unwrap();
        doc.validate().unwrap();
        assert_eq!(
            (
                doc.grid_mm,
                doc.objects[0].locked,
                doc.objects[0].options.clone()
            ),
            (5., false, ObjectOptions::default())
        );
        let mut changed = doc.clone();
        changed.objects[0].locked = true;
        changed.objects[0].flip_x = true;
        changed.objects[0].options.wrap = true;
        changed.objects[0].options.image_mode = ImageMode::Dither;
        changed.grid_mm = 2.;
        let back: LabelDocument =
            serde_json::from_str(&serde_json::to_string(&changed).unwrap()).unwrap();
        assert_eq!(back, changed);
        let plain = serde_json::to_string(&doc).unwrap();
        assert!(
            !plain.contains("locked") && !plain.contains("options"),
            "defaults are not written"
        );
    }

    #[test]
    fn out_of_range_options_are_rejected() {
        let mut doc = LabelDocument::blank(50., 30.);
        let id = rect(&mut doc, 1., 1., 5., 5.);
        doc.objects
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .options
            .line_spacing = 9.;
        assert!(doc.validate().is_err());
        doc.objects[0].options.line_spacing = 1.;
        doc.grid_mm = 0.;
        assert!(doc.validate().is_err());
    }
}
