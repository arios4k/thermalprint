//! CUPS client tools provide driver/IPP negotiation without linking libcups.
//! Arguments never pass through a shell. A successful lp call means submitted,
//! not physically printed; job status is queried separately.
use crate::{export, layout::Output, settings::Settings};
use anyhow::{Context, Result, bail};
use std::{io::Write, process::Command};
#[derive(Clone, Debug, Default)]
pub struct Printer {
    pub name: String,
    pub is_default: bool,
    pub description: String,
    pub media: Vec<String>,
    pub resolutions: Vec<String>,
    pub resolution_option: String,
    pub custom_media: bool,
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
pub fn discover() -> Result<Vec<Printer>> {
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
    file.write_all(&export::pdf_bytes(output)?)?;
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
