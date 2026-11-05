//! Import at print resolution; textures are derived separately by the UI.
use anyhow::{Context, Result, bail};
use image::{DynamicImage, RgbImage};
use pdfium_render::prelude::*;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub const RENDER_DPI: u32 = 300;
const MAX_PIXELS: u64 = 45_000_000;
#[derive(Clone)]
pub struct Page {
    pub image: RgbImage,
    pub width_mm: f32,
    pub height_mm: f32,
    pub index: usize,
    pub count: usize,
    pub path: PathBuf,
}
fn opaque(image: DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u16;
        image::Rgb(std::array::from_fn(|i| {
            ((p[i] as u16 * a + 255 * (255 - a) + 127) / 255) as u8
        }))
    })
}
pub fn load(path: &Path, index: usize) -> Result<Page> {
    if std::fs::metadata(path)?.len() > 256 * 1024 * 1024 {
        bail!("Document exceeds the 256 MB import limit");
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        match load_pdfium(path, index) {
            Ok(page) => return Ok(page),
            Err(e) => {
                tracing::debug!(error=%e,"PDFium unavailable or import failed; trying Poppler");
            }
        }
        return load_poppler(path,index).context("Cannot render PDF. Install libpdfium.so or Poppler utilities (pdfinfo and pdftoppm). Encrypted or damaged PDFs may need to be re-exported.");
    }
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(20000);
    limits.max_image_height = Some(20000);
    reader.limits(limits);
    let img = reader.decode().context("Unsupported or damaged image")?;
    if img.width() as u64 * img.height() as u64 > MAX_PIXELS {
        bail!("Image exceeds the 45 megapixel limit");
    }
    // Raster physical size is ambiguous; use 300 DPI and surface this in the UI.
    Ok(Page {
        width_mm: img.width() as f32 / 300. * 25.4,
        height_mm: img.height() as f32 / 300. * 25.4,
        image: opaque(img),
        index: 0,
        count: 1,
        path: path.into(),
    })
}
fn load_pdfium(path: &Path, index: usize) -> Result<Page> {
    let bindings = if let Some(dir) = std::env::var_os("THERMALPRINT_PDFIUM_DIR") {
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir))?
    } else {
        Pdfium::bind_to_system_library()?
    };
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium.load_pdf_from_file(path, None)?;
    let count = doc.pages().len() as usize;
    if index >= count {
        bail!("Page is outside the document");
    }
    let page = doc.pages().get(index.try_into()?)?;
    let width_mm = page.width().value / 72. * 25.4;
    let height_mm = page.height().value / 72. * 25.4;
    let w = (width_mm / 25.4 * RENDER_DPI as f32).ceil() as i32;
    let h = (height_mm / 25.4 * RENDER_DPI as f32).ceil() as i32;
    if w <= 0 || h <= 0 || w as u64 * h as u64 > MAX_PIXELS {
        bail!("PDF page exceeds the 45 megapixel render limit");
    }
    let image = opaque(
        page.render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(w)
                .set_target_height(h),
        )?
        .as_image()?,
    );
    Ok(Page {
        image,
        width_mm,
        height_mm,
        index,
        count,
        path: path.into(),
    })
}
fn load_poppler(path: &Path, index: usize) -> Result<Page> {
    let path = std::fs::canonicalize(path)?;
    let info = Command::new("pdfinfo")
        .arg(&path)
        .env("LC_ALL", "C")
        .output()?;
    if !info.status.success() {
        bail!("pdfinfo: {}", String::from_utf8_lossy(&info.stderr));
    }
    let info = String::from_utf8_lossy(&info.stdout);
    let count = info
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .context("PDF has no page count")?
        .trim()
        .parse::<usize>()?;
    if index >= count {
        bail!("Page is outside the document");
    }
    // Query this page: mixed-size documents must not inherit page one's dimensions.
    let n = (index + 1).to_string();
    let info = Command::new("pdfinfo")
        .args(["-f", &n, "-l", &n])
        .arg(&path)
        .env("LC_ALL", "C")
        .output()?;
    let text = String::from_utf8_lossy(&info.stdout);
    let size = text
        .lines()
        .find(|l| l.contains(" size:") || l.starts_with("Page size:"))
        .context("Missing PDF page dimensions")?;
    let dims = size
        .split_once("size:")
        .unwrap()
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    let mut width_mm = dims[0].parse::<f32>()? / 72. * 25.4;
    let mut height_mm = dims[2].parse::<f32>()? / 72. * 25.4;
    let rotation = text
        .lines()
        .find(|l| l.contains(" rot:"))
        .and_then(|l| l.split_whitespace().last())
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    if rotation.rem_euclid(180) == 90 {
        std::mem::swap(&mut width_mm, &mut height_mm);
    }
    if !width_mm.is_finite()
        || !height_mm.is_finite()
        || width_mm <= 0.
        || height_mm <= 0.
        || width_mm * height_mm / 25.4f32.powi(2) * (RENDER_DPI as f32).powi(2) > MAX_PIXELS as f32
    {
        bail!("PDF page exceeds the render limit");
    }
    let dir = tempfile::tempdir()?;
    let prefix = dir.path().join("page");
    let result = Command::new("pdftoppm")
        .args(["-f", &n, "-l", &n, "-singlefile", "-r", "300", "-png"])
        .arg(&path)
        .arg(&prefix)
        .output()?;
    if !result.status.success() {
        bail!(
            "PDF render failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let image = image::open(prefix.with_extension("png"))?.to_rgb8();
    Ok(Page {
        image,
        width_mm,
        height_mm,
        index,
        count,
        path,
    })
}
