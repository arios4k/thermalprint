//! Ready-made shipping symbols drawn as black shapes, and your own saved logos.
//! A symbol is added to a label as an ordinary image, so it prints and exports like any logo.
use anyhow::{Context, Result, bail};
use image::{GrayAlphaImage, LumaA, RgbaImage};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Fragile,
    ThisSideUp,
    KeepDry,
    Recycling,
    Hazard,
}

pub const ALL: [Symbol; 5] = [
    Symbol::Fragile,
    Symbol::ThisSideUp,
    Symbol::KeepDry,
    Symbol::Recycling,
    Symbol::Hazard,
];

impl Symbol {
    pub fn name(self) -> &'static str {
        match self {
            Symbol::Fragile => "Fragile",
            Symbol::ThisSideUp => "This side up",
            Symbol::KeepDry => "Keep dry",
            Symbol::Recycling => "Recycling",
            Symbol::Hazard => "Hazard",
        }
    }
}

type P = (f32, f32);

/// What to draw, in a 100 × 100 space: a line with round ends, or a solid polygon.
enum Shape {
    Line(Vec<P>, f32),
    Fill(Vec<P>),
}

fn arc(center: P, radius: f32, from_deg: f32, to_deg: f32, steps: usize) -> Vec<P> {
    (0..=steps)
        .map(|i| {
            let a = (from_deg + (to_deg - from_deg) * i as f32 / steps as f32).to_radians();
            (center.0 + radius * a.cos(), center.1 - radius * a.sin())
        })
        .collect()
}

fn lerp(a: P, b: P, t: f32) -> P {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

fn arrow_head(tip: P, from: P, size: f32) -> Vec<P> {
    let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (ux, uy) = (dx / length, dy / length);
    let base = (tip.0 - ux * size, tip.1 - uy * size);
    let (nx, ny) = (-uy, ux);
    vec![
        tip,
        (base.0 + nx * size * 0.62, base.1 + ny * size * 0.62),
        (base.0 - nx * size * 0.62, base.1 - ny * size * 0.62),
    ]
}

fn shapes(symbol: Symbol) -> Vec<Shape> {
    match symbol {
        Symbol::Fragile => vec![
            Shape::Line(
                vec![
                    (26., 12.),
                    (74., 12.),
                    (64., 44.),
                    (50., 54.),
                    (36., 44.),
                    (26., 12.),
                ],
                5.,
            ),
            Shape::Line(vec![(50., 54.), (50., 82.)], 5.),
            Shape::Line(vec![(30., 90.), (70., 90.)], 5.),
            Shape::Line(vec![(38., 12.), (46., 24.), (40., 32.), (48., 42.)], 3.5),
        ],
        Symbol::ThisSideUp => {
            let mut out = Vec::new();
            for x in [30., 70.] {
                out.push(Shape::Line(vec![(x, 88.), (x, 36.)], 7.));
                out.push(Shape::Fill(vec![(x, 10.), (x + 20., 42.), (x - 20., 42.)]));
            }
            out.push(Shape::Line(vec![(10., 94.), (90., 94.)], 4.));
            out
        }
        Symbol::KeepDry => {
            let mut canopy = arc((50., 50.), 38., 0., 180., 24);
            canopy.push((12., 50.));
            let mut out = vec![
                Shape::Line(canopy, 5.),
                Shape::Line(vec![(50., 12.), (50., 12.)], 5.),
                Shape::Line(vec![(50., 50.), (50., 78.)], 5.),
                Shape::Line(arc((43., 78.), 7., 0., -180., 10), 5.),
            ];
            for (x, y) in [(18., 66.), (34., 62.), (66., 66.), (82., 62.)] {
                out.push(Shape::Line(vec![(x, y), (x - 3., y + 9.)], 4.));
            }
            out
        }
        Symbol::Recycling => {
            let corners = [(50., 14.), (84., 74.), (16., 74.)];
            let mut out = Vec::new();
            for i in 0..3 {
                let (a, b) = (corners[i], corners[(i + 1) % 3]);
                let start = lerp(a, b, 0.16);
                let end = lerp(a, b, 0.74);
                out.push(Shape::Line(vec![start, end], 7.));
                out.push(Shape::Fill(arrow_head(lerp(a, b, 0.92), start, 17.)));
            }
            out
        }
        Symbol::Hazard => vec![
            Shape::Line(vec![(50., 8.), (93., 86.), (7., 86.), (50., 8.)], 6.),
            Shape::Line(vec![(50., 36.), (50., 62.)], 8.),
            Shape::Fill(arc((50., 74.), 5., 0., 360., 16)),
        ],
    }
}

fn distance_to_segment(p: P, a: P, b: P) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let length_sq = abx * abx + aby * aby;
    let t = if length_sq < 1e-6 {
        0.
    } else {
        (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / length_sq).clamp(0., 1.)
    };
    let (cx, cy) = (a.0 + abx * t, a.1 + aby * t);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

fn inside(p: P, polygon: &[P]) -> bool {
    let mut crossings = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[j]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            crossings = !crossings;
        }
        j = i;
    }
    crossings
}

/// The symbol as black shapes on a transparent square of `size` pixels.
pub fn render(symbol: Symbol, size: u32) -> RgbaImage {
    let size = size.clamp(16, 2048);
    let shapes = shapes(symbol);
    let mut gray = GrayAlphaImage::new(size, size);
    let scale = 100. / size as f32;
    const SAMPLES: u32 = 3;
    for y in 0..size {
        for x in 0..size {
            let mut covered = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let p = (
                        (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) * scale,
                        (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) * scale,
                    );
                    let hit = shapes.iter().any(|shape| match shape {
                        Shape::Line(points, width) => points
                            .windows(2)
                            .any(|w| distance_to_segment(p, w[0], w[1]) <= width / 2.),
                        Shape::Fill(polygon) => inside(p, polygon),
                    });
                    covered += hit as u32;
                }
            }
            let alpha = (covered * 255 / (SAMPLES * SAMPLES)) as u8;
            gray.put_pixel(x, y, LumaA([0, alpha]));
        }
    }
    image::DynamicImage::ImageLumaA8(gray).to_rgba8()
}

pub fn png_bytes(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    image.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )?;
    Ok(bytes)
}

// Your own logos: PNG files in a `logos` folder next to the settings.
pub const MAX_LOGOS: usize = 60;
const MAX_LOGO_BYTES: u64 = 9_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Logo {
    pub name: String,
    pub path: PathBuf,
}

pub fn logos_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "ThermalPrint", "ThermalPrint")
        .map(|dirs| dirs.config_dir().join("logos"))
}

fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned: String = cleaned.trim().chars().take(50).collect();
    if cleaned.is_empty() {
        "Logo".into()
    } else {
        cleaned
    }
}

pub fn list_logos_in(dir: &Path) -> Vec<Logo> {
    let mut found: Vec<Logo> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            let is_png = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"));
            if !(metadata.is_file() && is_png && metadata.len() <= MAX_LOGO_BYTES) {
                return None;
            }
            Some(Logo {
                name: path.file_stem()?.to_string_lossy().into_owned(),
                path,
            })
        })
        .collect();
    found.sort_by_key(|l| l.name.to_lowercase());
    found.truncate(MAX_LOGOS);
    found
}

/// Save an image (any format the app can read) as a PNG logo. The same name replaces the old one.
pub fn save_logo_in(dir: &Path, name: &str, image_bytes: &[u8]) -> Result<PathBuf> {
    if image_bytes.len() as u64 > MAX_LOGO_BYTES {
        bail!("The image is larger than 9 MB");
    }
    let decoded = image::load_from_memory(image_bytes).context("The image could not be read")?;
    std::fs::create_dir_all(dir).context("Could not create the logos folder")?;
    let path = dir.join(format!("{}.png", safe_name(name)));
    if !path.exists() && list_logos_in(dir).len() >= MAX_LOGOS {
        bail!("You already have {MAX_LOGOS} saved logos. Remove one first");
    }
    decoded.save_with_format(&path, image::ImageFormat::Png)?;
    Ok(path)
}

pub fn remove_logo_in(dir: &Path, path: &Path) -> Result<()> {
    let inside = path.parent().is_some_and(|parent| parent == dir);
    if !inside
        || path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("png"))
    {
        bail!("That file is not one of your logos");
    }
    std::fs::remove_file(path).context("Could not remove the logo")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(image: &RgbaImage) -> usize {
        image.pixels().filter(|p| p.0[3] > 128).count()
    }

    #[test]
    fn every_symbol_draws_black_shapes_with_room_around_them() {
        for symbol in ALL {
            let image = render(symbol, 96);
            assert_eq!(image.dimensions(), (96, 96));
            let covered = ink(&image);
            assert!(
                (300..6000).contains(&covered),
                "{symbol:?} covers {covered} pixels"
            );
            assert!(image.pixels().all(|p| p.0[0] == 0), "only black ink");
            // Not clipped at the edges.
            for x in 0..96 {
                assert_eq!(
                    image.get_pixel(x, 0).0[3],
                    0,
                    "{symbol:?} touches the top edge"
                );
            }
        }
    }

    #[test]
    fn symbols_differ_and_scale() {
        let a = render(Symbol::Fragile, 64);
        let b = render(Symbol::Hazard, 64);
        assert_ne!(a.as_raw(), b.as_raw());
        let small = ink(&render(Symbol::Recycling, 64)) as f32;
        let large = ink(&render(Symbol::Recycling, 128)) as f32;
        assert!((large / small - 4.).abs() < 0.6, "{small} {large}");
        assert_eq!(render(Symbol::Hazard, 1).width(), 16);
        let png = png_bytes(&a).unwrap();
        assert!(image::load_from_memory(&png).is_ok());
    }

    #[test]
    fn logos_are_saved_listed_replaced_and_removed_safely() {
        let dir = tempfile::tempdir().unwrap();
        let png = png_bytes(&render(Symbol::Hazard, 32)).unwrap();
        let path = save_logo_in(dir.path(), "My / shop logo", &png).unwrap();
        assert_eq!(path.file_name().unwrap(), "My _ shop logo.png");
        save_logo_in(dir.path(), "Another", &png).unwrap();
        save_logo_in(dir.path(), "My / shop logo", &png).unwrap();
        let names: Vec<_> = list_logos_in(dir.path())
            .into_iter()
            .map(|l| l.name)
            .collect();
        assert_eq!(names, ["Another", "My _ shop logo"]);
        assert!(save_logo_in(dir.path(), "Bad", b"not an image").is_err());
        let outside = tempfile::tempdir().unwrap();
        let stray = outside.path().join("x.png");
        std::fs::write(&stray, &png).unwrap();
        assert!(remove_logo_in(dir.path(), &stray).is_err());
        assert!(stray.exists());
        remove_logo_in(dir.path(), &path).unwrap();
        assert_eq!(list_logos_in(dir.path()).len(), 1);
    }
}
