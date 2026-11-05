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
                if ui.button("New").clicked() {
                    self.checkpoint();
                    self.doc = LabelDocument::blank(101.6, 152.4);
                    self.path = None;
                    self.selected.clear();
                    self.changed();
                }
                if ui.button("Open…").clicked() {
                    self.open();
                }
                if ui.button("Save").clicked() {
                    self.save(false);
                }
                if ui.button("Save As…").clicked() {
                    self.save(true);
                }
                if ui
                    .add_enabled(!self.undo.is_empty(), egui::Button::new("↶ Undo"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(!self.redo.is_empty(), egui::Button::new("↷ Redo"))
                    .clicked()
                {
                    self.redo();
                }
                if ui.button("−").clicked() {
                    self.zoom = (self.zoom / 1.25).max(0.25);
                }
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom * 1.25).min(5.);
                }
                if ui.button("Fit").clicked() {
                    self.zoom = 1.;
                    self.pan = Vec2::ZERO;
                }
                ui.checkbox(&mut self.grid, "Grid");
                ui.checkbox(&mut self.snap, "Snap");
                if ui.button("Export PDF").clicked() {
                    self.export("pdf", settings.dpi);
                }
                if ui.button("Export PNG").clicked() {
                    self.export("png", settings.dpi);
                }
                if ui.button("Export SVG").clicked(){self.export("svg",settings.dpi);}
                if ui.button("Export ZPL").clicked(){self.export("zpl",settings.dpi);}
                ui.checkbox(&mut self.raw_zpl,"Raw ZPL").on_hover_text("Use only with a ZPL-compatible raw CUPS queue; driver capability is not automatic.");
                if ui.button("Print").clicked() {
                    self.print(settings, printers);
                }
            });
        });
        egui::Panel::left("designer-elements")
            .default_size(215.)
            .size_range(190.0..=260.0)
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
        });
        egui::CentralPanel::default().show(root, |ui| self.canvas(ui));
        self.start_render(settings.dpi);
    }
    fn elements(&mut self, ui: &mut egui::Ui) {
        ui.heading("Elements");
        ui.label(RichText::new("Drag on canvas to position").small().weak());
        if ui.button("＋ Text").clicked() {
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
        if ui.button("＋ Address").clicked() {
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
        if ui.button("▥ Code 128").clicked() {
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
        if ui.button("▦ QR code").clicked() {
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
            if ui.button(format!("＋ {name}")).clicked() {
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
        if ui.button("＋ Image / logo…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "webp", "bmp"])
                .pick_file()
            {
                match std::fs::read(path) {
                    Ok(bytes) if bytes.len() <= 9_000_000 => self.add(
                        Element::Image {
                            data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
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
        ui.heading("Templates");
        for (i, t) in design::TEMPLATES.iter().enumerate() {
            if ui.button(t.name).on_hover_text(t.description).clicked() {
                self.checkpoint();
                self.doc = design::from_template(i);
                self.path = None;
                self.selected.clear();
                self.changed();
            }
        }
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
            if ui.button("Delete selected").clicked() {
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
                    .selected_text(format!("{kind:?}"))
                    .show_ui(ui, |ui| {
                        for k in [
                            BarcodeKind::Code128,
                            BarcodeKind::Code39,
                            BarcodeKind::Ean13,
                            BarcodeKind::UpcA,
                        ] {
                            ui.selectable_value(kind, k, format!("{k:?}"));
                        }
                    });
                ui.checkbox(show_text, "Show value below bars");
            }
            Element::Qr { data } => {
                ui.label("QR content");
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
                            ui.selectable_value(kind, k, format!("{k:?}"));
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
        ui.horizontal(|ui| {
            if ui.button("Bring forward").clicked() {
                self.layer(true);
            }
            if ui.button("Send backward").clicked() {
                self.layer(false);
            }
        });
        if ui.button("Duplicate").clicked() {
            self.duplicate();
        }
        if ui.button("Delete").clicked() {
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
