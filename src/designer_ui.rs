use crate::symbols::Symbol;
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
type LabelMaker = Arc<dyn Fn(usize) -> LabelDocument + Send + Sync>;
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

/// The number-range window: serials start, step, and how many labels to print.
#[derive(Clone, Debug)]
struct RangeState {
    start: u64,
    step: u64,
    count: u32,
}

/// The values typed into the QR content builder; they are not saved with the design.
#[derive(Clone, Debug, Default)]
struct QrForm {
    kind: usize,
    website: String,
    ssid: String,
    password: String,
    security: crate::qr_builder::WifiSecurity,
    hidden: bool,
    contact: crate::qr_builder::Contact,
    mail_to: String,
    mail_subject: String,
    mail_body: String,
    phone: String,
    error: Option<String>,
}

const QR_KINDS: [&str; 5] = ["Website", "Wi-Fi", "Contact card", "Email", "Phone number"];

/// Fill in the QR content from a form. Returns true when the content was replaced.
fn qr_builder_ui(ui: &mut egui::Ui, form: &mut QrForm, data: &mut String) -> bool {
    use crate::qr_builder::{QrContent, WifiSecurity};
    let mut applied = false;
    egui::CollapsingHeader::new(crate::i18n::tr("Build the content"))
        .id_salt("qr-builder")
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("qr-builder-kind")
                .selected_text(crate::i18n::tr(QR_KINDS[form.kind.min(4)]))
                .show_ui(ui, |ui| {
                    for (i, name) in QR_KINDS.iter().enumerate() {
                        ui.selectable_value(&mut form.kind, i, crate::i18n::tr(name));
                    }
                })
                .response
                .accessible_name(crate::i18n::tr("Kind of QR content"));
            let line = |ui: &mut egui::Ui, label: &str, text: &mut String| {
                ui.label(crate::i18n::tr(label));
                ui.add(egui::TextEdit::singleline(text))
                    .accessible_name(crate::i18n::tr(label));
            };
            match form.kind {
                0 => line(ui, "Web address", &mut form.website),
                1 => {
                    line(ui, "Network name", &mut form.ssid);
                    line(ui, "Password", &mut form.password);
                    ui.horizontal(|ui| {
                        for (security, name) in [
                            (WifiSecurity::Wpa, "WPA / WPA2"),
                            (WifiSecurity::Wep, "WEP"),
                            (WifiSecurity::Open, "No password"),
                        ] {
                            ui.selectable_value(
                                &mut form.security,
                                security,
                                crate::i18n::tr(name),
                            );
                        }
                    });
                    ui.checkbox(&mut form.hidden, crate::i18n::tr("Hidden network"));
                }
                2 => {
                    line(ui, "Name", &mut form.contact.name);
                    line(ui, "Organization", &mut form.contact.organization);
                    line(ui, "Phone", &mut form.contact.phone);
                    line(ui, "Email address", &mut form.contact.email);
                    line(ui, "Website", &mut form.contact.website);
                    line(ui, "Address", &mut form.contact.address);
                }
                3 => {
                    line(ui, "Email address", &mut form.mail_to);
                    line(ui, "Subject", &mut form.mail_subject);
                    line(ui, "Message", &mut form.mail_body);
                }
                _ => line(ui, "Phone", &mut form.phone),
            }
            if icon_action(
                ui,
                "Apply",
                "Use this content",
                "Replace the QR content with what you entered",
                true,
                false,
            )
            .clicked()
            {
                let content = match form.kind {
                    0 => QrContent::Website(form.website.clone()),
                    1 => QrContent::Wifi {
                        ssid: form.ssid.clone(),
                        password: form.password.clone(),
                        security: form.security,
                        hidden: form.hidden,
                    },
                    2 => QrContent::Contact(form.contact.clone()),
                    3 => QrContent::Email {
                        to: form.mail_to.clone(),
                        subject: form.mail_subject.clone(),
                        body: form.mail_body.clone(),
                    },
                    _ => QrContent::Phone(form.phone.clone()),
                };
                match content.payload() {
                    Ok(text) => {
                        *data = text;
                        form.error = None;
                        applied = true;
                    }
                    Err(error) => form.error = Some(format!("{error:#}")),
                }
            }
            if let Some(error) = &form.error {
                ui.colored_label(Color32::from_rgb(200, 60, 60), error);
            }
        });
    applied
}

/// The sheet window: several labels on one A4 or Letter page.
#[derive(Clone, Debug)]
struct SheetState {
    spec: crate::sheet::SheetSpec,
    count: u32,
    dpi: u32,
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
    /// Your own saved templates, shown beside the built-in ones.
    user_templates: Vec<crate::user_templates::UserTemplate>,
    /// Guide lines to draw during a drag: (is_vertical, position in mm).
    guides: Vec<(bool, f32)>,
    /// The border to shade as (left, top, right, bottom) in mm for this label: the printer's
    /// unprintable border and your safe margin, whichever is larger on each side.
    printable: Option<(f32, f32, f32, f32)>,
    /// Why no border is drawn, when it cannot be.
    printable_note: Option<&'static str>,
    /// A new safe margin chosen in the View menu; the app stores it with the printer.
    pub safe_margin_request: Option<f32>,
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
    range: Option<RangeState>,
    sheet: Option<SheetState>,
    /// The template gallery window: the search text while it is open.
    gallery: Option<String>,
    gallery_thumbs: std::collections::HashMap<String, egui::TextureHandle>,
    qr_form: QrForm,
    sscc_form: SsccForm,
    logos: Vec<crate::symbols::Logo>,
    addresses: Vec<crate::address_book::Entry>,
    fonts: Vec<String>,
    /// What is typed in the font chooser's search box.
    font_search: String,
    /// Where the open font chooser hangs from, or None when it is closed.
    font_picker: Option<Pos2>,
    /// Font name samples drawn for the open chooser (None: the font could not be drawn).
    font_samples: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    symbol_textures: Vec<(crate::symbols::Symbol, egui::TextureHandle)>,
    logo_textures: std::collections::HashMap<PathBuf, egui::TextureHandle>,
    /// Crash recovery: a design left over from an unexpected exit, offered at startup.
    recovered: Option<LabelDocument>,
    recovery_dir: Option<PathBuf>,
    recovery_written: Option<LabelDocument>,
    recovery_at: Option<std::time::Instant>,
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
            user_templates: crate::user_templates::list(),
            guides: Vec::new(),
            printable: None,
            printable_note: None,
            safe_margin_request: None,
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
            range: None,
            sheet: None,
            gallery: None,
            gallery_thumbs: Default::default(),
            qr_form: QrForm::default(),
            sscc_form: SsccForm::default(),
            logos: crate::symbols::logos_dir()
                .map(|dir| crate::symbols::list_logos_in(&dir))
                .unwrap_or_default(),
            addresses: crate::address_book::load(),
            fonts: crate::fonts::list(),
            font_search: String::new(),
            font_picker: None,
            font_samples: Default::default(),
            symbol_textures: Vec::new(),
            logo_textures: Default::default(),
            recovered: None,
            recovery_dir: None,
            recovery_written: None,
            recovery_at: None,
        }
    }
    /// Select the first text object and open the font chooser with a search typed in
    /// (documentation screenshots only).
    pub(crate) fn demo_font_chooser(&mut self, search: &str) {
        if let Some(id) = self
            .doc
            .objects
            .iter()
            .find(|o| matches!(o.element, Element::Text { .. }))
            .map(|o| o.id)
        {
            self.selected = vec![id];
        }
        self.font_search = search.into();
        self.font_picker = Some(Pos2::new(900., 200.));
    }
    /// Show a demo design with nothing selected (documentation screenshots only).
    pub(crate) fn demo_document(&mut self, doc: LabelDocument) {
        self.doc = doc;
        self.selected.clear();
        self.changed();
    }
    pub(crate) fn select_object(&mut self, id: u64) {
        if self.doc.objects.iter().any(|object| object.id == id) {
            self.selected = vec![id];
        }
    }
    /// Open one of the extra windows (documentation screenshots only).
    pub(crate) fn demo_window(&mut self, name: &str) {
        match name {
            "gallery" => self.gallery = Some(String::new()),
            "sheet" => {
                let (paper_w, paper_h) = self.doc.paper_size_mm();
                let spec = crate::sheet::SheetSpec::fitting(
                    crate::sheet::Page::A4,
                    paper_w,
                    paper_h,
                    5.,
                    0.,
                );
                self.sheet = Some(SheetState {
                    count: spec.cells().max(1) as u32,
                    spec,
                    dpi: 300,
                });
            }
            "range" => {
                self.range = Some(RangeState {
                    start: 100,
                    step: 5,
                    count: 20,
                })
            }
            _ => {}
        }
    }
    /// Select several objects at once (documentation screenshots only).
    pub(crate) fn select_objects(&mut self, ids: &[u64]) {
        self.selected = ids
            .iter()
            .copied()
            .filter(|id| self.doc.objects.iter().any(|o| o.id == *id))
            .collect();
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
    fn save_as_template(&mut self) {
        match crate::user_templates::save(&self.doc) {
            Ok((_, replaced)) => {
                self.status = if replaced {
                    "Template replaced"
                } else {
                    "Saved as a template"
                }
                .into();
                self.error = false;
            }
            Err(error) => {
                self.status = format!("Could not save the template: {error:#}");
                self.error = true;
            }
        }
        self.user_templates = crate::user_templates::list();
    }
    fn open_user_template(&mut self, path: &std::path::Path) {
        match LabelDocument::load(path) {
            Ok(mut doc) => {
                doc.title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(doc.title);
                self.request_action(PendingAction::Copy(Box::new(doc)));
            }
            Err(error) => {
                self.status = format!("Could not open the template: {error:#}");
                self.error = true;
            }
        }
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
    /// Turn on crash recovery (not in tests or documentation captures) and look for a copy
    /// left by an earlier session that ended unexpectedly.
    pub(crate) fn enable_recovery(&mut self) {
        if let Some(dir) = crate::recovery::recovery_dir() {
            self.enable_recovery_in(dir);
        }
    }
    fn enable_recovery_in(&mut self, dir: PathBuf) {
        self.recovered = crate::recovery::read_in(&dir);
        self.recovery_dir = Some(dir);
    }
    /// Keep a recovery copy of unsaved changes, a few seconds after the last edit.
    fn autosave(&mut self, ctx: &egui::Context) {
        let Some(dir) = self.recovery_dir.clone() else {
            return;
        };
        if self.recovered.is_some() {
            return;
        }
        if !self.is_modified() {
            if self.recovery_written.take().is_some() {
                crate::recovery::clear_in(&dir);
            }
            return;
        }
        if self.recovery_written.as_ref() == Some(&self.doc) {
            return;
        }
        let wait = std::time::Duration::from_secs(3);
        match self.recovery_at {
            Some(at) if at.elapsed() < wait => {
                ctx.request_repaint_after(wait - at.elapsed());
            }
            _ => {
                let _ = crate::recovery::write_in(&dir, &self.doc);
                self.recovery_written = Some(self.doc.clone());
                self.recovery_at = Some(std::time::Instant::now());
            }
        }
    }
    fn recovery_banner(&mut self, root: &mut egui::Ui) {
        let Some(doc) = self.recovered.clone() else {
            return;
        };
        egui::Panel::top("designer-recovery").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} {}",
                    crate::i18n::tr("Unsaved changes from your last session were found:"),
                    doc.title
                ));
                if icon_action(
                    ui,
                    "Refresh",
                    "Restore",
                    "Reopen the design from your last session as an unsaved copy",
                    true,
                    false,
                )
                .clicked()
                {
                    self.recovered = None;
                    self.replace_document(doc.clone(), None, false);
                    self.status = "Design restored · save to keep it".into();
                }
                if icon_action(
                    ui,
                    "Delete",
                    "Discard",
                    "Delete the recovered copy",
                    true,
                    false,
                )
                .clicked()
                {
                    self.recovered = None;
                    if let Some(dir) = &self.recovery_dir {
                        crate::recovery::clear_in(dir);
                    }
                }
            });
        });
    }
    pub(crate) fn request_close(&mut self) -> bool {
        if self.close_approved || !self.is_modified() {
            if let (Some(dir), None) = (&self.recovery_dir, &self.recovered) {
                crate::recovery::clear_in(dir);
            }
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
                let output = design_render::render_for_print(&doc, settings.dpi)?;
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
    /// A small preview of a design for the gallery.
    fn thumbnail(
        ctx: &egui::Context,
        key: &str,
        doc: &LabelDocument,
    ) -> Option<egui::TextureHandle> {
        let output = design_render::render_label(doc, 203).ok()?;
        let longest = output.image.width().max(output.image.height()).max(1) as f32;
        let scale = (140. / longest).min(1.);
        let (w, h) = (
            ((output.image.width() as f32 * scale) as u32).max(1),
            ((output.image.height() as f32 * scale) as u32).max(1),
        );
        let small =
            image::imageops::resize(&output.image, w, h, image::imageops::FilterType::Triangle);
        Some(ctx.load_texture(
            format!("gallery-{key}"),
            egui::ColorImage::from_rgb([w as usize, h as usize], small.as_raw()),
            egui::TextureOptions::LINEAR,
        ))
    }
    fn gallery_window(&mut self, ctx: &egui::Context) {
        let Some(mut query) = self.gallery.take() else {
            return;
        };
        let mut open = true;
        let mut open_builtin: Option<usize> = None;
        let mut open_user: Option<PathBuf> = None;
        let mut remove_user: Option<PathBuf> = None;
        // Preview a few more designs each frame so the window opens at once.
        let mut budget = 4;
        let mut missing = false;
        egui::Window::new(crate::i18n::tr("Template gallery"))
            .default_pos(window_start(ctx))
            .open(&mut open)
            .collapsible(false)
            .default_width(560.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Search"));
                    ui.add(egui::TextEdit::singleline(&mut query))
                        .accessible_name(crate::i18n::tr("Search templates"));
                });
                let needle = query.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(420.)
                    .show(ui, |ui| {
                        ui.label(RichText::new(crate::i18n::tr("Templates")).strong());
                        ui.horizontal_wrapped(|ui| {
                            for (i, t) in design::TEMPLATES.iter().enumerate() {
                                if !needle.is_empty() && !t.name.to_lowercase().contains(&needle) {
                                    continue;
                                }
                                let key = format!("builtin-{i}");
                                if !self.gallery_thumbs.contains_key(&key) && budget > 0 {
                                    budget -= 1;
                                    if let Some(texture) =
                                        Self::thumbnail(ctx, &key, &design::from_template(i))
                                    {
                                        self.gallery_thumbs.insert(key.clone(), texture);
                                    }
                                }
                                missing |= !self.gallery_thumbs.contains_key(&key);
                                let texture = self.gallery_thumbs.get(&key);
                                if gallery_card(ui, t.name, texture, t.description).clicked() {
                                    open_builtin = Some(i);
                                }
                            }
                        });
                        ui.add_space(6.);
                        ui.label(RichText::new(crate::i18n::tr("Your templates")).strong());
                        if self.user_templates.is_empty() {
                            ui.label(
                                RichText::new(crate::i18n::tr(
                                    "Nothing saved yet. Use File → Save as template… to add one.",
                                ))
                                .small()
                                .weak(),
                            );
                        }
                        ui.horizontal_wrapped(|ui| {
                            for t in self.user_templates.clone() {
                                if !needle.is_empty() && !t.name.to_lowercase().contains(&needle) {
                                    continue;
                                }
                                let key = format!("user-{}", t.path.display());
                                if !self.gallery_thumbs.contains_key(&key) && budget > 0 {
                                    budget -= 1;
                                    if let Ok(doc) = LabelDocument::load(&t.path)
                                        && let Some(texture) = Self::thumbnail(ctx, &key, &doc)
                                    {
                                        self.gallery_thumbs.insert(key.clone(), texture);
                                    }
                                }
                                missing |= !self.gallery_thumbs.contains_key(&key);
                                ui.vertical(|ui| {
                                    let texture = self.gallery_thumbs.get(&key);
                                    if gallery_card(ui, &t.name, texture, &t.name).clicked() {
                                        open_user = Some(t.path.clone());
                                    }
                                    if icon_action(
                                        ui,
                                        "Delete",
                                        "",
                                        "Remove this template",
                                        true,
                                        false,
                                    )
                                    .clicked()
                                    {
                                        remove_user = Some(t.path.clone());
                                    }
                                });
                            }
                        });
                    });
            });
        if missing {
            ctx.request_repaint();
        }
        if let Some(i) = open_builtin {
            self.request_template(i);
            open = false;
        }
        if let Some(path) = open_user {
            self.open_user_template(&path);
            open = false;
        }
        if let Some(path) = remove_user {
            match crate::user_templates::remove(&path) {
                Ok(()) => self.status = "Template removed".into(),
                Err(error) => {
                    self.status = format!("{error:#}");
                    self.error = true;
                }
            }
            self.user_templates = crate::user_templates::list();
            self.gallery_thumbs.retain(|k, _| !k.starts_with("user-"));
        }
        if open {
            self.gallery = Some(query);
        }
    }
    fn sheet_window(&mut self, ctx: &egui::Context, settings: &Settings, printers: &[Printer]) {
        use crate::sheet::{Page, SheetSpec};
        let Some(mut state) = self.sheet.take() else {
            return;
        };
        let (label_w, label_h) = self.doc.paper_size_mm();
        let mut open = true;
        let mut action: Option<bool> = None; // Some(true) = print, Some(false) = save PDF
        let printer = printers.iter().find(|p| p.name == settings.printer);
        egui::Window::new(crate::i18n::tr("Print on a sheet"))
            .default_pos(window_start(ctx))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(crate::i18n::tr("Place copies of this label on an A4 or Letter page, for an office printer or a sheet of sticky labels. ${serial} counts up from one label to the next."));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Paper"));
                    for page in [Page::A4, Page::Letter] {
                        ui.selectable_value(&mut state.spec.page, page, page.media());
                    }
                    ui.label(crate::i18n::tr("Resolution"));
                    egui::ComboBox::from_id_salt("sheet-dpi")
                        .selected_text(format!("{} DPI", state.dpi))
                        .show_ui(ui, |ui| {
                            for dpi in [203u32, 300, 600] {
                                ui.selectable_value(&mut state.dpi, dpi, format!("{dpi} DPI"));
                            }
                        })
                        .response
                        .accessible_name(crate::i18n::tr("Resolution"));
                });
                egui::Grid::new("sheet-grid").num_columns(2).show(ui, |ui| {
                    ui.label(crate::i18n::tr("Columns"));
                    ui.add(egui::DragValue::new(&mut state.spec.columns).range(1..=50))
                        .accessible_name(crate::i18n::tr("Columns"));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Rows"));
                    ui.add(egui::DragValue::new(&mut state.spec.rows).range(1..=50))
                        .accessible_name(crate::i18n::tr("Rows"));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Page margin"));
                    ui.add(
                        egui::DragValue::new(&mut state.spec.margin_mm)
                            .range(0.0..=50.0)
                            .speed(0.1)
                            .suffix(" mm"),
                    )
                    .accessible_name(crate::i18n::tr("Page margin in millimeters"));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Gap between labels"));
                    ui.add(
                        egui::DragValue::new(&mut state.spec.gap_mm)
                            .range(0.0..=50.0)
                            .speed(0.1)
                            .suffix(" mm"),
                    )
                    .accessible_name(crate::i18n::tr("Gap between labels in millimeters"));
                    ui.end_row();
                    ui.label(crate::i18n::tr("How many labels"));
                    ui.add(egui::DragValue::new(&mut state.count).range(1..=5000))
                        .accessible_name(crate::i18n::tr("Number of labels"));
                    ui.end_row();
                });
                if icon_action(
                    ui,
                    "Fit page",
                    "Fit as many as possible",
                    "Choose the most columns and rows of this label size that fit the page",
                    true,
                    false,
                )
                .clicked()
                {
                    let fit = SheetSpec::fitting(
                        state.spec.page,
                        label_w,
                        label_h,
                        state.spec.margin_mm,
                        state.spec.gap_mm,
                    );
                    state.spec.columns = fit.columns.max(1);
                    state.spec.rows = fit.rows.max(1);
                    state.count = state.spec.cells() as u32;
                }
                let positions = state.spec.positions(label_w, label_h);
                match &positions {
                    Ok(cells) => {
                        let sheets = crate::sheet::sheets_needed(state.count as usize, &state.spec);
                        ui.label(format!(
                            "{} {} · {} {}",
                            cells.len(),
                            crate::i18n::tr("labels per page"),
                            sheets,
                            crate::i18n::tr(if sheets == 1 { "page" } else { "pages" })
                        ));
                        let (pw, ph) = state.spec.page.size_mm();
                        let scale = 150. / ph;
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(pw * scale, ph * scale),
                            Sense::hover(),
                        );
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 2., Color32::WHITE);
                        painter.rect_stroke(
                            rect,
                            2.,
                            Stroke::new(1., Color32::GRAY),
                            StrokeKind::Inside,
                        );
                        for (i, (x, y)) in cells.iter().enumerate() {
                            let cell = Rect::from_min_size(
                                rect.min + Vec2::new(x * scale, y * scale),
                                Vec2::new(label_w * scale, label_h * scale),
                            );
                            let used = (i as u32) < state.count;
                            painter.rect_filled(
                                cell,
                                1.,
                                if used {
                                    Color32::from_rgb(120, 160, 230)
                                } else {
                                    Color32::from_gray(225)
                                },
                            );
                        }
                    }
                    Err(error) => {
                        ui.colored_label(Color32::from_rgb(200, 60, 60), format!("{error:#}"));
                    }
                }
                let can_print = printer.is_some_and(|p| p.media.iter().any(|m| m == state.spec.page.media()));
                if printer.is_some() && !can_print {
                    ui.label(
                        RichText::new(crate::i18n::tr(
                            "The selected printer does not list this paper. You can still save a PDF.",
                        ))
                        .weak(),
                    );
                } else if printer.is_none() {
                    ui.label(RichText::new(crate::i18n::tr("Choose a printer in Print Center first.")).weak());
                }
                ui.horizontal_wrapped(|ui| {
                    let ready = positions.is_ok() && self.job.is_none();
                    if icon_action(
                        ui,
                        "Export document",
                        "Save as PDF",
                        "Save the sheet as a PDF file",
                        ready,
                        false,
                    )
                    .clicked()
                    {
                        action = Some(false);
                    }
                    if icon_action(
                        ui,
                        "Print device",
                        "Print sheet",
                        "Send the sheet to the selected printer",
                        ready && can_print,
                        false,
                    )
                    .clicked()
                    {
                        action = Some(true);
                    }
                });
            });
        if let Some(print) = action {
            let doc = self.doc.clone();
            let spec = state.spec;
            let (count, dpi) = (state.count as usize, state.dpi);
            let mut settings = settings.clone();
            settings.media = spec.page.media().to_string();
            let target = if print {
                printer.cloned().map(|p| (p, None))
            } else {
                rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .set_file_name("labels-sheet.pdf")
                    .save_file()
                    .map(|path| (Printer::default(), Some(path)))
            };
            if let Some((printer, path)) = target {
                let first = doc.serial_number;
                self.start_job("Preparing sheet…", move || {
                    let outputs = (0..count)
                        .map(|i| {
                            let label = crate::batch::apply_range(&doc, first, 1, i);
                            design_render::render_for_print(&label, dpi)
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    let pages = crate::sheet::compose(&outputs, &spec)?;
                    match path {
                        Some(path) => {
                            for (n, page) in pages.iter().enumerate() {
                                let target = if pages.len() == 1 {
                                    path.clone()
                                } else {
                                    let stem = path
                                        .file_stem()
                                        .map(|s| s.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| "sheet".into());
                                    path.with_file_name(format!("{stem}-{}.pdf", n + 1))
                                };
                                export::save(page, &target)?;
                            }
                            Ok(format!("Saved {} sheet(s) as PDF", pages.len()))
                        }
                        None => {
                            for (n, page) in pages.iter().enumerate() {
                                printing::submit(page, &settings, &printer).map_err(|e| {
                                    anyhow::anyhow!(
                                        "Stopped at sheet {} of {}: {e:#}",
                                        n + 1,
                                        pages.len()
                                    )
                                })?;
                            }
                            Ok(format!(
                                "Submitted {} sheet(s). Use Job status to check the queue.",
                                pages.len()
                            ))
                        }
                    }
                });
                open = false;
            }
        }
        if open {
            self.sheet = Some(state);
        }
    }
    fn range_window(&mut self, ctx: &egui::Context, settings: &Settings, printers: &[Printer]) {
        let Some(mut state) = self.range.take() else {
            return;
        };
        let mut open = true;
        let mut start = false;
        let printer = printers.iter().find(|p| p.name == settings.printer);
        egui::Window::new(crate::i18n::tr("Print a number range"))
            .default_pos(window_start(ctx))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(crate::i18n::tr("Put ${serial} in text, barcode, or QR content. Use ${serial:4} for numbers padded with zeros, like 0007. Add a prefix by typing it before the number, for example INV-${serial:4}."));
                egui::Grid::new("range-grid").num_columns(2).show(ui, |ui| {
                    ui.label(crate::i18n::tr("First number"));
                    ui.add(egui::DragValue::new(&mut state.start).range(0..=999_999_999_999u64))
                        .accessible_name(crate::i18n::tr("First number"));
                    ui.end_row();
                    ui.label(crate::i18n::tr("Step"));
                    ui.add(egui::DragValue::new(&mut state.step).range(1..=1_000_000u64))
                        .accessible_name(crate::i18n::tr("Step between numbers"));
                    ui.end_row();
                    ui.label(crate::i18n::tr("How many labels"));
                    ui.add(egui::DragValue::new(&mut state.count).range(1..=crate::batch::MAX_RANGE))
                        .accessible_name(crate::i18n::tr("Number of labels"));
                    ui.end_row();
                });
                let (first, last) = crate::batch::range_ends(state.start, state.step, state.count);
                ui.label(
                    RichText::new(format!("{} {first} – {last}", crate::i18n::tr("Serials")))
                        .strong(),
                );
                let uses_serial = self.doc.objects.iter().any(|o| match &o.element {
                    Element::Text { text, .. } => text.contains("${serial"),
                    Element::Barcode { data, .. } | Element::Qr { data } => data.contains("${serial"),
                    _ => false,
                });
                if !uses_serial {
                    ui.colored_label(
                        Color32::from_rgb(200, 120, 20),
                        crate::i18n::tr("This design has no ${serial} yet, so every label would be the same."),
                    );
                }
                if printer.is_none() {
                    ui.label(RichText::new(crate::i18n::tr("Choose a printer in Print Center first.")).weak());
                }
                let label = format!("{} {}", crate::i18n::tr("Print labels:"), state.count);
                if icon_action(
                    ui,
                    "Print device",
                    &label,
                    "Every label is checked first; nothing prints if one has a problem.",
                    printer.is_some() && self.job.is_none(),
                    false,
                )
                .clicked()
                {
                    start = true;
                }
            });
        if start && let Some(printer) = printer.cloned() {
            let doc = self.doc.clone();
            let (first, step, total) = (state.start, state.step, state.count as usize);
            let make: LabelMaker =
                Arc::new(move |index| crate::batch::apply_range(&doc, first, step, index));
            self.run_labels(
                "Printing number range…",
                "label",
                total,
                make,
                settings,
                printer,
                "numbers",
            );
            open = false;
        }
        if open {
            self.range = Some(state);
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
            let total = table.rows.len();
            let make: LabelMaker =
                Arc::new(move |index| crate::batch::apply_row(&doc, &table, index));
            self.run_labels(
                "Printing batch…",
                "row",
                total,
                make,
                settings,
                printer,
                "batch of",
            );
            open = false;
        }
        if open {
            self.batch = Some(state);
        }
    }
    /// Print a run of labels: every label is rendered first, so nothing is sent if one is bad.
    #[allow(clippy::too_many_arguments)]
    fn run_labels(
        &mut self,
        status: &str,
        noun: &'static str,
        total: usize,
        make: LabelMaker,
        settings: &Settings,
        printer: Printer,
        history_word: &'static str,
    ) {
        let settings = settings.clone();
        let raw = self.raw_zpl;
        let title = self.doc.title.clone();
        let original = self.doc.clone();
        self.start_job(status, move || {
            let label_noun = if noun == "row" { "Row" } else { "Label" };
            crate::batch::preflight_with(label_noun, total, settings.dpi, |i| make(i)).map_err(
                |problems| anyhow::anyhow!("Nothing was printed. {}", problems.join(" · ")),
            )?;
            for index in 0..total {
                let label = make(index);
                if raw {
                    let bytes = zpl::encode_design(&label, settings.dpi)?;
                    printing::submit_raw_zpl(&bytes, &settings, &printer)
                } else {
                    let output = design_render::render_for_print(&label, settings.dpi)?;
                    printing::submit(&output, &settings, &printer)
                }
                .map_err(|e| {
                    anyhow::anyhow!("Stopped at {noun} {} of {total}: {e:#}", index + 1)
                })?;
            }
            let mut entry =
                crate::history::design_entry(&original, None, &settings.printer, settings.copies);
            entry.title = format!("{title} · {history_word} {total}");
            crate::history::record(entry);
            Ok(format!(
                "Submitted {total} labels. Use Job status to check the queue."
            ))
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
        self.autosave(&ctx);
        self.recovery_banner(root);
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
                        .button(crate::i18n::tr("Save as template…"))
                        .on_hover_text(crate::i18n::tr("Keep this design in your Templates menu, named after the label"))
                        .clicked()
                    {
                        self.save_as_template();
                        ui.close();
                    }
                    if ui
                        .button(crate::i18n::tr("Template gallery…"))
                        .on_hover_text(crate::i18n::tr("Browse the built-in templates and your own with previews"))
                        .clicked()
                    {
                        self.gallery = Some(String::new());
                        ui.close();
                    }
                    if ui
                        .button(crate::i18n::tr("Print on a sheet…"))
                        .on_hover_text(crate::i18n::tr("Place several labels on one A4 or Letter page"))
                        .clicked()
                    {
                        let (paper_w, paper_h) = self.doc.paper_size_mm();
                        let spec = crate::sheet::SheetSpec::fitting(
                            crate::sheet::Page::A4,
                            paper_w,
                            paper_h,
                            5.,
                            0.,
                        );
                        self.sheet = Some(SheetState {
                            count: spec.cells().max(1) as u32,
                            spec,
                            dpi: 300,
                        });
                        ui.close();
                    }
                    if ui
                        .button(crate::i18n::tr("Print a number range…"))
                        .on_hover_text(crate::i18n::tr("Print labels numbered by ${serial} with a start, step, and count"))
                        .clicked()
                    {
                        self.range = Some(RangeState {
                            start: self.doc.serial_number,
                            step: 1,
                            count: 10,
                        });
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
                        .on_hover_text(match self.printable_note {
                            Some(reason) => crate::i18n::tr(reason),
                            None => crate::i18n::tr("Shade the border your printer cannot print on"),
                        });
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("Safe margin"));
                        let mut margin = settings.safe_margin_mm;
                        if ui
                            .add(
                                egui::DragValue::new(&mut margin)
                                    .range(0.0..=crate::settings::MAX_SAFE_MARGIN_MM)
                                    .speed(0.1)
                                    .suffix(" mm"),
                            )
                            .on_hover_text(crate::i18n::tr("Keep content this far from the label's edge. It is shaded on the canvas and objects inside it are flagged, even when the printer's driver reports no unprintable border. It is remembered for each printer."))
                            .accessible_name(crate::i18n::tr("Safe margin in millimeters"))
                            .changed()
                        {
                            self.safe_margin_request = Some(margin);
                        }
                    });
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
        self.range_window(&ctx, settings, printers);
        self.sheet_window(&ctx, settings, printers);
        self.gallery_window(&ctx);
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
                    "{}{} · {:.1} × {:.1} mm{} · {}% · {} objects",
                    self.doc.title,
                    if self.is_modified() { " · Unsaved" } else { "" },
                    self.doc.width_mm,
                    self.doc.effective_height_mm(),
                    if self.doc.rotated_printing { " · rotated printing" } else { "" },
                    (self.zoom * 100.) as u32,
                    self.doc.objects.len()
                ));
                if self.error {
                    ui.colored_label(Color32::RED, &self.status);
                } else {
                    ui.label(&self.status);
                }
            });
            if let Some(margins) = self.printable {
                let outside = self.doc.objects_in_margins(margins);
                if !outside.is_empty() {
                    let names: Vec<String> = self
                        .doc
                        .objects
                        .iter()
                        .filter(|o| outside.contains(&o.id))
                        .map(|o| format!("{} #{}", o.label(), o.id))
                        .collect();
                    let color = if ui.visuals().dark_mode { Color32::from_rgb(245, 193, 99) } else { Color32::from_rgb(132, 83, 10) };
                    ui.colored_label(
                        color,
                        format!(
                            "{} {}",
                            if settings.safe_margin_mm > 0. {
                                crate::i18n::tr("These objects reach into the unprintable border or your safe margin and may be cut off:")
                            } else {
                                crate::i18n::tr("These objects reach into the printer's unprintable border and may be cut off:")
                            },
                            names.join(", ")
                        ),
                    );
                }
            }
            if self.show_printable
                && self.printable.is_none()
                && !settings.printer.is_empty()
                && let Some(reason) = self.printable_note
            {
                ui.label(RichText::new(crate::i18n::tr(reason)).small().weak());
            }
            if self.doc.rotated_printing {
                let (paper_w, paper_h) = self.doc.paper_size_mm();
                ui.label(
                    RichText::new(format!(
                        "{} {paper_w:.1} × {paper_h:.1} mm.",
                        crate::i18n::tr("Rotated printing: Print rotates it 90° to fit")
                    ))
                    .strong(),
                );
            }
            if let Some(printer) = printers.iter().find(|p| p.name == settings.printer) {
                if self.raw_zpl {
                    ui.label(crate::i18n::tr("Raw ZPL: verify printer compatibility and loaded label size."))
                        .on_hover_text(crate::help::explanation("ZPL"));
                } else if let Some(warning) = printer.media_mismatch(&settings.media, self.doc.paper_size_mm().0, self.doc.paper_size_mm().1) {
                    let color = if ui.visuals().dark_mode { Color32::from_rgb(245, 193, 99) }
                        else { Color32::from_rgb(132, 83, 10) };
                    ui.colored_label(color, warning);
                    ui.label(RichText::new(crate::i18n::tr("If the loaded paper matches the printer media, use the button below. To keep the current label size, choose matching media in Print Center instead.")).small().weak());
                    if let Some(&(width, height)) = printer.media_sizes.get(&settings.media)
                        && !self.doc.rotated_printing
                        && (width - self.doc.effective_height_mm()).abs() < 0.5
                        && (height - self.doc.width_mm).abs() < 0.5
                        && icon_action(ui, "Rotate stock", "Use rotated printing",
                            "This label is the printer stock turned on its side. Keep designing it this way and rotate it 90° when it prints. Ctrl+Z undoes this change.", true, false).clicked() {
                        self.checkpoint();
                        self.doc.rotated_printing = true;
                        self.doc.auto_length = false;
                        self.changed();
                        self.status = "Rotated printing: the label is rotated 90° when it prints".into();
                    }
                    if let Some(&(width, height)) = printer.media_sizes.get(&settings.media)
                        && icon_action(ui, "Rotate stock", "Match label to printer media",
                            "Set the canvas dimensions to the selected printer stock. Object sizes and positions are preserved; Ctrl+Z undoes this change.", true, false).clicked() {
                        self.checkpoint();
                        // With rotated printing the canvas stays turned on its side.
                        let (width, height) = if self.doc.rotated_printing { (height, width) } else { (width, height) };
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
        let printer = printers.iter().find(|p| p.name == settings.printer);
        let (paper_w, paper_h) = self.doc.paper_size_mm();
        let driver = printer
            .filter(|p| {
                p.media_sizes
                    .get(&settings.media)
                    .is_some_and(|&(w, h)| (w - paper_w).abs() < 0.5 && (h - paper_h).abs() < 0.5)
            })
            .and_then(|p| p.printable_margins_mm(&settings.media))
            .map(|border| self.doc.border_on_canvas(border));
        self.printable = effective_border(driver, settings.safe_margin_mm);
        self.printable_note = if self.printable.is_none() {
            Some(border_note(printer, &settings.media, (paper_w, paper_h)))
        } else {
            None
        };
        egui::CentralPanel::default().show(root, |ui| self.canvas(ui));
        self.shortcut(&ctx);
        self.start_render(settings.dpi);
    }
    /// The address book menu: pick an address to add it as a text block.
    fn address_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(crate::i18n::tr("Address book"));
        if self.addresses.is_empty() {
            ui.label(
                RichText::new(crate::i18n::tr(
                    "Nothing saved yet. Select a text object and choose Save to address book.",
                ))
                .small()
                .weak(),
            );
        }
        let mut add: Option<String> = None;
        let mut remove: Option<String> = None;
        for entry in &self.addresses {
            ui.horizontal(|ui| {
                if ui
                    .button(&entry.name)
                    .on_hover_text(&entry.address)
                    .clicked()
                {
                    add = Some(entry.address.clone());
                }
                if icon_action(ui, "Delete", "", "Remove this address", true, false).clicked() {
                    remove = Some(entry.name.clone());
                }
            });
        }
        if let Some(address) = add {
            let lines = address.lines().count().max(1) as f32;
            self.add(
                Element::Text {
                    text: address,
                    font_pt: 12.,
                    bold: false,
                    align: TextAlign::Left,
                },
                60.,
                (lines * 6.).clamp(8., 80.),
            );
            ui.close();
        }
        if let Some(name) = remove {
            match crate::address_book::remove(&name) {
                Ok(list) => {
                    self.addresses = list;
                    self.status = "Address removed".into();
                }
                Err(error) => {
                    self.status = format!("{error:#}");
                    self.error = true;
                }
            }
        }
    }
    fn save_address(&mut self, text: &str) {
        let name = text.lines().next().unwrap_or("").to_string();
        match crate::address_book::add(&name, text) {
            Ok(list) => {
                self.addresses = list;
                self.status = format!("Saved {name} to the address book");
                self.error = false;
            }
            Err(error) => {
                self.status = format!("Could not save the address: {error:#}");
                self.error = true;
            }
        }
    }
    /// The symbols and saved logos menu: pick one to add it to the label.
    fn symbols_menu(&mut self, ui: &mut egui::Ui) {
        use crate::symbols;
        if self.symbol_textures.is_empty() {
            for symbol in symbols::ALL {
                let image = symbols::render(symbol, 96);
                let texture = ui.ctx().load_texture(
                    format!("symbol-{}", symbol.name()),
                    on_white(&image),
                    egui::TextureOptions::LINEAR,
                );
                self.symbol_textures.push((symbol, texture));
            }
        }
        ui.label(crate::i18n::tr("Symbols"));
        let mut chosen: Option<Symbol> = None;
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(260.);
            for (symbol, texture) in &self.symbol_textures {
                let name = crate::i18n::tr(symbol.name());
                let response = ui.add(egui::Button::image(
                    egui::Image::new(texture).fit_to_exact_size(Vec2::splat(44.)),
                ));
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
                if response.on_hover_text(name).clicked() {
                    chosen = Some(*symbol);
                }
            }
        });
        if let Some(symbol) = chosen {
            let image = symbols::render(symbol, 400);
            if let Ok(png) = symbols::png_bytes(&image) {
                self.add(
                    Element::Image {
                        data_base64: base64::engine::general_purpose::STANDARD.encode(png),
                    },
                    25.,
                    25.,
                );
            }
            ui.close();
        }
        if !self.logos.is_empty() {
            ui.separator();
            ui.label(crate::i18n::tr("Your logos"));
        }
        let mut add_logo: Option<PathBuf> = None;
        let mut remove_logo: Option<PathBuf> = None;
        for logo in self.logos.clone() {
            let texture = self
                .logo_textures
                .entry(logo.path.clone())
                .or_insert_with(|| {
                    let color = image::open(&logo.path)
                        .map(|img| image::imageops::thumbnail(&img.to_rgba8(), 64, 64))
                        .map(|thumb| on_white(&thumb))
                        .unwrap_or_else(|_| egui::ColorImage::example());
                    ui.ctx()
                        .load_texture("logo-thumb", color, egui::TextureOptions::LINEAR)
                })
                .clone();
            ui.horizontal(|ui| {
                let response = ui.add(egui::Button::image_and_text(
                    egui::Image::new(&texture).fit_to_exact_size(Vec2::splat(28.)),
                    &logo.name,
                ));
                if response.clicked() {
                    add_logo = Some(logo.path.clone());
                }
                if icon_action(ui, "Delete", "", "Remove this logo", true, false).clicked() {
                    remove_logo = Some(logo.path.clone());
                }
            });
        }
        if let Some(path) = add_logo {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let (w, h) = image::load_from_memory(&bytes)
                        .map(|im| (im.width() as f32, im.height() as f32))
                        .unwrap_or((1., 1.));
                    let scale = 30. / w.max(h).max(1.);
                    self.add(
                        Element::Image {
                            data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                        },
                        (w * scale).max(3.),
                        (h * scale).max(3.),
                    );
                    ui.close();
                }
                Err(error) => {
                    self.status = format!("Could not read the logo: {error}");
                    self.error = true;
                }
            }
        }
        if let Some(path) = remove_logo
            && let Some(dir) = symbols::logos_dir()
        {
            match symbols::remove_logo_in(&dir, &path) {
                Ok(()) => self.status = "Logo removed".into(),
                Err(error) => {
                    self.status = format!("{error:#}");
                    self.error = true;
                }
            }
            self.refresh_logos();
        }
    }
    fn refresh_logos(&mut self) {
        self.logos = crate::symbols::logos_dir()
            .map(|dir| crate::symbols::list_logos_in(&dir))
            .unwrap_or_default();
        self.logo_textures.clear();
    }
    /// Put a picture in the middle of a QR code. The code switches to the highest error
    /// correction, and the picture starts as plain black and white, which scans best.
    fn set_qr_logo(&mut self, id: u64, path: &std::path::Path) {
        let result = std::fs::read(path)
            .map_err(anyhow::Error::from)
            .and_then(|bytes| crate::design_render::prepare_qr_logo(&bytes));
        match result {
            Ok(logo) => {
                if let Some(o) = self.doc.objects.iter_mut().find(|o| o.id == id) {
                    o.options.qr_logo = logo;
                    o.options.qr_level = design::QrLevel::High;
                    if o.options.image_mode == design::ImageMode::Original {
                        o.options.image_mode = design::ImageMode::Threshold;
                    }
                    self.status =
                        "Logo added to the QR code · the code now uses High error correction"
                            .into();
                    self.error = false;
                }
            }
            Err(error) => {
                self.status = format!("Could not use that logo: {error:#}");
                self.error = true;
            }
        }
    }
    fn save_logo(&mut self, data_base64: &str) {
        let result = base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .map_err(anyhow::Error::from)
            .and_then(|bytes| {
                let dir = crate::symbols::logos_dir()
                    .ok_or_else(|| anyhow::anyhow!("No settings folder is available"))?;
                let name = format!("Logo {}", crate::symbols::list_logos_in(&dir).len() + 1);
                crate::symbols::save_logo_in(&dir, &name, &bytes)
            });
        match result {
            Ok(_) => {
                self.status = "Saved to your logos · find it under Symbols and logos".into();
                self.error = false;
            }
            Err(error) => {
                self.status = format!("Could not save the logo: {error:#}");
                self.error = true;
            }
        }
        self.refresh_logos();
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
            let book = element_tool(ui, "Address book");
            egui::Popup::menu(&book).show(|ui| self.address_menu(ui));
            let symbols = element_tool(ui, "Symbols and logos");
            egui::Popup::menu(&symbols).show(|ui| self.symbols_menu(ui));
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
                if !self.user_templates.is_empty() {
                    ui.separator();
                    ui.label(crate::i18n::tr("Your templates"));
                }
                let mut open = None;
                let mut remove = None;
                for t in &self.user_templates {
                    ui.horizontal(|ui| {
                        if ui.button(&t.name).clicked() {
                            open = Some(t.path.clone());
                        }
                        if icon_action(ui, "Delete", "", "Remove this template", true, false)
                            .clicked()
                        {
                            remove = Some(t.path.clone());
                        }
                    });
                }
                if let Some(path) = open {
                    self.open_user_template(&path);
                    ui.close();
                }
                if let Some(path) = remove {
                    match crate::user_templates::remove(&path) {
                        Ok(()) => self.status = "Template removed".into(),
                        Err(error) => {
                            self.status = format!("{error:#}");
                            self.error = true;
                        }
                    }
                    self.user_templates = crate::user_templates::list();
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
                let mut rotated = self.doc.rotated_printing;
                ui.checkbox(&mut rotated, crate::i18n::tr("Rotate 90° when printing"))
                    .on_hover_text(crate::i18n::tr("For wide content: design the label turned on its side so the text reads straight, and it is rotated 90° when it prints, to fit the label stock as loaded."));
                if rotated != self.doc.rotated_printing {
                    self.doc.set_rotated_printing(rotated);
                    self.zoom = 1.;
                    self.pan = Vec2::ZERO;
                }
                if self.doc.rotated_printing {
                    let (paper_w, paper_h) = self.doc.paper_size_mm();
                    ui.label(
                        RichText::new(format!(
                            "{} {paper_w:.1} × {paper_h:.1} mm.",
                            crate::i18n::tr(if self.doc.rotate_counterclockwise {
                                "Rotated 90° counterclockwise to print on"
                            } else {
                                "Rotated 90° clockwise to print on"
                            })
                        ))
                        .small()
                        .weak(),
                    );
                    ui.checkbox(&mut self.doc.rotate_counterclockwise, crate::i18n::tr("Rotate the other way"))
                        .on_hover_text(crate::i18n::tr("Which way the design faces on the printed label. Check “As printed” beside the canvas."));
                }
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::tr("Serial variable"));
                    ui.add(egui::DragValue::new(&mut self.doc.serial_number))
                        .accessible_name(crate::i18n::tr("Serial variable"));
                });
                ui.add_enabled(
                    !self.doc.rotated_printing,
                    egui::Checkbox::new(&mut self.doc.auto_length, crate::i18n::tr("Auto-length (continuous roll)")),
                )
                .on_hover_text(crate::i18n::tr(
                    "Print only as long as the content needs, plus a bottom margin. Use it with continuous label rolls.",
                ))
                .on_disabled_hover_text(crate::i18n::tr("Not available with rotated printing."));
                if self.doc.auto_length {
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("Bottom margin"));
                        ui.add(
                            egui::DragValue::new(&mut self.doc.auto_margin_mm)
                                .range(0.0..=50.0)
                                .speed(0.1)
                                .suffix(" mm"),
                        )
                        .accessible_name(crate::i18n::tr("Bottom margin in millimeters"));
                    });
                    ui.label(
                        RichText::new(format!(
                            "{}: {:.1} mm",
                            crate::i18n::tr("Printed length"),
                            self.doc.effective_height_mm()
                        ))
                        .small()
                        .weak(),
                    );
                }
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
                for (icon, alignment) in [
                    ("Align left", design::Alignment::Left),
                    ("Align center", design::Alignment::CenterHorizontally),
                    ("Align right", design::Alignment::Right),
                    ("Align top", design::Alignment::Top),
                    ("Align middle", design::Alignment::CenterVertically),
                    ("Align bottom", design::Alignment::Bottom),
                ] {
                    if icon_action(ui, icon, "", icon, unlocked > 1, false).clicked() {
                        self.arrange(alignment, design::AlignTo::Selection);
                    }
                }
            });
            ui.label(RichText::new(crate::i18n::tr("Distribute evenly")).strong());
            ui.horizontal_wrapped(|ui| {
                for (icon, axis) in [
                    ("Distribute horizontally", design::Axis::Horizontal),
                    ("Distribute vertically", design::Axis::Vertical),
                ] {
                    if icon_action(ui, icon, "", icon, unlocked > 2, false).clicked() {
                        self.distribute(axis);
                    }
                }
            });
            ui.label(RichText::new(crate::i18n::tr("Center on label")).strong());
            ui.horizontal_wrapped(|ui| {
                for (icon, alignment) in [
                    (
                        "Center horizontally on label",
                        design::Alignment::CenterHorizontally,
                    ),
                    (
                        "Center vertically on label",
                        design::Alignment::CenterVertically,
                    ),
                ] {
                    if icon_action(ui, icon, "", icon, unlocked > 0, false).clicked() {
                        self.arrange_group_on_label(alignment);
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if icon_action(
                    ui,
                    "Lock",
                    "Lock all",
                    "Lock all selected objects",
                    true,
                    false,
                )
                .clicked()
                {
                    self.set_lock(true);
                }
                if icon_action(
                    ui,
                    "Unlock",
                    "Unlock all",
                    "Unlock all selected objects",
                    true,
                    false,
                )
                .clicked()
                {
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
        let mut save_logo: Option<String> = None;
        let mut add_font = false;
        let mut pick_qr_logo = false;
        let mut qr_logo_path: Option<std::path::PathBuf> = None;
        let mut save_address: Option<String> = None;
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
                if icon_action(
                    ui,
                    "Rotate 90",
                    "Rotate 90°",
                    "Turn the object a quarter turn clockwise",
                    true,
                    false,
                )
                .clicked()
                {
                    let angle = o.rotation + 90.;
                    o.rotate_to(angle);
                }
            });
            ui.horizontal_wrapped(|ui| {
                if icon_action(
                    ui,
                    "Flip horizontally",
                    "",
                    "Flip horizontally",
                    true,
                    o.flip_x,
                )
                .clicked()
                {
                    o.flip_x = !o.flip_x;
                }
                if icon_action(ui, "Flip vertically", "", "Flip vertically", true, o.flip_y)
                    .clicked()
                {
                    o.flip_y = !o.flip_y;
                }
                center.0 = icon_action(
                    ui,
                    "Center horizontally on label",
                    "",
                    "Center horizontally on label",
                    true,
                    false,
                )
                .clicked();
                center.1 = icon_action(
                    ui,
                    "Center vertically on label",
                    "",
                    "Center vertically on label",
                    true,
                    false,
                )
                .clicked();
            });
        });
        if icon_action(
            ui,
            if o.locked { "Lock" } else { "Unlock" },
            "Lock position and size",
            "A locked object cannot be moved, resized, rotated, or deleted from the canvas",
            true,
            o.locked,
        )
        .clicked()
        {
            o.locked = !o.locked;
        }
        match &mut o.element {
            Element::Text {
                text,
                font_pt,
                bold,
                align,
            } => {
                if icon_action(
                    ui,
                    "Address book",
                    "Save to address book",
                    "Keep this text, named after its first line, in your address book",
                    true,
                    false,
                )
                .clicked()
                {
                    save_address = Some(text.clone());
                }
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
                    ui.add_enabled(
                        crate::fonts::has_bold(&o.options.font),
                        egui::Checkbox::new(bold, crate::i18n::tr("Bold")),
                    )
                    .on_disabled_hover_text(crate::i18n::tr(
                        "This font has no bold style. Choose a bold font instead.",
                    ));
                    ui.checkbox(&mut o.options.italic, crate::i18n::tr("Italic"));
                    ui.checkbox(&mut o.options.inverse, crate::i18n::tr("Inverse"))
                        .on_hover_text(crate::i18n::tr("White letters on a black box"));
                });
                ui.horizontal_wrapped(|ui| {
                    let current = if o.options.font.is_empty() {
                        crate::i18n::tr("Default (DejaVu Sans)").to_string()
                    } else {
                        crate::i18n::tr(&crate::fonts::display_name(&o.options.font)).to_string()
                    };
                    let button = ui
                        .add(
                            egui::Button::new(format!("{current}     "))
                                .min_size(Vec2::new(200., 0.)),
                        )
                        .on_hover_text(crate::i18n::tr("Choose a font"))
                        .accessible_name(crate::i18n::tr("Font"));
                    // A small downward arrow, drawn because the font has no arrow character.
                    let tip = Pos2::new(button.rect.right() - 14., button.rect.center().y + 3.);
                    ui.painter().add(egui::Shape::convex_polygon(
                        vec![tip, tip + Vec2::new(-5., -6.), tip + Vec2::new(5., -6.)],
                        ui.style().interact(&button).fg_stroke.color,
                        Stroke::NONE,
                    ));
                    if button.clicked() {
                        self.font_picker = if self.font_picker.is_some() {
                            None
                        } else {
                            self.font_search.clear();
                            Some(button.rect.left_bottom() + Vec2::new(0., 4.))
                        };
                    }
                    if let Some(anchor) = self.font_picker
                        && !font_chooser(
                            ui.ctx(),
                            anchor,
                            &mut o.options.font,
                            &mut self.font_search,
                            &self.fonts,
                            &mut self.font_samples,
                        )
                    {
                        self.font_picker = None;
                        self.font_samples.clear();
                    }
                    if icon_action(
                        ui,
                        "Add text",
                        "",
                        "Add a font from a .ttf or .otf file",
                        true,
                        false,
                    )
                    .clicked()
                    {
                        add_font = true;
                    }
                });
                if !o.options.font.is_empty()
                    && crate::fonts::lookup(&o.options.font, false).is_none()
                {
                    ui.colored_label(
                        Color32::from_rgb(200, 120, 20),
                        crate::i18n::tr(
                            "This font is not installed on this computer, so the built-in font is used.",
                        ),
                    );
                } else if !o.options.font.is_empty()
                    && !o.options.font.starts_with(crate::fonts::BUILTIN_PREFIX)
                {
                    ui.label(
                        RichText::new(crate::i18n::tr(
                            "Another computer needs this font installed to print the label the same way.",
                        ))
                        .small()
                        .weak(),
                    );
                }
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
                            BarcodeKind::Gs1128,
                            BarcodeKind::Gs1DataMatrix,
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
                if matches!(kind, BarcodeKind::Gs1128 | BarcodeKind::Gs1DataMatrix) {
                    gs1_helper(ui, *kind, data, &mut self.sscc_form);
                }
            }
            Element::Qr { data } => {
                ui.label(crate::i18n::tr("QR content"))
                    .on_hover_text(crate::help::explanation("QR"));
                ui.add(egui::TextEdit::multiline(data).desired_rows(4))
                    .accessible_name(crate::i18n::tr("QR code content"));
                qr_builder_ui(ui, &mut self.qr_form, data);
                ui.add_enabled_ui(o.options.qr_logo.is_empty(), |ui| {
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
                });
                ui.separator();
                ui.label(RichText::new(crate::i18n::tr("Logo in the center")).strong())
                    .on_hover_text(crate::i18n::tr(
                        "A logo hides part of the code. ThermalPrint keeps it small enough, and uses the highest error correction, so phones can still scan it.",
                    ));
                let has_logo = !o.options.qr_logo.is_empty();
                ui.horizontal_wrapped(|ui| {
                    let label = if has_logo {
                        "Change logo…"
                    } else {
                        "Add logo…"
                    };
                    if ui.button(crate::i18n::tr(label)).clicked() {
                        pick_qr_logo = true;
                    }
                    if has_logo && ui.button(crate::i18n::tr("Remove logo")).clicked() {
                        o.options.qr_logo.clear();
                    }
                });
                if !self.logos.is_empty() {
                    egui::ComboBox::from_id_salt("qr-saved-logo")
                        .selected_text(crate::i18n::tr("Choose a saved logo"))
                        .show_ui(ui, |ui| {
                            for logo in &self.logos {
                                if ui.selectable_label(false, &logo.name).clicked() {
                                    qr_logo_path = Some(logo.path.clone());
                                }
                            }
                        });
                }
                if !o.options.qr_logo.is_empty() {
                    // A logo always uses the highest correction, whatever was chosen above.
                    o.options.qr_level = design::QrLevel::High;
                    let modules = crate::design_render::qr_modules(data, &o.options).unwrap_or(37);
                    let max = design::qr_logo_max_percent(modules);
                    let mut percent = o
                        .options
                        .qr_logo_percent
                        .clamp(design::QR_LOGO_MIN_PERCENT, max);
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::tr("Logo size"));
                        ui.add(
                            egui::Slider::new(&mut percent, design::QR_LOGO_MIN_PERCENT..=max)
                                .suffix(" %"),
                        )
                        .on_hover_text(crate::i18n::tr(
                            "The share of the code's width that is cleared for the logo",
                        ));
                    });
                    o.options.qr_logo_percent = percent;
                    ui.label(
                        RichText::new(format!(
                            "{} {max} %. {}",
                            crate::i18n::tr("Largest size that scans reliably:"),
                            crate::i18n::tr(
                                "Error correction is set to High while a logo is used."
                            )
                        ))
                        .small()
                        .weak(),
                    );
                    let wanted_mm = (modules + 8) as f32 * QR_LOGO_MIN_MODULE_MM;
                    if o.width_mm.min(o.height_mm) < wanted_mm {
                        ui.label(
                            RichText::new(format!(
                                "{} {:.0} mm",
                                crate::i18n::tr(
                                    "The code is small for a logo. Make it at least this wide:"
                                ),
                                wanted_mm.ceil()
                            ))
                            .small()
                            .color(Color32::from_rgb(200, 65, 55)),
                        );
                    }
                    egui::ComboBox::from_id_salt("qr-logo-mode")
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
                    if o.options.image_mode == design::ImageMode::Threshold {
                        ui.horizontal(|ui| {
                            ui.label(crate::i18n::tr("Cut-off"));
                            ui.add(egui::Slider::new(&mut o.options.threshold, 1..=254))
                                .on_hover_text(crate::i18n::tr(
                                    "Pixels darker than this print black",
                                ));
                        });
                    }
                    ui.checkbox(
                        &mut o.options.invert,
                        crate::i18n::tr("Invert black and white"),
                    );
                }
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
            Element::Image { data_base64 } => {
                ui.label(crate::i18n::tr(
                    "Embedded image. Resize with W/H or the canvas handle.",
                ));
                if icon_action(
                    ui,
                    "Save document",
                    "Save as logo",
                    "Keep this image under Symbols and logos so you can add it to any label",
                    true,
                    false,
                )
                .clicked()
                {
                    save_logo = Some(data_base64.clone());
                }
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
                if icon_action(
                    ui,
                    "Refresh",
                    "Reset image adjustments",
                    "Return the image mode, contrast, and cut-off to their defaults",
                    true,
                    false,
                )
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
        if add_font
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Font", &["ttf", "otf"])
                .pick_file()
        {
            match crate::fonts::install(&path) {
                Ok(name) => {
                    self.status = format!("Font {name} added");
                    self.error = false;
                    self.fonts = crate::fonts::list();
                    if let Some(o) = self.doc.objects.iter_mut().find(|o| o.id == id) {
                        o.options.font = name;
                    }
                }
                Err(error) => {
                    self.status = format!("{error:#}");
                    self.error = true;
                }
            }
        }
        if pick_qr_logo
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "webp", "bmp"])
                .pick_file()
        {
            qr_logo_path = Some(path);
        }
        if let Some(path) = qr_logo_path {
            self.set_qr_logo(id, &path);
        }
        if let Some(text) = save_address {
            self.save_address(&text);
        }
        if let Some(data) = save_logo {
            self.save_logo(&data);
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
        if self.doc.rotated_printing {
            sideways_banner(ui, &self.doc);
        }
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
        let panel = ui.max_rect();
        let mut available = (ui.available_size() - Vec2::splat(64.)).max(Vec2::splat(1.));
        if self.doc.rotated_printing {
            // Leave room on the right for the "As printed" card, so it never covers the label.
            available.x = (available.x - 2. * PRINTED_PREVIEW_ROOM).max(available.x * 0.5);
        }
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
                    let amber = Color32::from_rgba_unmultiplied(235, 150, 20, 95);
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
                        Stroke::new(1.5, Color32::from_rgb(210, 120, 10)),
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
        if self.doc.rotated_printing {
            printed_preview(ui, &tex, panel, &self.doc);
        }
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

/// A picture over white, so black shapes with a see-through background stay visible in dark mode.
fn on_white(image: &image::RgbaImage) -> egui::ColorImage {
    let mut rgb = Vec::with_capacity(image.width() as usize * image.height() as usize * 3);
    for p in image.pixels() {
        let a = p.0[3] as u16;
        for &channel in &p.0[..3] {
            rgb.push(((channel as u16 * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    egui::ColorImage::from_rgb([image.width() as usize, image.height() as usize], &rgb)
}

/// Where a new window opens: near the middle top of the application window.
fn window_start(ctx: &egui::Context) -> Pos2 {
    let area = ctx.content_rect();
    Pos2::new(
        (area.center().x - 200.).max(area.left() + 20.),
        area.top() + 70.,
    )
}

/// One card of the template gallery: the preview above the name, as a single button.
fn gallery_card(
    ui: &mut egui::Ui,
    name: &str,
    texture: Option<&egui::TextureHandle>,
    hover: &str,
) -> egui::Response {
    let shown = crate::i18n::tr(name);
    let response = ui
        .add(
            egui::Button::new("")
                .min_size(Vec2::new(150., 176.))
                .corner_radius(8.),
        )
        .on_hover_text(hover);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, shown));
    let painter = ui.painter_at(response.rect);
    let preview = Rect::from_min_size(
        response.rect.min + Vec2::new(5., 5.),
        Vec2::new(response.rect.width() - 10., 140.),
    );
    painter.rect_filled(preview, 4., Color32::WHITE);
    if let Some(texture) = texture {
        let size = texture.size_vec2();
        let scale = (preview.width() / size.x)
            .min(preview.height() / size.y)
            .min(1.);
        let image_rect = Rect::from_center_size(preview.center(), size * scale);
        painter.image(
            texture.id(),
            image_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
    }
    painter.text(
        Pos2::new(response.rect.center().x, response.rect.bottom() - 15.),
        egui::Align2::CENTER_CENTER,
        shown,
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    response
}

/// The icon for a template. It is chosen from the English name, which stays the template's identity.
pub(crate) fn template_icon(name: &str) -> &'static str {
    match name {
        "Blank custom" => "Blank custom",
        "Shipping 4 × 6" => "Shipping",
        "Return address" | "Name label" => "Add address",
        "Product barcode" | "Inventory" | "Asset tracking" => "Add Code 128",
        "QR code" => "Add QR code",
        "Shelf" => "Choose a label template",
        "Price tag" => "Price tag",
        "Fragile" => "Fragile",
        _ => "Add Rectangle",
    }
}

pub(crate) fn template_button(ui: &mut egui::Ui, name: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(218., 34.), Sense::click());
    let shown = crate::i18n::tr(name);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), shown));
    let visuals = ui.style().interact(&response);
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 5., visuals.bg_fill);
    }
    let icon = template_icon(name);
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

/// The border to shade and to check objects against: on each side, the larger of what the
/// printer's driver says it cannot print and the safe margin you chose. `None` when neither
/// gives a border.
fn effective_border(
    driver: Option<(f32, f32, f32, f32)>,
    safe_margin_mm: f32,
) -> Option<(f32, f32, f32, f32)> {
    let safe = if safe_margin_mm.is_finite() {
        safe_margin_mm.max(0.)
    } else {
        0.
    };
    if driver.is_none() && safe < 0.05 {
        return None;
    }
    let (left, top, right, bottom) = driver.unwrap_or_default();
    Some((
        left.max(safe),
        top.max(safe),
        right.max(safe),
        bottom.max(safe),
    ))
}

/// One row of the font chooser: a group heading, or a font with the name a design stores,
/// what its sample says, and its plain name.
enum FontRow {
    Heading(&'static str),
    Font {
        key: String,
        title: String,
        detail: String,
    },
}

/// Split "Condensed (Noto Sans Condensed)" into the sample text and the plain name.
fn font_title(name: &str) -> (String, String) {
    match name.split_once(" (") {
        Some((title, rest)) => (title.to_string(), rest.trim_end_matches(')').to_string()),
        None => (name.to_string(), name.to_string()),
    }
}

/// The font chooser: a window with a search box and one list of the built-in fonts, your
/// fonts, and the computer's fonts, each name drawn in its own font. Choosing a font closes
/// it, as do Esc and its close button. Returns whether it is still open.
fn font_chooser(
    ctx: &egui::Context,
    anchor: Pos2,
    font: &mut String,
    search: &mut String,
    own: &[String],
    samples: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
) -> bool {
    let query = search.trim().to_lowercase();
    let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
    let font_row = |key: String, name: &str| {
        let (title, detail) = font_title(name);
        FontRow::Font { key, title, detail }
    };
    // A font that turned out to have no Latin letters (or to be unreadable) is hidden once its
    // sample has been tried, unless the text already uses it.
    let unreadable = |key: &str| key != font.as_str() && matches!(samples.get(key), Some(None));
    let mut rows = vec![FontRow::Heading("Built in · prints the same everywhere")];
    let default = crate::i18n::tr("Default (DejaVu Sans)");
    if matches(default) {
        rows.push(font_row(String::new(), default));
    }
    for built_in in crate::fonts::BUILT_IN {
        let name = crate::i18n::tr(built_in.name);
        if matches(name) {
            rows.push(font_row(built_in.key(), name));
        }
    }
    let first_own = rows.len();
    rows.push(FontRow::Heading("Your fonts"));
    rows.extend(
        own.iter()
            .filter(|name| matches(name) && !unreadable(name))
            .map(|name| font_row(name.clone(), name)),
    );
    if rows.len() == first_own + 1 {
        rows.pop();
    }
    rows.push(FontRow::Heading("On this computer"));
    let before = rows.len();
    rows.extend(
        crate::fonts::system_fonts()
            .iter()
            .map(|system| {
                (
                    format!("{}{}", crate::fonts::SYSTEM_PREFIX, system.name),
                    &system.name,
                )
            })
            .filter(|(key, name)| matches(name) && !unreadable(key))
            .map(|(key, name)| font_row(key, name)),
    );
    let none_on_computer = rows.len() == before;

    let mut open = true;
    let mut chosen = false;
    egui::Window::new(crate::i18n::tr("Choose a font"))
        .default_pos(anchor)
        .default_size(Vec2::new(360., 560.))
        .min_height(320.)
        .collapsible(false)
        .resizable(true)
        .open(&mut open)
        .show(ctx, |ui| {
            let field = ui
                .add(
                    egui::TextEdit::singleline(search)
                        .hint_text(crate::i18n::tr("Search fonts"))
                        .desired_width(f32::INFINITY),
                )
                .accessible_name(crate::i18n::tr("Search fonts"));
            if !field.has_focus() && !ui.memory(|m| m.focused().is_some()) {
                field.request_focus();
            }
            // Rows of one height, so long lists scroll smoothly. Only the rows in view are
            // drawn, and at most a few new samples a frame, so scrolling never stalls.
            let row = 32.;
            let ppp = ctx.pixels_per_point();
            let mut budget = 8;
            ui.spacing_mut().item_spacing.y = 2.;
            egui::ScrollArea::vertical()
                .id_salt("font-chooser-list")
                .auto_shrink([false, false])
                .show_rows(ui, row, rows.len(), |ui, range| {
                    for entry in &rows[range] {
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), row),
                            match entry {
                                FontRow::Heading(_) => Sense::hover(),
                                FontRow::Font { .. } => Sense::click(),
                            },
                        );
                        let painter = ui.painter_at(rect);
                        let visuals = ui.visuals();
                        match entry {
                            FontRow::Heading(text) => {
                                painter.text(
                                    rect.left_center() + Vec2::new(4., 0.),
                                    egui::Align2::LEFT_CENTER,
                                    crate::i18n::tr(text),
                                    egui::FontId::proportional(12.),
                                    visuals.strong_text_color(),
                                );
                            }
                            FontRow::Font { key, title, detail } => {
                                let selected = font == key;
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::Role::Button,
                                        true,
                                        selected,
                                        detail,
                                    )
                                });
                                if selected {
                                    painter.rect_filled(rect, 6., visuals.selection.bg_fill);
                                } else if response.hovered() || response.has_focus() {
                                    painter.rect_filled(
                                        rect,
                                        6.,
                                        visuals.widgets.hovered.weak_bg_fill,
                                    );
                                }
                                let color = if selected {
                                    visuals.selection.stroke.color
                                } else {
                                    visuals.strong_text_color()
                                };
                                let detail_galley = painter.layout_no_wrap(
                                    detail.clone(),
                                    egui::FontId::proportional(11.),
                                    if selected {
                                        color
                                    } else {
                                        visuals.weak_text_color()
                                    },
                                );
                                // The sample stops short of the plain name on the right.
                                let sample_area = Rect::from_min_max(
                                    rect.min,
                                    Pos2::new(
                                        (rect.right() - detail_galley.size().x - 20.)
                                            .max(rect.left() + rect.width() * 0.4),
                                        rect.bottom(),
                                    ),
                                );
                                let sample_width = sample_area.width() - 8.;
                                if !samples.contains_key(key) && budget > 0 {
                                    budget -= 1;
                                    let drawn = crate::fonts::sample(
                                        key,
                                        title,
                                        18. * ppp,
                                        (sample_width * ppp) as u32,
                                    )
                                    .map(|(w, h, mask)| {
                                        let pixels: Vec<Color32> = mask
                                            .iter()
                                            .map(|&a| Color32::from_white_alpha(a))
                                            .collect();
                                        ctx.load_texture(
                                            format!("font-sample-{key}"),
                                            egui::ColorImage::new([w, h], pixels),
                                            egui::TextureOptions::LINEAR,
                                        )
                                    });
                                    samples.insert(key.clone(), drawn);
                                }
                                let sample_painter = painter.with_clip_rect(sample_area);
                                match samples.get(key) {
                                    Some(Some(texture)) => {
                                        let size = texture.size_vec2() / ppp;
                                        sample_painter.image(
                                            texture.id(),
                                            Rect::from_min_size(
                                                Pos2::new(
                                                    rect.left() + 8.,
                                                    rect.center().y - size.y / 2.,
                                                ),
                                                size,
                                            ),
                                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                                            color,
                                        );
                                    }
                                    // Not drawn yet, or a font that cannot be drawn: plain text.
                                    _ => {
                                        sample_painter.text(
                                            rect.left_center() + Vec2::new(8., 0.),
                                            egui::Align2::LEFT_CENTER,
                                            title,
                                            egui::FontId::proportional(15.),
                                            color,
                                        );
                                    }
                                }
                                painter.galley(
                                    Pos2::new(
                                        rect.right() - 8. - detail_galley.size().x,
                                        rect.center().y - detail_galley.size().y / 2.,
                                    ),
                                    detail_galley,
                                    color,
                                );
                                if response.clicked() {
                                    *font = key.clone();
                                    chosen = true;
                                }
                            }
                        }
                    }
                });
            if budget == 0 {
                ctx.request_repaint();
            }
            if none_on_computer {
                ui.label(
                    RichText::new(crate::i18n::tr("No fonts match."))
                        .small()
                        .weak(),
                );
            }
        });
    let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    open && !chosen && !escape
}

/// Where each corner of the printed label takes its picture from in the canvas render,
/// as texture coordinates for the top-left, top-right, bottom-right and bottom-left corners.
fn printed_corners(counterclockwise: bool) -> [Pos2; 4] {
    let (tl, tr, br, bl) = (
        Pos2::new(0., 0.),
        Pos2::new(1., 0.),
        Pos2::new(1., 1.),
        Pos2::new(0., 1.),
    );
    if counterclockwise {
        // Turning counterclockwise brings the canvas's right edge to the top.
        [tr, br, bl, tl]
    } else {
        // Turning clockwise brings the canvas's left edge to the top.
        [bl, tl, tr, br]
    }
}

/// A strip across the top of the canvas saying that rotated printing is on: the label is
/// designed on its side and rotated when it prints, so it never surprises anyone.
fn sideways_banner(ui: &mut egui::Ui, doc: &LabelDocument) {
    let (paper_w, paper_h) = doc.paper_size_mm();
    let (fill, text) = if ui.visuals().dark_mode {
        (
            Color32::from_rgb(28, 58, 92),
            Color32::from_rgb(190, 220, 255),
        )
    } else {
        (
            Color32::from_rgb(222, 236, 252),
            Color32::from_rgb(22, 70, 130),
        )
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                let (icon, _) = ui.allocate_exact_size(Vec2::splat(20.), Sense::hover());
                paint_tool_icon(ui.painter(), icon.center(), text, "Rotate stock");
                ui.label(
                    RichText::new(crate::i18n::tr("Rotated printing."))
                        .strong()
                        .color(text),
                );
                ui.label(
                    RichText::new(format!(
                        "{} {paper_w:.1} × {paper_h:.1} mm. {}",
                        crate::i18n::tr(if doc.rotate_counterclockwise {
                            "You are designing the label on its side. When it prints, it is rotated 90° counterclockwise to fit"
                        } else {
                            "You are designing the label on its side. When it prints, it is rotated 90° clockwise to fit"
                        }),
                        crate::i18n::tr("See “As printed” in the corner.")
                    ))
                    .color(text),
                );
            });
        });
    ui.add_space(4.);
}

/// Width kept free at the canvas's right edge for the "As printed" card.
const PRINTED_PREVIEW_ROOM: f32 = 150.;

/// A small card in the canvas's bottom-right corner showing a label with rotated printing
/// the way it comes out of the printer.
fn printed_preview(ui: &egui::Ui, texture: &egui::TextureHandle, panel: Rect, doc: &LabelDocument) {
    let (paper_w, paper_h) = doc.paper_size_mm();
    let height = 150_f32.min(panel.height() * 0.35).max(60.);
    let width = (height * paper_w / paper_h)
        .min(panel.width() * 0.3)
        .max(30.);
    let label = Vec2::new(width, height * width / (height * paper_w / paper_h));
    let caption = crate::i18n::tr("As printed");
    let card = Rect::from_min_size(
        panel.right_bottom() - label - Vec2::new(36., 58.),
        label + Vec2::new(24., 46.),
    );
    let painter = ui.painter();
    let visuals = ui.visuals();
    painter.rect_filled(
        card.translate(Vec2::new(0., 2.)),
        10.,
        Color32::from_black_alpha(30),
    );
    painter.rect_filled(card, 10., visuals.window_fill);
    painter.rect_stroke(card, 10., visuals.window_stroke, StrokeKind::Inside);
    painter.text(
        Pos2::new(card.left() + 12., card.top() + 16.),
        egui::Align2::LEFT_CENTER,
        caption,
        egui::FontId::proportional(12.),
        visuals.weak_text_color(),
    );
    let paper = Rect::from_min_size(Pos2::new(card.left() + 12., card.top() + 32.), label);
    let uv = printed_corners(doc.rotate_counterclockwise);
    let mut mesh = egui::Mesh::with_texture(texture.id());
    for (corner, uv) in [
        paper.left_top(),
        paper.right_top(),
        paper.right_bottom(),
        paper.left_bottom(),
    ]
    .into_iter()
    .zip(uv)
    {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: corner,
            uv,
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
    painter.rect_stroke(
        paper,
        2.,
        Stroke::new(1., visuals.widgets.noninteractive.bg_stroke.color),
        StrokeKind::Outside,
    );
}

/// Why no unprintable border can be shown, so that "Show printable area" never looks broken.
fn border_note(printer: Option<&Printer>, media: &str, label: (f32, f32)) -> &'static str {
    let Some(printer) = printer else {
        return "Choose a printer in Print Center to see its unprintable border, or set a safe margin in the View menu.";
    };
    if media.is_empty() {
        return "Select printer media in Print Center to see its unprintable border, or set a safe margin in the View menu.";
    }
    let matches = printer
        .media_sizes
        .get(media)
        .is_some_and(|&(w, h)| (w - label.0).abs() < 0.5 && (h - label.1).abs() < 0.5);
    if !matches {
        return "Choose media that matches this label's size to see the printer's unprintable border, or set a safe margin in the View menu.";
    }
    "The printer's driver reports no unprintable border for this media. Set a safe margin in the View menu to keep content away from the edge."
}

/// The smallest printed module width, in millimeters, at which a QR code with a logo scans
/// reliably on ordinary phone cameras and handheld scanners.
const QR_LOGO_MIN_MODULE_MM: f32 = 0.4;

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
/// The SSCC helper's inputs; they are not saved with the design.
#[derive(Clone, Debug, Default)]
struct SsccForm {
    extension: u8,
    prefix: String,
    serial: String,
    error: Option<String>,
}

/// Feedback on GS1 data, and a helper that builds an SSCC (pallet or parcel number).
fn gs1_helper(ui: &mut egui::Ui, kind: BarcodeKind, data: &mut String, form: &mut SsccForm) {
    if !data.contains("${") {
        let parsed = crate::gs1::parse(data).and_then(|fields| {
            if kind == BarcodeKind::Gs1DataMatrix {
                crate::gs1::data_matrix_payload(&fields)?;
            }
            Ok(fields)
        });
        match parsed {
            Ok(fields) => {
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        crate::i18n::tr("Prints as"),
                        crate::gs1::human(&fields)
                    ))
                    .small()
                    .weak(),
                );
            }
            Err(error) if !data.trim().is_empty() => {
                ui.colored_label(Color32::from_rgb(200, 60, 60), format!("{error:#}"));
            }
            Err(_) => {}
        }
    }
    egui::CollapsingHeader::new(crate::i18n::tr("SSCC helper"))
        .id_salt("sscc-helper")
        .show(ui, |ui| {
            ui.label(
                RichText::new(crate::i18n::tr(
                    "An SSCC numbers one pallet or parcel: extension digit, your GS1 company prefix, and a serial reference. The check digit is added for you.",
                ))
                .small()
                .weak(),
            );
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Extension digit"));
                ui.add(egui::DragValue::new(&mut form.extension).range(0..=9))
                    .accessible_name(crate::i18n::tr("Extension digit"));
            });
            ui.label(crate::i18n::tr("Company prefix"));
            ui.add(egui::TextEdit::singleline(&mut form.prefix))
                .accessible_name(crate::i18n::tr("Company prefix"));
            ui.label(crate::i18n::tr("Serial reference"));
            ui.add(egui::TextEdit::singleline(&mut form.serial))
                .accessible_name(crate::i18n::tr("Serial reference"));
            if icon_action(
                ui,
                "Apply",
                "Use this SSCC",
                "Replace the barcode content with (00) and the SSCC",
                true,
                false,
            )
            .clicked()
            {
                match crate::gs1::sscc(form.extension, form.prefix.trim(), form.serial.trim()) {
                    Ok(number) => {
                        *data = format!("(00){number}");
                        form.error = None;
                    }
                    Err(error) => form.error = Some(format!("{error:#}")),
                }
            }
            if let Some(error) = &form.error {
                ui.colored_label(Color32::from_rgb(200, 60, 60), error);
            }
        });
}

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
        BarcodeKind::Gs1128 => "GS1-128",
        BarcodeKind::Gs1DataMatrix => "GS1 Data Matrix",
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
        BarcodeKind::Gs1128 => {
            "The logistics barcode for pallets and cartons. Type GS1 data such as (01)09506000134352(10)LOT7."
        }
        BarcodeKind::Gs1DataMatrix => {
            "A small square symbol for GS1 data such as (01)09506000134352(10)LOT7. Put a batch or serial last."
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
        "Align left" => {
            path(&[(4., 3.), (4., 21.)]);
            path(&[(4., 7.), (19., 7.)]);
            path(&[(4., 12.), (12., 12.)]);
            path(&[(4., 17.), (15., 17.)]);
        }
        "Align center" => {
            path(&[(12., 3.), (12., 21.)]);
            path(&[(6., 7.), (18., 7.)]);
            path(&[(8., 12.), (16., 12.)]);
            path(&[(5., 17.), (19., 17.)]);
        }
        "Align right" => {
            path(&[(20., 3.), (20., 21.)]);
            path(&[(5., 7.), (20., 7.)]);
            path(&[(12., 12.), (20., 12.)]);
            path(&[(9., 17.), (20., 17.)]);
        }
        "Align top" => {
            path(&[(3., 4.), (21., 4.)]);
            path(&[(7., 4.), (7., 19.)]);
            path(&[(12., 4.), (12., 12.)]);
            path(&[(17., 4.), (17., 15.)]);
        }
        "Align middle" => {
            path(&[(3., 12.), (21., 12.)]);
            path(&[(7., 5.), (7., 19.)]);
            path(&[(12., 8.), (12., 16.)]);
            path(&[(17., 6.), (17., 18.)]);
        }
        "Align bottom" => {
            path(&[(3., 20.), (21., 20.)]);
            path(&[(7., 5.), (7., 20.)]);
            path(&[(12., 12.), (12., 20.)]);
            path(&[(17., 9.), (17., 20.)]);
        }
        "Distribute horizontally" => {
            path(&[(3., 3.), (3., 21.)]);
            path(&[(21., 3.), (21., 21.)]);
            outline(9., 7., 6., 10., 1.);
        }
        "Distribute vertically" => {
            path(&[(3., 3.), (21., 3.)]);
            path(&[(3., 21.), (21., 21.)]);
            outline(7., 9., 10., 6., 1.);
        }
        "Center horizontally on label" => {
            outline(8., 6., 8., 12., 1.);
            path(&[(12., 2.), (12., 4.)]);
            path(&[(12., 20.), (12., 22.)]);
        }
        "Center vertically on label" => {
            outline(6., 8., 12., 8., 1.);
            path(&[(2., 12.), (4., 12.)]);
            path(&[(20., 12.), (22., 12.)]);
        }
        "Lock" => {
            outline(5., 11., 14., 10., 2.);
            path(&[
                (8., 11.),
                (8., 8.),
                (10., 5.),
                (14., 5.),
                (16., 8.),
                (16., 11.),
            ]);
        }
        "Unlock" => {
            outline(5., 11., 14., 10., 2.);
            path(&[(8., 11.), (8., 8.), (10., 5.), (14., 5.), (16., 8.)]);
        }
        "Rotate 90" => {
            path(&[(20., 10.), (18., 5.), (12., 3.), (6., 5.), (3., 10.)]);
            path(&[(16., 10.), (20., 10.), (20., 6.)]);
            outline(8., 12., 8., 8., 1.);
        }
        "Flip horizontally" => {
            path(&[(12., 2.), (12., 22.)]);
            path(&[(9., 6.), (3., 18.), (9., 18.), (9., 6.)]);
            path(&[(15., 6.), (21., 18.), (15., 18.), (15., 6.)]);
        }
        "Flip vertically" => {
            path(&[(2., 12.), (22., 12.)]);
            path(&[(6., 9.), (18., 9.), (12., 3.), (6., 9.)]);
            path(&[(6., 15.), (18., 15.), (12., 21.), (6., 15.)]);
        }
        "Fit page" => {
            outline(3., 3., 18., 18., 2.);
            outline(6., 6., 5., 5., 1.);
            outline(13., 6., 5., 5., 1.);
            outline(6., 13., 5., 5., 1.);
            outline(13., 13., 5., 5., 1.);
        }
        "Address book" => {
            outline(5., 3., 15., 18., 2.);
            path(&[(3., 7.), (7., 7.)]);
            path(&[(3., 12.), (7., 12.)]);
            path(&[(3., 17.), (7., 17.)]);
            painter.circle_stroke(point(13., 10.), 2.5, stroke);
            path(&[(9., 17.), (11., 14.), (15., 14.), (17., 17.)]);
        }
        "Symbols and logos" => {
            path(&[(12., 3.), (22., 20.), (2., 20.), (12., 3.)]);
            path(&[(12., 9.), (12., 14.)]);
            path(&[(12., 17.), (12., 17.)]);
        }
        "Apply" => {
            path(&[(4., 12.), (9., 17.), (20., 6.)]);
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

#[cfg(test)]
mod inspector_render_tests {
    use super::*;

    fn frames(designer: &mut Designer) {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1280., 800.));
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| designer.show(ui, &Settings::default(), &[]),
            );
            output.textures_delta.clear();
        }
    }

    fn sample() -> Vec<Element> {
        vec![
            Element::Text {
                text: "A long line of text ${date:%d/%m/%Y} ${serial:4}".into(),
                font_pt: 12.,
                bold: false,
                align: TextAlign::Left,
            },
            Element::Barcode {
                kind: design::BarcodeKind::Ean13,
                data: "590123412345".into(),
                show_text: true,
            },
            Element::Qr {
                data: "https://example.com".into(),
            },
            Element::Shape {
                kind: design::ShapeKind::Rectangle,
                stroke_mm: 0.4,
                filled: false,
            },
            Element::Image {
                data_base64: String::new(),
            },
        ]
    }

    #[test]
    fn every_kind_of_object_shows_its_inspector_without_panicking() {
        for element in sample() {
            for locked in [false, true] {
                let mut designer = Designer::new();
                designer.doc = LabelDocument::blank(60., 40.);
                let id = designer.doc.add(element.clone(), 2., 2., 40., 12.);
                designer.doc.objects[0].locked = locked;
                designer.selected = vec![id];
                frames(&mut designer);
                assert_eq!(designer.selected, vec![id]);
            }
        }
    }

    #[test]
    fn a_multi_selection_shows_the_arrange_panel() {
        let mut designer = Designer::new();
        designer.doc = LabelDocument::blank(80., 50.);
        let mut ids = Vec::new();
        for (i, element) in sample().into_iter().enumerate() {
            ids.push(
                designer
                    .doc
                    .add(element, 2. + i as f32 * 3., 2. + i as f32 * 5., 20., 8.),
            );
        }
        designer.selected = ids.clone();
        frames(&mut designer);
        designer.arrange(design::Alignment::Left, design::AlignTo::Selection);
        designer.distribute(design::Axis::Horizontal);
        designer.set_lock(true);
        assert!(designer.doc.objects.iter().all(|o| o.locked));
        frames(&mut designer);
        designer.delete();
        assert_eq!(
            designer.doc.objects.len(),
            ids.len(),
            "locked objects survive delete"
        );
    }
}

#[cfg(test)]
mod range_window_tests {
    use super::*;

    #[test]
    fn the_range_window_opens_stays_open_and_never_prints_without_a_printer() {
        let ctx = egui::Context::default();
        let mut designer = Designer::new();
        designer.doc = LabelDocument::blank(50., 30.);
        designer.doc.add(
            Element::Text {
                text: "No. ${serial:4}".into(),
                font_pt: 14.,
                bold: false,
                align: TextAlign::Left,
            },
            2.,
            2.,
            40.,
            10.,
        );
        designer.range = Some(RangeState {
            start: 7,
            step: 5,
            count: 3,
        });
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900., 700.));
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |_| designer.range_window(&ctx, &Settings::default(), &[]),
            );
            output.textures_delta.clear();
        }
        assert!(designer.range.is_some());
        assert!(designer.job.is_none());
    }
}

#[cfg(test)]
mod recovery_flow_tests {
    use super::*;

    fn frame(designer: &mut Designer, ctx: &egui::Context) {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1280., 800.));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| designer.show(ui, &Settings::default(), &[]),
        );
        output.textures_delta.clear();
    }

    #[test]
    fn unsaved_work_is_kept_then_offered_back_after_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut designer = Designer::new();
        designer.enable_recovery_in(dir.path().to_path_buf());
        frame(&mut designer, &ctx);
        assert!(
            crate::recovery::read_in(dir.path()).is_none(),
            "nothing to keep yet"
        );
        designer.doc.title = "Work in progress".into();
        frame(&mut designer, &ctx);
        let kept = crate::recovery::read_in(dir.path()).expect("a copy is written");
        assert_eq!(kept.title, "Work in progress");

        // A new session finds it and offers it.
        let mut next = Designer::new();
        next.enable_recovery_in(dir.path().to_path_buf());
        assert_eq!(
            next.recovered.as_ref().map(|d| d.title.as_str()),
            Some("Work in progress")
        );
        frame(&mut next, &ctx);
        // Edits made while the offer is open never overwrite the recovered copy.
        next.doc.title = "Something else".into();
        frame(&mut next, &ctx);
        assert_eq!(
            crate::recovery::read_in(dir.path()).unwrap().title,
            "Work in progress"
        );
    }

    #[test]
    fn saving_or_discarding_removes_the_copy() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut designer = Designer::new();
        designer.enable_recovery_in(dir.path().to_path_buf());
        designer.doc.title = "Edited".into();
        frame(&mut designer, &ctx);
        assert!(crate::recovery::read_in(dir.path()).is_some());
        designer.saved_document = designer.doc.clone();
        frame(&mut designer, &ctx);
        assert!(
            crate::recovery::read_in(dir.path()).is_none(),
            "saved work needs no copy"
        );

        designer.doc.title = "Edited again".into();
        frame(&mut designer, &ctx);
        designer.close_approved = true;
        assert!(designer.request_close());
        assert!(
            crate::recovery::read_in(dir.path()).is_none(),
            "discarding removes it"
        );
    }
}

#[cfg(test)]
mod sheet_window_tests {
    use super::*;

    #[test]
    fn the_sheet_window_opens_and_stays_open_without_printing() {
        let ctx = egui::Context::default();
        let mut designer = Designer::new();
        designer.doc = LabelDocument::blank(50., 30.);
        let spec = crate::sheet::SheetSpec::fitting(crate::sheet::Page::A4, 50., 30., 5., 0.);
        assert_eq!((spec.columns, spec.rows), (4, 9));
        designer.sheet = Some(SheetState {
            count: 36,
            spec,
            dpi: 300,
        });
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(900., 800.));
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |_| designer.sheet_window(&ctx, &Settings::default(), &[]),
            );
            output.textures_delta.clear();
        }
        assert!(designer.sheet.is_some());
        assert!(designer.job.is_none());
    }
}

#[cfg(test)]
mod gallery_tests {
    use super::*;

    #[test]
    fn the_gallery_previews_every_built_in_template_and_filters_by_name() {
        let ctx = egui::Context::default();
        let mut designer = Designer::new();
        designer.gallery = Some(String::new());
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1000., 800.));
        for _ in 0..6 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |_| designer.gallery_window(&ctx),
            );
            output.textures_delta.clear();
        }
        let builtin = designer
            .gallery_thumbs
            .keys()
            .filter(|k| k.starts_with("builtin-"))
            .count();
        assert_eq!(builtin, design::TEMPLATES.len());
        assert!(
            designer.gallery.is_some(),
            "stays open until a template is picked"
        );
        designer.gallery = Some("zzz-no-match".into());
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |_| designer.gallery_window(&ctx),
        );
        output.textures_delta.clear();
        assert!(designer.gallery.is_some());
    }
}

#[cfg(test)]
mod printable_border_tests {
    use super::*;
    use std::collections::HashMap;

    fn printer_with(media: &str, size: (f32, f32)) -> Printer {
        Printer {
            name: "P".into(),
            media_sizes: HashMap::from([(media.to_string(), size)]),
            ..Default::default()
        }
    }

    #[test]
    fn the_larger_of_the_driver_border_and_the_safe_margin_wins_on_each_side() {
        let border = effective_border(Some((1., 3., 0.5, 2.)), 2.).unwrap();
        assert_eq!(border, (2., 3., 2., 2.));
    }

    #[test]
    fn a_safe_margin_alone_gives_a_border_even_without_a_printer_border() {
        assert_eq!(effective_border(None, 2.), Some((2., 2., 2., 2.)));
        assert_eq!(effective_border(None, 0.), None);
        assert_eq!(effective_border(None, f32::NAN), None);
        assert_eq!(effective_border(None, -3.), None);
        assert_eq!(
            effective_border(Some((1., 1., 1., 1.)), 0.),
            Some((1., 1., 1., 1.))
        );
    }

    #[test]
    fn objects_inside_a_safe_margin_are_flagged_when_the_driver_reports_nothing() {
        let mut doc = design::LabelDocument::blank(100., 150.);
        let near = doc.add(
            Element::Text {
                text: "x".into(),
                font_pt: 9.,
                bold: false,
                align: design::TextAlign::Left,
            },
            0.5,
            50.,
            30.,
            5.,
        );
        let far = doc.add(
            Element::Text {
                text: "y".into(),
                font_pt: 9.,
                bold: false,
                align: design::TextAlign::Left,
            },
            10.,
            50.,
            30.,
            5.,
        );
        assert!(doc.objects_in_margins((0., 0., 0., 0.)).is_empty());
        let border = effective_border(None, 2.).unwrap();
        let flagged = doc.objects_in_margins(border);
        assert!(flagged.contains(&near) && !flagged.contains(&far));
    }

    #[test]
    fn the_note_names_the_real_reason_nothing_is_shaded() {
        assert!(border_note(None, "", (100., 150.)).starts_with("Choose a printer"));
        let printer = printer_with("w4h6", (101.6, 152.4));
        assert!(
            border_note(Some(&printer), "", (101.6, 152.4)).starts_with("Select printer media")
        );
        assert!(
            border_note(Some(&printer), "w4h6", (50., 30.))
                .starts_with("Choose media that matches")
        );
        assert!(
            border_note(Some(&printer), "unknown", (101.6, 152.4))
                .starts_with("Choose media that matches")
        );
        assert!(
            border_note(Some(&printer), "w4h6", (101.6, 152.4))
                .contains("reports no unprintable border"),
            "matching media with no border means the driver says everything is printable"
        );
    }
}

#[cfg(test)]
mod printed_preview_tests {
    use super::*;

    /// The corner preview samples the canvas render the same way the print is turned.
    #[test]
    fn the_preview_shows_the_label_exactly_as_it_prints() {
        let source = image::RgbImage::from_fn(3, 2, |x, y| image::Rgb([(x * 10 + y) as u8, 0, 0]));
        for counterclockwise in [false, true] {
            let printed = if counterclockwise {
                image::imageops::rotate270(&source)
            } else {
                image::imageops::rotate90(&source)
            };
            let (w, h) = (printed.width() - 1, printed.height() - 1);
            let corners = [(0, 0), (w, 0), (w, h), (0, h)];
            for (uv, (x, y)) in printed_corners(counterclockwise).into_iter().zip(corners) {
                let sx = (uv.x * (source.width() - 1) as f32) as u32;
                let sy = (uv.y * (source.height() - 1) as f32) as u32;
                assert_eq!(
                    source.get_pixel(sx, sy),
                    printed.get_pixel(x, y),
                    "counterclockwise: {counterclockwise}, corner ({x}, {y})"
                );
            }
        }
    }
}
