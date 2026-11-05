use crate::{
    design,
    designer_ui::Designer,
    detection::{self, Crop, Detection},
    document::{self, Page},
    export,
    layout::{self, Output},
    printing::{self, Printer},
    settings::{Mode, Scale, Settings, ThemeChoice},
};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Area {
    Home,
    Designer,
    Import,
    PrintCenter,
}
enum Message {
    Loaded(Arc<Page>, Detection),
    Files(Vec<PathBuf>),
    BatchDone {
        submitted: usize,
        total: usize,
        detail: String,
    },
    Rendered(Arc<Output>),
    Printers(Vec<Printer>),
    Done(String),
    Cancelled,
    Error(String),
}
#[derive(Clone, Copy)]
enum DragKind {
    Draw,
    Move,
    Corner(usize),
}
#[derive(Clone)]
struct QueueItem {
    path: PathBuf,
    page: usize,
    crop: Crop,
    reviewed: bool,
    mode: Mode,
}
struct Drag {
    start: Pos2,
    crop: Crop,
    kind: DragKind,
}
pub struct ThermalPrint {
    settings: Settings,
    area: Area,
    designer: Designer,
    page: Option<Arc<Page>>,
    detection: Option<Detection>,
    crop: Crop,
    output: Option<Arc<Output>>,
    original_texture: Option<egui::TextureHandle>,
    output_texture: Option<egui::TextureHandle>,
    icon_texture: egui::TextureHandle,
    printers: Vec<Printer>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    busy: bool,
    status: String,
    error: bool,
    dirty: bool,
    original: bool,
    zoom: f32,
    reviewed: bool,
    drag: Option<Drag>,
    startup_file: Option<PathBuf>,
    queue: Vec<QueueItem>,
    selected: Option<usize>,
    pending_files: std::collections::VecDeque<PathBuf>,
    loading_selection: Option<usize>,
    about_open: bool,
}
impl ThermalPrint {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        cc.egui_ctx.set_theme(match settings.theme {
            ThemeChoice::System => egui::ThemePreference::System,
            ThemeChoice::Light => egui::ThemePreference::Light,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
        });
        cc.egui_ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = Vec2::new(10., 10.);
            style.spacing.button_padding = Vec2::new(12., 8.);
            style.visuals.selection.bg_fill = Color32::from_rgb(28, 111, 100);
        });
        let (tx, rx) = mpsc::channel();
        let icon_image =
            image::load_from_memory(include_bytes!("../data/org.thermalprint.ThermalPrint.png"))
                .expect("bundled icon must decode")
                .to_rgba8();
        let icon_texture = cc.egui_ctx.load_texture(
            "thermalprint-icon",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon_image.width() as usize, icon_image.height() as usize],
                icon_image.as_raw(),
            ),
            egui::TextureOptions::LINEAR,
        );
        let mut app = Self {
            settings,
            area: Area::Home,
            designer: Designer::new(),
            page: None,
            detection: None,
            crop: Crop::FULL,
            output: None,
            original_texture: None,
            output_texture: None,
            icon_texture,
            printers: vec![],
            tx,
            rx,
            busy: false,
            status: "Ready · Your documents stay on this computer".into(),
            error: false,
            dirty: false,
            original: false,
            zoom: 1.,
            reviewed: false,
            drag: None,
            startup_file: std::env::args_os().nth(1).map(PathBuf::from),
            queue: vec![],
            selected: None,
            pending_files: std::collections::VecDeque::new(),
            loading_selection: None,
            about_open: false,
        };
        app.refresh_printers();
        app
    }
    fn task(&mut self, status: &str, f: impl FnOnce() -> anyhow::Result<Message> + Send + 'static) {
        self.busy = true;
        self.error = false;
        self.status = status.into();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => {
                    tracing::error!(error=%e,"Operation failed");
                    Message::Error(format!("{e:#}"))
                }
                Err(_) => {
                    Message::Error("Processing failed unexpectedly. Try another document.".into())
                }
            };
            let _ = tx.send(message);
        });
    }
    fn refresh_printers(&mut self) {
        self.task("Looking for CUPS printers…", || {
            Ok(Message::Printers(printing::discover()?))
        });
    }
    fn load(&mut self, path: PathBuf, index: usize) {
        self.task("Rendering document at 300 DPI…", move || {
            let page = Arc::new(document::load(&path, index)?);
            let detection = detection::detect(&page.image);
            Ok(Message::Loaded(page, detection))
        });
    }
    fn open(&mut self) {
        self.task("Choose shipping labels…", || {
            let files = rfd::FileDialog::new()
                .add_filter(
                    "Labels",
                    &["pdf", "png", "jpg", "jpeg", "tif", "tiff", "bmp", "webp"],
                )
                .pick_files();
            Ok(files.map(Message::Files).unwrap_or(Message::Cancelled))
        });
    }
    fn save_selection(&mut self) {
        if let Some(i) = self.selected
            && let Some(item) = self.queue.get_mut(i)
        {
            item.crop = self.crop;
            item.reviewed = self.reviewed;
            item.mode = self.settings.mode;
        }
    }
    fn select(&mut self, i: usize) {
        if self.busy || i >= self.queue.len() {
            return;
        }
        self.save_selection();
        let item = &self.queue[i];
        self.loading_selection = Some(i);
        self.load(item.path.clone(), item.page);
    }
    fn append_files(&mut self, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        self.save_selection();
        self.pending_files.extend(files);
        self.load_next_file();
    }
    fn load_next_file(&mut self) {
        if let Some(path) = self.pending_files.pop_front() {
            self.loading_selection = None;
            self.load(path, 0);
        }
    }
    fn print_all(&mut self) {
        self.save_selection();
        if self.busy || self.queue.is_empty() {
            return;
        }
        let Some(printer) = self
            .printers
            .iter()
            .find(|p| p.name == self.settings.printer)
            .cloned()
        else {
            self.status = "Select a printer".into();
            return;
        };
        if self.settings.media.is_empty() {
            self.status = "Select printer media".into();
            return;
        }
        if self.queue.iter().any(|i| !i.reviewed) {
            self.status = "Review every queued label before printing all".into();
            self.error = true;
            return;
        }
        let queue = self.queue.clone();
        let settings = self.settings.clone();
        self.task("Submitting labels to CUPS in order…", move || {
            let total = queue.len();
            for (index, item) in queue.iter().enumerate() {
                let result = (|| -> anyhow::Result<()> {
                    let page = document::load(&item.path, item.page)?;
                    let crop = if item.mode == Mode::FullPage {
                        Crop::FULL
                    } else {
                        item.crop
                    };
                    let output = layout::render(&page, crop, &settings)?;
                    printing::submit(&output, &settings, &printer)?;
                    Ok(())
                })();
                if let Err(e) = result {
                    return Ok(Message::BatchDone {
                        submitted: index,
                        total,
                        detail: format!("Stopped at label {}: {e:#}", index + 1),
                    });
                }
            }
            Ok(Message::BatchDone {
                submitted: total,
                total,
                detail: "All labels submitted to CUPS in order".into(),
            })
        });
    }
    fn render(&mut self) {
        let Some(page) = self.page.clone() else {
            return;
        };
        let crop = if self.settings.mode == Mode::FullPage {
            Crop::FULL
        } else {
            self.crop
        };
        let settings = self.settings.clone();
        self.output = None;
        self.output_texture = None;
        self.dirty = false;
        self.task("Preparing label…", move || {
            Ok(Message::Rendered(Arc::new(layout::render(
                &page, crop, &settings,
            )?)))
        });
    }
    fn texture(ctx: &egui::Context, name: &str, image: &image::RgbImage) -> egui::TextureHandle {
        let im = if image.width() > 1600 || image.height() > 1600 {
            image::imageops::thumbnail(image, 1600, 1600)
        } else {
            image.clone()
        };
        ctx.load_texture(
            name,
            egui::ColorImage::from_rgb([im.width() as usize, im.height() as usize], im.as_raw()),
            egui::TextureOptions::LINEAR,
        )
    }
    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            self.busy = false;
            match msg {
                Message::Files(files) => self.append_files(files),
                Message::Loaded(page, d) => {
                    let selected = if let Some(i) = self.loading_selection.take() {
                        i
                    } else {
                        let i = self.queue.len();
                        self.queue.push(QueueItem {
                            path: page.path.clone(),
                            page: page.index,
                            crop: d.proposed,
                            reviewed: !d.needs_review,
                            mode: Mode::Automatic,
                        });
                        if page.count > 1 {
                            for n in 0..page.count {
                                if n != page.index {
                                    self.queue.push(QueueItem {
                                        path: page.path.clone(),
                                        page: n,
                                        crop: Crop::FULL,
                                        reviewed: false,
                                        mode: Mode::Automatic,
                                    });
                                }
                            }
                        }
                        i
                    };
                    self.selected = Some(selected);
                    self.settings.remember(page.path.clone());
                    self.original_texture = Some(Self::texture(ctx, "original", &page.image));
                    self.crop = self.queue[selected].crop;
                    self.reviewed = self.queue[selected].reviewed;
                    self.settings.mode = self.queue[selected].mode;
                    self.detection = Some(d);
                    self.page = Some(page);
                    self.output = None;
                    self.output_texture = None;
                    self.original = self.settings.mode == Mode::Manual;
                    self.dirty = true;
                    self.zoom = 1.;
                    self.status = format!(
                        "Label {} of {} ready for review",
                        selected + 1,
                        self.queue.len()
                    );
                    self.persist();
                    if !self.pending_files.is_empty() {
                        self.load_next_file();
                    }
                }
                Message::Rendered(output) => {
                    self.output_texture = Some(Self::texture(ctx, "output", &output.image));
                    self.status = if output.clipped {
                        "Adjust layout to resolve clipping".into()
                    } else {
                        "Label ready · Inspect barcode detail before printing".into()
                    };
                    self.output = Some(output);
                }
                Message::Printers(p) => {
                    self.printers = p;
                    if !self
                        .printers
                        .iter()
                        .any(|p| p.name == self.settings.printer)
                    {
                        self.settings.printer = self
                            .printers
                            .iter()
                            .find(|p| p.is_default)
                            .or(self.printers.first())
                            .map(|p| p.name.clone())
                            .unwrap_or_default();
                        self.settings.media.clear();
                        self.settings.resolution.clear();
                    }
                    self.status = if self.printers.is_empty() {
                        "No printers found. Add a label printer in your system settings; export is available.".into()
                    } else {
                        "CUPS printers refreshed".into()
                    };
                }
                Message::BatchDone {
                    submitted,
                    total,
                    detail,
                } => {
                    self.status = format!("{detail} · {submitted}/{total} submitted");
                    self.error = submitted < total;
                }
                Message::Done(s) => {
                    self.status = s;
                    self.error = false;
                }
                Message::Cancelled => self.status = "Ready".into(),
                Message::Error(e) => {
                    self.status = e;
                    self.error = true;
                    self.dirty = false;
                }
            }
        }
    }
    fn persist(&mut self) {
        if let Err(e) = self.settings.save() {
            self.status = format!("Could not save preferences: {e}");
            self.error = true;
        }
    }
    fn ready(&self) -> bool {
        !self.busy
            && !self.dirty
            && self.reviewed
            && self.output.as_ref().is_some_and(|o| !o.clipped)
    }
    fn print(&mut self) {
        if !self.ready() {
            return;
        }
        let Some(printer) = self
            .printers
            .iter()
            .find(|p| p.name == self.settings.printer)
            .cloned()
        else {
            self.status = "Select a CUPS printer".into();
            self.error = true;
            return;
        };
        let output = self.output.clone().unwrap();
        let settings = self.settings.clone();
        self.persist();
        self.task("Submitting to CUPS…", move || {
            Ok(Message::Done(format!(
                "Submitted · {}. Use Job status to check the queue.",
                printing::submit(&output, &settings, &printer)?
            )))
        });
    }
    fn export(&mut self, pdf: bool) {
        if !self.ready() {
            return;
        }
        let output = self.output.clone().unwrap();
        self.task("Exporting label…", move || {
            let extension = if pdf { "pdf" } else { "png" };
            if let Some(mut path) = rfd::FileDialog::new()
                .add_filter(extension, &[extension])
                .set_file_name(format!("label.{extension}"))
                .save_file()
            {
                if path.extension().is_none() {
                    path.set_extension(extension);
                }
                export::save(&output, &path)?;
                Ok(Message::Done(format!("Exported {}", path.display())))
            } else {
                Ok(Message::Cancelled)
            }
        });
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.);
            ui.heading(RichText::new("Your label workspace").size(30.));
            ui.label("Design a custom label or prepare a downloaded shipping label.");
            ui.add_space(20.);
        });
        ui.columns(2, |columns| {
            Self::section_frame(&columns[0]).show(&mut columns[0], |ui| {
                ui.heading("Create a label");
                ui.label(
                    "Design text, shapes, real barcodes and QR codes on a precisely sized canvas.",
                );
                ui.add_space(16.);
                if ui.button("Create new label").clicked() {
                    self.designer = Designer::new();
                    self.area = Area::Designer;
                }
            });
            Self::section_frame(&columns[1]).show(&mut columns[1], |ui| {
                ui.heading("Import & convert");
                ui.label(
                    "Open PDFs or images, remove instructions, review crops, then print in order.",
                );
                ui.add_space(16.);
                if ui.button("Open existing labels").clicked() {
                    self.area = Area::Import;
                    self.open();
                }
            });
        });
        ui.add_space(18.);
        ui.heading("Templates");
        ui.horizontal_wrapped(|ui| {
            for (i, t) in design::TEMPLATES.iter().enumerate() {
                if ui.button(t.name).on_hover_text(t.description).clicked() {
                    self.designer = Designer::new();
                    self.designer.doc = design::from_template(i);
                    self.area = Area::Designer;
                }
            }
        });
        ui.add_space(18.);
        ui.heading("Printer status");
        if let Some(p) = self
            .printers
            .iter()
            .find(|p| p.name == self.settings.printer)
        {
            ui.label(format!("{} · {}", p.name, printing::thermal_hint(p)));
        } else {
            ui.label("No printer selected. Export is available without a printer.");
        }
        ui.add_space(8.);
        ui.heading("Recent documents");
        for path in self.settings.recent.clone() {
            if ui.link(path.display().to_string()).clicked() {
                self.area = Area::Import;
                self.append_files(vec![path]);
            }
        }
    }
    fn print_center(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Print Center").size(28.));
        ui.label("Configure the CUPS queue, stock, and output resolution before printing.");
        ui.add_space(18.);
        let old = self.settings.clone();
        Self::section_frame(ui).show(ui, |ui| {
            ui.heading("Printer & media");
            egui::ComboBox::from_id_salt("center-printer")
                .selected_text(if self.settings.printer.is_empty() {
                    "Choose printer"
                } else {
                    &self.settings.printer
                })
                .show_ui(ui, |ui| {
                    for p in &self.printers {
                        ui.selectable_value(&mut self.settings.printer, p.name.clone(), &p.name);
                    }
                });
            if old.printer != self.settings.printer {
                self.settings.media.clear();
                self.settings.resolution.clear();
            }
            if let Some(p) = self
                .printers
                .iter()
                .find(|p| p.name == self.settings.printer)
            {
                ui.label(printing::thermal_hint(p));
                egui::ComboBox::from_id_salt("center-media")
                    .selected_text(if self.settings.media.is_empty() {
                        "Select media"
                    } else {
                        &self.settings.media
                    })
                    .show_ui(ui, |ui| {
                        if p.custom_media {
                            ui.selectable_value(
                                &mut self.settings.media,
                                "Custom".into(),
                                "Custom dimensions",
                            );
                        }
                        for m in &p.media {
                            ui.selectable_value(&mut self.settings.media, m.clone(), m);
                        }
                    });
                if !p.resolutions.is_empty() {
                    egui::ComboBox::from_id_salt("center-resolution")
                        .selected_text(if self.settings.resolution.is_empty() {
                            "Driver default"
                        } else {
                            &self.settings.resolution
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.settings.resolution,
                                String::new(),
                                "Driver default",
                            );
                            for r in &p.resolutions {
                                ui.selectable_value(&mut self.settings.resolution, r.clone(), r);
                            }
                        });
                }
            }
            if ui.button("Refresh printers").clicked() {
                self.refresh_printers();
            }
            ui.horizontal(|ui| {
                ui.label("Copies");
                ui.add(egui::DragValue::new(&mut self.settings.copies).range(1..=999));
                ui.label("DPI");
                egui::ComboBox::from_id_salt("center-dpi")
                    .selected_text(self.settings.dpi.to_string())
                    .show_ui(ui, |ui| {
                        for dpi in [203, 300, 600] {
                            ui.selectable_value(&mut self.settings.dpi, dpi, dpi.to_string());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Paper width");
                ui.add(
                    egui::DragValue::new(&mut self.settings.width_mm)
                        .range(1.0..=2000.0)
                        .suffix(" mm"),
                );
                ui.label("Height");
                ui.add(
                    egui::DragValue::new(&mut self.settings.height_mm)
                        .range(1.0..=2000.0)
                        .suffix(" mm"),
                );
            });
        });
        ui.add_space(12.);
        Self::section_frame(ui).show(ui, |ui| {
            ui.heading("Queue & diagnostics");
            ui.label(format!("{} imported labels queued", self.queue.len()));
            if ui.button("Open import queue").clicked() {
                self.area = Area::Import;
            }
            if ui.button("Check CUPS job status").clicked() {
                let p = self.settings.printer.clone();
                if !p.is_empty() {
                    self.task("Checking CUPS queue…", move || {
                        Ok(Message::Done(printing::job_status(&p)?))
                    });
                }
            }
            ui.label(RichText::new("A successful submission means CUPS accepted the job; check physical output and barcode readability.").small().weak());
        });
        if serde_json::to_string(&old).ok() != serde_json::to_string(&self.settings).ok() {
            self.persist();
        }
    }
    fn section_frame(ui: &egui::Ui) -> egui::Frame {
        egui::Frame::group(ui.style())
            .fill(if ui.visuals().dark_mode {
                Color32::from_rgb(34, 42, 45)
            } else {
                Color32::WHITE
            })
            .inner_margin(14)
            .corner_radius(12)
    }
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("WORKSPACE")
                .size(12.)
                .strong()
                .color(Color32::from_rgb(36, 146, 126)),
        );
        ui.heading(RichText::new("Prepare your label").size(24.));
        ui.label(RichText::new("OPEN  /  CHECK  /  PRINT").small().weak());
        ui.add_space(6.);
        if ui
            .add_enabled(
                !self.busy,
                egui::Button::new("Add labels...  Ctrl+O")
                    .min_size(Vec2::new(ui.available_width(), 38.)),
            )
            .clicked()
        {
            self.open();
        }
        if !self.queue.is_empty() {
            ui.add_space(8.);
            ui.label(RichText::new(format!("Print queue · {} labels", self.queue.len())).strong());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "Label {} of {}",
                        self.selected.map_or(0, |i| i + 1),
                        self.queue.len()
                    ))
                    .strong(),
                );
                if ui
                    .add_enabled(
                        !self.busy && self.selected.is_some_and(|i| i > 0),
                        egui::Button::new("Previous"),
                    )
                    .clicked()
                {
                    self.select(self.selected.unwrap() - 1);
                }
                if ui
                    .add_enabled(
                        !self.busy && self.selected.is_some_and(|i| i + 1 < self.queue.len()),
                        egui::Button::new("Next"),
                    )
                    .clicked()
                {
                    self.select(self.selected.unwrap() + 1);
                }
            });
            let mut action = None;
            for (i, item) in self.queue.iter().enumerate() {
                let name = item.path.file_name().unwrap_or_default().to_string_lossy();
                let label = format!(
                    "{}  {} · page {}  {}",
                    i + 1,
                    name,
                    item.page + 1,
                    if item.reviewed { "Ready" } else { "Review" }
                );
                if ui
                    .selectable_label(self.selected == Some(i), label)
                    .clicked()
                {
                    action = Some(i);
                }
            }
            if let Some(i) = action {
                self.select(i);
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.busy && self.selected.is_some_and(|i| i > 0),
                        egui::Button::new("Move up"),
                    )
                    .clicked()
                {
                    self.save_selection();
                    let i = self.selected.unwrap();
                    self.queue.swap(i, i - 1);
                    self.selected = Some(i - 1);
                }
                if ui
                    .add_enabled(
                        !self.busy && self.selected.is_some_and(|i| i + 1 < self.queue.len()),
                        egui::Button::new("Move down"),
                    )
                    .clicked()
                {
                    self.save_selection();
                    let i = self.selected.unwrap();
                    self.queue.swap(i, i + 1);
                    self.selected = Some(i + 1);
                }
                if ui
                    .add_enabled(
                        !self.busy && self.selected.is_some(),
                        egui::Button::new("Remove"),
                    )
                    .clicked()
                {
                    let next = self.selected.unwrap().min(self.queue.len() - 1);
                    self.queue.remove(self.selected.unwrap());
                    self.selected = None;
                    self.page = None;
                    self.output = None;
                    self.original_texture = None;
                    self.output_texture = None;
                    if !self.queue.is_empty() {
                        self.select(next.min(self.queue.len() - 1));
                    }
                }
            });
            if ui
                .add_enabled(
                    !self.busy
                        && !self.settings.printer.is_empty()
                        && !self.settings.media.is_empty(),
                    egui::Button::new(format!("Print all {} labels in order", self.queue.len()))
                        .min_size(Vec2::new(ui.available_width(), 38.)),
                )
                .clicked()
            {
                self.print_all();
            }
        }
        if !self.settings.recent.is_empty() {
            let mut chosen = None;
            egui::ComboBox::from_id_salt("recent")
                .selected_text("Recently opened")
                .width(ui.available_width() - 12.)
                .show_ui(ui, |ui| {
                    for path in &self.settings.recent {
                        if ui
                            .selectable_label(
                                false,
                                path.file_name().unwrap_or_default().to_string_lossy(),
                            )
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            chosen = Some(path.clone());
                        }
                    }
                });
            if !self.busy
                && let Some(p) = chosen
            {
                self.append_files(vec![p]);
            }
        }
        ui.separator();
        ui.add_enabled_ui(!self.busy,|ui| {
            if let Some(page)=self.page.clone(){
                ui.label(RichText::new(page.path.file_name().unwrap_or_default().to_string_lossy()).strong());
                ui.label(RichText::new(format!("{:.1} × {:.1} mm · {} × {} px",page.width_mm,page.height_mm,page.image.width(),page.image.height())).small().weak());
                if page.count>1 {let mut index=page.index+1;ui.horizontal(|ui|{ui.label("Page");if ui.add(egui::DragValue::new(&mut index).range(1..=page.count)).changed() && let Some(i)=self.queue.iter().position(|q|q.path==page.path&&q.page==index-1){self.select(i);}ui.label(format!("of {}",page.count));});}
            }else{ui.label("Drop a downloaded label anywhere.");ui.label(RichText::new("PDF · PNG · JPEG · TIFF · BMP · WebP").small().weak());}
            ui.add_space(12.);
            Self::section_frame(ui).show(ui, |ui| { ui.set_width(ui.available_width());
            ui.label(RichText::new("01  EXTRACT").size(15.).strong().color(Color32::from_rgb(36,146,126)));
            let old_mode=self.settings.mode;
            ui.horizontal_wrapped(|ui|{ui.selectable_value(&mut self.settings.mode,Mode::Automatic,"Automatic");ui.selectable_value(&mut self.settings.mode,Mode::Manual,"Manual");ui.selectable_value(&mut self.settings.mode,Mode::FullPage,"Full page");});
            if self.settings.mode!=old_mode {self.dirty=true;self.drag=None;self.zoom=1.;match self.settings.mode {Mode::Automatic=>{self.original=false;if let Some(d)=&self.detection {self.crop=d.proposed;self.reviewed = !d.needs_review;}},Mode::Manual=>{self.original=true;self.reviewed=false;},Mode::FullPage=>{self.original=false;self.reviewed=true;}}}
            if self.settings.mode==Mode::Automatic && let Some(d)=self.detection.clone(){ui.label(RichText::new(d.message).small());if !d.candidates.is_empty(){ui.horizontal_wrapped(|ui|{for (i,c) in d.candidates.iter().enumerate(){if ui.selectable_label(self.crop==*c,format!("Region {}",i+1)).clicked(){self.crop = *c;self.dirty=true;self.original=true;self.reviewed=false;}}});}}
            if self.settings.mode==Mode::Automatic && let Some(d)=self.detection.clone() && d.candidates.len()>1 && d.proposed==Crop::FULL
                && ui.button(format!("Queue all {} detected regions",d.candidates.len())).clicked()
                && let (Some(i),Some(page))=(self.selected,self.page.as_ref()) {
                        let items=d.candidates.into_iter().map(|crop|QueueItem{path:page.path.clone(),page:page.index,crop,reviewed:false,mode:Mode::Automatic}).collect::<Vec<_>>();
                        self.queue.splice(i..=i,items);
                        self.crop=self.queue[i].crop;
                        self.reviewed=false;
                        self.original=true;
                        self.dirty=true;
                        self.status="Review each detected region before printing all".into();
            }
            if self.settings.mode==Mode::Manual {ui.label(RichText::new("Drag to draw. Drag inside to move; drag a corner to resize. Crop coordinates refer to the original page.").small());}
            if self.page.is_some()&&self.settings.mode!=Mode::FullPage {
                if ui.button("Reset to entire page").clicked(){self.crop=Crop::FULL;self.dirty=true;self.reviewed=false;}
                ui.checkbox(&mut self.reviewed,"I checked the crop and barcode edges");
            }
            });
            ui.add_space(10.);
            Self::section_frame(ui).show(ui, |ui| { ui.set_width(ui.available_width());
            ui.label(RichText::new("02  PAPER & LAYOUT").size(15.).strong().color(Color32::from_rgb(36,146,126)));
            let before=serde_json::to_string(&self.settings).unwrap_or_default();
            egui::ComboBox::from_id_salt("paper").selected_text(format!("{:.1} × {:.1} mm",self.settings.width_mm,self.settings.height_mm)).width(ui.available_width()-12.).show_ui(ui,|ui| {
                for (name,w,h) in [("4 × 6 in",101.6,152.4),("4 × 4 in",101.6,101.6),("2 × 1 in",50.8,25.4),("100 × 150 mm",100.,150.)] {if ui.selectable_label(self.settings.width_mm==w&&self.settings.height_mm==h,name).clicked(){self.settings.width_mm=w;self.settings.height_mm=h;}}
            });
            ui.horizontal(|ui|{ui.label("W");ui.add(egui::DragValue::new(&mut self.settings.width_mm).range(10.0..=500.0).speed(0.1).suffix(" mm"));ui.label("H");ui.add(egui::DragValue::new(&mut self.settings.height_mm).range(10.0..=500.0).speed(0.1).suffix(" mm"));});
            if ui.button("Swap portrait / landscape").clicked(){std::mem::swap(&mut self.settings.width_mm,&mut self.settings.height_mm);}
            ui.horizontal(|ui|{ui.label("Rotate");egui::ComboBox::from_id_salt("rotation").selected_text(format!("{}°",self.settings.rotation)).show_ui(ui,|ui|{for r in [0,90,180,270]{ui.selectable_value(&mut self.settings.rotation,r,format!("{r}°"));}});ui.label("DPI");egui::ComboBox::from_id_salt("dpi").selected_text(self.settings.dpi.to_string()).width(60.).show_ui(ui,|ui|{for dpi in [203,300,600]{ui.selectable_value(&mut self.settings.dpi,dpi,dpi.to_string());}});});
            egui::ComboBox::from_id_salt("scale").selected_text(match self.settings.scale {Scale::Fit=>"Fit within paper",Scale::Actual=>"Actual size",Scale::Custom=>"Custom scale"}).show_ui(ui,|ui|{ui.selectable_value(&mut self.settings.scale,Scale::Fit,"Fit within paper");ui.selectable_value(&mut self.settings.scale,Scale::Actual,"Actual size");ui.selectable_value(&mut self.settings.scale,Scale::Custom,"Custom scale");});
            if self.settings.scale==Scale::Custom {ui.add(egui::Slider::new(&mut self.settings.scale_percent,10.0..=400.0).suffix("%"));}
            egui::CollapsingHeader::new("Margins & image enhancement").show(ui,|ui|{
                ui.add(egui::DragValue::new(&mut self.settings.margin_mm).range(0.0..=20.0).speed(0.1).prefix("Margin ").suffix(" mm"));
                ui.checkbox(&mut self.settings.enhance,"Black-and-white threshold (advanced)");ui.label(RichText::new("Default processing preserves tones. Thresholding can damage fine barcode detail.").small());
            });
            if before!=serde_json::to_string(&self.settings).unwrap_or_default(){self.dirty=true;}
            });
            ui.add_space(10.);
            Self::section_frame(ui).show(ui, |ui| { ui.set_width(ui.available_width());
            ui.label(RichText::new("03  PRINT & EXPORT").size(15.).strong().color(Color32::from_rgb(36,146,126)));
            let previous=self.settings.printer.clone();
            egui::ComboBox::from_id_salt("printer").selected_text(if previous.is_empty(){"Choose a printer"}else{&previous}).width(ui.available_width()-12.).show_ui(ui,|ui|{for p in &self.printers{ui.selectable_value(&mut self.settings.printer,p.name.clone(),format!("{}{}",p.name,if p.is_default{" (default)"}else{""}));}});
            if previous!=self.settings.printer {self.settings.media.clear();self.settings.resolution.clear();}
            if let Some(p)=self.printers.iter().find(|p|p.name==self.settings.printer).cloned(){
                ui.label(RichText::new(&p.description).small().weak());
                ui.label(RichText::new(printing::thermal_hint(&p)).small().weak());
                egui::ComboBox::from_id_salt("media").selected_text(if self.settings.media.is_empty(){"Choose matching driver media"}else{&self.settings.media}).width(ui.available_width()-12.).show_ui(ui,|ui|{if p.custom_media {ui.selectable_value(&mut self.settings.media,"Custom".into(),"Custom — match output dimensions");}for m in &p.media {ui.selectable_value(&mut self.settings.media,m.clone(),m);}});
                ui.label(RichText::new("Driver media must match the paper dimensions above. Check your printer's unprintable margins.").small());
                if !p.resolutions.is_empty(){egui::ComboBox::from_id_salt("resolution").selected_text(if self.settings.resolution.is_empty(){"Driver default resolution"}else{&self.settings.resolution}).show_ui(ui,|ui|{ui.selectable_value(&mut self.settings.resolution,String::new(),"Driver default resolution");for r in &p.resolutions{ui.selectable_value(&mut self.settings.resolution,r.clone(),r);}});}
            }
            ui.horizontal(|ui|{ui.label("Copies");ui.add(egui::DragValue::new(&mut self.settings.copies).range(1..=999));if ui.small_button("Refresh printers").clicked(){self.refresh_printers();}});
            if ui.add_enabled(self.ready()&&!self.settings.printer.is_empty()&&!self.settings.media.is_empty(),egui::Button::new(RichText::new("Print label   Ctrl+P").color(Color32::WHITE)).fill(Color32::from_rgb(28,111,100)).min_size(Vec2::new(ui.available_width(),42.))).clicked(){self.print();}
            ui.horizontal(|ui|{if ui.add_enabled(self.ready(),egui::Button::new("Export PDF")).clicked(){self.export(true);}
                if ui.add_enabled(self.ready(),egui::Button::new("Export PNG")).clicked(){self.export(false);}});
            if ui.add_enabled(!self.settings.printer.is_empty(),egui::Button::new("Job status")).clicked(){let p=self.settings.printer.clone();self.task("Checking CUPS queue…",move||Ok(Message::Done(printing::job_status(&p)?)));}
            });
        });
    }
    fn preview(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Preview");
            ui.selectable_value(&mut self.original, true, "Original / crop");
            ui.selectable_value(&mut self.original, false, "Thermal label");
            if ui.button("−").clicked() {
                self.zoom = (self.zoom / 1.25).max(0.25);
            }
            if ui.button("+").clicked() {
                self.zoom = (self.zoom * 1.25).min(6.);
            }
            if ui.button("Fit").clicked() {
                self.zoom = 1.;
            }
            ui.label(format!("{:.0}%", self.zoom * 100.));
        });
        ui.separator();
        if self.page.is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.15);
                ui.add(egui::Image::new(&self.icon_texture).fit_to_exact_size(Vec2::splat(94.)));
                ui.add_space(18.);
                ui.label(RichText::new("Labels, ready to go.").size(28.).strong());
                ui.add_space(12.);
                ui.label(RichText::new("Drop PDFs or images to build a print queue").size(16.));
                ui.label(
                    RichText::new("From any marketplace or carrier. No account needed.").weak(),
                );
                ui.add_space(20.);
                if ui
                    .add_enabled(
                        !self.busy,
                        egui::Button::new("Add labels...").min_size(Vec2::new(180., 42.)),
                    )
                    .clicked()
                {
                    self.open();
                }
            });
            return;
        }
        if let Some(output) = &self.output {
            for warning in &output.warnings {
                ui.colored_label(Color32::from_rgb(180, 110, 20), warning);
            }
        }
        if !self.reviewed {
            ui.label(
                RichText::new("Check the original crop, then confirm it in the left panel.")
                    .color(Color32::from_rgb(180, 110, 20)),
            );
        }
        ui.label(
            RichText::new(if self.original {
                "Original document · Green boundary is the crop · Crop before rotating"
            } else {
                "Paper boundary · Final output uses the full-resolution render"
            })
            .small()
            .weak(),
        );
        let texture = if self.original {
            self.original_texture.clone()
        } else {
            self.output_texture.clone()
        };
        let Some(texture) = texture else {
            ui.spinner();
            return;
        };
        let available = ui.available_size() - Vec2::splat(40.);
        let aspect = texture.size_vec2();
        let fit = (available.x / aspect.x)
            .min(available.y / aspect.y)
            .max(0.05);
        let size = aspect * fit * self.zoom;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (outer, _) = ui.allocate_exact_size(
                    Vec2::new(size.x.max(available.x), size.y.max(available.y)) + Vec2::splat(24.),
                    Sense::hover(),
                );
                let rect = Rect::from_center_size(outer.center(), size);
                ui.painter()
                    .rect_filled(rect.expand(3.), 2., Color32::from_gray(130));
                ui.painter().image(
                    texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                    Color32::WHITE,
                );
                if self.original && self.settings.mode != Mode::FullPage {
                    let crop_rect = Rect::from_min_size(
                        rect.min + Vec2::new(self.crop.x * size.x, self.crop.y * size.y),
                        Vec2::new(self.crop.w * size.x, self.crop.h * size.y),
                    );
                    let color = Color32::from_rgb(0, 170, 130);
                    ui.painter().rect_stroke(
                        crop_rect,
                        0.,
                        Stroke::new(2., color),
                        StrokeKind::Inside,
                    );
                    for p in [
                        crop_rect.left_top(),
                        crop_rect.right_top(),
                        crop_rect.right_bottom(),
                        crop_rect.left_bottom(),
                    ] {
                        ui.painter().rect_filled(
                            Rect::from_center_size(p, Vec2::splat(9.)),
                            2.,
                            color,
                        );
                    }
                    if self.settings.mode == Mode::Manual && !self.busy {
                        let response =
                            ui.interact(rect, ui.scope_id().with("crop-editor"), Sense::drag());
                        if response.drag_started()
                            && let Some(pos) = response.interact_pointer_pos()
                        {
                            let corners = [
                                crop_rect.left_top(),
                                crop_rect.right_top(),
                                crop_rect.right_bottom(),
                                crop_rect.left_bottom(),
                            ];
                            let kind = if let Some(i) =
                                corners.iter().position(|c| c.distance(pos) < 16.)
                            {
                                DragKind::Corner(i)
                            } else if crop_rect.contains(pos) {
                                DragKind::Move
                            } else {
                                DragKind::Draw
                            };
                            self.drag = Some(Drag {
                                start: Pos2::new(
                                    (pos.x - rect.min.x) / size.x,
                                    (pos.y - rect.min.y) / size.y,
                                ),
                                crop: self.crop,
                                kind,
                            });
                        }
                        if response.dragged()
                            && let (Some(d), Some(pos)) =
                                (&self.drag, response.interact_pointer_pos())
                        {
                            let p = Pos2::new(
                                ((pos.x - rect.min.x) / size.x).clamp(0., 1.),
                                ((pos.y - rect.min.y) / size.y).clamp(0., 1.),
                            );
                            let c = d.crop;
                            self.crop = match d.kind {
                                DragKind::Move => Crop {
                                    x: (c.x + p.x - d.start.x).clamp(0., 1. - c.w),
                                    y: (c.y + p.y - d.start.y).clamp(0., 1. - c.h),
                                    ..c
                                },
                                DragKind::Draw => Crop {
                                    x: p.x.min(d.start.x),
                                    y: p.y.min(d.start.y),
                                    w: (p.x - d.start.x).abs(),
                                    h: (p.y - d.start.y).abs(),
                                },
                                DragKind::Corner(i) => {
                                    let opposite = match i {
                                        0 => Pos2::new(c.x + c.w, c.y + c.h),
                                        1 => Pos2::new(c.x, c.y + c.h),
                                        2 => Pos2::new(c.x, c.y),
                                        _ => Pos2::new(c.x + c.w, c.y),
                                    };
                                    Crop {
                                        x: p.x.min(opposite.x),
                                        y: p.y.min(opposite.y),
                                        w: (p.x - opposite.x).abs(),
                                        h: (p.y - opposite.y).abs(),
                                    }
                                }
                            }
                            .bounded();
                            self.reviewed = false;
                        }
                        if response.drag_stopped() {
                            self.drag = None;
                            self.dirty = true;
                        }
                    }
                }
            });
    }
}
impl eframe::App for ThermalPrint {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        if !self.busy
            && let Some(path) = self.startup_file.take()
        {
            self.area = Area::Import;
            self.append_files(vec![path]);
        }
        if !self.busy {
            let dropped = ctx.input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .map(|f| f.path().to_path_buf())
                    .collect::<Vec<_>>()
            });
            if !dropped.is_empty() {
                self.area = Area::Import;
                self.append_files(dropped);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
                self.area = Area::Import;
                self.open();
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::P)) {
                self.print();
            }
        }
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new(&self.icon_texture).fit_to_exact_size(Vec2::splat(34.)));
                ui.heading(RichText::new("ThermalPrint").size(23.));
                ui.label(RichText::new("Universal shipping labels").weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("About").clicked() {
                        self.about_open = true;
                    }
                    let old = self.settings.theme;
                    egui::ComboBox::from_id_salt("theme")
                        .selected_text(match old {
                            ThemeChoice::System => "System theme",
                            ThemeChoice::Light => "Light",
                            ThemeChoice::Dark => "Dark",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.settings.theme,
                                ThemeChoice::System,
                                "System theme",
                            );
                            ui.selectable_value(
                                &mut self.settings.theme,
                                ThemeChoice::Light,
                                "Light",
                            );
                            ui.selectable_value(
                                &mut self.settings.theme,
                                ThemeChoice::Dark,
                                "Dark",
                            );
                        });
                    if self.settings.theme != old {
                        ctx.set_theme(match self.settings.theme {
                            ThemeChoice::System => egui::ThemePreference::System,
                            ThemeChoice::Light => egui::ThemePreference::Light,
                            ThemeChoice::Dark => egui::ThemePreference::Dark,
                        });
                        self.persist();
                    }
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.area, Area::Home, "Home");
                ui.selectable_value(&mut self.area, Area::Designer, "Label Designer");
                ui.selectable_value(&mut self.area, Area::Import, "Import & Convert");
                ui.selectable_value(&mut self.area, Area::PrintCenter, "Print Center");
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self.busy {
                    ui.spinner();
                }
                ui.label(RichText::new(&self.status).color(if self.error {
                    Color32::from_rgb(200, 65, 55)
                } else {
                    ui.visuals().text_color()
                }));
            });
        });
        match self.area {
            Area::Home => {
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.home(ui));
                });
            }
            Area::Designer => {
                self.designer.show(ui, &self.settings, &self.printers);
            }
            Area::PrintCenter => {
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.print_center(ui));
                });
            }
            Area::Import => {
                egui::Panel::left("settings")
                    .frame(
                        egui::Frame::new()
                            .fill(if ui.visuals().dark_mode {
                                Color32::from_rgb(25, 32, 35)
                            } else {
                                Color32::from_rgb(244, 247, 246)
                            })
                            .inner_margin(14),
                    )
                    .default_size(370.)
                    .size_range(330.0..=480.0)
                    .resizable(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.sidebar(ui));
                    });
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(if ui.visuals().dark_mode {
                                Color32::from_rgb(22, 28, 31)
                            } else {
                                Color32::from_rgb(249, 251, 250)
                            })
                            .inner_margin(18),
                    )
                    .show(ui, |ui| self.preview(ui));
            }
        }
        if self.about_open {
            let icon = self.icon_texture.clone();
            egui::Window::new("About ThermalPrint")
                .id(egui::Id::unique("thermalprint-about"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .open(&mut self.about_open)
                .show(ui.ctx(), |ui| {
                    ui.set_min_width(340.);
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.);
                        ui.add(egui::Image::new(&icon).fit_to_exact_size(Vec2::splat(88.)));
                        ui.add_space(12.);
                        ui.label(RichText::new("ThermalPrint").size(27.).strong());
                        ui.label(
                            RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).weak(),
                        );
                        ui.add_space(14.);
                        ui.label("Design and prepare thermal labels for Linux.");
                        ui.label("Built for Linux, with Rust and care.");
                        ui.add_space(16.);
                        ui.label(
                            RichText::new("AUTHOR")
                                .small()
                                .strong()
                                .color(Color32::from_rgb(36, 146, 126)),
                        );
                        ui.label(RichText::new("Alexis Rios-Negron").size(17.).strong());
                        ui.add_space(12.);
                        ui.label(
                            RichText::new("MORE BY ALEXIS")
                                .small()
                                .strong()
                                .color(Color32::from_rgb(36, 146, 126)),
                        );
                        ui.hyperlink_to("Voltkit", "https://github.com/arios4k/voltkit");
                        ui.hyperlink_to(
                            "Battery Health Center",
                            "https://github.com/arios4k/battery-health-center",
                        );
                        ui.add_space(14.);
                        ui.label(RichText::new("Open source · MIT License").small().weak());
                        ui.add_space(12.);
                    });
                });
        }
        self.save_selection();
        if self.dirty && !self.busy && self.drag.is_none() {
            self.render();
            self.persist();
        }
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    fn save(&mut self, _: &mut dyn eframe::Storage) {
        self.persist();
    }
}
