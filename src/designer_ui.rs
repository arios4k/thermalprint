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
    Rotate,
}
struct Drag {
    kind: DragKind,
    start: Pos2,
    initial: Vec<DesignObject>,
    center: Pos2,
}
#[derive(Clone, Debug)]
enum PendingAction {
    New,
    Open(PathBuf),
    Template(usize),
    /// A past print reopened as an unsaved copy.
    Copy(Box<LabelDocument>),
    Close,
}

/// The CSV merge window: the loaded table and which row is previewed.
struct BatchState {
    file: String,
    table: Option<crate::batch::Table>,
    error: Option<String>,
    row: usize,
    preview: Option<(usize, egui::TextureHandle)>,
}

pub struct Designer {
    pub doc: LabelDocument,
    pub path: Option<PathBuf>,
    saved_document: LabelDocument,
    pending_action: Option<PendingAction>,
    close_approved: bool,
    selected: Vec<u64>,
    clipboard: Vec<DesignObject>,
    undo: Vec<LabelDocument>,
    redo: Vec<LabelDocument>,
    pub focus_canvas: bool,
    show_properties: bool,
    pub grid: bool,
    pub snap: bool,
    /// Snap to other objects' edges and centers while dragging, and show guide lines.
    pub smart_guides: bool,
    /// Draw the printer's unprintable border on the canvas when it is known.
    pub show_printable: bool,
    /// Guide lines to draw during a drag: (is_vertical, position in mm).
    guides: Vec<(bool, f32)>,
    /// The printer's unprintable border as (left, top, right, bottom) in mm for this label.
    printable: Option<(f32, f32, f32, f32)>,
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
    batch: Option<BatchState>,
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
            saved_document: design::from_template(0),
            pending_action: None,
            close_approved: false,
            selected: vec![],
            clipboard: vec![],
            undo: vec![],
            redo: vec![],
            focus_canvas: false,
            show_properties: true,
            grid: false,
            snap: true,
            smart_guides: true,
            show_printable: true,
            guides: Vec::new(),
            printable: None,
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
            batch: None,
        }
    }
    pub(crate) fn select_object(&mut self, id: u64) {
        if self.doc.objects.iter().any(|object| object.id == id) {
            self.selected = vec![id];
        }
    }
    pub(crate) fn rotate_selected_to(&mut self, degrees: f32) {
        if self.selected.len() != 1 || !degrees.is_finite() {
            return;
        }
        self.checkpoint();
        if let Some(object) = self
            .doc
            .objects
            .iter_mut()
            .find(|object| self.selected.contains(&object.id))
        {
            object.rotate_to(degrees);
            self.changed();
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
        let locked = self
            .doc
            .objects
            .iter()
            .filter(|o| self.selected.contains(&o.id) && o.locked)
            .count();
        if locked == self.selected.len() {
            self.status = "Locked objects cannot be deleted. Unlock them first.".into();
            self.error = true;
            return;
        }
        self.checkpoint();
        self.doc
            .objects
            .retain(|o| !self.selected.contains(&o.id) || o.locked);
        self.selected
            .retain(|id| self.doc.objects.iter().any(|o| o.id == *id));
        self.changed();
    }
    /// Move the selection with the arrow keys; locked objects stay where they are.
    fn nudge(&mut self, dx_mm: f32, dy_mm: f32) {
        if self.selected.is_empty()
            || self
                .doc
                .objects
                .iter()
                .all(|o| !self.selected.contains(&o.id) || o.locked)
        {
            return;
        }
        self.checkpoint();
        let ids = self.selected.clone();
        self.doc.nudge(&ids, dx_mm, dy_mm);
        self.changed();
    }
    fn arrange(&mut self, alignment: design::Alignment, to: design::AlignTo) {
        self.checkpoint();
        let ids = self.selected.clone();
        self.doc.align(&ids, alignment, to);
        self.changed();
    }
    /// Center the selection as one group on the label, keeping the objects' arrangement.
    fn arrange_group_on_label(&mut self, alignment: design::Alignment) {
        let moving: Vec<&DesignObject> = self
            .doc
            .objects
            .iter()
            .filter(|o| self.selected.contains(&o.id) && !o.locked)
            .collect();
        if moving.is_empty() {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for object in &moving {
            let (w, h) = object.rotated_size_mm();
            x0 = x0.min(object.x_mm);
            y0 = y0.min(object.y_mm);
            x1 = x1.max(object.x_mm + w);
            y1 = y1.max(object.y_mm + h);
        }
        let (dx, dy) = match alignment {
            design::Alignment::CenterHorizontally => {
                ((self.doc.width_mm - (x1 - x0)) / 2. - x0, 0.)
            }
            _ => (0., (self.doc.height_mm - (y1 - y0)) / 2. - y0),
        };
        self.checkpoint();
        let ids = self.selected.clone();
        self.doc.nudge(&ids, dx, dy);
        self.changed();
    }
    fn set_lock(&mut self, locked: bool) {
        self.checkpoint();
        for object in self
            .doc
            .objects
            .iter_mut()
            .filter(|o| self.selected.contains(&o.id))
        {
            object.locked = locked;
        }
        self.changed();
    }
    fn distribute(&mut self, axis: design::Axis) {
        self.checkpoint();
        let ids = self.selected.clone();
        self.doc.distribute(&ids, axis);
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
    pub(crate) fn is_modified(&self) -> bool {
        self.doc != self.saved_document
    }
    fn save_to_path(&mut self, path: PathBuf) -> bool {
        match self.doc.save(&path) {
            Ok(()) => {
                self.path = Some(path.clone());
                self.saved_document = self.doc.clone();
                self.status = format!("Saved {}", path.display());
                self.error = false;
                true
            }
            Err(error) => {
                self.status = format!("Save failed: {error:#}");
                self.error = true;
                false
            }
        }
    }
    fn save(&mut self, as_new: bool) -> bool {
        let path = if !as_new { self.path.clone() } else { None }.or_else(|| {
            rfd::FileDialog::new()
                .add_filter("ThermalPrint design", &["thermalprint"])
                .set_file_name(format!("{}.thermalprint", self.doc.title.replace(' ', "-")))
                .save_file()
        });
        let Some(mut path) = path else {
            return false;
        };
        if path.extension().is_none() {
            path.set_extension("thermalprint");
        }
        self.save_to_path(path)
    }
    fn open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("ThermalPrint design", &["thermalprint"])
            .pick_file()
        {
            self.request_open(path);
        }
    }
    fn request_action(&mut self, action: PendingAction) {
        if self.is_modified() {
            self.pending_action = Some(action);
        } else {
            self.apply_action(action);
        }
    }
    pub(crate) fn request_new(&mut self) {
        self.request_action(PendingAction::New);
    }
    pub(crate) fn request_template(&mut self, index: usize) {
        self.request_action(PendingAction::Template(index));
    }
    /// Open the batch window with a sample table (documentation screenshots only).
    pub(crate) fn demo_batch(&mut self, csv: &[u8]) {
        self.batch = Some(BatchState {
            file: "customers.csv".into(),
            table: crate::batch::parse_table(csv).ok(),
            error: None,
            row: 1,
            preview: None,
        });
    }
    pub(crate) fn request_copy(&mut self, doc: LabelDocument) {
        self.request_action(PendingAction::Copy(Box::new(doc)));
    }
    pub(crate) fn request_open(&mut self, path: PathBuf) {
        self.request_action(PendingAction::Open(path));
    }
    pub(crate) fn request_close(&mut self) -> bool {
        if self.close_approved || !self.is_modified() {
            return true;
        }
        self.pending_action = Some(PendingAction::Close);
        false
    }
    pub(crate) fn protect_window_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.viewport().close_requested()) && !self.request_close() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.request_repaint();
        }
    }
    fn replace_document(&mut self, doc: LabelDocument, path: Option<PathBuf>, saved: bool) {
        self.saved_document = if saved {
            doc.clone()
        } else {
            LabelDocument::blank(doc.width_mm, doc.height_mm)
        };
        self.doc = doc;
        self.path = path;
        self.selected.clear();
        self.undo.clear();
        self.redo.clear();
        self.drag = None;
        self.zoom = 1.;
        self.pan = Vec2::ZERO;
        self.changed();
    }
    fn apply_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::New => {
                self.replace_document(LabelDocument::blank(101.6, 152.4), None, true);
                self.status = "New label".into();
            }
            PendingAction::Template(index) => {
                self.replace_document(design::from_template(index), None, false);
                self.status = "Template opened · save to keep your label".into();
            }
            PendingAction::Copy(doc) => {
                self.replace_document(*doc, None, false);
                self.status = "Past label opened as a copy · save to keep it".into();
            }
            PendingAction::Open(path) => match LabelDocument::load(&path) {
                Ok(doc) => {
                    self.replace_document(doc, Some(path), true);
                    self.status = "Design opened".into();
                }
                Err(error) => {
                    self.status = format!("Cannot open design: {error:#}");
                    self.error = true;
                }
            },
            PendingAction::Close => self.close_approved = true,
        }
    }
    // Called from the app shell so switching workspaces never hides this prompt.
    pub(crate) fn confirm_pending(&mut self, ctx: &egui::Context) {
        if self.pending_action.is_none() {
            return;
        }
        let mut proceed = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::unique("unsaved-designer-label"))
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .inner_margin(24)
                    .corner_radius(16),
            )
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 96.).clamp(240., 440.));
                let accent = ui.visuals().selection.bg_fill;
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.), Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        12.,
                        if ui.visuals().dark_mode {
                            Color32::from_rgb(29, 47, 77)
                        } else {
                            Color32::from_rgb(225, 234, 249)
                        },
                    );
                    paint_tool_icon(
                        ui.painter(),
                        rect.center(),
                        if ui.visuals().dark_mode {
                            Color32::from_rgb(140, 185, 255)
                        } else {
                            accent
                        },
                        "Save document",
                    );
                    ui.add_space(4.);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(crate::i18n::tr("Save changes?"))
                                .size(24.)
                                .strong(),
                        );
                        ui.label(
                            RichText::new(crate::i18n::tr("Your label has unsaved changes."))
                                .weak(),
                        );
                    });
                });
                ui.add_space(12.);
                egui::Frame::new()
                    .fill(ui.visuals().extreme_bg_color)
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .corner_radius(10)
                    .inner_margin(14)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(crate::i18n::tr("Label")).small().weak());
                        ui.label(RichText::new(&self.doc.title).strong());
                    });
                ui.add_space(4.);
                ui.label(
                    RichText::new(crate::i18n::tr(
                        "Save your changes to keep them, or discard them to continue.",
                    ))
                    .weak(),
                );
                if self.error {
                    ui.add_space(4.);
                    ui.colored_label(ui.visuals().error_fg_color, &self.status);
                }
                ui.add_space(16.);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(crate::i18n::tr("Save")).color(Color32::WHITE),
                            )
                            .fill(accent)
                            .min_size(Vec2::new(96., 38.)),
                        )
                        .clicked()
                    {
                        proceed = self.save(false);
                    }
                    if ui
                        .add(
                            egui::Button::new(crate::i18n::tr("Discard"))
                                .min_size(Vec2::new(96., 38.)),
                        )
                        .clicked()
                    {
                        proceed = true;
                    }
                    if ui
                        .add(
                            egui::Button::new(crate::i18n::tr("Cancel"))
                                .min_size(Vec2::new(96., 38.)),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                });
            });
        if proceed {
            if let Some(action) = self.pending_action.take() {
                self.apply_action(action);
            }
            if self.close_approved {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else if cancel || response.should_close() {
            self.pending_action = None;
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
        let path = self.path.clone();
        self.start_job("Submitting design to CUPS…", move || {
            let submitted = if raw {
                let bytes = zpl::encode_design(&doc, settings.dpi)?;
                printing::submit_raw_zpl(&bytes, &settings, &printer)
            } else {
                let output = design_render::render_label(&doc, settings.dpi)?;
                printing::submit(&output, &settings, &printer)
            }?;
            crate::history::record(crate::history::design_entry(
                &doc,
                path.as_deref(),
                &settings.printer,
                settings.copies,
            ));
            Ok(submitted)
        });
    }
    fn batch_choose(state: &mut BatchState) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Spreadsheet export", &["csv", "tsv", "txt"])
            .pick_file()
        else {
            return;
        };
        state.preview = None;
        state.row = 0;
        let loaded = std::fs::metadata(&path)
            .map_err(anyhow::Error::from)
            .and_then(|meta| {
                if meta.len() > crate::batch::MAX_BYTES as u64 {
                    anyhow::bail!(
                        "The file is larger than {} MB",
                        crate::batch::MAX_BYTES / 1_000_000
                    );
                }
                Ok(std::fs::read(&path)?)
            })
            .and_then(|bytes| crate::batch::parse_table(&bytes));
        state.file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match loaded {
            Ok(table) => {
                state.table = Some(table);
                state.error = None;
            }
            Err(error) => {
                state.table = None;
                state.error = Some(format!("{error:#}"));
            }
        }
    }
    fn batch_window(&mut self, ctx: &egui::Context, settings: &Settings, printers: &[Printer]) {
        let Some(mut state) = self.batch.take() else {
            return;
        };
        let mut open = true;
        let mut start = false;
        let printer = printers.iter().find(|p| p.name == settings.printer);
        egui::Window::new(crate::i18n::tr("Batch print from CSV"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(480.)
            .show(ctx, |ui| {
                ui.label(crate::i18n::tr("Put ${column} placeholders in text, barcode, and QR content. Each row of your spreadsheet becomes one label."));
                ui.label(RichText::new(crate::i18n::tr("Export a CSV from your spreadsheet. Column names are not case-sensitive; ${serial} counts up from the design's serial number.")).small().weak());
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("Choose CSV file…")).clicked() {
                        Self::batch_choose(&mut state);
                    }
                    if ui
                        .button(crate::i18n::tr("Help"))
                        .on_hover_text(crate::i18n::tr("Open the user manual in your browser (F1)"))
                        .clicked()
                    {
                        ctx.open_url(egui::OpenUrl::new_tab(crate::help::manual_page("batch.html")));
                    }
                });
                if let Some(error) = &state.error {
                    ui.colored_label(Color32::RED, error);
                }
                let Some(table) = state.table.clone() else {
                    return;
                };
                let found = crate::batch::compare(&self.doc, &table);
                ui.label(format!(
                    "{} · {} rows · {} columns",
                    state.file,
                    table.rows.len(),
                    table.headers.len()
                ));
                if found.used == 0 && found.missing.is_empty() {
                    ui.colored_label(
                        Color32::from_rgb(200, 120, 20),
                        "This design has no ${column} placeholders yet. Type them into a text, barcode, or QR object, for example ${Name}.",
                    );
                }
                for name in &found.missing {
                    ui.colored_label(
                        Color32::from_rgb(200, 60, 60),
                        format!("The design uses ${{{name}}} but the file has no column with that name."),
                    );
                }
                if !found.unused.is_empty() {
                    ui.label(
                        RichText::new(format!("Columns not used: {}", found.unused.join(", ")))
                            .small()
                            .weak(),
                    );
                }
                ui.separator();
                state.row = state.row.min(table.rows.len() - 1);
                let mut shown = state.row + 1;
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Preview row"));
                    ui.add(egui::DragValue::new(&mut shown).range(1..=table.rows.len()))
                        .accessible_name(crate::i18n::tr("Preview row number"));
                    ui.label(format!("of {}", table.rows.len()));
                });
                state.row = shown - 1;
                if state.preview.as_ref().map(|(row, _)| *row) != Some(state.row) {
                    let label = crate::batch::apply_row(&self.doc, &table, state.row);
                    state.preview = design_render::render_label(&label, 203)
                        .ok()
                        .map(|output| {
                            (
                                state.row,
                                ctx.load_texture(
                                    "batch-preview",
                                    egui::ColorImage::from_rgb(
                                        [output.image.width() as usize, output.image.height() as usize],
                                        output.image.as_raw(),
                                    ),
                                    egui::TextureOptions::LINEAR,
                                ),
                            )
                        });
                }
                match &state.preview {
                    Some((_, texture)) => {
                        let size = texture.size_vec2();
                        let scale = (260. / size.x).min(260. / size.y).min(1.);
                        ui.add(egui::Image::new(texture).fit_to_exact_size(size * scale));
                    }
                    None => {
                        ui.label(RichText::new(crate::i18n::tr("This row cannot be previewed. Printing will report the problem.")).weak());
                    }
                }
                ui.separator();
                let ready = printer.is_some() && found.missing.is_empty() && found.used > 0;
                if printer.is_none() {
                    ui.label(RichText::new(crate::i18n::tr("Choose a printer in Print Center first.")).weak());
                }
                let copies = settings.copies;
                if ui
                    .add_enabled(
                        ready && self.job.is_none(),
                        egui::Button::new(format!(
                            "Print {} labels{}",
                            table.rows.len(),
                            if copies > 1 { format!(" × {copies} copies") } else { String::new() }
                        )),
                    )
                    .on_hover_text(crate::i18n::tr("Every row is checked first; nothing prints if a row has a problem."))
                    .clicked()
                {
                    start = true;
                }
            });
        if start && let (Some(table), Some(printer)) = (state.table.clone(), printer.cloned()) {
            let doc = self.doc.clone();
            let settings = settings.clone();
            let raw = self.raw_zpl;
            let total = table.rows.len();
            self.start_job("Printing batch…", move || {
                crate::batch::preflight(&doc, &table, settings.dpi).map_err(|problems| {
                    anyhow::anyhow!("Nothing was printed. {}", problems.join(" · "))
                })?;
                for index in 0..total {
                    let label = crate::batch::apply_row(&doc, &table, index);
                    if raw {
                        let bytes = zpl::encode_design(&label, settings.dpi)?;
                        printing::submit_raw_zpl(&bytes, &settings, &printer)
                    } else {
                        let output = design_render::render_label(&label, settings.dpi)?;
                        printing::submit(&output, &settings, &printer)
                    }
                    .map_err(|e| {
                        anyhow::anyhow!("Stopped at row {} of {total}: {e:#}", index + 1)
                    })?;
                }
                let mut entry =
                    crate::history::design_entry(&doc, None, &settings.printer, settings.copies);
                entry.title = format!("{} · batch of {total}", doc.title);
                crate::history::record(entry);
                Ok(format!(
                    "Submitted {total} labels. Use Job status to check the queue."
                ))
            });
            open = false;
        }
        if open {
            self.batch = Some(state);
        }
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
        if ctx.memory(|memory| memory.top_modal_layer().is_some()) {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save(false);
        }
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
        // Arrow keys nudge by 0.1 mm, or 1 mm with Shift.
        for (key, dx, dy) in [
            (egui::Key::ArrowLeft, -1., 0.),
            (egui::Key::ArrowRight, 1., 0.),
            (egui::Key::ArrowUp, 0., -1.),
            (egui::Key::ArrowDown, 0., 1.),
        ] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, key)) {
                self.nudge(dx, dy);
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                self.nudge(dx * 0.1, dy * 0.1);
            }
        }
    }
    pub fn show(&mut self, root: &mut egui::Ui, settings: &Settings, printers: &[Printer]) {
        let ctx = root.ctx().clone();
        self.poll(&ctx);
        self.poll_job(&ctx);
        egui::Panel::top("designer-toolbar").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.menu_button(crate::i18n::tr("File"), |ui| {
                    if ui.button(crate::i18n::tr("New label")).clicked() {
                        self.request_new();
                        ui.close();
                    }
                    if ui.button(crate::i18n::tr("Save as…")).clicked() {
                        self.save(true);
                        ui.close();
                    }
                    if ui
                        .button(crate::i18n::tr("Batch print from CSV…"))
                        .on_hover_text(crate::i18n::tr("Print one label per row of a spreadsheet, filling ${column} placeholders"))
                        .clicked()
                    {
                        self.batch = Some(BatchState {
                            file: String::new(),
                            table: None,
                            error: None,
                            row: 0,
                            preview: None,
                        });
                        ui.close();
                    }
                });
                if icon_action(
                    ui,
                    "Import label",
                    "Open",
                    "Open a saved label design",
                    true,
                    false,
                )
                .clicked()
                {
                    self.open();
                }
                if icon_action(
                    ui,
                    "Save document",
                    "Save",
                    "Save design (Ctrl+S)",
                    true,
                    false,
                )
                .clicked()
                {
                    self.save(false);
                }
                ui.separator();
                if icon_action(
                    ui,
                    "Undo",
                    "",
                    "Undo (Ctrl+Z)",
                    !self.undo.is_empty(),
                    false,
                )
                .clicked()
                {
                    self.undo();
                }
                if icon_action(
                    ui,
                    "Redo",
                    "",
                    "Redo (Ctrl+Y)",
                    !self.redo.is_empty(),
                    false,
                )
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
                if icon_action(
                    ui,
                    "Fit preview",
                    "",
                    "Fit label to canvas and reset panning",
                    true,
                    false,
                )
                .clicked()
                {
                    self.zoom = 1.;
                    self.pan = Vec2::ZERO;
                }
                ui.menu_button(crate::i18n::tr("View"), |ui| {
                    ui.checkbox(&mut self.grid, crate::i18n::tr("Show grid"));
                    ui.checkbox(&mut self.snap, crate::i18n::tr("Snap to grid"));
                    ui.checkbox(&mut self.smart_guides, crate::i18n::tr("Smart guides"))
                        .on_hover_text(crate::i18n::tr("Snap to other objects' edges and centers while dragging"));
                    ui.checkbox(&mut self.show_printable, crate::i18n::tr("Show printable area"))
                        .on_hover_text(crate::i18n::tr("Shade the border your printer cannot print on"));
                    ui.checkbox(&mut self.show_properties, crate::i18n::tr("Properties panel"));
                    ui.checkbox(&mut self.focus_canvas, crate::i18n::tr("Focus canvas"));
                });
                ui.menu_button(crate::i18n::tr("Export"), |ui| {
                    for (name, format) in [
                        ("PDF document", "pdf"),
                        ("PNG image", "png"),
                        ("SVG vector", "svg"),
                        ("ZPL label", "zpl"),
                    ] {
                        if ui.button(name).clicked() {
                            self.export(format, settings.dpi);
                            ui.close();
                        }
                    }
                    ui.separator();
                    ui.checkbox(&mut self.raw_zpl, crate::i18n::tr("Print using raw ZPL"))
                        .on_hover_text(crate::i18n::tr("Requires a ZPL-compatible printer and raw CUPS queue."));
                });
                if ui
                    .selectable_label(self.focus_canvas, crate::i18n::tr("Focus"))
                    .on_hover_text(crate::i18n::tr("Hide navigation and properties to expand the canvas"))
                    .clicked()
                {
                    self.focus_canvas = !self.focus_canvas;
                }
                if icon_action(
                    ui,
                    "Print device",
                    "Print",
                    "Print label using the selected printer",
                    true,
                    true,
                )
                .clicked()
                {
                    self.print(settings, printers);
                }
            });
        });
        self.batch_window(&ctx, settings, printers);
        egui::Panel::left("designer-elements")
            .default_size(68.)
            .size_range(68.0..=68.0)
            .resizable(false)
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.elements(ui));
            });
        if self.show_properties && !self.focus_canvas {
            egui::Panel::right("designer-inspector")
                .default_size(265.)
                .size_range(260.0..=420.0)
                .resizable(true)
                .show(root, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
                });
        }
        egui::Panel::bottom("designer-status").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{}{} · {:.1} × {:.1} mm · {}% · {} objects",
                    self.doc.title,
                    if self.is_modified() { " · Unsaved" } else { "" },
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
                    ui.label(crate::i18n::tr("Raw ZPL: verify printer compatibility and loaded label size."))
                        .on_hover_text(crate::help::explanation("ZPL"));
                } else if let Some(warning) = printer.media_mismatch(&settings.media, self.doc.width_mm, self.doc.height_mm) {
                    let color = if ui.visuals().dark_mode { Color32::from_rgb(245, 193, 99) }
                        else { Color32::from_rgb(132, 83, 10) };
                    ui.colored_label(color, warning);
                    ui.label(RichText::new(crate::i18n::tr("If the loaded paper matches the printer media, use the button below. To keep the current label size, choose matching media in Print Center instead.")).small().weak());
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
                    ui.label(crate::i18n::tr("Select printer media in Print Center before printing."));
                }
            } else {
                ui.label(crate::i18n::tr("Choose a printer in Print Center to print this design. Export is available without a printer."));
            }
        });
        self.printable = printers
            .iter()
            .find(|p| p.name == settings.printer)
            .filter(|p| {
                p.media_sizes.get(&settings.media).is_some_and(|&(w, h)| {
                    (w - self.doc.width_mm).abs() < 0.5 && (h - self.doc.height_mm).abs() < 0.5
                })
            })
            .and_then(|p| p.printable_margins_mm(&settings.media));
        egui::CentralPanel::default().show(root, |ui| self.canvas(ui));
        self.shortcut(&ctx);
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
                ui.label(crate::i18n::tr("Templates"));
                for (i, t) in design::TEMPLATES.iter().enumerate() {
                    if template_button(ui, t.name)
                        .on_hover_text(t.description)
                        .clicked()
                    {
                        self.request_template(i);
                        ui.close();
                    }
                }
            });
        });
    }
    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading(crate::i18n::tr("Properties"));
        let before = self.doc.clone();
        let selected_name = self
            .selected
            .first()
            .and_then(|id| self.doc.objects.iter().find(|object| object.id == *id))
            .map(|object| format!("{} #{}", object.label(), object.id))
            .unwrap_or_else(|| "Select an object".into());
        ui.label(crate::i18n::tr("Object"));
        egui::ComboBox::from_id_salt("designer-object-picker")
            .selected_text(selected_name)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(
                        self.selected.is_empty(),
                        crate::i18n::tr("Label setup (no selection)"),
                    )
                    .clicked()
                {
                    self.selected.clear();
                }
                for object in &self.doc.objects {
                    if ui
                        .selectable_label(
                            self.selected.contains(&object.id),
                            format!("{} #{}", object.label(), object.id),
                        )
                        .clicked()
                    {
                        self.selected = vec![object.id];
                    }
                }
            })
            .response
            .accessible_name(crate::i18n::tr("Select label object"));
        egui::CollapsingHeader::new("Label setup")
            .default_open(self.selected.is_empty())
            .show(ui, |ui| {
                let label = ui.label(crate::i18n::tr("Label name"));
                ui.text_edit_singleline(&mut self.doc.title)
                    .labelled_by(label.id);
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Width"));
                    ui.add(
                        egui::DragValue::new(&mut self.doc.width_mm)
                            .range(1.0..=2000.0)
                            .speed(0.2)
                            .suffix(" mm"),
                    )
                    .accessible_name(crate::i18n::tr("Label width in millimeters"));
                    ui.label(crate::i18n::tr("Height"));
                    ui.add(
                        egui::DragValue::new(&mut self.doc.height_mm)
                            .range(1.0..=2000.0)
                            .speed(0.2)
                            .suffix(" mm"),
                    )
                    .accessible_name(crate::i18n::tr("Label height in millimeters"));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Serial variable"));
                    ui.add(egui::DragValue::new(&mut self.doc.serial_number))
                        .accessible_name(crate::i18n::tr("Serial variable"));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Grid size"));
                    ui.add(
                        egui::DragValue::new(&mut self.doc.grid_mm)
                            .range(0.5..=50.0)
                            .speed(0.1)
                            .suffix(" mm"),
                    )
                    .on_hover_text(crate::i18n::tr(
                        "Spacing of the guide grid. Snapping moves in steps of a fifth of it.",
                    ))
                    .accessible_name(crate::i18n::tr("Grid size in millimeters"));
                });
            });
        ui.separator();
        if self.selected.len() > 1 {
            ui.label(format!("{} objects selected", self.selected.len()));
            let unlocked = self
                .doc
                .objects
                .iter()
                .filter(|o| self.selected.contains(&o.id) && !o.locked)
                .count();
            ui.add_space(4.);
            ui.label(RichText::new(crate::i18n::tr("Align")).strong());
            ui.horizontal_wrapped(|ui| {
                for (name, alignment) in [
                    ("Left", design::Alignment::Left),
                    ("Center", design::Alignment::CenterHorizontally),
                    ("Right", design::Alignment::Right),
                    ("Top", design::Alignment::Top),
                    ("Middle", design::Alignment::CenterVertically),
                    ("Bottom", design::Alignment::Bottom),
                ] {
                    if ui
                        .add_enabled(unlocked > 1, egui::Button::new(crate::i18n::tr(name)))
                        .on_hover_text(crate::i18n::tr(
                            "Align the selected objects with each other",
                        ))
                        .clicked()
                    {
                        self.arrange(alignment, design::AlignTo::Selection);
                    }
                }
            });
            ui.label(RichText::new(crate::i18n::tr("Distribute evenly")).strong());
            ui.horizontal_wrapped(|ui| {
                for (name, axis) in [
                    ("Horizontally", design::Axis::Horizontal),
                    ("Vertically", design::Axis::Vertical),
                ] {
                    if ui
                        .add_enabled(unlocked > 2, egui::Button::new(crate::i18n::tr(name)))
                        .on_hover_text(crate::i18n::tr(
                            "Make the gaps between three or more objects equal",
                        ))
                        .clicked()
                    {
                        self.distribute(axis);
                    }
                }
            });
            ui.label(RichText::new(crate::i18n::tr("Center on label")).strong());
            ui.horizontal_wrapped(|ui| {
                for (name, alignment) in [
                    ("Horizontally", design::Alignment::CenterHorizontally),
                    ("Vertically", design::Alignment::CenterVertically),
                ] {
                    if ui
                        .add_enabled(unlocked > 0, egui::Button::new(crate::i18n::tr(name)))
                        .on_hover_text(crate::i18n::tr(
                            "Center the selected objects as a group on the label",
                        ))
                        .clicked()
                    {
                        self.arrange_group_on_label(alignment);
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button(crate::i18n::tr("Lock all")).clicked() {
                    self.set_lock(true);
                }
                if ui.button(crate::i18n::tr("Unlock all")).clicked() {
                    self.set_lock(false);
                }
            });
            ui.separator();
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
            ui.label(crate::i18n::tr("Select an object on the canvas."));
            ui.label(RichText::new(crate::i18n::tr("Shift-click to select multiple objects. Use Ctrl+C/V, Ctrl+D, Delete, Ctrl+Z/Y.")).small().weak());
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
        let locked = o.locked;
        let mut center = (false, false);
        ui.add_enabled_ui(!locked, |ui| {
            ui.horizontal(|ui| {
                ui.label("X");
                ui.add(egui::DragValue::new(&mut o.x_mm).speed(0.1).suffix(" mm"))
                    .accessible_name(crate::i18n::tr("Object horizontal position in millimeters"));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut o.y_mm).speed(0.1).suffix(" mm"))
                    .accessible_name(crate::i18n::tr("Object vertical position in millimeters"));
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("W"));
                ui.add(
                    egui::DragValue::new(&mut o.width_mm)
                        .range(1.0..=2000.0)
                        .speed(0.1)
                        .suffix(" mm"),
                )
                .accessible_name(crate::i18n::tr("Object width in millimeters"));
                ui.label(crate::i18n::tr("H"));
                ui.add(
                    egui::DragValue::new(&mut o.height_mm)
                        .range(1.0..=2000.0)
                        .speed(0.1)
                        .suffix(" mm"),
                )
                .accessible_name(crate::i18n::tr("Object height in millimeters"));
            });
            ui.horizontal(|ui| {
                let label = ui.label(crate::i18n::tr("Rotation"));
                let mut angle = o.rotation;
                if ui
                    .add(
                        egui::DragValue::new(&mut angle)
                            .speed(0.5)
                            .range(-360.0..=360.0)
                            .max_decimals(2)
                            .suffix("°"),
                    )
                    .labelled_by(label.id)
                    .changed()
                {
                    o.rotate_to(angle);
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(crate::i18n::tr("Rotate 90°"))
                    .on_hover_text(crate::i18n::tr("Turn the object a quarter turn clockwise"))
                    .clicked()
                {
                    let angle = o.rotation + 90.;
                    o.rotate_to(angle);
                }
                ui.checkbox(&mut o.flip_x, crate::i18n::tr("Flip horizontally"));
                ui.checkbox(&mut o.flip_y, crate::i18n::tr("Flip vertically"));
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::i18n::tr("Center on label"));
                center.0 = ui.small_button(crate::i18n::tr("Horizontally")).clicked();
                center.1 = ui.small_button(crate::i18n::tr("Vertically")).clicked();
            });
        });
        ui.checkbox(&mut o.locked, crate::i18n::tr("Lock position and size"))
            .on_hover_text(crate::i18n::tr(
                "A locked object cannot be moved, resized, rotated, or deleted from the canvas",
            ));
        match &mut o.element {
            Element::Text {
                text,
                font_pt,
                bold,
                align,
            } => {
                ui.label(crate::i18n::tr("Text · ${date} and ${serial} are dynamic"));
                ui.add(egui::TextEdit::multiline(text).desired_rows(6))
                    .accessible_name(crate::i18n::tr("Object text"));
                ui.add(
                    egui::DragValue::new(font_pt)
                        .range(3.0..=300.0)
                        .suffix(" pt"),
                )
                .accessible_name(crate::i18n::tr("Font size in points"));
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(bold, crate::i18n::tr("Bold"));
                    ui.checkbox(&mut o.options.italic, crate::i18n::tr("Italic"));
                    ui.checkbox(&mut o.options.inverse, crate::i18n::tr("Inverse"))
                        .on_hover_text(crate::i18n::tr("White letters on a black box"));
                });
                ui.horizontal(|ui| {
                    ui.selectable_value(align, TextAlign::Left, crate::i18n::tr("Left"));
                    ui.selectable_value(align, TextAlign::Center, crate::i18n::tr("Center"));
                    ui.selectable_value(align, TextAlign::Right, crate::i18n::tr("Right"));
                });
                ui.checkbox(&mut o.options.wrap, crate::i18n::tr("Wrap long lines"))
                    .on_hover_text(crate::i18n::tr(
                        "Break lines at spaces so they fit the box width",
                    ));
                ui.checkbox(&mut o.options.fit, crate::i18n::tr("Shrink to fit the box"))
                    .on_hover_text(crate::i18n::tr(
                        "Make the text smaller until all of it fits inside the box",
                    ));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Line spacing"));
                    ui.add(
                        egui::DragValue::new(&mut o.options.line_spacing)
                            .range(0.5..=3.0)
                            .speed(0.02)
                            .max_decimals(2)
                            .suffix(" ×"),
                    )
                    .accessible_name(crate::i18n::tr("Line spacing multiplier"));
                });
            }
            Element::Barcode {
                kind,
                data,
                show_text,
            } => {
                ui.label(crate::i18n::tr("Enter your real product code or reference"));
                ui.text_edit_singleline(data)
                    .accessible_name(crate::i18n::tr("Barcode or QR code content"));
                egui::ComboBox::from_id_salt("barcode-kind")
                    .selected_text(barcode_name(*kind))
                    .show_ui(ui, |ui| {
                        for k in [
                            BarcodeKind::Code128,
                            BarcodeKind::Code39,
                            BarcodeKind::Code93,
                            BarcodeKind::Ean13,
                            BarcodeKind::Ean8,
                            BarcodeKind::UpcA,
                            BarcodeKind::Itf,
                            BarcodeKind::Codabar,
                            BarcodeKind::DataMatrix,
                        ] {
                            ui.selectable_value(kind, k, barcode_name(k))
                                .on_hover_text(barcode_hint(k));
                        }
                    });
                ui.checkbox(show_text, crate::i18n::tr("Show value below bars"));
                if *show_text {
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("Text size"));
                        ui.add(
                            egui::DragValue::new(&mut o.options.barcode_text_pt)
                                .range(0.0..=60.0)
                                .speed(0.2)
                                .custom_formatter(|value, _| {
                                    if value == 0. {
                                        "Auto".to_owned()
                                    } else {
                                        format!("{value:.1} pt")
                                    }
                                })
                                .custom_parser(|text| {
                                    let text = text.trim().trim_end_matches("pt").trim();
                                    if text.eq_ignore_ascii_case("auto") {
                                        Some(0.)
                                    } else {
                                        text.parse().ok()
                                    }
                                }),
                        )
                        .accessible_name(crate::i18n::tr("Readable text size in points"));
                    });
                    if o.options.barcode_text_pt > 0. && o.options.barcode_text_pt < 2. {
                        o.options.barcode_text_pt = 2.;
                    }
                    ui.checkbox(
                        &mut o.options.text_above,
                        crate::i18n::tr("Text above the bars"),
                    );
                }
                if let Some(hint) = check_digit_hint(*kind, data) {
                    ui.label(RichText::new(hint).small().weak());
                }
            }
            Element::Qr { data } => {
                ui.label(crate::i18n::tr("QR content"))
                    .on_hover_text(crate::help::explanation("QR"));
                ui.add(egui::TextEdit::multiline(data).desired_rows(4))
                    .accessible_name(crate::i18n::tr("QR code content"));
                egui::ComboBox::from_id_salt("qr-level")
                    .selected_text(qr_level_name(o.options.qr_level))
                    .show_ui(ui, |ui| {
                        for level in [
                            design::QrLevel::Low,
                            design::QrLevel::Medium,
                            design::QrLevel::Quartile,
                            design::QrLevel::High,
                        ] {
                            ui.selectable_value(&mut o.options.qr_level, level, qr_level_name(level))
                                .on_hover_text(crate::i18n::tr(
                                    "More error correction survives more damage but makes a denser code",
                                ));
                        }
                    })
                    .response
                    .accessible_name(crate::i18n::tr("QR error correction level"));
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
                )
                .accessible_name(crate::i18n::tr("Shape stroke width in millimeters"));
                ui.checkbox(filled, crate::i18n::tr("Filled"));
            }
            Element::Image { .. } => {
                ui.label(crate::i18n::tr(
                    "Embedded image. Resize with W/H or the canvas handle.",
                ));
                egui::ComboBox::from_id_salt("image-mode")
                    .selected_text(image_mode_name(o.options.image_mode))
                    .show_ui(ui, |ui| {
                        for mode in [
                            design::ImageMode::Original,
                            design::ImageMode::Threshold,
                            design::ImageMode::Dither,
                        ] {
                            ui.selectable_value(
                                &mut o.options.image_mode,
                                mode,
                                image_mode_name(mode),
                            )
                            .on_hover_text(crate::i18n::tr(image_mode_hint(mode)));
                        }
                    })
                    .response
                    .accessible_name(crate::i18n::tr(
                        "How the image is reduced to black and white",
                    ));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Contrast"));
                    let mut percent = o.options.contrast * 100.;
                    if ui
                        .add(
                            egui::DragValue::new(&mut percent)
                                .range(-100.0..=100.0)
                                .speed(1.)
                                .suffix(" %"),
                        )
                        .accessible_name(crate::i18n::tr("Image contrast in percent"))
                        .changed()
                    {
                        o.options.contrast = percent / 100.;
                    }
                });
                if o.options.image_mode == design::ImageMode::Threshold {
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("Cut-off"));
                        ui.add(egui::Slider::new(&mut o.options.threshold, 1..=254))
                            .on_hover_text(crate::i18n::tr("Pixels darker than this print black"));
                    });
                }
                ui.checkbox(
                    &mut o.options.invert,
                    crate::i18n::tr("Invert black and white"),
                );
                if ui
                    .small_button(crate::i18n::tr("Reset image adjustments"))
                    .clicked()
                {
                    let keep = o.options.clone();
                    o.options = design::ObjectOptions {
                        wrap: keep.wrap,
                        ..Default::default()
                    };
                }
            }
        }
        if center.0 {
            self.arrange(
                design::Alignment::CenterHorizontally,
                design::AlignTo::Label,
            );
        }
        if center.1 {
            self.arrange(design::Alignment::CenterVertically, design::AlignTo::Label);
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
        ui.label(
            RichText::new(crate::i18n::tr(
                "Drag to move · square to resize · circle to rotate · Shift: 15°",
            ))
            .small()
            .weak(),
        );
        let Some(tex) = self.texture.clone() else {
            ui.spinner();
            return;
        };
        let available = (ui.available_size() - Vec2::splat(64.)).max(Vec2::splat(1.));
        let source = tex.size_vec2();
        let fit = (available.x / source.x)
            .min(available.y / source.y)
            .max(0.05);
        let size = source * fit * self.zoom;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (outer, _) = ui.allocate_exact_size(
                    Vec2::new(size.x.max(available.x), size.y.max(available.y)) + Vec2::splat(64.),
                    Sense::hover(),
                );
                let paper = Rect::from_center_size(outer.center() + self.pan, size);
                ui.painter().rect_filled(
                    paper.expand(6.),
                    8.,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                );
                ui.painter().image(
                    tex.id(),
                    paper,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                    Color32::WHITE,
                );
                if self.grid {
                    let color = Color32::from_rgba_unmultiplied(90, 110, 130, 45);
                    let step = (size.x / self.doc.width_mm * self.doc.grid_mm).max(4.);
                    let mut x = paper.left();
                    while x < paper.right() {
                        ui.painter().line_segment(
                            [Pos2::new(x, paper.top()), Pos2::new(x, paper.bottom())],
                            Stroke::new(1., color),
                        );
                        x += step;
                    }
                    let mut y = paper.top();
                    while y < paper.bottom() {
                        ui.painter().line_segment(
                            [Pos2::new(paper.left(), y), Pos2::new(paper.right(), y)],
                            Stroke::new(1., color),
                        );
                        y += (size.y / self.doc.height_mm * self.doc.grid_mm).max(4.);
                    }
                }
                if self.show_printable
                    && let Some((left, top, right, bottom)) = self.printable
                {
                    let (sx, sy) = (
                        paper.width() / self.doc.width_mm,
                        paper.height() / self.doc.height_mm,
                    );
                    let amber = Color32::from_rgba_unmultiplied(235, 150, 20, 60);
                    let inner = Rect::from_min_max(
                        Pos2::new(paper.left() + left * sx, paper.top() + top * sy),
                        Pos2::new(paper.right() - right * sx, paper.bottom() - bottom * sy),
                    );
                    for band in [
                        Rect::from_min_max(paper.min, Pos2::new(paper.right(), inner.top())),
                        Rect::from_min_max(Pos2::new(paper.left(), inner.bottom()), paper.max),
                        Rect::from_min_max(
                            Pos2::new(paper.left(), inner.top()),
                            Pos2::new(inner.left(), inner.bottom()),
                        ),
                        Rect::from_min_max(
                            Pos2::new(inner.right(), inner.top()),
                            Pos2::new(paper.right(), inner.bottom()),
                        ),
                    ] {
                        ui.painter().rect_filled(band, 0., amber);
                    }
                    ui.painter().rect_stroke(
                        inner,
                        0.,
                        Stroke::new(1., Color32::from_rgb(210, 120, 10)),
                        StrokeKind::Inside,
                    );
                }
                let single = if self.selected.len() == 1 {
                    self.doc
                        .objects
                        .iter()
                        .find(|object| self.selected.contains(&object.id) && !object.locked)
                } else {
                    None
                };
                let handles = single.map(|object| self.object_geometry(object, paper));
                let blue = Color32::from_rgb(70, 130, 240);
                for object in &self.doc.objects {
                    if self.selected.contains(&object.id) {
                        let geometry = self.object_geometry(object, paper);
                        ui.painter().add(egui::Shape::closed_line(
                            geometry.corners.to_vec(),
                            Stroke::new(2., blue),
                        ));
                        if self.selected.len() == 1 {
                            let top = geometry.corners[0]
                                + (geometry.corners[1] - geometry.corners[0]) * 0.5;
                            ui.painter().line_segment(
                                [top, geometry.rotation_handle],
                                Stroke::new(1.5, blue),
                            );
                            ui.painter().circle_filled(
                                geometry.rotation_handle,
                                7.,
                                Color32::WHITE,
                            );
                            ui.painter().circle_stroke(
                                geometry.rotation_handle,
                                7.,
                                Stroke::new(2., blue),
                            );
                            ui.painter().rect_filled(
                                Rect::from_center_size(geometry.corners[2], Vec2::splat(10.)),
                                2.,
                                blue,
                            );
                            if self
                                .drag
                                .as_ref()
                                .is_some_and(|drag| matches!(drag.kind, DragKind::Rotate))
                            {
                                ui.painter().text(
                                    geometry.rotation_handle + Vec2::new(13., 0.),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{:.1}°", object.rotation),
                                    egui::TextStyle::Body.resolve(ui.style()),
                                    blue,
                                );
                            }
                        }
                    }
                }
                for &(vertical, position) in &self.guides {
                    let magenta = Color32::from_rgb(220, 50, 160);
                    let points = if vertical {
                        let x = paper.left() + position / self.doc.width_mm * paper.width();
                        [
                            Pos2::new(x, paper.top() - 4.),
                            Pos2::new(x, paper.bottom() + 4.),
                        ]
                    } else {
                        let y = paper.top() + position / self.doc.height_mm * paper.height();
                        [
                            Pos2::new(paper.left() - 4., y),
                            Pos2::new(paper.right() + 4., y),
                        ]
                    };
                    ui.painter().line_segment(points, Stroke::new(1., magenta));
                }
                let mut response = ui.interact(
                    outer,
                    ui.scope_id().with("design-canvas"),
                    Sense::click_and_drag(),
                );
                if let (Some(geometry), Some(pointer)) = (handles, response.hover_pos()) {
                    if geometry.rotation_handle.distance(pointer) <= 14. {
                        response = response
                            .on_hover_cursor(egui::CursorIcon::Crosshair)
                            .on_hover_text(crate::i18n::tr(
                                "Drag to rotate freely. Hold Shift to snap to 15°.",
                            ));
                    } else if geometry.corners[2].distance(pointer) <= 14. {
                        response = response.on_hover_cursor(egui::CursorIcon::ResizeNwSe);
                    }
                }
                if response.clicked()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let handle_clicked = handles.is_some_and(|geometry| {
                        geometry.rotation_handle.distance(pointer) <= 14.
                            || geometry.corners[2].distance(pointer) <= 14.
                    });
                    if !handle_clicked {
                        if let Some(id) = self.hit(pointer, paper) {
                            if ui.input(|input| input.modifiers.shift) {
                                if self.selected.contains(&id) {
                                    self.selected.retain(|selected| *selected != id);
                                } else {
                                    self.selected.push(id);
                                }
                            } else {
                                self.selected = vec![id];
                            }
                        } else {
                            self.selected.clear();
                        }
                    }
                }
                if response.drag_started_by(egui::PointerButton::Primary)
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let start = ui
                        .input(|input| input.pointer.press_origin())
                        .unwrap_or(pointer);
                    let kind = if handles
                        .is_some_and(|geometry| geometry.rotation_handle.distance(start) <= 14.)
                    {
                        Some(DragKind::Rotate)
                    } else if handles
                        .is_some_and(|geometry| geometry.corners[2].distance(start) <= 14.)
                    {
                        Some(DragKind::Resize)
                    } else if let Some(id) = self.hit(start, paper) {
                        if !self.selected.contains(&id) {
                            self.selected = vec![id];
                        }
                        Some(DragKind::Move)
                    } else {
                        None
                    };
                    // Locked objects never move, resize, or rotate from the canvas.
                    let initial: Vec<DesignObject> = self
                        .doc
                        .objects
                        .iter()
                        .filter(|object| self.selected.contains(&object.id) && !object.locked)
                        .cloned()
                        .collect();
                    if let Some(kind) = kind
                        && !initial.is_empty()
                    {
                        self.checkpoint();
                        self.drag = Some(Drag {
                            kind,
                            start,
                            center: handles.map_or(start, |geometry| geometry.center),
                            initial,
                        });
                    }
                }
                if response.dragged_by(egui::PointerButton::Primary)
                    && let (Some(drag), Some(pointer)) =
                        (&self.drag, response.interact_pointer_pos())
                {
                    let delta = Vec2::new(
                        (pointer.x - drag.start.x) / size.x * self.doc.width_mm,
                        (pointer.y - drag.start.y) / size.y * self.doc.height_mm,
                    );
                    let step = if self.snap {
                        (self.doc.grid_mm / 5.).max(0.1)
                    } else {
                        0.1
                    };
                    let (guided_x, guided_y, guides) =
                        if matches!(drag.kind, DragKind::Move) && self.smart_guides {
                            let threshold = 6. / (size.x / self.doc.width_mm);
                            smart_snap(&self.doc, &drag.initial, delta, threshold)
                        } else {
                            (None, None, Vec::new())
                        };
                    self.guides = guides;
                    let angle_delta = vector_angle(pointer - drag.center)
                        - vector_angle(drag.start - drag.center);
                    let snap_angle = ui.input(|input| input.modifiers.shift);
                    for initial in &drag.initial {
                        if let Some(object) = self
                            .doc
                            .objects
                            .iter_mut()
                            .find(|object| object.id == initial.id)
                        {
                            match drag.kind {
                                DragKind::Move => {
                                    object.x_mm = match guided_x {
                                        Some(dx) => initial.x_mm + dx,
                                        None => ((initial.x_mm + delta.x) / step).round() * step,
                                    };
                                    object.y_mm = match guided_y {
                                        Some(dy) => initial.y_mm + dy,
                                        None => ((initial.y_mm + delta.y) / step).round() * step,
                                    };
                                }
                                DragKind::Resize => resize_rotated(object, initial, delta, step),
                                DragKind::Rotate => {
                                    let angle = rotation_from_drag(
                                        initial.rotation,
                                        angle_delta,
                                        snap_angle,
                                    );
                                    let (cx, cy) = initial.center_mm();
                                    object.rotation = angle;
                                    let (width, height) = object.rotated_size_mm();
                                    object.x_mm = cx - width / 2.;
                                    object.y_mm = cy - height / 2.;
                                }
                            }
                        }
                    }
                    self.changed();
                }
                if response.drag_stopped() {
                    self.drag = None;
                    self.guides.clear();
                }
                if response.dragged_by(egui::PointerButton::Middle) {
                    self.pan += response.drag_delta();
                }
            });
    }
    fn object_geometry(&self, object: &DesignObject, paper: Rect) -> ObjectGeometry {
        let (cx, cy) = object.center_mm();
        let center = Pos2::new(
            paper.left() + cx / self.doc.width_mm * paper.width(),
            paper.top() + cy / self.doc.height_mm * paper.height(),
        );
        let (sin, cos) = object.rotation.rem_euclid(360.).to_radians().sin_cos();
        let w = object.width_mm / 2.;
        let h = object.height_mm / 2.;
        let corners = [(-w, -h), (w, -h), (w, h), (-w, h)].map(|(x, y)| {
            center
                + Vec2::new(
                    (cos * x - sin * y) / self.doc.width_mm * paper.width(),
                    (sin * x + cos * y) / self.doc.height_mm * paper.height(),
                )
        });
        let top = corners[0] + (corners[1] - corners[0]) * 0.5;
        let rotation_handle = top + (top - center).normalized() * 28.;
        ObjectGeometry {
            center,
            corners,
            rotation_handle,
        }
    }
    fn hit(&self, pointer: Pos2, paper: Rect) -> Option<u64> {
        let point_mm = Vec2::new(
            (pointer.x - paper.left()) / paper.width() * self.doc.width_mm,
            (pointer.y - paper.top()) / paper.height() * self.doc.height_mm,
        );
        let tolerance = 6. * self.doc.width_mm / paper.width();
        self.doc
            .objects
            .iter()
            .rev()
            .find(|object| {
                let (cx, cy) = object.center_mm();
                let (sin, cos) = object.rotation.rem_euclid(360.).to_radians().sin_cos();
                let dx = point_mm.x - cx;
                let dy = point_mm.y - cy;
                (cos * dx + sin * dy).abs() <= object.width_mm / 2. + tolerance
                    && (-sin * dx + cos * dy).abs() <= object.height_mm / 2. + tolerance
            })
            .map(|object| object.id)
    }
}

#[derive(Clone, Copy)]
struct ObjectGeometry {
    center: Pos2,
    corners: [Pos2; 4],
    rotation_handle: Pos2,
}

fn vector_angle(vector: Vec2) -> f32 {
    vector.y.atan2(vector.x).to_degrees()
}
fn rotation_from_drag(initial: f32, delta: f32, snap: bool) -> f32 {
    let angle = initial + delta;
    (if snap {
        (angle / 15.).round() * 15.
    } else {
        angle
    })
    .rem_euclid(360.)
}
fn resize_rotated(object: &mut DesignObject, initial: &DesignObject, delta: Vec2, step: f32) {
    let (sin, cos) = initial.rotation.rem_euclid(360.).to_radians().sin_cos();
    let local = Vec2::new(
        cos * delta.x + sin * delta.y,
        -sin * delta.x + cos * delta.y,
    );
    let width = (((initial.width_mm + local.x) / step).round() * step).clamp(1., 2000.);
    let height = (((initial.height_mm + local.y) / step).round() * step).clamp(1., 2000.);
    let (cx, cy) = initial.center_mm();
    let dw = (width - initial.width_mm) / 2.;
    let dh = (height - initial.height_mm) / 2.;
    object.width_mm = width;
    object.height_mm = height;
    let (bounds_w, bounds_h) = object.rotated_size_mm();
    object.x_mm = cx + cos * dw - sin * dh - bounds_w / 2.;
    object.y_mm = cy + sin * dw + cos * dh - bounds_h / 2.;
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
    let shown = crate::i18n::tr(name);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), shown));
    let visuals = ui.style().interact(&response);
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 5., visuals.bg_fill);
    }
    // The icon is chosen from the English name, which stays the template's identity.
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
        shown,
        egui::TextStyle::Button.resolve(ui.style()),
        visuals.fg_stroke.color,
    );
    response
}

fn qr_level_name(level: design::QrLevel) -> &'static str {
    match level {
        design::QrLevel::Low => "Low (7%)",
        design::QrLevel::Medium => "Medium (15%)",
        design::QrLevel::Quartile => "Quartile (25%)",
        design::QrLevel::High => "High (30%)",
    }
}
fn image_mode_name(mode: design::ImageMode) -> &'static str {
    match mode {
        design::ImageMode::Original => "Original",
        design::ImageMode::Threshold => "Black and white",
        design::ImageMode::Dither => "Dithered",
    }
}
fn image_mode_hint(mode: design::ImageMode) -> &'static str {
    match mode {
        design::ImageMode::Original => "Draw the picture as it is and let the printer convert it",
        design::ImageMode::Threshold => {
            "Every pixel becomes black or white. Good for logos and line art"
        }
        design::ImageMode::Dither => "Shading made of black dots. Good for photos",
    }
}
/// What to tell the user about the check digit of an EAN or UPC value.
fn check_digit_hint(kind: BarcodeKind, data: &str) -> Option<String> {
    let body = match kind {
        BarcodeKind::Ean13 => 12,
        BarcodeKind::Ean8 => 7,
        BarcodeKind::UpcA => 11,
        _ => return None,
    };
    let digit = design_render::check_digit(kind, data)?;
    if data.len() == body {
        Some(format!("Check digit {digit} is added when printed"))
    } else if data.len() == body + 1 {
        if data.ends_with(digit) {
            Some("The check digit is correct".into())
        } else {
            Some(format!("Wrong check digit: it should be {digit}"))
        }
    } else {
        None
    }
}

/// Snap a move to the edges and centers of other objects and of the label. Returns the offset
/// to use on each axis when something was close enough, and the guide lines that were hit.
fn smart_snap(
    doc: &LabelDocument,
    moving: &[DesignObject],
    delta: Vec2,
    threshold_mm: f32,
) -> (Option<f32>, Option<f32>, Vec<(bool, f32)>) {
    let outline = |o: &DesignObject| {
        let (w, h) = o.rotated_size_mm();
        (o.x_mm, o.y_mm, o.x_mm + w, o.y_mm + h)
    };
    let bounds = moving
        .iter()
        .map(outline)
        .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, b| {
            (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
        });
    let ids: Vec<u64> = moving.iter().map(|o| o.id).collect();
    let mut xs = vec![0., doc.width_mm / 2., doc.width_mm];
    let mut ys = vec![0., doc.height_mm / 2., doc.height_mm];
    for other in doc.objects.iter().filter(|o| !ids.contains(&o.id)) {
        let (x0, y0, x1, y1) = outline(other);
        xs.extend([x0, (x0 + x1) / 2., x1]);
        ys.extend([y0, (y0 + y1) / 2., y1]);
    }
    let best = |anchors: [f32; 3], candidates: &[f32]| {
        let mut best: Option<(f32, f32)> = None;
        for anchor in anchors {
            for &candidate in candidates {
                let gap = candidate - anchor;
                if gap.abs() <= threshold_mm && best.is_none_or(|(g, _)| gap.abs() < g.abs()) {
                    best = Some((gap, candidate));
                }
            }
        }
        best
    };
    let (nx0, ny0) = (bounds.0 + delta.x, bounds.1 + delta.y);
    let (nx1, ny1) = (bounds.2 + delta.x, bounds.3 + delta.y);
    let horizontal = best([nx0, (nx0 + nx1) / 2., nx1], &xs);
    let vertical = best([ny0, (ny0 + ny1) / 2., ny1], &ys);
    let mut guides = Vec::new();
    if let Some((_, at)) = horizontal {
        guides.push((true, at));
    }
    if let Some((_, at)) = vertical {
        guides.push((false, at));
    }
    (
        horizontal.map(|(gap, _)| delta.x + gap),
        vertical.map(|(gap, _)| delta.y + gap),
        guides,
    )
}

fn barcode_name(kind: BarcodeKind) -> &'static str {
    match kind {
        BarcodeKind::Code128 => "Code 128",
        BarcodeKind::Code39 => "Code 39",
        BarcodeKind::Code93 => "Code 93",
        BarcodeKind::Ean13 => "EAN-13",
        BarcodeKind::Ean8 => "EAN-8",
        BarcodeKind::UpcA => "UPC-A",
        BarcodeKind::Itf => "ITF (Interleaved 2 of 5)",
        BarcodeKind::Codabar => "Codabar",
        BarcodeKind::DataMatrix => "Data Matrix",
    }
}

fn barcode_hint(kind: BarcodeKind) -> &'static str {
    match kind {
        BarcodeKind::Code128 => "A compact barcode for letters, digits, and product references.",
        BarcodeKind::Code39 => {
            "A barcode for uppercase letters, digits, and a limited set of symbols."
        }
        BarcodeKind::Code93 => "A denser alternative to Code 39 for letters, digits, and symbols.",
        BarcodeKind::Ean13 => crate::help::explanation("EAN"),
        BarcodeKind::Ean8 => {
            "A short retail barcode for small packages. Enter 7 digits, or 8 with the check digit."
        }
        BarcodeKind::UpcA => crate::help::explanation("UPC"),
        BarcodeKind::Itf => "Digits only, in pairs. Common on shipping cartons.",
        BarcodeKind::Codabar => {
            "Digits and a few symbols, framed by A–D. Used in libraries and blood banks."
        }
        BarcodeKind::DataMatrix => {
            "A small square 2D symbol for tiny parts and labels. Keep the object roughly square."
        }
    }
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
        "External link" => {
            path(&[(10., 4.), (4., 4.), (4., 20.), (20., 20.), (20., 14.)]);
            path(&[(13., 3.), (21., 3.), (21., 11.)]);
            path(&[(11., 13.), (21., 3.)]);
        }
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
        "Collapse navigation" | "Expand navigation" => {
            outline(3., 4., 18., 16., 2.);
            path(&[(9., 4.), (9., 20.)]);
            if label == "Collapse navigation" {
                path(&[(16., 9.), (13., 12.), (16., 15.)]);
            } else {
                path(&[(13., 9.), (16., 12.), (13., 15.)]);
            }
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
    let (label, tooltip) = (crate::i18n::tr(label), crate::i18n::tr(tooltip));
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

#[cfg(test)]
mod document_lifecycle_tests {
    use super::*;

    #[test]
    fn opening_saved_design_preserves_path_and_starts_clean() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("label with spaces.thermalprint");
        let doc = design::from_template(1);
        doc.save(&path).unwrap();
        let mut designer = Designer::new();
        designer.request_open(path.clone());
        assert_eq!(designer.doc, doc);
        assert_eq!(designer.path, Some(path));
        assert!(!designer.is_modified());
        designer.checkpoint();
        designer.doc.title.push_str(" edited");
        assert!(designer.is_modified());
        designer.undo();
        assert!(!designer.is_modified());
        designer.redo();
        assert!(designer.is_modified());
    }

    #[test]
    fn destructive_actions_wait_for_confirmation_and_cancel_preserves_changes() {
        let mut designer = Designer::new();
        designer.doc.title = "Unsaved work".into();
        let original = designer.doc.clone();
        designer.request_new();
        assert!(matches!(designer.pending_action, Some(PendingAction::New)));
        assert_eq!(designer.doc, original);
        designer.pending_action = None; // Cancel
        assert_eq!(designer.doc, original);
        designer.request_template(1);
        assert!(matches!(
            designer.pending_action,
            Some(PendingAction::Template(1))
        ));
        assert_eq!(designer.doc, original);
        designer.request_open(PathBuf::from("another.thermalprint"));
        assert!(matches!(
            designer.pending_action,
            Some(PendingAction::Open(_))
        ));
        assert_eq!(designer.doc, original);
        assert!(!designer.request_close());
        assert!(matches!(
            designer.pending_action,
            Some(PendingAction::Close)
        ));
        assert_eq!(designer.doc, original);
    }

    #[test]
    fn save_tracks_latest_changes_and_failed_save_keeps_pending_action() {
        let directory = tempfile::tempdir().unwrap();
        let mut designer = Designer::new();
        designer.doc.title = "First revision".into();
        let path = directory.path().join("label.thermalprint");
        assert!(designer.save_to_path(path.clone()));
        assert!(!designer.is_modified());
        designer.doc.title = "Latest revision".into();
        assert!(designer.is_modified());
        designer.request_new();
        assert!(!designer.save_to_path(directory.path().join("missing/label.thermalprint")));
        assert!(designer.is_modified());
        assert!(designer.pending_action.is_some());
        assert!(designer.save_to_path(path.clone()));
        assert!(!designer.is_modified());
        assert_eq!(LabelDocument::load(&path).unwrap().title, "Latest revision");
        let action = designer.pending_action.take().unwrap();
        designer.apply_action(action);
        assert!(!designer.is_modified());
        assert!(designer.path.is_none());
    }

    #[test]
    fn invalid_open_keeps_current_document_and_close_discard_is_not_reprompted() {
        let mut designer = Designer::new();
        designer.request_open(PathBuf::from("/missing/label.thermalprint"));
        assert!(designer.error);
        assert_eq!(designer.doc, designer.saved_document);
        designer.doc.title.push_str(" modified");
        assert!(!designer.request_close());
        let action = designer.pending_action.take().unwrap();
        designer.apply_action(action);
        assert!(designer.request_close());
    }

    #[test]
    fn only_design_extension_routes_to_designer() {
        for path in [
            "label.thermalprint",
            "LABEL.THERMALPRINT",
            "folder with spaces/label.thermalprint",
        ] {
            assert!(design::is_design_path(std::path::Path::new(path)));
        }
        for path in [
            "label.pdf",
            "label.png",
            "label.thermaprint",
            "label.thermalprint.json",
        ] {
            assert!(!design::is_design_path(std::path::Path::new(path)));
        }
    }
}

#[cfg(test)]
mod rotation_interaction_tests {
    use super::*;

    #[test]
    fn rotation_wraps_and_only_snaps_when_requested() {
        assert_eq!(rotation_from_drag(350., 20., false), 10.);
        assert_eq!(rotation_from_drag(0., 12.25, false), 12.25);
        assert_eq!(rotation_from_drag(0., 17., true), 15.);
        assert_eq!(rotation_from_drag(0., -17., true), 345.);
    }

    #[test]
    fn rotated_hit_testing_and_resize_follow_the_object_axes() {
        let mut designer = Designer::new();
        designer.doc = LabelDocument::blank(100., 100.);
        let id = designer.doc.add(
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
        designer.doc.objects[0].rotate_to(45.);
        let initial = designer.doc.objects[0].clone();
        let paper = Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.));
        let geometry = designer.object_geometry(&initial, paper);
        assert_eq!(designer.hit(geometry.center, paper), Some(id));
        assert_eq!(
            designer.hit(geometry.center + Vec2::new(-60., 60.), paper),
            None
        );
        assert!(
            (geometry
                .rotation_handle
                .distance(geometry.corners[0] + (geometry.corners[1] - geometry.corners[0]) * 0.5)
                - 28.)
                .abs()
                < 0.001
        );
        let mut resized = initial.clone();
        resize_rotated(&mut resized, &initial, Vec2::splat(10.), 0.1);
        // A drag along the local X axis widens the object and keeps its opposite corner fixed.
        assert!(resized.width_mm > initial.width_mm);
        assert!((resized.height_mm - initial.height_mm).abs() < 0.01);
        let resized_geometry = designer.object_geometry(&resized, paper);
        assert!(resized_geometry.corners[0].distance(geometry.corners[0]) < 0.01);
        designer.checkpoint();
        designer.doc.objects[0].rotate_to(123.45);
        designer.undo();
        assert_eq!(designer.doc.objects[0].rotation, 45.);
        designer.redo();
        assert_eq!(designer.doc.objects[0].rotation, 123.45);
    }
}

#[cfg(test)]
mod mouse_rotation_tests {
    use super::*;

    #[test]
    fn dragging_rotation_handle_changes_angle_and_undoes_as_one_edit() {
        let ctx = egui::Context::default();
        let mut designer = Designer::new();
        designer.doc = LabelDocument::blank(100., 100.);
        let id = designer.doc.add(
            Element::Shape {
                kind: ShapeKind::Rectangle,
                stroke_mm: 0.4,
                filled: true,
            },
            30.,
            40.,
            30.,
            10.,
        );
        designer.saved_document = designer.doc.clone();
        designer.selected = vec![id];
        designer.texture = Some(ctx.load_texture(
            "rotation-test-page",
            egui::ColorImage::from_rgba_unmultiplied([1, 1], &[255; 4]),
            Default::default(),
        ));
        let frame = |designer: &mut Designer, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| designer.canvas(ui));
                },
            );
            output.textures_delta.clear();
            output
        };
        frame(&mut designer, vec![]);
        let output = frame(&mut designer, vec![]);
        let handle = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Circle(circle) if circle.radius == 7. => Some(circle.center),
                _ => None,
            })
            .expect("rotation handle");
        let center = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Path(path) if path.closed && path.points.len() == 4 => {
                    Some(Pos2::new(
                        path.points.iter().map(|point| point.x).sum::<f32>() / 4.,
                        path.points.iter().map(|point| point.y).sum::<f32>() / 4.,
                    ))
                }
                _ => None,
            })
            .expect("selected object outline");
        let vector = handle - center;
        let (sin, cos) = 23.5_f32.to_radians().sin_cos();
        let target = center
            + Vec2::new(
                cos * vector.x - sin * vector.y,
                sin * vector.x + cos * vector.y,
            );
        let button = |pressed| egui::Event::PointerButton {
            pos: if pressed { handle } else { target },
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut designer,
            vec![egui::Event::PointerMoved(handle), button(true)],
        );
        frame(&mut designer, vec![egui::Event::PointerMoved(target)]);
        frame(&mut designer, vec![button(false)]);
        assert!(
            (designer.doc.objects[0].rotation - 23.5).abs() < 0.01,
            "angle: {}",
            designer.doc.objects[0].rotation
        );
        assert_eq!(designer.undo.len(), 1);
        assert!(designer.is_modified());
        designer.undo();
        assert_eq!(designer.doc.objects[0].rotation, 0.);
        assert!(!designer.is_modified());
        designer.redo();
        assert!((designer.doc.objects[0].rotation - 23.5).abs() < 0.01);
    }
}

#[cfg(test)]
mod window_close_tests {
    use super::*;

    #[test]
    fn hidden_window_close_is_cancelled_until_unsaved_changes_are_resolved() {
        let ctx = egui::Context::default();
        let mut designer = Designer::new();
        designer.doc.title.push_str(" changed");
        let mut input = egui::RawInput::default();
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.events.push(egui::ViewportEvent::Close);
        viewport.minimized = Some(true);
        let output = ctx.run_logic(&input, |ctx| designer.protect_window_close(ctx));
        let commands = output
            .viewport_commands
            .get(&egui::ViewportId::ROOT)
            .unwrap();
        assert!(commands.contains(&egui::ViewportCommand::CancelClose));
        assert!(commands.contains(&egui::ViewportCommand::Minimized(false)));
        assert!(matches!(
            designer.pending_action,
            Some(PendingAction::Close)
        ));
        let action = designer.pending_action.take().unwrap();
        designer.apply_action(action);
        let output = ctx.run_logic(&input, |ctx| designer.protect_window_close(ctx));
        assert!(
            !output
                .viewport_commands
                .get(&egui::ViewportId::ROOT)
                .is_some_and(|commands| commands.contains(&egui::ViewportCommand::CancelClose))
        );
    }
}

#[cfg(test)]
mod batch_window_tests {
    use super::*;

    #[test]
    fn batch_window_previews_the_selected_row_and_stays_open() {
        let ctx = egui::Context::default();
        let mut designer = Designer::new();
        designer.doc = LabelDocument::blank(50., 30.);
        designer.doc.add(
            Element::Text {
                text: "${Name}".into(),
                font_pt: 14.,
                bold: false,
                align: TextAlign::Left,
            },
            2.,
            2.,
            40.,
            10.,
        );
        designer.batch = Some(BatchState {
            file: "people.csv".into(),
            table: Some(crate::batch::parse_table(b"Name\nAda\nBo\n").unwrap()),
            error: None,
            row: 1,
            preview: None,
        });
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900., 700.));
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |_| designer.batch_window(&ctx, &Settings::default(), &[]),
            );
            output.textures_delta.clear();
        }
        let state = designer.batch.as_ref().expect("window stays open");
        assert_eq!(state.preview.as_ref().map(|(row, _)| *row), Some(1));
        assert!(designer.job.is_none(), "nothing prints without a printer");
    }
}

#[cfg(test)]
mod history_reopen_tests {
    use super::*;

    #[test]
    fn a_past_print_reopens_as_an_unsaved_copy() {
        let mut designer = Designer::new();
        let mut past = LabelDocument::blank(40., 20.);
        past.title = "Shelf tag".into();
        designer.request_copy(past.clone());
        assert_eq!(designer.doc, past);
        assert!(designer.path.is_none(), "a copy has no file yet");
        assert!(designer.is_modified(), "so closing it asks to save");
    }
}

#[cfg(test)]
mod smart_guide_tests {
    use super::*;

    fn square(doc: &mut LabelDocument, x: f32, y: f32) -> DesignObject {
        let id = doc.add(
            Element::Shape {
                kind: design::ShapeKind::Rectangle,
                stroke_mm: 0.4,
                filled: false,
            },
            x,
            y,
            10.,
            10.,
        );
        doc.objects.iter().find(|o| o.id == id).unwrap().clone()
    }

    #[test]
    fn a_move_snaps_to_a_nearby_edge_and_reports_the_guide() {
        let mut doc = LabelDocument::blank(100., 100.);
        let mover = square(&mut doc, 10., 10.);
        let _anchor = square(&mut doc, 50., 60.);
        // Moving right by 39.6 puts the mover's right edge (49.6) next to the other's left edge (50).
        let (x, y, guides) =
            smart_snap(&doc, std::slice::from_ref(&mover), Vec2::new(39.6, 0.), 1.);
        assert!((x.unwrap() - 40.).abs() < 1e-4, "{x:?}");
        assert_eq!(y, None);
        assert_eq!(guides, [(true, 50.)]);
        // Too far away: nothing snaps.
        assert_eq!(smart_snap(&doc, &[mover], Vec2::new(25., 0.), 1.).0, None);
    }

    #[test]
    fn moves_snap_to_the_label_center_and_ignore_the_moving_objects_themselves() {
        let mut doc = LabelDocument::blank(100., 100.);
        let mover = square(&mut doc, 10., 10.);
        // Centered horizontally would be x = 45; moving by 34.8 lands at 44.8.
        let (x, _, guides) = smart_snap(&doc, &[mover], Vec2::new(34.8, 0.), 1.);
        assert!((x.unwrap() - 35.).abs() < 1e-4);
        assert!(guides.contains(&(true, 50.)));
    }
}
