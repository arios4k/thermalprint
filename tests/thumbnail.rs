//! The thumbnailer is started by file managers with no display available.
use std::process::Command;
use thermalprint::design::{Element, LabelDocument, TextAlign};

#[test]
fn the_program_draws_a_thumbnail_without_a_display() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = LabelDocument::blank(60., 40.);
    doc.add(
        Element::Text {
            text: "Thumbnail".into(),
            font_pt: 18.,
            bold: true,
            align: TextAlign::Left,
        },
        3.,
        3.,
        50.,
        12.,
    );
    let input = dir.path().join("sample.thermalprint");
    doc.save(&input).unwrap();
    let output = dir.path().join("sample.png");
    let status = Command::new(env!("CARGO_BIN_EXE_thermalprint"))
        .args(["--thumbnail"])
        .arg(&input)
        .arg(&output)
        .arg("256")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .status()
        .unwrap();
    assert!(status.success());
    let png = image::open(&output).unwrap();
    assert_eq!(png.width(), 256);
    let bad = Command::new(env!("CARGO_BIN_EXE_thermalprint"))
        .args(["--thumbnail", "/nonexistent.thermalprint"])
        .arg(dir.path().join("x.png"))
        .arg("256")
        .env_remove("DISPLAY")
        .status()
        .unwrap();
    assert_eq!(bad.code(), Some(1));
}
