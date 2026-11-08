//! Preview pictures of saved designs for file managers.
//!
//! Desktop file managers run a "thumbnailer" program for file types they cannot preview.
//! `thermalprint --thumbnail INPUT OUTPUT SIZE` draws the label and saves it as a PNG that
//! fits in a SIZE × SIZE square. It never opens a window and never touches the network.
use crate::{design::LabelDocument, design_render};
use anyhow::{Context, Result, bail};
use std::path::Path;

pub const MIN_SIZE: u32 = 16;
pub const MAX_SIZE: u32 = 1024;

pub fn write(input: &Path, output: &Path, size: u32) -> Result<()> {
    if !(MIN_SIZE..=MAX_SIZE).contains(&size) {
        bail!("The thumbnail size must be {MIN_SIZE} to {MAX_SIZE} pixels");
    }
    let design =
        LabelDocument::load(input).context("This is not a readable ThermalPrint design")?;
    let rendered = design_render::render_label(&design, 203)?;
    let longest = rendered.image.width().max(rendered.image.height());
    let image = if longest > size {
        let scale = size as f32 / longest as f32;
        image::imageops::resize(
            &rendered.image,
            ((rendered.image.width() as f32 * scale).round() as u32).max(1),
            ((rendered.image.height() as f32 * scale).round() as u32).max(1),
            image::imageops::FilterType::Triangle,
        )
    } else {
        rendered.image
    };
    image
        .save_with_format(output, image::ImageFormat::Png)
        .context("The thumbnail could not be saved")
}

/// Handle `--thumbnail INPUT OUTPUT SIZE` when it is what the program was started with.
/// Returns the exit code to use, or `None` for a normal start.
pub fn run_from_args(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("--thumbnail") {
        return None;
    }
    let [_, _, input, output, size] = args else {
        eprintln!("Usage: thermalprint --thumbnail INPUT OUTPUT SIZE");
        return Some(2);
    };
    let Ok(size) = size.parse::<u32>() else {
        eprintln!("The size must be a whole number of pixels");
        return Some(2);
    };
    match write(Path::new(input), Path::new(output), size) {
        Ok(()) => Some(0),
        Err(error) => {
            eprintln!("{error:#}");
            Some(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{Element, TextAlign};

    fn saved_design(dir: &Path, width: f32, height: f32) -> std::path::PathBuf {
        let mut doc = LabelDocument::blank(width, height);
        doc.add(
            Element::Text {
                text: "Hello".into(),
                font_pt: 16.,
                bold: true,
                align: TextAlign::Left,
            },
            2.,
            2.,
            width - 4.,
            10.,
        );
        let path = dir.join("label.thermalprint");
        doc.save(&path).unwrap();
        path
    }

    #[test]
    fn a_design_becomes_a_png_that_fits_the_requested_square() {
        let dir = tempfile::tempdir().unwrap();
        let input = saved_design(dir.path(), 101.6, 152.4);
        let output = dir.path().join("out.png");
        write(&input, &output, 128).unwrap();
        let png = image::open(&output).unwrap();
        assert_eq!(png.height(), 128, "the long side fills the square");
        assert!((png.width() as f32 / png.height() as f32 - 101.6 / 152.4).abs() < 0.02);
        assert!(
            png.to_rgb8().pixels().any(|p| p.0[0] < 100),
            "the text is drawn"
        );
    }

    #[test]
    fn a_small_label_is_not_enlarged_and_bad_input_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let input = saved_design(dir.path(), 20., 10.);
        let output = dir.path().join("small.png");
        write(&input, &output, 1024).unwrap();
        assert!(image::open(&output).unwrap().width() < 1024);
        assert!(write(&input, &output, 8).is_err());
        assert!(write(&input, &output, 5000).is_err());
        let junk = dir.path().join("junk.thermalprint");
        std::fs::write(&junk, "not json").unwrap();
        assert!(write(&junk, &dir.path().join("x.png"), 128).is_err());
        assert!(write(&dir.path().join("missing"), &dir.path().join("x.png"), 128).is_err());
    }

    #[test]
    fn the_command_line_is_recognised_and_checked() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(run_from_args(&args(&["thermalprint"])), None);
        assert_eq!(
            run_from_args(&args(&["thermalprint", "label.thermalprint"])),
            None
        );
        assert_eq!(
            run_from_args(&args(&["thermalprint", "--thumbnail"])),
            Some(2)
        );
        assert_eq!(
            run_from_args(&args(&["thermalprint", "--thumbnail", "a", "b", "big"])),
            Some(2)
        );
        assert_eq!(
            run_from_args(&args(&[
                "thermalprint",
                "--thumbnail",
                "/nonexistent",
                "/tmp/x.png",
                "64"
            ])),
            Some(1)
        );
    }
}
