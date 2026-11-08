//! CUPS client tools provide driver/IPP negotiation without linking libcups.
//! Arguments never pass through a shell. A successful lp call means submitted,
//! not physically printed; job status is queried separately.
use crate::{export, layout::Output, settings::Settings};
use anyhow::{Context, Result, bail};
use std::{
    collections::HashMap,
    io::{Read, Write},
    process::Command,
    time::Duration,
};
#[derive(Clone, Debug, Default)]
pub struct Printer {
    pub name: String,
    pub is_default: bool,
    pub description: String,
    pub media: Vec<String>,
    pub media_labels: HashMap<String, String>,
    pub media_sizes: HashMap<String, (f32, f32)>,
    /// Unprintable margins per media in millimeters: (left, bottom, right, top) as the driver
    /// reports them, measured from the paper edges.
    pub media_margins: HashMap<String, (f32, f32, f32, f32)>,
    pub resolutions: Vec<String>,
    pub resolution_option: String,
    pub custom_media: bool,
}
impl Printer {
    /// The unprintable border for a media choice as (left, top, right, bottom) in design
    /// orientation, or None when the driver does not say or the border is negligible.
    pub fn printable_margins_mm(&self, media: &str) -> Option<(f32, f32, f32, f32)> {
        let &(left, bottom, right, top) = self.media_margins.get(media)?;
        let largest = left.max(bottom).max(right).max(top);
        (largest >= 0.05).then_some((left, top, right, bottom))
    }
    pub fn matching_media(&self, width_mm: f32, height_mm: f32) -> Option<String> {
        self.media
            .iter()
            .find(|code| {
                self.media_sizes.get(*code).is_some_and(|&(w, h)| {
                    (w - width_mm).abs() < 0.5 && (h - height_mm).abs() < 0.5
                })
            })
            .cloned()
            .or_else(|| self.custom_media.then(|| "Custom".into()))
    }
    pub fn media_mismatch(&self, media: &str, width: f32, height: f32) -> Option<String> {
        if media == "Custom" {
            return None;
        }
        let &(w, h) = self.media_sizes.get(media)?;
        ((w - width).abs() >= 0.5 || (h - height).abs() >= 0.5).then(||
            format!("Output is {width:.1} × {height:.1} mm; printer media is {w:.1} × {h:.1} mm. Choose matching media or change the output dimensions."))
    }
    pub fn media_label(&self, code: &str) -> String {
        match code {
            "" => "Select media".into(),
            "Custom" => "Custom — match output dimensions".into(),
            _ => self
                .media_labels
                .get(code)
                .cloned()
                .unwrap_or_else(|| format!("{code} — driver-defined size")),
        }
    }
}

/// Read the public driver description through CUPS; the on-disk PPD may be root-only.
/// HTTP/1.0 avoids chunked transfer encoding, and the response is bounded.
fn driver_description(name: &str) -> Option<String> {
    let server = std::env::var("CUPS_SERVER").ok();
    driver_description_from(name, server.as_deref(), std::env::var_os("SNAP").is_none())
}

fn driver_description_from(
    name: &str,
    server: Option<&str>,
    allow_local_ppd: bool,
) -> Option<String> {
    if !name
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return None;
    }
    let socket = match server {
        Some(path) if path.starts_with('/') => path,
        Some(_) => return None, // Never substitute a local driver for a remote server.
        None => "/run/cups/cups.sock",
    };
    if allow_local_ppd && let Ok(ppd) = std::fs::read_to_string(format!("/etc/cups/ppd/{name}.ppd"))
    {
        return Some(ppd);
    }
    let mut stream = std::os::unix::net::UnixStream::connect(socket).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .ok()?;
    // One write keeps the request in a single packet for the server to read.
    stream
        .write_all(
            format!(
                "GET /printers/{name}.ppd HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .ok()?;
    let mut bytes = Vec::new();
    stream.take(4 * 1024 * 1024).read_to_end(&mut bytes).ok()?;
    let response = String::from_utf8_lossy(&bytes);
    let (header, body) = response.split_once("\r\n\r\n")?;
    if header.lines().next()?.split_whitespace().nth(1)? != "200"
        || !body.starts_with("*PPD-Adobe:")
    {
        return None;
    }
    Some(body.into())
}

fn media_sizes(ppd: &str) -> HashMap<String, (f32, f32)> {
    ppd.lines()
        .filter_map(|line| {
            let (choice, dimensions) = line.strip_prefix("*PaperDimension ")?.split_once(':')?;
            let code = choice.split('/').next()?.trim();
            let mut values = dimensions.trim().trim_matches('"').split_whitespace();
            let width: f32 = values.next()?.parse().ok()?;
            let height: f32 = values.next()?.parse().ok()?;
            (width.is_finite() && height.is_finite() && width > 0. && height > 0.)
                .then(|| (code.into(), (width * 25.4 / 72., height * 25.4 / 72.)))
        })
        .collect()
}

/// Unprintable margins from `*ImageableArea` lines, which give the printable rectangle in
/// points from the lower-left corner of the page.
fn media_margins(ppd: &str) -> HashMap<String, (f32, f32, f32, f32)> {
    let sizes = media_sizes(ppd);
    let to_mm = 25.4 / 72.;
    ppd.lines()
        .filter_map(|line| {
            let (choice, area) = line.strip_prefix("*ImageableArea ")?.split_once(':')?;
            let code = choice.split('/').next()?.trim();
            let &(width, height) = sizes.get(code)?;
            let values: Vec<f32> = area
                .trim()
                .trim_matches('"')
                .split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect();
            let [llx, lly, urx, ury] = values[..] else {
                return None;
            };
            if !values.iter().all(|v| v.is_finite()) {
                return None;
            }
            let margins = (
                (llx * to_mm).max(0.),
                (lly * to_mm).max(0.),
                (width - urx * to_mm).max(0.),
                (height - ury * to_mm).max(0.),
            );
            Some((code.to_owned(), margins))
        })
        .collect()
}

fn media_labels(ppd: &str) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    for line in ppd.lines() {
        let Some(choice) = line.strip_prefix("*PageSize ") else {
            continue;
        };
        let Some((choice, _)) = choice.split_once(':') else {
            continue;
        };
        let Some((code, description)) = choice.split_once('/') else {
            continue;
        };
        let description = description.trim();
        if !description.is_empty() {
            // Drivers often put a short name before a more useful dimension label.
            let description = description
                .split_once(" (")
                .and_then(|(_, rest)| rest.strip_suffix(')'))
                .filter(|text| text.contains("in") || text.contains("mm"))
                .unwrap_or(description);
            labels.insert(code.into(), description.replace(" x ", " × "));
        }
    }
    labels
}

fn run(program: &str, args: &[&str]) -> Result<String> {
    let o = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .with_context(|| {
            format!("Could not start {program}. Install the CUPS client utilities.")
        })?;
    if !o.status.success() {
        bail!("{program}: {}", String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}
/// Inside a Snap, printing works only while the `cups` interface is connected
/// (a sideloaded package is not auto-connected). Returns a hint when the
/// printing socket is missing; `None` outside a Snap or when it is available.
pub fn snap_connection_hint() -> Option<&'static str> {
    snap_hint_for(
        std::env::var_os("SNAP").is_some(),
        std::env::var("CUPS_SERVER").ok().as_deref(),
    )
}

fn snap_hint_for(in_snap: bool, server: Option<&str>) -> Option<&'static str> {
    const HINT: &str = "Printing is not connected. In a terminal run: sudo snap connect thermalprint:cups cups:cups (install the cups Snap first if needed)";
    let socket = match server {
        Some(path) if path.starts_with('/') => path,
        Some(_) => return None, // A remote server is the user's explicit choice.
        None => "/var/cups/cups.sock",
    };
    (in_snap && !std::path::Path::new(socket).exists()).then_some(HINT)
}

pub fn discover() -> Result<Vec<Printer>> {
    if let Some(hint) = snap_connection_hint() {
        bail!(hint);
    }
    let printers = run("lpstat", &["-p"])?;
    let default = run("lpstat", &["-d"]).unwrap_or_default();
    let default = default
        .trim()
        .split_once(": ")
        .map(|(_, v)| v)
        .unwrap_or("");
    let mut list = vec![];
    for line in printers.lines().filter(|l| l.starts_with("printer ")) {
        let Some(name) = line.split_whitespace().nth(1) else {
            continue;
        };
        let mut p = Printer {
            name: name.into(),
            is_default: name == default,
            description: line.into(),
            ..Default::default()
        };
        let options = run("lpoptions", &["-p", name, "-l"]).unwrap_or_default();
        for line in options.lines() {
            let Some((key, values)) = line.split_once(':') else {
                continue;
            };
            let key = key.split('/').next().unwrap_or(key);
            let choices: Vec<String> = values
                .split_whitespace()
                .map(|v| v.trim_start_matches('*').to_owned())
                .collect();
            if key == "PageSize" || (key == "media" && p.media.is_empty()) {
                p.custom_media = choices.iter().any(|v| v.starts_with("Custom"));
                p.media = choices
                    .into_iter()
                    .filter(|v| !v.starts_with("Custom"))
                    .collect();
            } else if ["Resolution", "printer-resolution"].contains(&key) {
                p.resolutions = choices;
                p.resolution_option = key.into();
            }
        }
        if let Some(ppd) = driver_description(name) {
            p.media_labels = media_labels(&ppd);
            p.media_sizes = media_sizes(&ppd);
            p.media_margins = media_margins(&ppd);
        }
        list.push(p);
    }
    Ok(list)
}
pub fn submit(output: &Output, s: &Settings, p: &Printer) -> Result<String> {
    if output.clipped {
        bail!("Printing blocked: content would be clipped");
    }
    if s.printer != p.name {
        bail!("Printer selection has changed. Refresh printers and try again.");
    }
    if !(1..=999).contains(&s.copies) {
        bail!("Copies must be between 1 and 999");
    }
    let media = if s.media == "Custom" && p.custom_media {
        format!("Custom.{:.2}x{:.2}mm", output.width_mm, output.height_mm)
    } else if !s.media.is_empty() && p.media.contains(&s.media) {
        s.media.clone()
    } else {
        bail!(
            "Select an advertised printer media size. If the driver exposes no media, configure a label queue in CUPS first."
        );
    };
    let mut file = tempfile::Builder::new().suffix(".pdf").tempfile()?;
    // Printer alignment correction applies to the printed copy only, never to exports.
    let aligned = output.shifted(s.offset_x_mm, s.offset_y_mm);
    file.write_all(&export::pdf_bytes(&aligned)?)?;
    file.flush()?;
    let mut cmd = Command::new("lp");
    cmd.args([
        "-d",
        &p.name,
        "-n",
        &s.copies.to_string(),
        "-t",
        "ThermalPrint label",
        "-o",
        &format!("media={media}"),
        "-o",
        "print-scaling=none",
        "-o",
        "scaling=100",
        "-o",
        "fit-to-page=false",
        "-o",
        "sides=one-sided",
        // Never let a queue default add start/end banner pages: on label stock
        // each banner prints as an extra label.
        "-o",
        "job-sheets=none",
    ]);
    if !s.resolution.is_empty() {
        if !p.resolutions.contains(&s.resolution) {
            bail!("Selected printer resolution is no longer supported");
        }
        cmd.args(["-o", &format!("{}={}", p.resolution_option, s.resolution)]);
    }
    let o = cmd
        .arg(file.path())
        .env("LC_ALL", "C")
        .output()
        .context("Could not start CUPS lp")?;
    if !o.status.success() {
        bail!(
            "CUPS rejected the job: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        );
    }
    let text = String::from_utf8_lossy(&o.stdout).trim().to_owned();
    if text.is_empty() {
        Ok("Job submitted to CUPS".into())
    } else {
        Ok(text)
    }
}
pub fn job_status(printer: &str) -> Result<String> {
    let pending = run("lpstat", &["-W", "not-completed", "-o", printer])?;
    let state = run("lpstat", &["-p", printer])?;
    Ok(format!(
        "{}\n{}",
        state.trim(),
        if pending.trim().is_empty() {
            "No pending jobs. This does not confirm physical delivery."
        } else {
            pending.trim()
        }
    ))
}

/// CUPS has no authoritative thermal-device flag. This is a UI hint only.
pub fn thermal_hint(p: &Printer) -> &'static str {
    let name = p.name.to_ascii_lowercase();
    let known = [
        "zebra",
        "rollo",
        "orgsta",
        "dymo",
        "munbyn",
        "phomemo",
        "niimbot",
        "labelwriter",
        "ql-",
        "td-",
    ]
    .iter()
    .any(|part| name.contains(part));
    let label_media = p.media.iter().any(|m| {
        let m = m.to_ascii_lowercase();
        m.contains("4x6") || m.contains("100x150") || m.contains("label") || m.contains("2x1")
    });
    if known && label_media {
        "Likely thermal label printer"
    } else if known {
        "Likely label printer · check media"
    } else if label_media {
        "Supports label-sized media · verify printer type"
    } else {
        "Printer type unknown · check its driver and paper"
    }
}

/// Submit native ZPL to a deliberately configured raw CUPS queue. The user
/// must know that this destination understands ZPL; CUPS cannot infer it.
pub fn submit_raw_zpl(data: &[u8], s: &Settings, p: &Printer) -> Result<String> {
    if s.printer != p.name {
        bail!("Printer selection changed");
    }
    let mut file = tempfile::Builder::new().suffix(".zpl").tempfile()?;
    file.write_all(data)?;
    file.flush()?;
    let mut last = String::new();
    for _ in 0..s.copies {
        let o = Command::new("lp")
            .args([
                "-d",
                &p.name,
                "-o",
                "raw",
                "-o",
                "job-sheets=none",
                "-t",
                "ThermalPrint ZPL label",
            ])
            .arg(file.path())
            .output()
            .context("Could not start CUPS lp")?;
        if !o.status.success() {
            bail!(
                "CUPS rejected raw ZPL: {}",
                String::from_utf8_lossy(&o.stderr)
            );
        }
        last = String::from_utf8_lossy(&o.stdout).trim().into();
    }
    Ok(format!(
        "{} ZPL copies submitted. Last job: {last}",
        s.copies
    ))
}

#[cfg(test)]
mod media_tests {
    use super::*;
    #[test]
    fn mismatch_uses_known_dimensions_and_tolerates_driver_rounding() {
        let p = Printer {
            media_sizes: HashMap::from([("stock".into(), (101.6, 152.4))]),
            ..Default::default()
        };
        assert!(p.media_mismatch("stock", 101.5, 152.3).is_none());
        assert!(p.media_mismatch("stock", 100., 150.).is_some());
        assert!(p.media_mismatch("stock", 152.4, 101.6).is_some());
        assert!(p.media_mismatch("Custom", 50., 50.).is_none());
        assert!(p.media_mismatch("unknown", 50., 50.).is_none());
    }
    #[test]
    fn default_media_matches_output_dimensions() {
        let printer = Printer {
            media: vec!["LB06".into(), "w4h6".into()],
            media_sizes: media_sizes(
                "*PaperDimension LB06/40 x 20 mm: \"113.4 56.7\"\n*PaperDimension w4h6/4 x 6: \"288 432\"",
            ),
            custom_media: true,
            ..Default::default()
        };
        assert_eq!(
            printer.matching_media(101.6, 152.4).as_deref(),
            Some("w4h6")
        );
        assert_eq!(printer.matching_media(40., 20.).as_deref(), Some("LB06"));
        assert_eq!(printer.matching_media(60., 60.).as_deref(), Some("Custom"));
        let unsupported = Printer {
            custom_media: false,
            ..printer
        };
        assert_eq!(unsupported.matching_media(60., 60.), None);
    }
    #[test]
    fn driver_media_descriptions_preserve_codes_and_dimensions() {
        let labels = media_labels(
            r#"*PageSize w4h6/4 x 6 (4.00 in x 6.00 in): "code"
*PageSize LB06/40mm x 20mm (40 mm x 20 mm): "code"
*PageSize A4/A4: "code"
*PageSize Opaque: "code"
*PageRegion LB06/Wrong description: "code""#,
        );
        assert_eq!(labels["w4h6"], "4.00 in × 6.00 in");
        assert_eq!(labels["LB06"], "40 mm × 20 mm");
        assert_eq!(labels["A4"], "A4");
        let printer = Printer {
            media_labels: labels,
            ..Default::default()
        };
        assert_eq!(printer.media_label("LB06"), "40 mm × 20 mm");
        assert_eq!(
            printer.media_label("Opaque"),
            "Opaque — driver-defined size"
        );
        assert_eq!(
            printer.media_label("Custom"),
            "Custom — match output dimensions"
        );
    }
}

#[cfg(test)]
mod snap_hint_tests {
    use super::*;

    #[test]
    fn hints_only_in_a_snap_when_the_printing_socket_is_missing() {
        assert!(snap_hint_for(true, Some("/nonexistent/cups.sock")).is_some());
        assert!(snap_hint_for(false, Some("/nonexistent/cups.sock")).is_none());
        assert!(snap_hint_for(true, Some("print.example:631")).is_none());
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("cups.sock");
        std::fs::write(&socket, "").unwrap();
        assert!(snap_hint_for(true, socket.to_str()).is_none());
    }
}

#[cfg(test)]
mod confined_driver_tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn reads_driver_description_from_the_selected_socket_without_host_ppd_access() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("cups proxy.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // The request can arrive in several pieces; read through the header end.
            let mut request = Vec::new();
            let mut chunk = [0; 256];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let size = stream.read(&mut chunk).unwrap();
                assert!(size > 0, "client closed before finishing its request");
                request.extend_from_slice(&chunk[..size]);
            }
            assert!(
                String::from_utf8_lossy(&request)
                    .starts_with("GET /printers/label_printer.ppd HTTP/1.0")
            );
            stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n*PPD-Adobe: \"4.3\"\n*PaperDimension Label/Label stock: \"288 432\"\n").unwrap();
        });
        let ppd = driver_description_from("label_printer", socket.to_str(), false).unwrap();
        assert_eq!(media_sizes(&ppd).get("Label"), Some(&(101.6, 152.4)));
        server.join().unwrap();
    }

    #[test]
    fn remote_servers_and_invalid_queue_names_do_not_probe_a_local_socket() {
        assert!(
            driver_description_from("label_printer", Some("print.example:631"), false).is_none()
        );
        assert!(driver_description_from("../private", Some("/missing/cups.sock"), false).is_none());
    }
}

#[cfg(test)]
mod margin_tests {
    use super::*;

    const PPD: &str = "*PaperDimension w4h6/4 x 6: \"288 432\"\n*ImageableArea w4h6/4 x 6: \"7.2 5.76 280.8 424.8\"\n*PaperDimension A4/A4: \"595 842\"\n*ImageableArea A4/A4: \"0 0 595 842\"\n*ImageableArea Orphan/none: \"1 1 2 2\"\n*PaperDimension Bad/bad: \"100 100\"\n*ImageableArea Bad/bad: \"nonsense\"";

    #[test]
    fn imageable_areas_become_unprintable_margins_in_millimeters() {
        let margins = media_margins(PPD);
        let (left, bottom, right, top) = margins["w4h6"];
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        assert!(near(left, 2.54) && near(bottom, 2.032) && near(right, 2.54) && near(top, 2.54));
        assert_eq!(
            margins["A4"],
            (0., 0., 0., 0.),
            "a full-page area has no margin"
        );
        assert!(!margins.contains_key("Orphan") && !margins.contains_key("Bad"));
    }

    #[test]
    fn printers_report_margins_in_design_orientation_and_ignore_negligible_ones() {
        let printer = Printer {
            media_margins: media_margins(PPD),
            ..Default::default()
        };
        let (left, top, right, bottom) = printer.printable_margins_mm("w4h6").unwrap();
        assert!(
            (left - 2.54).abs() < 0.01
                && (bottom - 2.032).abs() < 0.01
                && (top - 2.54).abs() < 0.01
                && (right - 2.54).abs() < 0.01
        );
        assert_eq!(
            printer.printable_margins_mm("A4"),
            None,
            "a full-page area has nothing to show"
        );
        assert_eq!(printer.printable_margins_mm("unknown"), None);
        let flush = Printer {
            media_margins: [("m".into(), (0., 0., 0., 0.))].into(),
            ..Default::default()
        };
        assert_eq!(flush.printable_margins_mm("m"), None);
    }
}
