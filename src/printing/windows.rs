use super::Printer;
use crate::{layout::Output, settings::Settings};
use anyhow::{Context, Result, bail, ensure};
use std::{
    io,
    mem::{size_of, size_of_val},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::POINT,
    Graphics::{Gdi::*, Printing::*},
    Storage::Xps::*,
};

struct DeviceGeometry {
    dpi_x: i32,
    dpi_y: i32,
    physical_width: i32,
    physical_height: i32,
    offset_x: i32,
    offset_y: i32,
    margins_mm: (f32, f32, f32, f32),
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn decode(text: &[u16]) -> String {
    String::from_utf16_lossy(text.split(|&ch| ch == 0).next().unwrap_or_default())
}

// The spooler returns pointers to null-terminated strings in its result buffer.
unsafe fn spooler_string(text: *const u16) -> String {
    if text.is_null() {
        return String::new();
    }
    let end = (0..).find(|&i| unsafe { *text.add(i) == 0 }).unwrap();
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, end) })
}

fn win32(ok: bool, operation: &str) -> Result<()> {
    if !ok {
        return Err(io::Error::last_os_error()).context(operation.to_owned());
    }
    Ok(())
}

struct Queue(PRINTER_HANDLE);

impl Queue {
    fn open(name: &[u16]) -> Result<Self> {
        let mut handle = PRINTER_HANDLE::default();
        win32(
            unsafe { OpenPrinterW(name.as_ptr(), &mut handle, null()) } != 0,
            "Open Windows printer",
        )?;
        Ok(Self(handle))
    }

    fn info(&self) -> Result<Vec<usize>> {
        let mut bytes = 0;
        unsafe { GetPrinterW(self.0, 2, null_mut(), 0, &mut bytes) };
        win32(bytes > 0, "Query Windows printer")?;
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        win32(
            unsafe { GetPrinterW(self.0, 2, buffer.as_mut_ptr().cast(), bytes, &mut bytes) } != 0,
            "Read Windows printer information",
        )?;
        Ok(buffer)
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        unsafe { ClosePrinter(self.0) };
    }
}

pub fn snap_connection_hint() -> Option<&'static str> {
    None
}

pub fn discover() -> Result<Vec<Printer>> {
    let mut default_len = 0;
    unsafe { GetDefaultPrinterW(null_mut(), &mut default_len) };
    let mut default = vec![0; default_len as usize];
    if default_len > 0 && unsafe { GetDefaultPrinterW(default.as_mut_ptr(), &mut default_len) } == 0
    {
        default.clear();
    }
    let default = decode(&default);

    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let (mut bytes, mut count) = (0, 0);
    let initial = unsafe { EnumPrintersW(flags, null(), 4, null_mut(), 0, &mut bytes, &mut count) };
    if bytes == 0 {
        win32(initial != 0, "Enumerate Windows printers")?;
        return Ok(Vec::new());
    }
    let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
    win32(
        unsafe {
            EnumPrintersW(
                flags,
                null(),
                4,
                buffer.as_mut_ptr().cast(),
                bytes,
                &mut bytes,
                &mut count,
            )
        } != 0,
        "Enumerate Windows printers",
    )?;
    let entries = unsafe {
        std::slice::from_raw_parts(buffer.as_ptr().cast::<PRINTER_INFO_4W>(), count as usize)
    };
    Ok(entries
        .iter()
        .map(|entry| {
            let name = unsafe { spooler_string(entry.pPrinterName) };
            let mut printer = Printer {
                is_default: name == default,
                description: name.clone(),
                name,
                ..Default::default()
            };
            if let Err(error) = read_capabilities(&mut printer) {
                tracing::debug!(printer = %printer.name, %error, "Printer capabilities unavailable");
            }
            printer
        })
        .collect())
}

fn read_capabilities(printer: &mut Printer) -> Result<()> {
    let name = wide(&printer.name);
    let queue = Queue::open(&name)?;
    let info = queue.info()?;
    let info = unsafe { &*info.as_ptr().cast::<PRINTER_INFO_2W>() };
    let driver = unsafe { spooler_string(info.pDriverName) };
    if !driver.is_empty() {
        printer.description = driver;
    }
    let caps = |kind, buffer| unsafe {
        DeviceCapabilitiesW(name.as_ptr(), info.pPortName, kind, buffer, null())
    };
    let count = caps(DC_PAPERS, null_mut());
    if count > 0
        && caps(DC_PAPERNAMES, null_mut()) == count
        && caps(DC_PAPERSIZE, null_mut()) == count
    {
        let mut ids = vec![0u16; count as usize];
        let mut names = vec![[0u16; 64]; count as usize];
        let mut sizes = vec![POINT::default(); count as usize];
        if caps(DC_PAPERS, ids.as_mut_ptr()) == count
            && caps(DC_PAPERNAMES, names.as_mut_ptr().cast()) == count
            && caps(DC_PAPERSIZE, sizes.as_mut_ptr().cast()) == count
        {
            for ((id, label), size) in ids.into_iter().zip(names).zip(sizes) {
                if id == 0 || size.x <= 0 || size.y <= 0 {
                    continue;
                }
                let code = id.to_string();
                let label = decode(&label);
                let (w, h) = (
                    super::tenths_mm_to_mm(size.x),
                    super::tenths_mm_to_mm(size.y),
                );
                printer.media.push(code.clone());
                printer
                    .media_labels
                    .insert(code.clone(), format!("{label} · {w} × {h} mm"));
                printer.media_sizes.insert(code, (w, h));
            }
        }
    }
    let count = caps(DC_ENUMRESOLUTIONS, null_mut());
    if count > 0 {
        let mut values = vec![[0i32; 2]; count as usize];
        if caps(DC_ENUMRESOLUTIONS, values.as_mut_ptr().cast()) == count {
            for [x, y] in values {
                if x > 0 && y > 0 && x <= i16::MAX.into() && y <= i16::MAX.into() {
                    let dpi = format!("{x}x{y}dpi");
                    if !printer.resolutions.contains(&dpi) {
                        printer.resolutions.push(dpi);
                    }
                }
            }
        }
    }
    let fields = caps(DC_FIELDS, null_mut());
    let custom_fields = DM_PAPERLENGTH | DM_PAPERWIDTH;
    printer.custom_media = fields >= 0 && fields as u32 & custom_fields == custom_fields;
    Ok(())
}

struct PrinterDc(HDC);

impl Drop for PrinterDc {
    fn drop(&mut self) {
        unsafe { DeleteDC(self.0) };
    }
}

fn printer_dc(p: &Printer, s: &Settings, width_mm: f32, height_mm: f32) -> Result<PrinterDc> {
    let name = wide(&p.name);
    let queue = Queue::open(&name)?;
    let properties = |out, input, flags| unsafe {
        DocumentPropertiesW(null_mut(), queue.0, name.as_ptr(), out, input, flags)
    };
    let bytes = properties(null_mut(), null(), 0);
    ensure!(
        bytes >= size_of::<DEVMODEW>() as i32,
        "Printer driver did not return print settings"
    );
    let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
    let mode = buffer.as_mut_ptr().cast::<DEVMODEW>();
    ensure!(
        properties(mode, null(), DM_OUT_BUFFER) == 1,
        "Could not load printer settings"
    );

    // Preserve driver-private data appended to DEVMODE and have the driver validate changes.
    unsafe {
        let fields = &mut (*mode).Anonymous1.Anonymous1;
        let landscape = if s.media == "Custom" {
            width_mm > height_mm
        } else {
            p.media_sizes
                .get(&s.media)
                .is_some_and(|&(media_width, media_height)| {
                    (width_mm - media_height).abs() < 0.5
                        && (height_mm - media_width).abs() < 0.5
                        && (media_width - media_height).abs() >= 0.5
                })
        };
        fields.dmOrientation = if landscape {
            DMORIENT_LANDSCAPE as i16
        } else {
            DMORIENT_PORTRAIT as i16
        };
        fields.dmCopies = 1;
        if (*mode).dmFields & DM_SCALE != 0 {
            fields.dmScale = 100;
        }
        (*mode).dmDuplex = DMDUP_SIMPLEX;
        (*mode).dmFields |= DM_ORIENTATION | DM_COPIES | DM_DUPLEX;
        (*mode).dmFields &= !(DM_FORMNAME | DM_PAPERWIDTH | DM_PAPERLENGTH | DM_PAPERSIZE);
        if s.media == "Custom" && p.custom_media {
            fields.dmPaperSize = 0;
            let (paper_width, paper_length) = if landscape {
                (height_mm, width_mm)
            } else {
                (width_mm, height_mm)
            };
            fields.dmPaperWidth = (paper_width * 10.).round() as i16;
            fields.dmPaperLength = (paper_length * 10.).round() as i16;
            (*mode).dmFields |= DM_PAPERWIDTH | DM_PAPERLENGTH;
        } else {
            ensure!(
                p.media.contains(&s.media),
                "Select a size advertised by the Windows printer"
            );
            fields.dmPaperSize = s.media.parse::<u16>()? as i16;
            (*mode).dmFields |= DM_PAPERSIZE;
        }
        if !s.resolution.is_empty() {
            ensure!(
                p.resolutions.contains(&s.resolution),
                "Selected printer resolution is no longer supported"
            );
            let (x, y) = s
                .resolution
                .trim_end_matches("dpi")
                .split_once('x')
                .context("Invalid printer resolution")?;
            fields.dmPrintQuality = x.parse()?;
            (*mode).dmYResolution = y.parse()?;
            (*mode).dmFields |= DM_PRINTQUALITY | DM_YRESOLUTION;
        }
    }
    ensure!(
        properties(mode, mode, DM_IN_BUFFER | DM_OUT_BUFFER) == 1,
        "Printer driver rejected the print settings"
    );
    let dc = unsafe { CreateDCW(null(), name.as_ptr(), null(), mode) };
    win32(!dc.is_null(), "Create Windows printer device context")?;
    Ok(PrinterDc(dc))
}

fn device_geometry(dc: &PrinterDc, width_mm: f32, height_mm: f32) -> Result<DeviceGeometry> {
    let cap = |index: u32| unsafe { GetDeviceCaps(dc.0, index as i32) };
    let (dpi_x, dpi_y) = (cap(LOGPIXELSX), cap(LOGPIXELSY));
    ensure!(
        dpi_x > 0 && dpi_y > 0,
        "Printer driver did not report a usable resolution"
    );
    let (physical_width, physical_height) = (cap(PHYSICALWIDTH), cap(PHYSICALHEIGHT));
    let (actual_w, actual_h) = (
        physical_width as f32 * 25.4 / dpi_x as f32,
        physical_height as f32 * 25.4 / dpi_y as f32,
    );
    ensure!(
        (actual_w - width_mm).abs() < 0.5 && (actual_h - height_mm).abs() < 0.5,
        "Selected media is {width_mm:.1} × {height_mm:.1} mm, but the Windows driver configured {actual_w:.1} × {actual_h:.1} mm. Check the selected label size and driver paper setting."
    );
    let (offset_x, offset_y) = (cap(PHYSICALOFFSETX), cap(PHYSICALOFFSETY));
    let margins_mm = super::margins_from_device_caps(
        (physical_width, physical_height),
        (offset_x, offset_y),
        (cap(HORZRES), cap(VERTRES)),
        (dpi_x, dpi_y),
    )
    .context("Windows printer driver returned invalid printable-area metrics")?;
    Ok(DeviceGeometry {
        dpi_x,
        dpi_y,
        physical_width,
        physical_height,
        offset_x,
        offset_y,
        margins_mm,
    })
}

pub fn alignment_geometry(
    s: &Settings,
    p: &Printer,
    width_mm: f32,
    height_mm: f32,
) -> Result<(u32, (f32, f32, f32, f32))> {
    let dc = printer_dc(p, s, width_mm, height_mm)?;
    let geometry = device_geometry(&dc, width_mm, height_mm)?;
    if !s.resolution.is_empty() {
        ensure!(
            s.resolution == format!("{}x{}dpi", geometry.dpi_x, geometry.dpi_y),
            "Printer driver changed the requested resolution"
        );
    }
    Ok((geometry.dpi_x as u32, geometry.margins_mm))
}

fn check_selection(s: &Settings, p: &Printer) -> Result<()> {
    ensure!(
        s.printer == p.name,
        "Printer selection has changed. Refresh printers and try again."
    );
    ensure!(
        (1..=999).contains(&s.copies),
        "Copies must be between 1 and 999"
    );
    Ok(())
}

pub fn submit(output: &Output, s: &Settings, p: &Printer) -> Result<String> {
    check_selection(s, p)?;
    ensure!(
        !output.clipped,
        "{}",
        output
            .clipping_message()
            .unwrap_or("Printing blocked: content would be clipped")
    );
    let dc = printer_dc(p, s, output.width_mm, output.height_mm)?;
    let geometry = device_geometry(&dc, output.width_mm, output.height_mm)?;
    let (dpi_x, dpi_y) = (geometry.dpi_x, geometry.dpi_y);
    if !s.resolution.is_empty() {
        ensure!(
            s.resolution == format!("{dpi_x}x{dpi_y}dpi"),
            "Printer driver changed the requested resolution"
        );
    }
    let output = output.shifted(s.offset_x_mm, s.offset_y_mm);
    let (w, h) = (output.image.width() as i32, output.image.height() as i32);
    if let Some(overflow) = super::outside_printable_area_mm(&output, geometry.margins_mm) {
        bail!(
            "Printing blocked: content extends {:.1} mm beyond printable width and {:.1} mm beyond printable height (left {:.1}, right {:.1}, top {:.1}, bottom {:.1}). Reduce scale, move content inward, or choose media with a larger printable area.",
            overflow.0 + overflow.2,
            overflow.1 + overflow.3,
            overflow.0,
            overflow.2,
            overflow.1,
            overflow.3
        );
    }
    let pixels: Vec<[u8; 4]> = output
        .image
        .pixels()
        .map(|p| [p[2], p[1], p[0], 0])
        .collect();
    let bitmap = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    let title = wide("ThermalPrint label");
    let doc = DOCINFOW {
        cbSize: size_of::<DOCINFOW>() as i32,
        lpszDocName: title.as_ptr(),
        ..Default::default()
    };
    let id = unsafe { StartDocW(dc.0, &doc) };
    win32(id > 0, "Start Windows print job")?;
    let result = (|| -> Result<()> {
        for _ in 0..s.copies {
            win32(unsafe { StartPage(dc.0) } > 0, "Start label page")?;
            unsafe { SetStretchBltMode(dc.0, COLORONCOLOR) };
            let lines = unsafe {
                StretchDIBits(
                    dc.0,
                    -geometry.offset_x,
                    -geometry.offset_y,
                    geometry.physical_width,
                    geometry.physical_height,
                    0,
                    0,
                    w,
                    h,
                    pixels.as_ptr().cast(),
                    &bitmap,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                )
            };
            win32(lines > 0, "Send label raster to printer driver")?;
            win32(unsafe { EndPage(dc.0) } > 0, "Finish label page")?;
        }
        win32(unsafe { EndDoc(dc.0) } > 0, "Submit Windows print job")
    })();
    if result.is_err() {
        unsafe { AbortDoc(dc.0) };
    }
    result?;
    Ok(format!("Job {id} submitted to the Windows print queue"))
}

pub fn job_status(printer: &str) -> Result<String> {
    let queue = Queue::open(&wide(printer))?;
    let buffer = queue.info()?;
    let info = unsafe { &*buffer.as_ptr().cast::<PRINTER_INFO_2W>() };
    let state = [
        (PRINTER_STATUS_OFFLINE, "Offline"),
        (PRINTER_STATUS_PAUSED, "Paused"),
        (PRINTER_STATUS_PAPER_OUT, "Out of paper"),
        (PRINTER_STATUS_PAPER_JAM, "Paper jam"),
        (PRINTER_STATUS_ERROR, "Printer error"),
        (PRINTER_STATUS_PRINTING, "Printing"),
    ]
    .into_iter()
    .find_map(|(flag, label)| (info.Status & flag != 0).then_some(label))
    .unwrap_or("Available");
    Ok(format!("{printer}: {state}\nPending jobs: {}", info.cJobs))
}

pub fn submit_raw(
    data: &[u8],
    language: crate::raw::RawLanguage,
    s: &Settings,
    p: &Printer,
) -> Result<String> {
    let name = language.name();
    check_selection(s, p)?;
    let queue = Queue::open(&wide(&p.name))?;
    let mut title = wide(&format!("ThermalPrint {name} label"));
    let mut raw = wide("RAW");
    let doc = DOC_INFO_1W {
        pDocName: title.as_mut_ptr(),
        pDatatype: raw.as_mut_ptr(),
        ..Default::default()
    };
    let len: u32 = size_of_val(data).try_into()?;
    let id = unsafe { StartDocPrinterW(queue.0, 1, &doc) };
    win32(id != 0, &format!("Start raw {name} print job"))?;
    let result = (|| -> Result<()> {
        for _ in 0..s.copies {
            win32(
                unsafe { StartPagePrinter(queue.0) } != 0,
                &format!("Start raw {name} label"),
            )?;
            let mut written = 0;
            win32(
                unsafe { WritePrinter(queue.0, data.as_ptr().cast(), len, &mut written) } != 0,
                &format!("Write raw {name} label"),
            )?;
            if written != len {
                bail!("Printer accepted only {written} of {len} {name} bytes");
            }
            win32(
                unsafe { EndPagePrinter(queue.0) } != 0,
                &format!("Finish raw {name} label"),
            )?;
        }
        win32(
            unsafe { EndDocPrinter(queue.0) } != 0,
            &format!("Submit raw {name} job"),
        )
    })();
    if result.is_err() {
        unsafe { AbortPrinter(queue.0) };
    }
    result?;
    Ok(format!("{} {name} copies submitted in job {id}", s.copies))
}
