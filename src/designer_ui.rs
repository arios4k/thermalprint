use crate::{
    design::{self, BarcodeKind, DesignObject, Element, LabelDocument, ShapeKind, TextAlign},
    design_render, export,
    layout::Output,
    printing::{self, Printer},
    settings::Settings,
    zpl,
};
use base64::Engine as _;
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
};
#[derive(Clone, Copy)]
enum DragKind {
    Move,
    Resize,
}
struct Drag {
    kind: DragKind,
    start: Pos2,
    initial: Vec<(u64, f32, f32, f32, f32)>,
}
pub struct Designer {
    pub doc: LabelDocument,
    pub path: Option<PathBuf>,
    selected: Vec<u64>,
    clipboard: Vec<DesignObject>,
    undo: Vec<LabelDocument>,
    redo: Vec<LabelDocument>,
    pub grid: bool,
    pub snap: bool,
    pub raw_zpl: bool,
    pub zoom: f32,
    pub pan: Vec2,
    revision: u64,
    rendered_revision: Option<u64>,
    worker: Option<(u64, mpsc::Receiver<anyhow::Result<Output>>)>,
    texture: Option<egui::TextureHandle>,
    output: Option<Arc<Output>>,
    drag: Option<Drag>,
    pub status: String,
    pub error: bool,
    job: Option<mpsc::Receiver<anyhow::Result<String>>>,
}
impl Default for Designer {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::collapsible_if)]
impl Designer {
    pub fn new() -> Self {
        Self {
            doc: design::from_template(0),
            path: None,
            selected: vec![],
            clipboard: vec![],
            undo: vec![],
            redo: vec![],
            grid: false,
            snap: true,
            raw_zpl: false,
            zoom: 1.,
            pan: Vec2::ZERO,
            revision: 1,
            rendered_revision: None,
            worker: None,
            texture: None,
            output: None,
            drag: None,
            status: "Create a label or choose a template".into(),
            error: false,
            job: None,
        }
    }
    fn checkpoint(&mut self) {
        self.undo.push(self.doc.clone());
        if self.undo.len() > 80 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.error = false;
    }
    fn undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.redo.push(self.doc.clone());
            self.doc = d;
            self.selected.clear();
            self.changed();
        }
    }
    fn redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.undo.push(self.doc.clone());
            self.doc = d;
            self.selected.clear();
            self.changed();
        }
    }
    fn add(&mut self, element: Element, w: f32, h: f32) {
        self.checkpoint();
        let x = ((self.doc.width_mm - w) / 2.).max(1.);
        let y = ((self.doc.height_mm - h) / 2.).max(1.);
        let id = self.doc.add(element, x, y, w, h);
        self.selected = vec![id];
        self.changed();
    }
    fn copy(&mut self) {
        self.clipboard = self
            .doc
            .objects
            .iter()
            .filter(|o| self.selected.contains(&o.id))
            .cloned()
            .collect();
    }
    fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        self.checkpoint();
        self.selected.clear();
        for mut o in self.clipboard.clone() {
            o.id = self.doc.next_id;
            self.doc.next_id += 1;
            o.x_mm += 3.;
            o.y_mm += 3.;
            self.selected.push(o.id);
            self.doc.objects.push(o);
        }
        self.changed();
    }
    fn delete(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.checkpoint();
        self.doc.objects.retain(|o| !self.selected.contains(&o.id));
        self.selected.clear();
        self.changed();
    }
    fn duplicate(&mut self) {
        self.copy();
        self.paste();
    }
    fn layer(&mut self, forward: bool) {
        if self.selected.len() != 1 {
            return;
        }
        let Some(i) = self
            .doc
            .objects
            .iter()
            .position(|o| self.selected.contains(&o.id))
        else {
            return;
        };
        let j = if forward { i + 1 } else { i.saturating_sub(1) };
        if j >= self.doc.objects.len() || j == i {
            return;
        }
        self.checkpoint();
        self.doc.objects.swap(i, j);
        self.changed();
    }
    fn save(&mut self, as_new: bool) {
        let path = if !as_new { self.path.clone() } else { None }.or_else(|| {
            rfd::FileDialog::new()
                .add_filter("ThermalPrint design", &["thermalprint"])
                .set_file_name(format!("{}.thermalprint", self.doc.title.replace(' ', "-")))
                .save_file()
        });
        if let Some(mut p) = path {
            if p.extension().is_none() {
                p.set_extension("thermalprint");
            }
            match self.doc.save(&p) {
                Ok(()) => {
                    self.path = Some(p.clone());
                    self.status = format!("Saved {}", p.display());
                    self.error = false;
                }
                Err(e) => {
                    self.status = format!("Save failed: {e:#}");
                    self.error = true;
                }
            }
        }
    }
    fn open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("ThermalPrint design", &["thermalprint"])
            .pick_file()
        {
            match LabelDocument::load(&path) {
                Ok(doc) => {
                    self.checkpoint();
                    self.doc = doc;
                    self.path = Some(path);
                    self.selected.clear();
                    self.changed();
                    self.status = "Design opened".into();
                }
                Err(e) => {
                    self.status = format!("Cannot open design: {e:#}");
                    self.error = true;
                }
            }
        }
    }
    fn start_job(
        &mut self,
        status: &str,
        run: impl FnOnce() -> anyhow::Result<String> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.job = Some(rx);
        self.status = status.into();
        self.error = false;
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Operation failed unexpectedly")));
            let _ = tx.send(result);
        });
    }
    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.job else { return };
        let Ok(result) = rx.try_recv() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        };
        self.job = None;
        match result {
            Ok(s) => {
                self.status = s;
                self.error = false;
            }
            Err(e) => {
                self.status = format!("{e:#}");
                self.error = true;
            }
        }
    }
    fn export(&mut self, ext: &'static str, dpi: u32) {
        if self.job.is_some() {
            return;
        }
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter(ext, &[ext])
            .set_file_name(format!("label.{ext}"))
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension(ext);
        }
        let doc = self.doc.clone();
        self.start_job("Exporting design…", move || {
            if ext == "zpl" {
                std::fs::write(&path, zpl::encode_design(&doc, dpi)?)?;
            } else {
                let output = design_render::render_label(&doc, dpi)?;
                export::save(&output, &path)?;
            }
            Ok(format!("Exported {}", path.display()))
        });
    }
    fn print(&mut self, settings: &Settings, printers: &[Printer]) {
        if self.job.is_some() {
            return;
        }
        let Some(printer) = printers
            .iter()
            .find(|p| p.name == settings.printer)
            .cloned()
        else {
            self.status = "Choose a printer in Print Center".into();
            self.error = true;
            return;
        };
        let doc = self.doc.clone();
        let settings = settings.clone();
        let raw = self.raw_zpl;
        self.start_job("Submitting design to CUPS…", move || {
            if raw {
                let bytes = zpl::encode_design(&doc, settings.dpi)?;
                printing::submit_raw_zpl(&bytes, &settings, &printer)
            } else {
                let output = design_render::render_label(&doc, settings.dpi)?;
                printing::submit(&output, &settings, &printer)
            }
        });
    }
    fn start_render(&mut self, dpi: u32) {
        if self.worker.is_some() || self.rendered_revision == Some(self.revision) {
            return;
        }
        let doc = self.doc.clone();
        let (tx, rx) = mpsc::channel();
        let rev = self.revision;
        self.worker = Some((rev, rx));
        std::thread::spawn(move || {
            let _ = tx.send(design_render::render_label(&doc, dpi));
        });
    }
    fn poll(&mut self, ctx: &egui::Context) {
        let Some((rev, rx)) = &self.worker else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        };
        let rev = *rev;
        self.worker = None;
        if rev != self.revision {
            return;
        }
        match result {
            Ok(output) => {
                let im = if output.image.width() > 1400 || output.image.height() > 1400 {
                    image::imageops::thumbnail(&output.image, 1400, 1400)
                } else {
                    output.image.clone()
                };
                self.texture = Some(ctx.load_texture(
                    "design-preview",
                    egui::ColorImage::from_rgb(
                        [im.width() as usize, im.height() as usize],
                        im.as_raw(),
                    ),
                    egui::TextureOptions::LINEAR,
                ));
                self.output = Some(Arc::new(output));
                self.rendered_revision = Some(rev);
            }
            Err(e) => {
                self.status = format!("Design preview: {e:#}");
                self.error = true;
                self.texture = None;
                self.output = None;
                self.rendered_revision = Some(rev);
            }
        }
    }
    fn shortcut(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            self.undo();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)) {
            self.redo();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)) {
            self.copy();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::V)) {
            self.paste();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D)) {
            self.duplicate();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) {
            self.delete();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save(false);
        }
    }
    pub fn show(&mut self, root: &mut egui::Ui, settings: &Settings, printers: &[Printer]) {
        let ctx = root.ctx().clone();
        self.poll(&ctx);
        self.poll_job(&ctx);
        self.shortcut(&ctx);
        egui::Panel::top("designer-toolbar").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Label Designer");
                ui.separator();
                if icon_action(ui, "New document", "New", "Create a blank label", true, false).clicked() {
                    self.checkpoint();
                    self.doc = LabelDocument::blank(101.6, 152.4);
                    self.path = None;
                    self.selected.clear();
                    self.changed();
                }
                if icon_action(ui, "Import label", "Open…", "Open a saved label design", true, false).clicked() {
                    self.open();
                }
                if icon_action(ui, "Save document", "Save", "Save design (Ctrl+S)", true, false).clicked() {
                    self.save(false);
                }
                if icon_action(ui, "Save as", "Save As…", "Save a copy of this design", true, false).clicked() {
                    self.save(true);
                }
                if icon_action(ui, "Undo", "", "Undo (Ctrl+Z)", !self.undo.is_empty(), false)
                    .clicked()
                {
                    self.undo();
                }
                if icon_action(ui, "Redo", "", "Redo (Ctrl+Y)", !self.redo.is_empty(), false)
                    .clicked()
                {
                    self.redo();
                }
                if icon_action(ui, "Zoom out", "", "Zoom out", true, false).clicked() {
                    self.zoom = (self.zoom / 1.25).max(0.25);
                }
                if icon_action(ui, "Zoom in", "", "Zoom in", true, false).clicked() {
                    self.zoom = (self.zoom * 1.25).min(5.);
                }
                if icon_action(ui, "Fit preview", "", "Fit label to canvas and reset panning", true, false).clicked() {
                    self.zoom = 1.;
                    self.pan = Vec2::ZERO;
                }
                if icon_action(ui, "Grid", "Grid", "Show canvas grid", true, self.grid).clicked() { self.grid = !self.grid; }
                if icon_action(ui, "Snap", "Snap", "Snap objects to the grid when moving or resizing", true, self.snap).clicked() { self.snap = !self.snap; }
                if icon_action(ui, "Export document", "PDF", "Export label as PDF", true, false).clicked() {
                    self.export("pdf", settings.dpi);
                }
                if icon_action(ui, "Add image / logo...", "PNG", "Export label as PNG image", true, false).clicked() {
                    self.export("png", settings.dpi);
                }
                if icon_action(ui, "Export document", "SVG", "Export label as SVG vector image", true, false).clicked(){self.export("svg",settings.dpi);}
                if icon_action(ui, "Export document", "ZPL", "Export label as Zebra Programming Language", true, false).clicked(){self.export("zpl",settings.dpi);}
                if icon_action(ui, "Export document", "Raw ZPL", "Use only with a ZPL-compatible raw CUPS queue; driver capability is not automatic.", true, self.raw_zpl).clicked() { self.raw_zpl = !self.raw_zpl; }
                if icon_action(ui, "Print device", "Print", "Print label using the selected printer", true, false).clicked() {
                    self.print(settings, printers);
                }
            });
        });
        egui::Panel::left("designer-elements")
            .default_size(68.)
            .size_range(68.0..=68.0)
            .resizable(false)
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.elements(ui));
            });
        egui::Panel::right("designer-inspector")
            .default_size(265.)
            .size_range(240.0..=340.0)
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
            });
        egui::Panel::bottom("designer-status").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} · {:.1} × {:.1} mm · {}% · {} objects",
                    self.doc.title,
                    self.doc.width_mm,
                    self.doc.height_mm,
                    (self.zoom * 100.) as u32,
                    self.doc.objects.len()
                ));
                if self.error {
                    ui.colored_label(Color32::RED, &self.status);
                } else {
                    ui.label(&self.status);
                }
            });
            if let Some(printer) = printers.iter().find(|p| p.name == settings.printer) {
                if self.raw_zpl {
                    ui.label("Raw ZPL: verify printer compatibility and loaded label size.")
                        .on_hover_text(crate::help::explanation("ZPL"));
                } else if let Some(warning) = printer.media_mismatch(&settings.media, self.doc.width_mm, self.doc.height_mm) {
                    let color = if ui.visuals().dark_mode { Color32::from_rgb(245, 193, 99) }
                        else { Color32::from_rgb(132, 83, 10) };
                    ui.colored_label(color, warning);
                    ui.label(RichText::new("If the loaded paper matches the printer media, use the button below. To keep the current label size, choose matching media in Print Center instead.").small().weak());
                    if let Some(&(width, height)) = printer.media_sizes.get(&settings.media)
                        && icon_action(ui, "Rotate stock", "Match label to printer media",
                            "Set the canvas dimensions to the selected printer stock. Object sizes and positions are preserved; Ctrl+Z undoes this change.", true, false).clicked() {
                        self.checkpoint();
                        self.doc.width_mm = width;
                        self.doc.height_mm = height;
                        self.zoom = 1.;
                        self.pan = Vec2::ZERO;
                        self.changed();
                        self.status = format!("Label matched to printer media: {width:.1} × {height:.1} mm");
                    }
                } else if settings.media.is_empty() {
                    ui.label("Select printer media in Print Center before printing.");
                }
            } else {
                ui.label("Choose a printer in Print Center to print this design. Export is available without a printer.");
            }
        });
        egui::CentralPanel::default().show(root, |ui| self.canvas(ui));
        self.start_render(settings.dpi);
    }
    fn elements(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(6.);
            if element_tool(ui, "Add text").clicked() {
                self.add(
                    Element::Text {
                        text: "Your text".into(),
                        font_pt: 14.,
                        bold: false,
                        align: TextAlign::Left,
                    },
                    55.,
                    12.,
                );
            }
            if element_tool(ui, "Add address").clicked() {
                self.add(
                    Element::Text {
                        text: "Name\nStreet address\nCity, region ZIP".into(),
                        font_pt: 11.,
                        bold: false,
                        align: TextAlign::Left,
                    },
                    70.,
                    32.,
                );
            }
            if element_tool(ui, "Add Code 128").clicked() {
                self.add(
                    Element::Barcode {
                        kind: BarcodeKind::Code128,
                        data: "SKU-001".into(),
                        show_text: true,
                    },
                    65.,
                    22.,
                );
            }
            if element_tool(ui, "Add QR code").clicked() {
                self.add(
                    Element::Qr {
                        data: "https://example.com".into(),
                    },
                    27.,
                    27.,
                );
            }
            for (name, kind) in [
                ("Rectangle", ShapeKind::Rectangle),
                ("Ellipse", ShapeKind::Ellipse),
                ("Line", ShapeKind::Line),
                ("Horizontal divider", ShapeKind::DividerHorizontal),
                ("Vertical divider", ShapeKind::DividerVertical),
            ] {
                if element_tool(ui, &format!("Add {name}")).clicked() {
                    self.add(
                        Element::Shape {
                            kind,
                            stroke_mm: 0.4,
                            filled: false,
                        },
                        45.,
                        if kind == ShapeKind::DividerHorizontal {
                            1.
                        } else {
                            20.
                        },
                    );
                }
            }
            if element_tool(ui, "Add image / logo...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Image", &["png", "jpg", "jpeg", "webp", "bmp"])
                    .pick_file()
                {
                    match std::fs::read(path) {
                        Ok(bytes) if bytes.len() <= 9_000_000 => self.add(
                            Element::Image {
                                data_base64: base64::engine::general_purpose::STANDARD
                                    .encode(bytes),
                            },
                            35.,
                            25.,
                        ),
                        _ => {
                            self.status = "Image could not be read or exceeds 9 MB".into();
                            self.error = true;
                        }
                    }
                }
            }
            ui.separator();
            let templates = element_tool(ui, "Choose a label template");
            egui::Popup::menu(&templates).show(|ui| {
                ui.label("Templates");
                for (i, t) in design::TEMPLATES.iter().enumerate() {
                    if template_button(ui, t.name)
                        .on_hover_text(t.description)
                        .clicked()
                    {
                        self.checkpoint();
                        self.doc = design::from_template(i);
                        ui.close();
                        self.path = None;
                        self.selected.clear();
                        self.changed();
                    }
                }
            });
        });
    }
    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Properties");
        let before = self.doc.clone();
        ui.label("Label");
        ui.text_edit_singleline(&mut self.doc.title);
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.add(
                egui::DragValue::new(&mut self.doc.width_mm)
                    .range(1.0..=2000.0)
                    .speed(0.2)
                    .suffix(" mm"),
            );
            ui.label("Height");
            ui.add(
                egui::DragValue::new(&mut self.doc.height_mm)
                    .range(1.0..=2000.0)
                    .speed(0.2)
                    .suffix(" mm"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Serial variable");
            ui.add(egui::DragValue::new(&mut self.doc.serial_number));
        });
        ui.separator();
        if self.selected.len() > 1 {
            ui.label(format!("{} objects selected", self.selected.len()));
            if icon_action(
                ui,
                "Delete",
                "Delete selected",
                "Delete all selected objects",
                true,
                false,
            )
            .clicked()
            {
                self.delete();
            }
            return;
        }
        let Some(id) = self.selected.first().copied() else {
            ui.label("Select an object on the canvas.");
            ui.label(RichText::new("Shift-click to select multiple objects. Use Ctrl+C/V, Ctrl+D, Delete, Ctrl+Z/Y.").small().weak());
            if self.doc != before {
                self.undo.push(before);
                self.redo.clear();
                self.changed();
            }
            return;
        };
        let Some(o) = self.doc.objects.iter_mut().find(|o| o.id == id) else {
            return;
        };
        ui.heading(format!("{} #{}", o.label(), id));
        ui.horizontal(|ui| {
            ui.label("X");
            ui.add(egui::DragValue::new(&mut o.x_mm).speed(0.1).suffix(" mm"));
            ui.label("Y");
            ui.add(egui::DragValue::new(&mut o.y_mm).speed(0.1).suffix(" mm"));
        });
        ui.horizontal(|ui| {
            ui.label("W");
            ui.add(
                egui::DragValue::new(&mut o.width_mm)
                    .range(1.0..=2000.0)
                    .speed(0.1)
                    .suffix(" mm"),
            );
            ui.label("H");
            ui.add(
                egui::DragValue::new(&mut o.height_mm)
                    .range(1.0..=2000.0)
                    .speed(0.1)
                    .suffix(" mm"),
            );
        });
        egui::ComboBox::from_id_salt("design-rotate")
            .selected_text(format!("Rotation {}°", o.rotation))
            .show_ui(ui, |ui| {
                for r in [0, 90, 180, 270] {
                    ui.selectable_value(&mut o.rotation, r, format!("{r}°"));
                }
            });
        match &mut o.element {
            Element::Text {
                text,
                font_pt,
                bold,
                align,
            } => {
                ui.label("Text · ${date} and ${serial} are dynamic");
                ui.add(egui::TextEdit::multiline(text).desired_rows(6));
                ui.add(
                    egui::DragValue::new(font_pt)
                        .range(3.0..=300.0)
                        .suffix(" pt"),
                );
                ui.checkbox(bold, "Bold");
                ui.horizontal(|ui| {
                    ui.selectable_value(align, TextAlign::Left, "Left");
                    ui.selectable_value(align, TextAlign::Center, "Center");
                    ui.selectable_value(align, TextAlign::Right, "Right");
                });
            }
            Element::Barcode {
                kind,
                data,
                show_text,
            } => {
                ui.label("Enter your real product code or reference");
                ui.text_edit_singleline(data);
                egui::ComboBox::from_id_salt("barcode-kind")
                    .selected_text(match kind {
                        BarcodeKind::Code128 => "Code 128", BarcodeKind::Code39 => "Code 39",
                        BarcodeKind::Ean13 => "EAN-13", BarcodeKind::UpcA => "UPC-A",
                    })
                    .show_ui(ui, |ui| {
                        for k in [
                            BarcodeKind::Code128,
                            BarcodeKind::Code39,
                            BarcodeKind::Ean13,
                            BarcodeKind::UpcA,
                        ] {
                            let (name, hint) = match k {
                                BarcodeKind::Code128 => ("Code 128", "A compact barcode for letters, digits, and product references."),
                                BarcodeKind::Code39 => ("Code 39", "A barcode for uppercase letters, digits, and a limited set of symbols."),
                                BarcodeKind::Ean13 => ("EAN-13", crate::help::explanation("EAN")),
                                BarcodeKind::UpcA => ("UPC-A", crate::help::explanation("UPC")),
                            };
                            ui.selectable_value(kind, k, name).on_hover_text(hint);
                        }
                    });
                ui.checkbox(show_text, "Show value below bars");
            }
            Element::Qr { data } => {
                ui.label("QR content")
                    .on_hover_text(crate::help::explanation("QR"));
                ui.add(egui::TextEdit::multiline(data).desired_rows(4));
            }
            Element::Shape {
                kind,
                stroke_mm,
                filled,
            } => {
                egui::ComboBox::from_id_salt("shape-kind")
                    .selected_text(format!("{kind:?}"))
                    .show_ui(ui, |ui| {
                        for k in [
                            ShapeKind::Rectangle,
                            ShapeKind::Ellipse,
                            ShapeKind::Line,
                            ShapeKind::DividerHorizontal,
                            ShapeKind::DividerVertical,
                        ] {
                            let name = match k {
                                ShapeKind::Rectangle => "Rectangle",
                                ShapeKind::Ellipse => "Ellipse",
                                ShapeKind::Line => "Line",
                                ShapeKind::DividerHorizontal => "Horizontal divider",
                                ShapeKind::DividerVertical => "Vertical divider",
                            };
                            ui.selectable_value(kind, k, name);
                        }
                    });
                ui.add(
                    egui::DragValue::new(stroke_mm)
                        .range(0.05..=20.)
                        .suffix(" mm"),
                );
                ui.checkbox(filled, "Filled");
            }
            Element::Image { .. } => {
                ui.label("Embedded image. Resize with W/H or the canvas handle.");
            }
        }
        ui.horizontal_wrapped(|ui| {
            if icon_action(
                ui,
                "Move up",
                "Forward",
                "Move object one layer forward",
                true,
                false,
            )
            .clicked()
            {
                self.layer(true);
            }
            if icon_action(
                ui,
                "Move down",
                "Backward",
                "Move object one layer backward",
                true,
                false,
            )
            .clicked()
            {
                self.layer(false);
            }
        });
        if icon_action(
            ui,
            "Duplicate",
            "Duplicate",
            "Duplicate selected objects (Ctrl+D)",
            true,
            false,
        )
        .clicked()
        {
            self.duplicate();
        }
        if icon_action(
            ui,
            "Delete",
            "Delete",
            "Delete selected objects",
            true,
            false,
        )
        .clicked()
        {
            self.delete();
        }
        if self.doc != before {
            self.undo.push(before);
            self.redo.clear();
            self.changed();
        }
    }
    fn canvas(&mut self, ui: &mut egui::Ui) {
        ui.heading("Canvas");
        ui.label(RichText::new("Select and drag objects · drag the lower-right handle to resize · Shift-click for multiple selection").small().weak());
        let Some(tex) = self.texture.clone() else {
            ui.spinner();
            return;
        };
        let available = ui.available_size() - Vec2::splat(40.);
        let source = tex.size_vec2();
        let fit = (available.x / source.x)
            .min(available.y / source.y)
            .max(0.05);
        let size = source * fit * self.zoom;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (outer, _) = ui.allocate_exact_size(
                    Vec2::new(size.x.max(available.x), size.y.max(available.y)) + Vec2::splat(30.),
                    Sense::hover(),
                );
                let rect = Rect::from_center_size(outer.center() + self.pan, size);
                ui.painter()
                    .rect_filled(rect.expand(6.), 8., Color32::from_gray(110));
                ui.painter().image(
                    tex.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                    Color32::WHITE,
                );
                if self.grid {
                    let step = (size.x / self.doc.width_mm * 5.).max(5.);
                    let color = Color32::from_rgba_unmultiplied(90, 110, 130, 45);
                    let mut x = rect.left();
                    while x < rect.right() {
                        ui.painter().line_segment(
                            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                            Stroke::new(1., color),
                        );
                        x += step;
                    }
                    let mut y = rect.top();
                    while y < rect.bottom() {
                        ui.painter().line_segment(
                            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                            Stroke::new(1., color),
                        );
                        y += size.y / self.doc.height_mm * 5.;
                    }
                }
                for o in &self.doc.objects {
                    if self.selected.contains(&o.id) {
                        let r = self.object_rect(o, rect);
                        ui.painter().rect_stroke(
                            r,
                            2.,
                            Stroke::new(2., Color32::from_rgb(70, 130, 240)),
                            StrokeKind::Outside,
                        );
                        ui.painter().rect_filled(
                            Rect::from_center_size(r.right_bottom(), Vec2::splat(10.)),
                            2.,
                            Color32::from_rgb(70, 130, 240),
                        );
                    }
                }
                let response = ui.interact(
                    rect,
                    ui.scope_id().with("design-canvas"),
                    Sense::click_and_drag(),
                );
                if response.clicked() {
                    if let Some(p) = response.interact_pointer_pos() {
                        if let Some(id) = self.hit(p, rect) {
                            let shift = ui.input(|i| i.modifiers.shift);
                            if shift {
                                if self.selected.contains(&id) {
                                    self.selected.retain(|x| *x != id)
                                } else {
                                    self.selected.push(id)
                                }
                            } else {
                                self.selected = vec![id];
                            }
                        } else {
                            self.selected.clear();
                        }
                    }
                }
                if response.drag_started() {
                    if let Some(p) = response.interact_pointer_pos() {
                        if let Some(id) = self.hit(p, rect) {
                            if !self.selected.contains(&id) {
                                self.selected = vec![id];
                            }
                            self.checkpoint();
                            let kind = if self.selected.len() == 1
                                && self
                                    .doc
                                    .objects
                                    .iter()
                                    .find(|o| o.id == id)
                                    .is_some_and(|o| {
                                        self.object_rect(o, rect).right_bottom().distance(p) < 17.
                                    }) {
                                DragKind::Resize
                            } else {
                                DragKind::Move
                            };
                            self.drag = Some(Drag {
                                kind,
                                start: p,
                                initial: self
                                    .doc
                                    .objects
                                    .iter()
                                    .filter(|o| self.selected.contains(&o.id))
                                    .map(|o| (o.id, o.x_mm, o.y_mm, o.width_mm, o.height_mm))
                                    .collect(),
                            });
                        }
                    }
                }
                if response.dragged() {
                    if let (Some(d), Some(pos)) = (&self.drag, response.interact_pointer_pos()) {
                        let dx = (pos.x - d.start.x) / size.x * self.doc.width_mm;
                        let dy = (pos.y - d.start.y) / size.y * self.doc.height_mm;
                        let step = if self.snap { 1.0 } else { 0.1 };
                        for (id, x, y, w, h) in &d.initial {
                            if let Some(o) = self.doc.objects.iter_mut().find(|o| o.id == *id) {
                                match d.kind {
                                    DragKind::Move => {
                                        o.x_mm = ((x + dx) / step).round() * step;
                                        o.y_mm = ((y + dy) / step).round() * step;
                                    }
                                    DragKind::Resize => {
                                        o.width_mm = ((w + dx).max(1.) / step).round() * step;
                                        o.height_mm = ((h + dy).max(1.) / step).round() * step;
                                    }
                                }
                            }
                        }
                        self.changed();
                    }
                }
                if response.drag_stopped() {
                    self.drag = None;
                }
                if response.dragged_by(egui::PointerButton::Middle) {
                    self.pan += response.drag_delta();
                }
            });
    }
    fn object_rect(&self, o: &DesignObject, paper: Rect) -> Rect {
        let x = paper.left() + o.x_mm / self.doc.width_mm * paper.width();
        let y = paper.top() + o.y_mm / self.doc.height_mm * paper.height();
        let w = o.width_mm / self.doc.width_mm * paper.width();
        let h = o.height_mm / self.doc.height_mm * paper.height();
        Rect::from_min_size(
            Pos2::new(x, y),
            Vec2::new(
                if o.rotation % 180 == 90 { h } else { w },
                if o.rotation % 180 == 90 { w } else { h },
            ),
        )
    }
    fn hit(&self, p: Pos2, paper: Rect) -> Option<u64> {
        self.doc
            .objects
            .iter()
            .rev()
            .find(|o| self.object_rect(o, paper).expand(8.).contains(p))
            .map(|o| o.id)
    }
}

/// A shared 24-point outline grid keeps the tools visually balanced.
fn element_tool(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let response = ui
        .add(
            egui::Button::new("")
                .frame(false)
                .min_size(Vec2::splat(42.)),
        )
        .on_hover_text(crate::help::tooltip(label));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let painter = ui.painter();
    if response.hovered() || response.has_focus() {
        painter.rect_filled(
            response.rect.shrink(1.),
            9.,
            ui.visuals().widgets.hovered.bg_fill,
        );
    }
    let color = if response.hovered() && !ui.visuals().dark_mode {
        ui.visuals().selection.bg_fill
    } else if response.hovered() {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().text_color()
    };
    paint_tool_icon(painter, response.rect.center(), color, label);
    response
}

pub(crate) fn template_button(ui: &mut egui::Ui, name: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(218., 34.), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), name));
    let visuals = ui.style().interact(&response);
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 5., visuals.bg_fill);
    }
    let icon = match name {
        "Blank custom" => "Blank custom",
        "Shipping 4 × 6" => "Shipping",
        "Return address" | "Name label" => "Add address",
        "Product barcode" | "Inventory" | "Asset tracking" => "Add Code 128",
        "QR code" => "Add QR code",
        "Shelf" => "Choose a label template",
        "Price tag" => "Price tag",
        "Fragile" => "Fragile",
        _ => "Add Rectangle",
    };
    paint_tool_icon(
        ui.painter(),
        Pos2::new(rect.left() + 19., rect.center().y),
        visuals.fg_stroke.color,
        icon,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 40., rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::TextStyle::Button.resolve(ui.style()),
        visuals.fg_stroke.color,
    );
    response
}

pub(crate) fn paint_tool_icon(painter: &egui::Painter, center: Pos2, color: Color32, label: &str) {
    let origin = center - Vec2::splat(12.);
    let stroke = Stroke::new(1.65, color);
    let point = |x: f32, y: f32| origin + Vec2::new(x, y);
    // Round caps soften short strokes and keep small icons legible.
    let path = |points: &[(f32, f32)]| {
        let points: Vec<_> = points.iter().map(|&(x, y)| point(x, y)).collect();
        painter.add(egui::Shape::line(points.clone(), stroke));
        if let (Some(first), Some(last)) = (points.first(), points.last()) {
            painter.circle_filled(*first, stroke.width / 2., color);
            painter.circle_filled(*last, stroke.width / 2., color);
        }
    };
    let outline = |x, y, w, h, radius| {
        painter.rect_stroke(
            Rect::from_min_size(point(x, y), Vec2::new(w, h)),
            radius,
            stroke,
            StrokeKind::Middle,
        );
    };
    match label {
        "Home" => {
            path(&[(3., 11.), (12., 3.), (21., 11.)]);
            path(&[
                (5., 10.),
                (5., 21.),
                (10., 21.),
                (10., 15.),
                (14., 15.),
                (14., 21.),
                (19., 21.),
                (19., 10.),
            ]);
        }
        "Preview" => {
            path(&[
                (2., 12.),
                (6., 7.),
                (12., 5.),
                (18., 7.),
                (22., 12.),
                (18., 17.),
                (12., 19.),
                (6., 17.),
                (2., 12.),
            ]);
            painter.circle_stroke(point(12., 12.), 3., stroke);
        }
        "Previous" | "Next" | "Move up" | "Move down" => {
            let points = match label {
                "Previous" => [(15., 5.), (8., 12.), (15., 19.)],
                "Next" => [(9., 5.), (16., 12.), (9., 19.)],
                "Move up" => [(5., 15.), (12., 8.), (19., 15.)],
                _ => [(5., 9.), (12., 16.), (19., 9.)],
            };
            path(&points);
        }
        "Delete" => {
            path(&[(4., 6.), (20., 6.)]);
            path(&[(9., 3.), (15., 3.)]);
            path(&[(6., 6.), (7., 21.), (17., 21.), (18., 6.)]);
            path(&[(10., 10.), (10., 17.)]);
            path(&[(14., 10.), (14., 17.)]);
        }
        "Duplicate" => {
            outline(8., 8., 13., 13., 2.);
            path(&[(5., 16.), (3., 16.), (3., 3.), (16., 3.), (16., 5.)]);
        }
        "Refresh" | "Rotate stock" => {
            path(&[(20., 10.), (18., 5.), (12., 3.), (6., 5.), (3., 10.)]);
            path(&[(4., 14.), (6., 19.), (12., 21.), (18., 19.), (21., 14.)]);
            path(&[(16., 10.), (20., 10.), (20., 6.)]);
            path(&[(4., 18.), (4., 14.), (8., 14.)]);
        }
        "Status" => {
            outline(3., 3., 18., 18., 3.);
            path(&[(7., 12.), (10., 15.), (17., 8.)]);
        }
        "New document" => {
            outline(4., 3., 14., 18., 2.);
            path(&[(8., 12.), (14., 12.)]);
            path(&[(11., 9.), (11., 15.)]);
        }
        "Save document" | "Save as" => {
            path(&[
                (4., 3.),
                (17., 3.),
                (21., 7.),
                (21., 21.),
                (3., 21.),
                (3., 3.),
                (4., 3.),
            ]);
            outline(7., 3., 9., 6., 1.);
            outline(7., 14., 10., 7., 1.);
            if label == "Save as" {
                path(&[(16., 11.), (22., 11.)]);
                path(&[(19., 8.), (19., 14.)]);
            }
        }
        "Undo" | "Redo" => {
            let flip = |x: f32| if label == "Redo" { 24. - x } else { x };
            path(&[(flip(9.), 5.), (flip(4.), 10.), (flip(9.), 15.)]);
            path(&[
                (flip(4.), 10.),
                (flip(15.), 10.),
                (flip(19.), 13.),
                (flip(19.), 17.),
                (flip(16.), 20.),
            ]);
        }
        "Grid" => {
            outline(3., 3., 18., 18., 2.);
            for n in [9., 15.] {
                path(&[(n, 3.), (n, 21.)]);
                path(&[(3., n), (21., n)]);
            }
        }
        "Snap" => {
            path(&[
                (5., 4.),
                (5., 14.),
                (8., 19.),
                (12., 21.),
                (16., 19.),
                (19., 14.),
                (19., 4.),
            ]);
            path(&[
                (5., 8.),
                (9., 8.),
                (9., 14.),
                (12., 17.),
                (15., 14.),
                (15., 8.),
                (19., 8.),
            ]);
        }
        "Export document" => {
            path(&[(10., 3.), (4., 3.), (4., 21.), (19., 21.), (19., 14.)]);
            path(&[(12., 12.), (21., 3.)]);
            path(&[(15., 3.), (21., 3.), (21., 9.)]);
        }
        "Auto extract" => {
            path(&[(4., 20.), (16., 8.), (19., 11.), (7., 23.), (4., 20.)]);
            path(&[(16., 2.), (16., 5.)]);
            path(&[(21., 6.), (23., 6.)]);
            path(&[(7., 5.), (9., 7.)]);
        }
        "Zoom in" | "Zoom out" => {
            painter.circle_stroke(point(10., 10.), 6., stroke);
            path(&[(15., 15.), (21., 21.)]);
            path(&[(7., 10.), (13., 10.)]);
            if label == "Zoom in" {
                path(&[(10., 7.), (10., 13.)]);
            }
        }
        "Fit preview" => {
            path(&[(3., 9.), (3., 3.), (9., 3.)]);
            path(&[(15., 3.), (21., 3.), (21., 9.)]);
            path(&[(21., 15.), (21., 21.), (15., 21.)]);
            path(&[(9., 21.), (3., 21.), (3., 15.)]);
        }
        "Crop label" => {
            path(&[(7., 3.), (7., 17.), (21., 17.)]);
            path(&[(3., 7.), (17., 7.), (17., 21.)]);
        }
        "Print device" => {
            outline(3., 8., 18., 10., 2.);
            outline(7., 3., 10., 5., 1.);
            outline(7., 14., 10., 7., 1.);
            painter.circle_filled(point(17., 11.), 1., color);
        }
        "System" => {
            outline(3., 4., 18., 13., 2.);
            path(&[(12., 17.), (12., 21.)]);
            path(&[(8., 21.), (16., 21.)]);
        }
        "Light" => {
            painter.circle_stroke(point(12., 12.), 4., stroke);
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::TAU / 8.;
                let (dy, dx) = angle.sin_cos();
                path(&[
                    (12. + dx * 7., 12. + dy * 7.),
                    (12. + dx * 10., 12. + dy * 10.),
                ]);
            }
        }
        "Dark" => {
            // Two circular arcs form a crescent without painting over the background.
            let mut moon = Vec::new();
            for i in 0..=24 {
                let angle = (90. + i as f32 * 270. / 24.).to_radians();
                moon.push((12. + 9. * angle.cos(), 12. + 9. * angle.sin()));
            }
            // Use a smooth inset arc between the outer tips.
            for i in 1..=24 {
                let t = i as f32 / 24.;
                let u = 1. - t;
                moon.push((
                    u * u * 21. + 2. * u * t * 6. + t * t * 12.,
                    u * u * 12. + 2. * u * t * 6. + t * t * 21.,
                ));
            }
            path(&moon);
        }
        "Design label" => {
            outline(3., 4., 15., 17., 2.);
            path(&[(7., 8.), (12., 8.)]);
            path(&[(7., 12.), (10., 12.)]);
            path(&[
                (11., 17.),
                (12., 13.),
                (19., 6.),
                (22., 9.),
                (15., 16.),
                (11., 17.),
            ]);
            path(&[(17., 8.), (20., 11.)]);
        }
        "Import label" => {
            path(&[(14., 3.), (5., 3.), (5., 21.), (19., 21.), (19., 14.)]);
            path(&[(13., 11.), (21., 3.)]);
            path(&[(13., 6.), (13., 11.), (18., 11.)]);
            path(&[(9., 16.), (15., 16.)]);
        }
        "Blank custom" => {
            outline(5., 3., 14., 18., 2.);
        }
        "Shipping" => {
            outline(3., 6., 18., 15., 2.);
            path(&[(3., 10.), (12., 14.), (21., 10.)]);
            path(&[(12., 14.), (12., 21.)]);
            path(&[(3., 6.), (12., 3.), (21., 6.)]);
            path(&[(12., 3.), (12., 10.)]);
        }
        "Price tag" => {
            path(&[
                (3., 3.),
                (12., 3.),
                (21., 12.),
                (12., 21.),
                (3., 12.),
                (3., 3.),
            ]);
            painter.circle_stroke(point(7.5, 7.5), 1.5, stroke);
        }
        "Fragile" => {
            path(&[
                (7., 3.),
                (6., 10.),
                (8., 13.),
                (12., 14.),
                (16., 13.),
                (18., 10.),
                (17., 3.),
                (7., 3.),
            ]);
            path(&[(12., 14.), (12., 21.)]);
            path(&[(8., 21.), (16., 21.)]);
            path(&[(7., 7.), (17., 7.)]);
        }
        "Add text" => {
            path(&[(5., 7.), (5., 4.), (19., 4.), (19., 7.)]);
            path(&[(12., 4.), (12., 20.)]);
            path(&[(8., 20.), (16., 20.)]);
        }
        "Add address" => {
            outline(3., 4., 18., 16., 3.);
            painter.circle_stroke(point(8., 10.), 2., stroke);
            path(&[(5., 16.), (5.8, 14.), (10.2, 14.), (11., 16.)]);
            path(&[(14., 9.), (18., 9.)]);
            path(&[(14., 13.), (18., 13.)]);
            path(&[(14., 17.), (17., 17.)]);
        }
        "Add Code 128" => {
            path(&[(6., 3.), (3., 3.), (3., 7.)]);
            path(&[(18., 3.), (21., 3.), (21., 7.)]);
            path(&[(3., 17.), (3., 21.), (6., 21.)]);
            path(&[(18., 21.), (21., 21.), (21., 17.)]);
            for (x, w) in [(6., 1.5), (9., 2.), (13., 1.), (16., 2.)] {
                painter.rect_filled(
                    Rect::from_min_size(point(x, 7.), Vec2::new(w, 10.)),
                    0.5,
                    color,
                );
            }
        }
        "Add QR code" => {
            for (x, y) in [(3., 3.), (15., 3.), (3., 15.)] {
                outline(x, y, 6., 6., 1.);
                painter.circle_filled(point(x + 3., y + 3.), 0.9, color);
            }
            path(&[(15., 15.), (15., 18.), (18., 18.), (18., 21.), (21., 21.)]);
            path(&[(21., 15.), (21., 16.)]);
            path(&[(15., 21.), (15.5, 21.)]);
        }
        "Add Rectangle" => outline(3., 5., 18., 14., 2.),
        "Add Ellipse" => {
            painter.add(egui::epaint::EllipseShape::stroke(
                point(12., 12.),
                Vec2::new(9., 7.),
                stroke,
            ));
        }
        "Add Line" => {
            path(&[(5., 19.), (19., 5.)]);
            for (x, y) in [(5., 19.), (19., 5.)] {
                painter.circle_filled(point(x, y), 2., color);
            }
        }
        "Add Horizontal divider" => {
            path(&[(3., 12.), (21., 12.)]);
            path(&[(7., 4.), (17., 4.)]);
            path(&[(7., 20.), (17., 20.)]);
        }
        "Add Vertical divider" => {
            path(&[(12., 3.), (12., 21.)]);
            path(&[(4., 7.), (4., 17.)]);
            path(&[(20., 7.), (20., 17.)]);
        }
        "Choose a label template" => {
            outline(3., 3., 18., 18., 3.);
            path(&[(3., 9.), (21., 9.)]);
            path(&[(10., 9.), (10., 21.)]);
            path(&[(14., 13.), (17., 13.)]);
            path(&[(14., 17.), (17., 17.)]);
        }
        _ => {
            outline(3., 3., 18., 18., 3.);
            painter.circle_stroke(point(16., 8.), 2., stroke);
            path(&[(3., 17.), (9., 11.), (15., 17.), (18., 14.), (21., 17.)]);
        }
    }
}

pub(crate) fn icon_action(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    tooltip: &str,
    enabled: bool,
    selected: bool,
) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let text_width = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), Color32::WHITE)
        .size()
        .x;
    // Reserve actual icon, gap, and text widths instead of padding with spaces.
    let width = if label.is_empty() {
        36.
    } else {
        53. + text_width
    };
    let response = ui
        .add_enabled(
            enabled,
            egui::Button::new("")
                .min_size(Vec2::new(width, 36.))
                .corner_radius(8.)
                .selected(selected),
        )
        .on_hover_text(crate::help::tooltip(tooltip));
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, tooltip));
    let center = if label.is_empty() {
        response.rect.center()
    } else {
        Pos2::new(response.rect.left() + 19., response.rect.center().y)
    };
    let color = if !enabled || !ui.is_enabled() {
        ui.visuals().weak_text_color()
    } else if selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.style().interact(&response).fg_stroke.color
    };
    crate::designer_ui::paint_tool_icon(ui.painter(), center, color, icon);
    if !label.is_empty() {
        ui.painter().text(
            Pos2::new(response.rect.left() + 41., response.rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            font,
            color,
        );
    }

    response
}
