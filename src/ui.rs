mod about;
mod navigation;
mod screenshots;
mod window_chrome;
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
    time::{Duration, Instant},
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
    app_icons: about::AppIcons,
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
    watcher: Option<crate::watch::Watcher>,
    history: Vec<crate::history::Entry>,
    history_stamp: Option<std::time::SystemTime>,
    last_watch: Instant,
    queue: Vec<QueueItem>,
    selected: Option<usize>,
    pending_files: std::collections::VecDeque<PathBuf>,
    loading_selection: Option<usize>,
    about_open: bool,
    license_open: bool,
    terms_open: bool,
    terms_search: String,
    capture: Option<screenshots::Capture>,
}
impl ThermalPrint {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let screenshot_directory = std::env::var_os("THERMALPRINT_SCREENSHOTS").map(PathBuf::from);
        let screenshot_style =
            std::env::var("THERMALPRINT_SCREENSHOTS_STYLE").unwrap_or_else(|_| "light".into());
        let settings = if screenshot_directory.is_some() {
            Settings {
                theme: if screenshot_style == "dark" {
                    ThemeChoice::Dark
                } else {
                    ThemeChoice::Light
                },
                ..Settings::default()
            }
        } else {
            Settings::load()
        };
        cc.egui_ctx.set_theme(match settings.theme {
            ThemeChoice::System => egui::ThemePreference::System,
            ThemeChoice::Light => egui::ThemePreference::Light,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
        });
        cc.egui_ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = Vec2::new(10., 8.);
            style.spacing.button_padding = Vec2::new(12., 6.);
            style.spacing.interact_size.y = 32.;
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(14.));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(14.));
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(12.));
            style.visuals.weak_text_color = Some(if style.visuals.dark_mode {
                Color32::from_rgb(180, 186, 196)
            } else {
                Color32::from_rgb(84, 92, 106)
            });
            style.visuals.selection.bg_fill = Color32::from_rgb(46, 101, 190);
            style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
            style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
            style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);
            style
                .text_styles
                .insert(egui::TextStyle::Heading, egui::FontId::proportional(23.));
            if !style.visuals.dark_mode {
                style.visuals.panel_fill = Color32::from_rgb(242, 244, 248);
                style.visuals.window_fill = Color32::WHITE;
                style.visuals.override_text_color = Some(Color32::from_rgb(42, 48, 59));
            }
            style.visuals.selection.stroke = Stroke::new(1., Color32::WHITE);
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
            app_icons: about::AppIcons::load(&cc.egui_ctx),
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
            watcher: None,
            history: vec![],
            history_stamp: None,
            last_watch: Instant::now(),
            queue: vec![],
            selected: None,
            pending_files: std::collections::VecDeque::new(),
            loading_selection: None,
            about_open: false,
            license_open: false,
            terms_open: false,
            terms_search: String::new(),
            capture: None,
        };
        app.sync_watcher();
        // Documentation screenshots always use English, whatever the system language.
        if screenshot_directory.is_some() {
            // THERMALPRINT_SCREENSHOTS_LANGUAGE (es, de, fr) previews a translation.
            crate::i18n::set_resolved(
                std::env::var("THERMALPRINT_SCREENSHOTS_LANGUAGE")
                    .map_or(crate::i18n::Resolved::English, |code| {
                        crate::i18n::detect(&code)
                    }),
            );
        } else {
            crate::i18n::init();
        }
        if let Some(directory) = screenshot_directory {
            screenshots::seed(&mut app, &cc.egui_ctx);
            app.capture = Some(screenshots::Capture::new(directory, screenshot_style));
        } else {
            app.designer.enable_recovery();
            app.refresh_printers();
        }
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
    /// Start or stop the folder watcher to match the saved settings.
    fn sync_watcher(&mut self) {
        let wanted = self
            .settings
            .watch_enabled
            .then(|| self.settings.watch_folder.clone())
            .flatten();
        let Some(folder) = wanted else {
            self.watcher = None;
            return;
        };
        if self.watcher.as_ref().is_some_and(|w| w.dir() == folder) {
            return;
        }
        match crate::watch::check(&folder).and_then(|()| crate::watch::Watcher::new(&folder)) {
            Ok(watcher) => self.watcher = Some(watcher),
            Err(error) => {
                self.watcher = None;
                self.settings.watch_enabled = false;
                self.status = format!("Watch folder is off · {error:#}");
                self.error = true;
            }
        }
    }
    fn watch_tick(&mut self, ctx: &egui::Context) {
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        ctx.request_repaint_after(Duration::from_secs(2));
        if self.busy || self.last_watch.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_watch = Instant::now();
        match watcher.scan() {
            Ok(files) if files.is_empty() => {}
            Ok(files) => {
                let count = files.len();
                let visible = matches!(self.area, Area::Home | Area::Import);
                if visible {
                    self.area = Area::Import;
                }
                self.append_files(files);
                if !visible {
                    self.status = format!(
                        "{count} new label(s) from the watch folder are waiting in Import & Convert"
                    );
                }
            }
            Err(error) => {
                self.watcher = None;
                self.settings.watch_enabled = false;
                self.status = format!("Watch folder is off · {error:#}");
                self.error = true;
            }
        }
    }
    fn watch_controls(&mut self, ui: &mut egui::Ui) {
        let capturing = self.capture.is_some();
        egui::CollapsingHeader::new(crate::i18n::tr("Watch a folder"))
            .default_open(capturing)
            .show(ui, |ui| {
            ui.label(RichText::new(crate::i18n::tr("Add new PDF and image labels automatically when they appear in a folder, such as Downloads. Files already there are ignored.")).small().weak());
            let folder = self.settings.watch_folder.clone();
            ui.horizontal_wrapped(|ui| {
                if ui.button(crate::i18n::tr("Choose folder…")).clicked()
                    && let Some(picked) = rfd::FileDialog::new().pick_folder()
                {
                    self.settings.watch_folder = Some(picked);
                    self.settings.watch_enabled = true;
                    self.sync_watcher();
                    self.persist();
                }
                if folder.is_some() && ui.button(crate::i18n::tr("Clear")).clicked() {
                    self.settings.watch_folder = None;
                    self.settings.watch_enabled = false;
                    self.sync_watcher();
                    self.persist();
                }
            });
            match &folder {
                Some(folder) => {
                    ui.label(RichText::new(folder.display().to_string()).small());
                    if let Some(hint) = crate::watch::current_snap_hint(folder) {
                        ui.label(RichText::new(hint).small().weak());
                    }
                }
                None => {
                    ui.label(RichText::new(crate::i18n::tr("No folder chosen")).small().weak());
                }
            }
            let mut enabled = self.settings.watch_enabled;
            if ui
                .add_enabled(folder.is_some(), egui::Checkbox::new(&mut enabled, crate::i18n::tr("Watch for new labels")))
                .changed()
            {
                self.settings.watch_enabled = enabled;
                self.sync_watcher();
                self.persist();
            }
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
                    crate::history::record(crate::history::import_entry(
                        &item.path,
                        item.page,
                        &settings.printer,
                        settings.copies,
                    ));
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
                        let chosen = self
                            .printers
                            .iter()
                            .find(|p| p.is_default)
                            .or(self.printers.first())
                            .map(|p| p.name.clone())
                            .unwrap_or_default();
                        if !self.settings.switch_printer(&chosen) {
                            self.settings.media.clear();
                            self.settings.resolution.clear();
                        }
                    }
                    self.status = if self.printers.is_empty() {
                        printing::snap_connection_hint()
                            .unwrap_or("No printers found. Add a label printer in your system settings; export is available.")
                            .into()
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
        if self.capture.is_some() {
            return;
        }
        self.settings.store_profile();
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
        let source = self
            .selected
            .and_then(|i| self.queue.get(i))
            .map(|item| (item.path.clone(), item.page));
        self.persist();
        self.task("Submitting to CUPS…", move || {
            let submitted = printing::submit(&output, &settings, &printer)?;
            if let Some((path, page)) = source {
                crate::history::record(crate::history::import_entry(
                    &path,
                    page,
                    &settings.printer,
                    settings.copies,
                ));
            }
            Ok(Message::Done(format!(
                "Submitted · {submitted}. Use Job status to check the queue."
            )))
        });
    }
    /// Open the manual page that explains the screen you are on.
    fn open_manual(&self, ctx: &egui::Context) {
        let page = match self.area {
            Area::Home => "",
            Area::Designer => "designer.html",
            Area::Import => "import.html",
            Area::PrintCenter => "print-center.html",
        };
        ctx.open_url(egui::OpenUrl::new_tab(crate::help::manual_page(page)));
    }
    fn print_alignment_test(&mut self) {
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
        let settings = self.settings.clone();
        self.persist();
        self.task("Submitting alignment test…", move || {
            let output = crate::calibration::alignment_test(
                settings.width_mm,
                settings.height_mm,
                settings.dpi,
            );
            Ok(Message::Done(format!(
                "Alignment test submitted · {}",
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
    fn open_path(&mut self, path: PathBuf) {
        if design::is_design_path(&path) {
            self.area = Area::Designer;
            self.designer.request_open(path);
        } else {
            self.area = Area::Import;
            self.append_files(vec![path]);
        }
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.);
            ui.heading(RichText::new(crate::i18n::tr("Your label workspace")).size(30.));
            ui.label(crate::i18n::tr(
                "Design a custom label or prepare a downloaded shipping label.",
            ));
            ui.add_space(20.);
        });
        ui.columns(2, |columns| {
            if Self::home_card(
                &mut columns[0],
                "Create a label",
                "Build something of your own",
                "Add text, shapes, barcodes and QR codes to a precisely sized canvas.",
                "Create new label",
                "Design label",
                Color32::from_rgb(36, 146, 126),
            )
            .clicked()
            {
                self.designer.request_new();
                self.area = Area::Designer;
            }
            if Self::home_card(
                &mut columns[1],
                "Import & convert",
                "Get existing labels ready to print",
                "Open PDFs or images, remove instructions, review crops, then print in order.",
                "Open existing labels",
                "Import label",
                Color32::from_rgb(71, 119, 205),
            )
            .clicked()
            {
                self.area = Area::Import;
                self.open();
            }
        });
        ui.add_space(18.);
        ui.heading(crate::i18n::tr("Templates"));
        ui.horizontal_wrapped(|ui| {
            for (i, t) in design::TEMPLATES.iter().enumerate() {
                if crate::designer_ui::template_button(ui, t.name)
                    .on_hover_text(t.description)
                    .clicked()
                {
                    self.designer.request_template(i);
                    self.area = Area::Designer;
                }
            }
        });
        ui.add_space(18.);
        ui.heading(crate::i18n::tr("Printer status"));
        if let Some(p) = self
            .printers
            .iter()
            .find(|p| p.name == self.settings.printer)
        {
            ui.label(format!("{} · {}", p.name, printing::thermal_hint(p)));
        } else {
            ui.label(crate::i18n::tr(
                "No printer selected. Export is available without a printer.",
            ));
        }
        ui.add_space(8.);
        ui.heading(crate::i18n::tr("Recent documents"));
        for path in self.settings.recent.clone() {
            if ui.link(path.display().to_string()).clicked() {
                self.area = Area::Import;
                self.append_files(vec![path]);
            }
        }
    }
    fn select_default_media(&mut self) {
        if self.settings.media.is_empty()
            && let Some(printer) = self
                .printers
                .iter()
                .find(|p| p.name == self.settings.printer)
            && let Some(media) =
                printer.matching_media(self.settings.width_mm, self.settings.height_mm)
        {
            self.settings.media = media;
            self.persist();
        }
    }
    fn print_readiness(&mut self, ui: &mut egui::Ui, imported_label: bool) {
        let printer = self
            .printers
            .iter()
            .find(|p| p.name == self.settings.printer);
        let state = if self.busy || (imported_label && self.dirty) {
            Some(("Preparing", "Wait for the current operation to finish."))
        } else if imported_label && self.page.is_none() {
            Some((
                "Add a label",
                "Import a PDF or image to start preparing a print.",
            ))
        } else if imported_label && self.output.as_ref().is_some_and(|o| o.clipped) {
            Some((
                "Adjust the layout",
                "The label would be clipped. Use Fit within paper or change the output dimensions.",
            ))
        } else if imported_label && !self.reviewed {
            Some((
                "Review the crop",
                "Check the original crop and barcode edges, then confirm the review in Extract.",
            ))
        } else if imported_label && self.output.is_none() {
            Some(("Preparing preview", "The label preview is being generated."))
        } else if printer.is_none() {
            Some((
                "Choose a printer",
                "Select an available printer before printing. You can still export a prepared label.",
            ))
        } else if printer.is_some_and(|p| {
            self.settings.media != "Custom" && !p.media.contains(&self.settings.media)
                || self.settings.media == "Custom" && !p.custom_media
        }) {
            Some((
                "Choose printer media",
                "Select a supported media size that matches the loaded labels.",
            ))
        } else {
            None
        };
        if let Some((title, text)) = state {
            crate::help::notice(ui, title, text, true);
        } else if let Some(p) = printer {
            if let Some(message) = p.media_mismatch(
                &self.settings.media,
                self.settings.width_mm,
                self.settings.height_mm,
            ) {
                crate::help::notice(ui, "Label size differs from printer media", &message, true);
                ui.label(RichText::new(crate::i18n::tr("If the loaded paper matches the printer media, match the output below. Otherwise select media that matches the label size you want.")).small().weak());
                if let Some(&(w, h)) = p.media_sizes.get(&self.settings.media)
                    && ui.button(crate::i18n::tr("Match output to printer media")).on_hover_text(crate::i18n::tr("Update the output width and height to match the selected printer stock")).clicked() {
                    self.settings.width_mm = w;
                    self.settings.height_mm = h;
                    self.dirty = self.page.is_some();
                    self.persist();
                }
            } else {
                let verified = self.settings.media == "Custom"
                    || p.media_sizes.contains_key(&self.settings.media);
                let title = if imported_label && verified {
                    "Ready to submit"
                } else if verified {
                    "Printer configured"
                } else {
                    "Verify the loaded stock"
                };
                let text = if !verified {
                    "The driver does not report dimensions for this media. Check that the loaded labels match the output size."
                } else if imported_label {
                    "Crop reviewed and output prepared. Check the loaded stock before printing."
                } else {
                    "Media matches the output dimensions. Import or design a label to print."
                };
                crate::help::notice(ui, title, text, !verified);
            }
        }
    }
    fn print_center(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new(crate::i18n::tr("Print Center")).size(28.));
        ui.label(
            RichText::new(crate::i18n::tr(
                "Set up your printer and check your next print run.",
            ))
            .weak(),
        );
        ui.add_space(14.);
        let old = self.settings.clone();
        if ui.available_width() >= 900. {
            ui.columns(2, |columns| {
                self.print_settings(&mut columns[0]);
                self.print_overview(&mut columns[1]);
            });
        } else {
            self.print_settings(ui);
            ui.add_space(12.);
            self.print_overview(ui);
        }
        self.recent_prints(ui);
        if serde_json::to_string(&old).ok() != serde_json::to_string(&self.settings).ok() {
            self.persist();
        }
    }
    /// Reload the history file when another print (including a Designer print) changed it.
    fn refresh_history(&mut self) {
        if self.capture.is_some() {
            return;
        }
        let stamp = crate::history::history_path()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());
        if stamp != self.history_stamp || (stamp.is_none() && !self.history.is_empty()) {
            self.history_stamp = stamp;
            self.history = crate::history::load();
        }
    }
    /// Open a past label for review. Nothing is printed until the user presses Print.
    fn reopen(&mut self, entry: &crate::history::Entry) {
        use crate::history::Kind;
        match (entry.kind, &entry.design, &entry.source) {
            (Kind::Design, Some(doc), _) => {
                self.area = Area::Designer;
                self.designer.request_copy(doc.clone());
            }
            (Kind::Design, None, Some(path)) if path.is_file() => {
                self.area = Area::Designer;
                self.designer.request_open(path.clone());
            }
            (Kind::Import, _, Some(path)) if path.is_file() => {
                self.area = Area::Import;
                self.append_files(vec![path.clone()]);
            }
            _ => {
                self.status =
                    "That label is no longer available. Add the file again with Add labels…".into();
                self.error = true;
            }
        }
    }
    fn recent_prints(&mut self, ui: &mut egui::Ui) {
        self.refresh_history();
        if self.history.is_empty() {
            return;
        }
        ui.add_space(12.);
        let mut open = None;
        let mut clear = false;
        Self::section_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Self::center_heading(ui, crate::i18n::tr("Recent prints"), "Status");
            ui.label(
                RichText::new(
                    "Open a past label to review it. Nothing prints until you press Print.",
                )
                .small()
                .weak(),
            );
            for (index, entry) in self.history.iter().take(10).enumerate() {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(crate::i18n::tr("Open"))
                        .on_hover_text(crate::i18n::tr("Open this label for review"))
                        .clicked()
                    {
                        open = Some(index);
                    }
                    ui.label(RichText::new(&entry.title).strong());
                    ui.label(
                        RichText::new(format!(
                            "{} · {} × {}",
                            entry.time, entry.printer, entry.copies
                        ))
                        .small()
                        .weak(),
                    );
                });
            }
            if ui.small_button(crate::i18n::tr("Clear history")).clicked() {
                clear = true;
            }
        });
        if clear {
            crate::history::clear();
            self.history.clear();
            self.history_stamp = None;
        } else if let Some(index) = open {
            let entry = self.history[index].clone();
            self.reopen(&entry);
        }
    }
    fn center_heading(ui: &mut egui::Ui, title: &str, icon: &str) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.), Sense::hover());
            let accent = if ui.visuals().dark_mode {
                Color32::from_rgb(88, 199, 173)
            } else {
                Color32::from_rgb(28, 111, 100)
            };
            crate::designer_ui::paint_tool_icon(ui.painter(), rect.center(), accent, icon);
            ui.heading(title);
        });
        ui.add_space(4.);
    }
    fn print_settings(&mut self, ui: &mut egui::Ui) {
        let previous_printer = self.settings.printer.clone();
        let mut chosen_printer = previous_printer.clone();
        Self::section_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Self::center_heading(ui, crate::i18n::tr("Printer & media"), "Print device");
            ui.label(crate::i18n::tr("Printer queue"));
            egui::ComboBox::from_id_salt("center-printer")
                .selected_text(if chosen_printer.is_empty() {
                    "Choose printer"
                } else {
                    &chosen_printer
                })
                .show_ui(ui, |ui| {
                    for p in &self.printers {
                        ui.selectable_value(&mut chosen_printer, p.name.clone(), &p.name);
                    }
                });
            if previous_printer != chosen_printer {
                // Each printer keeps its own media, resolution, size, copies, and alignment.
                if !self.settings.switch_printer(&chosen_printer) {
                    self.settings.media.clear();
                    self.settings.resolution.clear();
                    self.select_default_media();
                }
            }
            if self.printers.is_empty() {
                ui.label(
                    RichText::new(
                        "No printers found. Add a printer in your system settings, then refresh.",
                    )
                    .weak(),
                );
            }
            if let Some(p) = self
                .printers
                .iter()
                .find(|p| p.name == self.settings.printer)
            {
                ui.label(printing::thermal_hint(p));
                ui.label(crate::i18n::tr("Printer media size"));
                egui::ComboBox::from_id_salt("center-media")
                    .selected_text(p.media_label(&self.settings.media))
                    .show_ui(ui, |ui| {
                        if p.custom_media {
                            ui.selectable_value(
                                &mut self.settings.media,
                                "Custom".into(),
                                "Custom dimensions",
                            );
                        }
                        for m in &p.media {
                            ui.selectable_value(&mut self.settings.media, m.clone(), p.media_label(m)).on_hover_text(format!("Driver code: {m}"));
                        }
                    });
                ui.label(RichText::new(crate::i18n::tr("Choose the driver media that matches your loaded labels. Custom uses the output dimensions.")).small().weak());
                if !p.resolutions.is_empty() {
                    ui.collapsing(crate::i18n::tr("Advanced driver resolution"), |ui| {
                    ui.label(crate::i18n::tr("Printer resolution"));
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
                    });
                }
            }
            if crate::designer_ui::icon_action(ui, "Refresh", "Refresh printers", "Refresh available CUPS printers", true, false).clicked() {
                self.refresh_printers();
            }
            ui.separator();
            ui.label(RichText::new(crate::i18n::tr("Output settings")).strong());
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Copies"));
                ui.add(egui::DragValue::new(&mut self.settings.copies).range(1..=999));
                ui.label(crate::i18n::tr("Rendering DPI")).on_hover_text(crate::help::explanation("DPI"));
                egui::ComboBox::from_id_salt("center-dpi")
                    .selected_text(self.settings.dpi.to_string())
                    .show_ui(ui, |ui| {
                        for dpi in [203, 300, 600] {
                            ui.selectable_value(&mut self.settings.dpi, dpi, dpi.to_string());
                        }
                    });
            });
            ui.label(crate::i18n::tr("Output label size"));
            egui::ComboBox::from_id_salt("center-label-size")
                .selected_text(format!("{:.1} × {:.1} mm", self.settings.width_mm, self.settings.height_mm))
                .show_ui(ui, |ui| {
                    for (name, width, height) in [
                        ("4 × 6 in", 101.6, 152.4), ("4 × 4 in", 101.6, 101.6),
                        ("2 × 1 in", 50.8, 25.4), ("100 × 150 mm", 100., 150.),
                    ] {
                        if ui.selectable_label(self.settings.width_mm == width && self.settings.height_mm == height, name).clicked() {
                            self.settings.width_mm = width;
                            self.settings.height_mm = height;
                        }
                    }
                });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Width"));
                ui.add(
                    egui::DragValue::new(&mut self.settings.width_mm)
                        .range(1.0..=2000.0)
                        .suffix(" mm"),
                );
                ui.label(crate::i18n::tr("Height"));
                ui.add(
                    egui::DragValue::new(&mut self.settings.height_mm)
                        .range(1.0..=2000.0)
                        .suffix(" mm"),
                );
            });
            if crate::designer_ui::icon_action(ui, "Rotate stock", "Swap portrait / landscape", "Swap label width and height", true, false).clicked() {
                std::mem::swap(&mut self.settings.width_mm, &mut self.settings.height_mm);
            }
            ui.label(RichText::new(crate::i18n::tr("These dimensions control the output and stock preview. Match them to the printer media size above.")).small().weak());
            ui.separator();
            ui.label(RichText::new(crate::i18n::tr("Alignment")).strong());
            ui.label(RichText::new(crate::i18n::tr("Settings above are remembered for each printer. Use the offset if labels print slightly off-center.")).small().weak());
            ui.horizontal(|ui| {
                ui.label(crate::i18n::tr("Horizontal"));
                ui.add(
                    egui::DragValue::new(&mut self.settings.offset_x_mm)
                        .range(-crate::settings::MAX_OFFSET_MM..=crate::settings::MAX_OFFSET_MM)
                        .speed(0.1)
                        .suffix(" mm"),
                )
                .accessible_name(crate::i18n::tr("Horizontal print offset in millimeters"));
                ui.label(crate::i18n::tr("Vertical"));
                ui.add(
                    egui::DragValue::new(&mut self.settings.offset_y_mm)
                        .range(-crate::settings::MAX_OFFSET_MM..=crate::settings::MAX_OFFSET_MM)
                        .speed(0.1)
                        .suffix(" mm"),
                )
                .accessible_name(crate::i18n::tr("Vertical print offset in millimeters"));
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.busy && !self.settings.printer.is_empty(), egui::Button::new(crate::i18n::tr("Print alignment test")))
                    .on_hover_text(crate::i18n::tr("Prints a ruler label with a 1 mm border so you can measure the offset"))
                    .clicked()
                {
                    self.print_alignment_test();
                }
                if ui
                    .add_enabled(
                        self.settings.offset_x_mm != 0. || self.settings.offset_y_mm != 0.,
                        egui::Button::new(crate::i18n::tr("Reset offset")),
                    )
                    .clicked()
                {
                    self.settings.offset_x_mm = 0.;
                    self.settings.offset_y_mm = 0.;
                }
            });
        });
    }
    fn print_overview(&mut self, ui: &mut egui::Ui) {
        Self::section_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Self::center_heading(ui, crate::i18n::tr("Label stock"), "Blank custom");
            ui.horizontal(|ui| {
                let (area, _) = ui.allocate_exact_size(Vec2::new(112., 138.), Sense::hover());
                let size = Vec2::new(
                    self.settings.width_mm.max(1.),
                    self.settings.height_mm.max(1.),
                );
                let scale = (88. / size.x).min(114. / size.y);
                let paper = Rect::from_center_size(area.center(), size * scale);
                ui.painter().rect_filled(paper, 4., Color32::WHITE);
                ui.painter().rect_stroke(
                    paper,
                    4.,
                    Stroke::new(1.5, Color32::from_rgb(128, 151, 145)),
                    StrokeKind::Inside,
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} × {} mm",
                            self.settings.width_mm, self.settings.height_mm
                        ))
                        .size(21.)
                        .strong(),
                    );
                    ui.label(format!(
                        "{} DPI · {} {}",
                        self.settings.dpi,
                        self.settings.copies,
                        if self.settings.copies == 1 {
                            "copy"
                        } else {
                            "copies"
                        }
                    ));
                    ui.label(
                        RichText::new(crate::i18n::tr("Output label proportions"))
                            .small()
                            .weak(),
                    );
                    ui.label(format!(
                        "Printer media: {}",
                        match self.settings.media.as_str() {
                            "" => "Not selected".into(),
                            "Custom" => "Custom — matches output".into(),
                            media => self
                                .printers
                                .iter()
                                .find(|p| p.name == self.settings.printer)
                                .map(|p| p.media_label(media))
                                .unwrap_or_else(|| media.into()),
                        }
                    ));
                });
            });
        });
        ui.add_space(12.);
        Self::section_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Self::center_heading(ui, crate::i18n::tr("Queue & diagnostics"), "Choose a label template");
            ui.horizontal(|ui| { ui.label(crate::i18n::tr("Printing service: CUPS")); crate::help::hint(ui, "CUPS"); });
            self.print_readiness(ui, false);
            ui.add_space(6.);
            ui.label(format!("{} imported labels queued", self.queue.len()));
            if crate::designer_ui::icon_action(ui, "Import label", "Open import queue", "Open imported labels for review", true, false).clicked() {
                self.area = Area::Import;
            }
            if crate::designer_ui::icon_action(ui, "Status", "Check CUPS job status", "Check jobs submitted to the selected printer", !self.settings.printer.is_empty() && !self.busy, false).clicked() {
                let p = self.settings.printer.clone();
                if !p.is_empty() {
                    self.task("Checking CUPS queue…", move || {
                        Ok(Message::Done(printing::job_status(&p)?))
                    });
                }
            }
            ui.label(RichText::new(crate::i18n::tr("A successful submission means CUPS accepted the job; check physical output and barcode readability.")).small().weak());

        });
    }
    #[allow(clippy::too_many_arguments)]
    fn home_card(
        ui: &mut egui::Ui,
        title: &str,
        subtitle: &str,
        description: &str,
        action: &str,
        icon: &str,
        accent: Color32,
    ) -> egui::Response {
        let (title, subtitle, description, action) = (
            crate::i18n::tr(title),
            crate::i18n::tr(subtitle),
            crate::i18n::tr(description),
            crate::i18n::tr(action),
        );
        let width = ui.available_width();
        Self::section_frame(ui)
            .inner_margin(22)
            .corner_radius(18)
            .stroke(Stroke::new(
                1.,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .show(ui, |ui| {
                ui.set_width((width - 46.).max(0.));
                ui.set_min_height(250.);
                let card_top = ui.cursor().top();
                let (badge, _) = ui.allocate_exact_size(Vec2::splat(48.), Sense::hover());
                ui.painter().rect_filled(
                    badge,
                    14.,
                    accent.gamma_multiply(if ui.visuals().dark_mode { 0.25 } else { 0.10 }),
                );
                crate::designer_ui::paint_tool_icon(ui.painter(), badge.center(), accent, icon);
                ui.add_space(8.);
                ui.heading(RichText::new(title).size(24.));
                ui.label(RichText::new(subtitle).strong());
                ui.add_space(2.);
                ui.label(RichText::new(description).weak());
                ui.add_space((card_top + 250. - ui.cursor().top() - 40.).max(12.));
                ui.add_sized(
                    [ui.available_width(), 40.],
                    egui::Button::new(RichText::new(action).strong().color(Color32::WHITE))
                        .fill(accent)
                        .corner_radius(10.),
                )
            })
            .inner
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
    fn import_step_heading(ui: &mut egui::Ui, title: &str, icon: &str) {
        let accent = if ui.visuals().dark_mode {
            Color32::from_rgb(88, 199, 173)
        } else {
            Color32::from_rgb(28, 111, 100)
        };
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.), Sense::hover());
            crate::designer_ui::paint_tool_icon(ui.painter(), rect.center(), accent, icon);
            ui.label(RichText::new(title).size(15.).strong().color(accent));
        });
    }
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new(crate::i18n::tr("WORKSPACE"))
                .size(12.)
                .strong()
                .color(Color32::from_rgb(36, 146, 126)),
        );
        ui.heading(RichText::new(crate::i18n::tr("Prepare your label")).size(24.));
        ui.label(
            RichText::new(crate::i18n::tr("OPEN  /  CHECK  /  PRINT"))
                .small()
                .weak(),
        );
        ui.add_space(6.);
        if crate::designer_ui::icon_action(
            ui,
            "Import label",
            "Add labels…",
            "Import PDF or image labels (Ctrl+O)",
            !self.busy,
            false,
        )
        .clicked()
        {
            self.open();
        }
        self.watch_controls(ui);
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
                if crate::designer_ui::icon_action(
                    ui,
                    "Previous",
                    "",
                    "View the previous label",
                    !self.busy && self.selected.is_some_and(|i| i > 0),
                    false,
                )
                .clicked()
                {
                    self.select(self.selected.unwrap() - 1);
                }
                if crate::designer_ui::icon_action(
                    ui,
                    "Next",
                    "",
                    "View the next label",
                    !self.busy && self.selected.is_some_and(|i| i + 1 < self.queue.len()),
                    false,
                )
                .clicked()
                {
                    self.select(self.selected.unwrap() + 1);
                }
            });
            let mut action = None;
            egui::CollapsingHeader::new("Queued labels")
                .default_open(false)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("import-queue-list")
                        .max_height(150.)
                        .show(ui, |ui| {
                            for (i, item) in self.queue.iter().enumerate() {
                                let name =
                                    item.path.file_name().unwrap_or_default().to_string_lossy();
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
                        });
                });
            if let Some(i) = action {
                self.select(i);
            }
            ui.horizontal(|ui| {
                if crate::designer_ui::icon_action(
                    ui,
                    "Move up",
                    "",
                    "Move label earlier in the print queue",
                    !self.busy && self.selected.is_some_and(|i| i > 0),
                    false,
                )
                .clicked()
                {
                    self.save_selection();
                    let i = self.selected.unwrap();
                    self.queue.swap(i, i - 1);
                    self.selected = Some(i - 1);
                }
                if crate::designer_ui::icon_action(
                    ui,
                    "Move down",
                    "",
                    "Move label later in the print queue",
                    !self.busy && self.selected.is_some_and(|i| i + 1 < self.queue.len()),
                    false,
                )
                .clicked()
                {
                    self.save_selection();
                    let i = self.selected.unwrap();
                    self.queue.swap(i, i + 1);
                    self.selected = Some(i + 1);
                }
                if crate::designer_ui::icon_action(
                    ui,
                    "Delete",
                    "",
                    "Remove this label from the import queue",
                    !self.busy && self.selected.is_some(),
                    false,
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
                if page.count>1 {let mut index=page.index+1;ui.horizontal(|ui|{ui.label(crate::i18n::tr("Page"));if ui.add(egui::DragValue::new(&mut index).range(1..=page.count)).changed() && let Some(i)=self.queue.iter().position(|q|q.path==page.path&&q.page==index-1){self.select(i);}ui.label(format!("of {}",page.count));});}
            }else{ui.label(crate::i18n::tr("Drop a downloaded label anywhere."));ui.label(RichText::new("PDF · PNG · JPEG · TIFF · BMP · WebP").small().weak()).on_hover_text(crate::help::tooltip("PDF · PNG · JPEG · TIFF · BMP · WebP"));}
            ui.add_space(12.);
            Self::section_frame(ui).show(ui, |ui| { ui.set_width(ui.available_width());
            Self::import_step_heading(ui, "01  EXTRACT", "Crop label");
            let old_mode=self.settings.mode;
            ui.horizontal_wrapped(|ui|{for (mode, icon, label, tip) in [
                (Mode::Automatic, "Auto extract", "Automatic", "Automatically detect the label region"),
                (Mode::Manual, "Crop label", "Manual", "Manually adjust the crop on the original document"),
                (Mode::FullPage, "Blank custom", "Full page", "Use the entire page without cropping"),
            ] { if crate::designer_ui::icon_action(ui, icon, label, tip, true, self.settings.mode == mode).clicked() { self.settings.mode = mode; } }});
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
            if self.settings.mode==Mode::Manual {ui.label(RichText::new(crate::i18n::tr("Drag to draw. Drag inside to move; drag a corner to resize. Crop coordinates refer to the original page.")).small());}
            if self.page.is_some()&&self.settings.mode!=Mode::FullPage {
                if crate::designer_ui::icon_action(ui, "Fit preview", "Reset to entire page", "Reset the crop to use the entire original page", true, false).clicked(){self.crop=Crop::FULL;self.dirty=true;self.reviewed=false;}
                ui.checkbox(&mut self.reviewed,crate::i18n::tr("I checked the crop and barcode edges"));
            }
            });
            ui.add_space(10.);
            Self::section_frame(ui).show(ui, |ui| { ui.set_width(ui.available_width());
            Self::import_step_heading(ui, "02  PAPER & LAYOUT", "Choose a label template");
            let before=serde_json::to_string(&self.settings).unwrap_or_default();
            egui::ComboBox::from_id_salt("paper").selected_text(format!("{:.1} × {:.1} mm",self.settings.width_mm,self.settings.height_mm)).width(ui.available_width()-12.).show_ui(ui,|ui| {
                for (name,w,h) in [("4 × 6 in",101.6,152.4),("4 × 4 in",101.6,101.6),("2 × 1 in",50.8,25.4),("100 × 150 mm",100.,150.)] {if ui.selectable_label(self.settings.width_mm==w&&self.settings.height_mm==h,name).clicked(){self.settings.width_mm=w;self.settings.height_mm=h;}}
            });
            ui.horizontal(|ui|{ui.label(crate::i18n::tr("W"));ui.add(egui::DragValue::new(&mut self.settings.width_mm).range(10.0..=500.0).speed(0.1).suffix(" mm"));ui.label(crate::i18n::tr("H"));ui.add(egui::DragValue::new(&mut self.settings.height_mm).range(10.0..=500.0).speed(0.1).suffix(" mm"));});
            if crate::designer_ui::icon_action(ui, "Rotate stock", "Swap portrait / landscape", "Swap label width and height", true, false).clicked(){std::mem::swap(&mut self.settings.width_mm,&mut self.settings.height_mm);}
            egui::CollapsingHeader::new("Advanced layout & resolution").show(ui, |ui| {
            ui.horizontal(|ui|{ui.label(crate::i18n::tr("Rotate"));egui::ComboBox::from_id_salt("rotation").selected_text(format!("{}°",self.settings.rotation)).show_ui(ui,|ui|{for r in [0,90,180,270]{ui.selectable_value(&mut self.settings.rotation,r,format!("{r}°"));}});ui.label(crate::i18n::tr("Rendering DPI")).on_hover_text(crate::help::explanation("DPI"));egui::ComboBox::from_id_salt("dpi").selected_text(self.settings.dpi.to_string()).width(60.).show_ui(ui,|ui|{for dpi in [203,300,600]{ui.selectable_value(&mut self.settings.dpi,dpi,dpi.to_string());}});});
            egui::ComboBox::from_id_salt("scale").selected_text(match self.settings.scale {Scale::Fit=>"Fit within paper",Scale::Actual=>"Actual size",Scale::Custom=>"Custom scale"}).show_ui(ui,|ui|{ui.selectable_value(&mut self.settings.scale,Scale::Fit,crate::i18n::tr("Fit within paper"));ui.selectable_value(&mut self.settings.scale,Scale::Actual,crate::i18n::tr("Actual size"));ui.selectable_value(&mut self.settings.scale,Scale::Custom,crate::i18n::tr("Custom scale"));});
            if self.settings.scale==Scale::Custom {ui.add(egui::Slider::new(&mut self.settings.scale_percent,10.0..=400.0).suffix("%"));}
            });
            egui::CollapsingHeader::new("Margins & image enhancement").show(ui,|ui|{
                ui.add(egui::DragValue::new(&mut self.settings.margin_mm).range(0.0..=20.0).speed(0.1).prefix("Margin ").suffix(" mm"));
                ui.checkbox(&mut self.settings.enhance,crate::i18n::tr("Black-and-white threshold (advanced)"));ui.label(RichText::new(crate::i18n::tr("Default processing preserves tones. Thresholding can damage fine barcode detail.")).small());
            });
            if before!=serde_json::to_string(&self.settings).unwrap_or_default(){self.dirty=true;}
            });
            ui.add_space(10.);
            Self::section_frame(ui).show(ui, |ui| { ui.set_width(ui.available_width());
            Self::import_step_heading(ui, "03  PRINT & EXPORT", "Print device");
            ui.horizontal(|ui| { ui.label(crate::i18n::tr("Printing service: CUPS")); crate::help::hint(ui, "CUPS"); });
            let previous=self.settings.printer.clone();
            egui::ComboBox::from_id_salt("printer").selected_text(if previous.is_empty(){"Choose a printer"}else{&previous}).width(ui.available_width()-12.).show_ui(ui,|ui|{for p in &self.printers{ui.selectable_value(&mut self.settings.printer,p.name.clone(),format!("{}{}",p.name,if p.is_default{" (default)"}else{""}));}});
            if previous!=self.settings.printer {self.settings.media.clear();self.settings.resolution.clear();self.select_default_media();}
            if let Some(p)=self.printers.iter().find(|p|p.name==self.settings.printer).cloned(){
                ui.label(RichText::new(&p.description).small().weak());
                ui.label(RichText::new(printing::thermal_hint(&p)).small().weak());
                egui::ComboBox::from_id_salt("media").selected_text(p.media_label(&self.settings.media)).width(ui.available_width()-12.).show_ui(ui,|ui|{if p.custom_media {ui.selectable_value(&mut self.settings.media,"Custom".into(),crate::i18n::tr("Custom — match output dimensions"));}for m in &p.media {ui.selectable_value(&mut self.settings.media,m.clone(),p.media_label(m)).on_hover_text(format!("Driver code: {m}"));}});
                ui.label(RichText::new(crate::i18n::tr("Driver media must match the paper dimensions above. Check your printer's unprintable margins.")).small());
                if !p.resolutions.is_empty(){ui.collapsing(crate::i18n::tr("Advanced driver resolution"), |ui| {egui::ComboBox::from_id_salt("resolution").selected_text(if self.settings.resolution.is_empty(){"Driver default resolution"}else{&self.settings.resolution}).show_ui(ui,|ui|{ui.selectable_value(&mut self.settings.resolution,String::new(),crate::i18n::tr("Driver default resolution"));for r in &p.resolutions{ui.selectable_value(&mut self.settings.resolution,r.clone(),r);}});});}
            }
            ui.horizontal(|ui|{ui.label(crate::i18n::tr("Copies"));ui.add(egui::DragValue::new(&mut self.settings.copies).range(1..=999));if crate::designer_ui::icon_action(ui, "Refresh", "", "Refresh available CUPS printers", !self.busy, false).clicked(){self.refresh_printers();}});
            self.print_readiness(ui, true);
            if ui.add_enabled(self.ready()&&!self.settings.printer.is_empty()&&!self.settings.media.is_empty(),egui::Button::new(RichText::new(crate::i18n::tr("Print label   Ctrl+P")).color(Color32::WHITE)).fill(Color32::from_rgb(28,111,100)).min_size(Vec2::new(ui.available_width(),42.))).clicked(){self.print();}
            ui.horizontal(|ui|{if crate::designer_ui::icon_action(ui, "Export document", "PDF", "Export the prepared label as PDF", self.ready(), false).clicked(){self.export(true);}
                if crate::designer_ui::icon_action(ui, "Add image / logo...", "PNG", "Export the prepared label as PNG", self.ready(), false).clicked(){self.export(false);}});
            if crate::designer_ui::icon_action(ui, "Status", "Job status", "Check jobs submitted to the selected CUPS printer", !self.settings.printer.is_empty() && !self.busy, false).clicked(){let p=self.settings.printer.clone();self.task("Checking CUPS queue…",move||Ok(Message::Done(printing::job_status(&p)?)));}
            });
        });
    }
    fn preview(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            Self::import_step_heading(ui, "Preview", "Preview");
            if crate::designer_ui::icon_action(
                ui,
                "Crop label",
                "Original / crop",
                "View the original document and adjust the crop",
                true,
                self.original,
            )
            .clicked()
            {
                self.original = true;
            }
            if crate::designer_ui::icon_action(
                ui,
                "Blank custom",
                "Thermal label",
                "Preview the final label as it will print",
                true,
                !self.original,
            )
            .clicked()
            {
                self.original = false;
            }
            if crate::designer_ui::icon_action(
                ui,
                "Zoom out",
                "",
                "Zoom out",
                self.zoom > 0.25,
                false,
            )
            .clicked()
            {
                self.zoom = (self.zoom / 1.25).max(0.25);
            }
            if crate::designer_ui::icon_action(ui, "Zoom in", "", "Zoom in", self.zoom < 6., false)
                .clicked()
            {
                self.zoom = (self.zoom * 1.25).min(6.);
            }
            if crate::designer_ui::icon_action(
                ui,
                "Fit preview",
                "",
                "Fit label to the preview area",
                true,
                false,
            )
            .clicked()
            {
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
                ui.label(
                    RichText::new(crate::i18n::tr("Labels, ready to go."))
                        .size(28.)
                        .strong(),
                );
                ui.add_space(12.);
                ui.label(
                    RichText::new(crate::i18n::tr(
                        "Drop PDFs or images to build a print queue",
                    ))
                    .size(16.),
                );
                ui.label(
                    RichText::new(crate::i18n::tr(
                        "From any marketplace or carrier. No account needed.",
                    ))
                    .weak(),
                );
                ui.add_space(20.);
                if crate::designer_ui::icon_action(
                    ui,
                    "Import label",
                    "Add labels…",
                    "Import PDF or image labels (Ctrl+O)",
                    !self.busy,
                    false,
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
                RichText::new(crate::i18n::tr(
                    "Check the original crop, then confirm it in the left panel.",
                ))
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
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.capture.is_none() {
            self.designer.protect_window_close(ctx);
        }
    }

    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        if self.capture.is_some() {
            // Documentation capture is unattended; ignore incidental pointer/scroll input.
            input.events.clear();
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.watch_tick(&ctx);
        if let Some(mut capture) = self.capture.take() {
            capture.tick(self, &ctx);
            self.capture = Some(capture);
        }
        self.select_default_media();
        if !self.busy
            && let Some(path) = self.startup_file.take()
        {
            self.open_path(path);
        }
        if !self.busy {
            let dropped = ctx.input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .map(|f| f.path().to_path_buf())
                    .collect::<Vec<_>>()
            });
            if dropped.len() == 1 && design::is_design_path(&dropped[0]) {
                self.open_path(dropped[0].clone());
            } else if !dropped.is_empty() {
                self.area = Area::Import;
                self.append_files(dropped);
            }
            if !self.about_open
                && !self.license_open
                && !self.terms_open
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O))
            {
                self.area = Area::Import;
                self.open();
            }
            if !self.about_open
                && !self.license_open
                && !self.terms_open
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::P))
            {
                self.print();
            }
        }
        // F1 works everywhere, including while a dialog is open.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.open_manual(&ctx);
        }
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                window_chrome::title(ui, &self.icon_texture);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    window_chrome::controls(ui);
                    let old = self.settings.theme;
                    let menu = ui
                        .add(egui::Button::new("").min_size(Vec2::splat(38.)))
                        .on_hover_text(crate::i18n::tr("Main menu"));
                    menu.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, "Main menu")
                    });
                    for offset in [-6., 0., 6.] {
                        let center = menu.rect.center() + Vec2::new(0., offset);
                        ui.painter().line_segment(
                            [center - Vec2::new(8., 0.), center + Vec2::new(8., 0.)],
                            Stroke::new(1.8, ui.style().interact(&menu).fg_stroke.color),
                        );
                    }
                    egui::Popup::menu(&menu).show(|ui| {
                        ui.set_min_width(164.);
                        ui.label(RichText::new(crate::i18n::tr("Appearance")).small().weak());
                        ui.horizontal(|ui| {
                            for (theme, label) in [
                                (ThemeChoice::System, "System"),
                                (ThemeChoice::Light, "Light"),
                                (ThemeChoice::Dark, "Dark"),
                            ] {
                                let selected = self.settings.theme == theme;
                                let response = ui
                                    .add(
                                        egui::Button::new("")
                                            .min_size(Vec2::new(44., 36.))
                                            .corner_radius(8.)
                                            .selected(selected),
                                    )
                                    .on_hover_text(format!("{label} theme"));
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::Role::Button,
                                        ui.is_enabled(),
                                        selected,
                                        format!("{label} theme"),
                                    )
                                });
                                let color = if selected {
                                    ui.visuals().selection.stroke.color
                                } else {
                                    ui.style().interact(&response).fg_stroke.color
                                };
                                crate::designer_ui::paint_tool_icon(
                                    ui.painter(),
                                    response.rect.center(),
                                    color,
                                    label,
                                );
                                if response.clicked() {
                                    self.settings.theme = theme;
                                }
                            }
                        });
                        ui.separator();
                        if ui.button(crate::i18n::tr("User manual")).clicked() {
                            self.open_manual(&ctx);
                            ui.close();
                        }
                        if ui.button(crate::i18n::tr("Printing terms")).clicked() {
                            self.terms_open = true;
                            ui.close();
                        }
                        if ui.button(crate::i18n::tr("About ThermalPrint")).clicked() {
                            self.about_open = true;
                            ui.close();
                        }
                        if ui.button(crate::i18n::tr("Quit")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    if self.settings.theme != old {
                        ctx.set_theme(match self.settings.theme {
                            ThemeChoice::System => egui::ThemePreference::System,
                            ThemeChoice::Light => egui::ThemePreference::Light,
                            ThemeChoice::Dark => egui::ThemePreference::Dark,
                        });
                        self.persist();
                    }
                    // In a right-to-left row, this button lands just left of the main menu.
                    let help = ui
                        .add(
                            egui::Button::new(RichText::new("?").size(18.).strong())
                                .min_size(Vec2::splat(38.)),
                        )
                        .on_hover_text(crate::i18n::tr(
                            "Open the user manual in your browser (F1)",
                        ));
                    help.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, true, "Help")
                    });
                    if help.clicked() {
                        self.open_manual(&ctx);
                    }
                });
            });
        });
        if self.area != Area::Designer || !self.designer.focus_canvas {
            egui::Panel::left("workspace-navigation")
                .default_size(if self.settings.navigation_collapsed {
                    60.
                } else {
                    200.
                })
                .exact_size(if self.settings.navigation_collapsed {
                    60.
                } else {
                    200.
                })
                .resizable(false)
                .show(ui, |ui| {
                    ui.add_space(12.);
                    let action = if self.settings.navigation_collapsed {
                        "Expand navigation"
                    } else {
                        "Collapse navigation"
                    };
                    let toggle = ui
                        .add(
                            egui::Button::new("")
                                .frame(false)
                                .min_size(Vec2::splat(36.)),
                        )
                        .accessible_name(action)
                        .on_hover_text(action);
                    toggle.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), action)
                    });
                    if toggle.hovered() || toggle.has_focus() {
                        ui.painter().rect_stroke(
                            toggle.rect.shrink(1.),
                            8.,
                            Stroke::new(
                                2.,
                                if ui.visuals().dark_mode {
                                    Color32::from_rgb(140, 185, 255)
                                } else {
                                    ui.visuals().selection.bg_fill
                                },
                            ),
                            StrokeKind::Inside,
                        );
                    }
                    crate::designer_ui::paint_tool_icon(
                        ui.painter(),
                        toggle.rect.center(),
                        ui.visuals().text_color(),
                        action,
                    );
                    if toggle.clicked() {
                        self.settings.navigation_collapsed = !self.settings.navigation_collapsed;
                        self.persist();
                    }
                    ui.add_space(8.);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.;
                        for (area, icon, label, tip) in [
                            (
                                Area::Home,
                                "Home",
                                "Home",
                                "Open your workspace and label templates",
                            ),
                            (
                                Area::Designer,
                                "Design label",
                                "Label Designer",
                                "Create and edit a custom label",
                            ),
                            (
                                Area::Import,
                                "Import label",
                                "Import & Convert",
                                "Import, crop, and prepare existing labels",
                            ),
                            (
                                Area::PrintCenter,
                                "Print device",
                                "Print Center",
                                "Configure printer, stock, and queue settings",
                            ),
                        ] {
                            if navigation::item(
                                ui,
                                icon,
                                if self.settings.navigation_collapsed {
                                    ""
                                } else {
                                    label
                                },
                                tip,
                                self.area == area,
                            )
                            .clicked()
                            {
                                self.area = area;
                            }
                        }
                    });
                });
        }
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
                    .default_size(420.)
                    .size_range(330.0..=480.0)
                    .resizable(true)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.;
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
        if self.terms_open {
            egui::Window::new(crate::i18n::tr("Printing terms"))
                .id(egui::Id::unique("printing-terms-window"))
                .default_width(600.)
                .default_height(580.)
                .max_width((ctx.content_rect().width() - 48.).max(260.))
                .max_height((ctx.content_rect().height() - 48.).max(260.))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(true)
                .open(&mut self.terms_open)
                .show(&ctx, |ui| crate::help::glossary(ui, &mut self.terms_search));
        }
        if self.about_open {
            let (close, license, _) = about::show(&ctx, &self.icon_texture, &self.app_icons);
            if license {
                self.license_open = true;
            }
            if close && !self.license_open {
                self.about_open = false;
            }
        }
        if self.license_open {
            let response =
                egui::Modal::new(egui::Id::unique("thermalprint-license")).show(&ctx, |ui| {
                    ui.set_width((ctx.content_rect().width() - 72.).clamp(260., 480.));
                    ui.horizontal(|ui| {
                        ui.heading(crate::i18n::tr("MIT License"));
                        if ui.button(crate::i18n::tr("Back")).clicked() {
                            ui.close();
                        }
                    });
                    egui::ScrollArea::vertical()
                        .max_height((ctx.content_rect().height() - 160.).max(200.))
                        .show(ui, |ui| {
                            ui.label(include_str!("../LICENSE"));
                        });
                });
            if response.should_close() {
                self.license_open = false;
            }
        }
        self.designer.confirm_pending(&ctx);
        window_chrome::resize_edges(&ctx);
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
