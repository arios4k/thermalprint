//! Guided setup and review actions share the existing Print Center and print pipeline.
use super::*;

impl ThermalPrint {
    pub(super) fn show_window(&mut self, ctx: &egui::Context) {
        self.background_startup.cancel();
        self.background_hidden = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    pub(super) fn maintain_tray(&mut self, ctx: &egui::Context) {
        if !self.settings.background_printing {
            self.tray = None;
            self.tray_ready = false;
            self.background_startup.cancel();
            return;
        }
        if self.last_tray_check.elapsed() >= Duration::from_secs(1) {
            self.last_tray_check = Instant::now();
            let was_ready = self.tray_ready;
            if self.tray.is_some() && !crate::background::tray_available() {
                self.tray = None;
            }
            let result = (|| -> anyhow::Result<()> {
                if self.tray.is_none() {
                    self.tray = Some(crate::background::Tray::new()?);
                }
                self.tray
                    .as_ref()
                    .expect("tray was created")
                    .status(self.auto_paused, self.settings.auto_print)
            })();
            self.tray_ready = result.is_ok();
            match result {
                Ok(()) => {
                    if !was_ready {
                        tracing::info!("Desktop tray icon is ready");
                    }
                    if self.tray_error.take().is_some()
                        && self
                            .status
                            .starts_with("Background printing needs a tray icon:")
                    {
                        self.status = crate::i18n::tr("Tray icon restored").into();
                        self.error = false;
                    }
                }
                Err(error) => {
                    self.tray = None;
                    self.tray_error = Some(format!("{error:#}"));
                    if !self.background_startup.waiting() {
                        self.status = format!("Background printing needs a tray icon: {error:#}");
                        self.error = true;
                    }
                }
            }
            if !self.tray_ready && self.background_hidden {
                self.show_window(ctx);
            }
        }
        match self
            .background_startup
            .poll(self.tray_ready, ctx.cumulative_frame_nr() > 0)
        {
            crate::background::StartupAction::Hide => {
                tracing::info!("Background startup: hiding the window after tray initialization");
                self.background_hidden = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
            crate::background::StartupAction::Show => {
                tracing::warn!(
                    "Background startup: desktop tray did not become ready within 30 seconds"
                );
                self.show_window(ctx);
                self.status = format!(
                    "Background printing needs a tray icon: {}",
                    self.tray_error
                        .as_deref()
                        .unwrap_or("The desktop status tray is not ready.")
                );
                self.error = true;
            }
            crate::background::StartupAction::Wait => {}
        }
        // Get past eframe's first-show handling before applying the startup hide.
        if self.background_startup.waiting() && ctx.cumulative_frame_nr() == 0 {
            ctx.request_repaint();
        }
    }

    pub(super) fn background_controls(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(crate::i18n::tr("Background printing")).default_open(true).show(ui, |ui| {
            let mut enabled = self.settings.background_printing;
            let label = crate::i18n::tr("Keep running when the window is closed");
            if ui.checkbox(&mut enabled, label).changed() {
                if enabled {
                    match crate::background::Tray::new() {
                        Ok(tray) => {
                            self.tray_ready = tray.status(self.auto_paused, self.settings.auto_print).is_ok();
                            self.tray = Some(tray);
                            self.settings.background_printing = true;
                            self.persist();
                        }
                        Err(error) => {
                            self.status = format!("Cannot create a tray icon. Enable your desktop's tray support and try again: {error:#}");
                            self.error = true;
                        }
                    }
                } else {
                    match crate::background::set_autostart(false) {
                        Ok(()) => {
                            self.settings.background_printing = false;
                            self.settings.start_at_login = false;
                            self.tray = None;
                            self.tray_ready = false;
                            self.persist();
                        }
                        Err(error) => {
                            self.status = format!("Cannot disable startup: {error:#}");
                            self.error = true;
                        }
                    }
                }
            }
            ui.label(crate::i18n::tr("The tray menu opens ThermalPrint, pauses automatic printing, or quits. Your computer must be awake."));
            let mut startup = self.settings.start_at_login;
            let startup_toggle = egui::Checkbox::new(
                &mut startup,
                crate::i18n::tr("Start at login"),
            );
            if ui.add_enabled(self.settings.background_printing, startup_toggle).changed() {
                match crate::background::set_autostart(startup) {
                    Ok(()) => {
                        self.settings.start_at_login = startup;
                        self.persist();
                    }
                    Err(error) => {
                        self.status = format!("Cannot update startup: {error:#}");
                        self.error = true;
                    }
                }
            }
            ui.label(crate::i18n::tr("Automatic printing checks file contents to prevent repeat prints, even after a file is renamed."));
            if crate::designer_ui::icon_button_enabled(ui, !self.busy, "Delete", crate::i18n::tr("Clear duplicate protection…")).clicked() {
                self.duplicate_clear = true;
            }
        });
        if !self.duplicate_clear {
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("clear-automatic-duplicates"))
            .show(ui.ctx(), |ui| {
                ui.set_max_width(440.);
                ui.heading(crate::i18n::tr("Clear duplicate protection?"));
                ui.label(crate::i18n::tr("Previously printed files can print automatically again if they arrive in the watch folder."));
                ui.horizontal(|ui| {
                    if crate::designer_ui::icon_button(ui, "Close", crate::i18n::tr("Cancel")).clicked() {
                        self.duplicate_clear = false;
                    }
                    if crate::designer_ui::icon_button(ui, "Delete", crate::i18n::tr("Clear duplicate protection")).clicked() {
                        match crate::duplicates::clear() {
                            Ok(()) => {
                                self.duplicate_clear = false;
                                self.notices.push("Duplicate protection cleared", false, None);
                            }
                            Err(error) => {
                                self.status = format!("{error:#}");
                                self.error = true;
                            }
                        }
                    }
                });
            });
        if response.should_close() {
            self.duplicate_clear = false;
        }
    }

    /// The setup assistant: four short steps, each with its own controls, so nothing has to
    /// be found elsewhere on the page.
    pub(super) fn stock_setup(&mut self, ui: &mut egui::Ui) {
        use crate::i18n::tr;
        const STEPS: [(&str, &str); 4] = [
            ("Printer", "Choose the printer you will print on."),
            (
                "Paper or label size",
                "Choose the size loaded in the printer. Label printers list their label sizes; office printers list every paper size they support, such as Letter, Legal, or A4.",
            ),
            (
                "Alignment",
                "Optional. Print one test page and compare its 1 mm border to the edges. If it is off, move it with the offsets.",
            ),
            (
                "Save",
                "Name this setup so you can switch back to it in one click from Label stock.",
            ),
        ];
        Self::section_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Self::center_heading(ui, tr("Printer setup assistant"), "Status");
            let Some(step) = self.setup_step else {
                ui.label(tr("Choose a printer and paper size, check alignment, and save it all as a stock profile you can load later."));
                if crate::designer_ui::icon_button(ui, "Settings", tr("Set up printer and label stock")).clicked() {
                    self.start_stock_setup();
                }
                self.media_hints(ui);
                return;
            };
            if step >= STEPS.len() {
                // The finish: say plainly what was saved and that it is now in use.
                let saved = self.stock_saved.clone().and_then(|key| {
                    let stock = self.settings.stock_profiles.get(&key)?.clone();
                    Some((key, stock))
                });
                ui.label(
                    RichText::new(format!("✔ {}", tr("Stock profile saved")))
                        .size(18.)
                        .strong()
                        .color(theme::accent(ui.visuals())),
                );
                if let Some((key, stock)) = saved {
                    ui.label(RichText::new(crate::settings::stock_display_name(&key)).strong());
                    ui.label(RichText::new(self.stock_summary(&stock)).weak());
                }
                ui.label(tr("It is now the active profile. Switch profiles any time from Active profile at the top of Print Center."));
                ui.add_space(6.);
                ui.horizontal_wrapped(|ui| {
                    if crate::designer_ui::icon_button(ui, "Status", tr("Finish")).clicked() {
                        self.setup_step = None;
                    }
                    if crate::designer_ui::icon_button(ui, "Settings", tr("Set up another")).clicked() {
                        self.start_stock_setup();
                    }
                });
                return;
            }
            ui.horizontal_wrapped(|ui| {
                for (i, (title, _)) in STEPS.iter().enumerate() {
                    let text = RichText::new(format!("{} {}", i + 1, tr(title)));
                    // The selected chip has its own highlight; later steps are dimmed.
                    let text = if i == step {
                        text.strong()
                    } else if i < step {
                        text
                    } else {
                        text.weak()
                    };
                    if ui.selectable_label(i == step, text).clicked()
                        && (i <= step || !self.settings.printer.is_empty())
                    {
                        self.setup_step = Some(i);
                    }
                }
            });
            ui.add_space(4.);
            ui.label(RichText::new(format!("{} {} / {} · {}",
                tr("Step"),
                step + 1,
                STEPS.len(),
                tr(STEPS[step].0)
            )).strong());
            ui.label(tr(STEPS[step].1));
            ui.add_space(6.);
            let mut ready = true;
            match step {
                0 => {
                    self.printer_picker(ui, "setup-printer");
                    ready = !self.settings.printer.is_empty();
                }
                1 => {
                    self.media_picker(ui, "setup-media");
                    self.size_controls(ui, "setup");
                    ready = !self.settings.media.is_empty();
                }
                2 => {
                    if crate::designer_ui::icon_button_enabled(ui, !self.busy, "Print device", tr("Print one test label")).clicked() {
                        self.print_alignment_test();
                    }
                    self.offset_controls(ui);
                }
                _ => {
                    if self.save_stock_form(ui) {
                        self.setup_step = Some(STEPS.len());
                    }
                }
            }
            ui.add_space(6.);
            ui.horizontal_wrapped(|ui| {
                if step > 0 && crate::designer_ui::icon_button(ui, "Back", tr("Back")).clicked() {
                    self.setup_step = Some(step - 1);
                }
                if step + 1 < STEPS.len()
                    && crate::designer_ui::icon_button_enabled(ui, ready, "Next", tr("Next")).clicked()
                {
                    self.setup_step = Some(step + 1);
                }
                if crate::designer_ui::icon_button(ui, "Close", tr("Close assistant")).clicked() {
                    self.setup_step = None;
                }
            });
        });
    }

    /// Start the assistant for a new profile, with an empty name.
    pub(super) fn start_stock_setup(&mut self) {
        self.stock_name.clear();
        self.stock_saved = None;
        self.setup_step = Some(0);
    }

    /// The active stock profile, and whether the settings changed since it was loaded or saved.
    pub(super) fn active_stock(&self) -> Option<(String, bool)> {
        let key = self.settings.active_stock.clone()?;
        let stock = self.settings.stock_profiles.get(&key)?;
        let changed = stock.printer != self.settings.printer
            || stock.settings != self.settings.current_profile();
        Some((key, changed))
    }

    /// Switch to a saved profile: its printer, paper, resolution, and alignment.
    pub(super) fn load_stock(&mut self, key: &str) {
        let Some(stock) = self.settings.stock_profiles.get(key).cloned() else {
            return;
        };
        if stock.printer != self.settings.printer {
            self.select_printer(&stock.printer);
        }
        self.settings.apply_profile(&stock.settings);
        self.settings.active_stock = Some(key.to_owned());
        self.stock_saved = None;
        // The open design follows the profile's label size too; imported labels re-render
        // at the new size because the output settings changed.
        self.designer
            .match_paper(stock.settings.width_mm, stock.settings.height_mm);
        self.dirty = true;
        self.persist();
        self.notices.push(
            format!(
                "{} {}",
                crate::i18n::tr("Active profile:"),
                crate::settings::stock_display_name(key)
            ),
            false,
            None,
        );
    }

    /// Save the current settings into the active profile.
    pub(super) fn update_active_stock(&mut self) {
        let Some((key, _)) = self.active_stock() else {
            return;
        };
        let stock = crate::settings::StockProfile {
            printer: self.settings.printer.clone(),
            settings: self.settings.current_profile(),
        };
        // A profile belongs to its printer; one moved to another printer gets a new key.
        let new_key =
            crate::settings::stock_key(&stock.printer, crate::settings::stock_display_name(&key));
        self.settings.stock_profiles.remove(&key);
        self.settings.stock_profiles.insert(new_key.clone(), stock);
        self.settings.active_stock = Some(new_key);
        self.persist();
        self.notices
            .push(crate::i18n::tr("Stock profile updated"), false, None);
    }

    /// A warning when the output size and the printer's paper do not match, with the fix.
    fn media_hints(&mut self, ui: &mut egui::Ui) {
        let Some(printer) = self.selected_printer().cloned() else {
            return;
        };
        if let Some(reason) = printer.media_mismatch(
            &self.settings.media,
            self.settings.width_mm,
            self.settings.height_mm,
        ) {
            ui.colored_label(ui.visuals().warn_fg_color, reason);
            if crate::designer_ui::icon_button(ui, "Ruler", crate::i18n::tr("Use the paper size"))
                .clicked()
                && let Ok((width, height)) = printer.selected_media_size(&self.settings)
            {
                self.settings.width_mm = width;
                self.settings.height_mm = height;
                self.dirty = true;
            }
        }
    }

    pub(super) fn selected_printer(&self) -> Option<&Printer> {
        self.printers
            .iter()
            .find(|p| p.name == self.settings.printer)
    }

    /// Switch printers. Each printer keeps its own paper, resolution, size, and alignment.
    pub(super) fn select_printer(&mut self, name: &str) {
        if !self.settings.switch_printer(name) {
            self.settings.media.clear();
            self.settings.resolution.clear();
        }
        self.select_default_media();
        self.dirty = true;
    }

    /// The printer chooser, with a note when the printer may not print.
    pub(super) fn printer_picker(&mut self, ui: &mut egui::Ui, id: &str) {
        use crate::i18n::tr;
        let mut chosen = self.settings.printer.clone();
        egui::ComboBox::from_id_salt(id)
            .selected_text(if chosen.is_empty() {
                tr("Choose printer").to_owned()
            } else {
                chosen.clone()
            })
            .show_ui(ui, |ui| {
                for p in &self.printers {
                    ui.selectable_value(&mut chosen, p.name.clone(), &p.name);
                }
            });
        if chosen != self.settings.printer {
            self.select_printer(&chosen);
        }
        if self.finding_printers {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr("Looking for printers…"));
            });
        } else if self.printers.is_empty() {
            ui.label(
                RichText::new(tr(
                    "No printers found. Add a printer in your system settings, then refresh.",
                ))
                .weak(),
            );
        }
        if let Some(p) = self.selected_printer() {
            ui.label(crate::i18n::tr(printing::thermal_hint(p)));
            if let Some(problem) = &p.problem {
                ui.colored_label(ui.visuals().warn_fg_color, problem);
            }
        }
        if crate::designer_ui::icon_button_enabled(
            ui,
            !self.finding_printers,
            "Refresh",
            tr("Refresh printers"),
        )
        .clicked()
        {
            self.refresh_printers();
        }
    }

    /// Every paper or label size the selected printer supports. Choosing one whose size is
    /// known also sets the output width and height, so they always match.
    pub(super) fn media_picker(&mut self, ui: &mut egui::Ui, id: &str) {
        use crate::i18n::tr;
        let Some(p) = self.selected_printer().cloned() else {
            ui.label(tr("Choose a printer first."));
            return;
        };
        let mut media = self.settings.media.clone();
        ui.label(tr("Paper or label size"));
        egui::ComboBox::from_id_salt(id)
            .selected_text(p.media_label(&media))
            .width(ui.available_width().min(360.))
            .show_ui(ui, |ui| {
                if p.custom_media {
                    ui.selectable_value(&mut media, "Custom".into(), tr("Custom dimensions"));
                }
                for m in &p.media {
                    ui.selectable_value(&mut media, m.clone(), p.media_label(m))
                        .on_hover_text(format!("Driver code: {m}"));
                }
            });
        if media != self.settings.media {
            self.settings.media = media;
            if self.settings.media != "Custom"
                && let Ok((width, height)) = p.selected_media_size(&self.settings)
            {
                self.settings.width_mm = width;
                self.settings.height_mm = height;
            }
            self.dirty = true;
        }
        if p.media.is_empty() && !p.custom_media {
            ui.label(tr(
                "This printer does not list its paper sizes. Configure them in its driver.",
            ));
        }
    }

    /// The sizes to offer for the output: every size the selected printer supports, then
    /// common label sizes it does not list.
    pub(super) fn paper_presets(&self) -> Vec<(String, f32, f32)> {
        let mut out: Vec<(String, f32, f32)> = Vec::new();
        if let Some(p) = self.selected_printer() {
            for code in &p.media {
                if let Some(&(w, h)) = p.media_sizes.get(code) {
                    out.push((p.media_label(code), w, h));
                }
            }
        }
        for (name, w, h) in [
            ("4 × 6 in", 101.6, 152.4),
            ("4 × 4 in", 101.6, 101.6),
            ("2 × 1 in", 50.8, 25.4),
            ("100 × 150 mm", 100., 150.),
            ("Letter", 215.9, 279.4),
            ("Legal", 215.9, 355.6),
            ("A4", 210., 297.),
        ] {
            if !out
                .iter()
                .any(|&(_, ow, oh)| (ow - w).abs() < 0.5 && (oh - h).abs() < 0.5)
            {
                out.push((name.to_owned(), w, h));
            }
        }
        out
    }

    /// Choose the output size from the printer's sizes. When the printer has that paper,
    /// its media is selected too, so the two always match.
    pub(super) fn paper_size_picker(&mut self, ui: &mut egui::Ui, id: &str) {
        let presets = self.paper_presets();
        let mut picked = None;
        egui::ComboBox::from_id_salt(id)
            .selected_text(format!(
                "{:.1} × {:.1} mm",
                self.settings.width_mm, self.settings.height_mm
            ))
            .width(ui.available_width().min(360.))
            .height(400.)
            .show_ui(ui, |ui| {
                for (name, w, h) in &presets {
                    let on = (self.settings.width_mm - w).abs() < 0.05
                        && (self.settings.height_mm - h).abs() < 0.05;
                    if ui.selectable_label(on, name).clicked() {
                        picked = Some((*w, *h));
                    }
                }
            });
        if let Some((w, h)) = picked {
            self.settings.width_mm = w;
            self.settings.height_mm = h;
            self.select_default_media();
            self.dirty = true;
        }
    }

    /// The active profile, always in view where printing is set up: switch to another in one
    /// step, or save changes back into it.
    pub(super) fn stock_quick_switch(&mut self, ui: &mut egui::Ui, id: &str) {
        use crate::i18n::tr;
        if self.settings.stock_profiles.is_empty() {
            return;
        }
        let active = self.active_stock();
        let mut stocks: Vec<_> = self
            .settings
            .stock_profiles
            .iter()
            .map(|(k, s)| (k.clone(), s.printer.clone()))
            .collect();
        stocks.sort_by_key(|(k, printer)| (printer.clone(), k.to_lowercase()));
        let mut load = None;
        let mut update = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr("Active profile:")).strong());
            egui::ComboBox::from_id_salt(id)
                .selected_text(active.as_ref().map_or_else(
                    || tr("None").to_owned(),
                    |(k, _)| crate::settings::stock_display_name(k).to_owned(),
                ))
                .width(ui.available_width().min(280.))
                .show_ui(ui, |ui| {
                    for (key, printer) in &stocks {
                        let text =
                            format!("{} — {printer}", crate::settings::stock_display_name(key));
                        let on = active.as_ref().is_some_and(|(k, _)| k == key);
                        if ui.selectable_label(on, text).clicked() && !on {
                            load = Some(key.clone());
                        }
                    }
                });
            if let Some((_, true)) = &active {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    tr("Changed since it was loaded"),
                );
                update = crate::designer_ui::icon_button(ui, "Save document", tr("Update profile"))
                    .clicked();
            }
        });
        if let Some(key) = load {
            self.load_stock(&key);
        }
        if update {
            self.update_active_stock();
        }
    }

    /// Output width, height, orientation, and DPI.
    pub(super) fn size_controls(&mut self, ui: &mut egui::Ui, id: &str) {
        use crate::i18n::tr;
        let mut size_changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Width"));
            size_changed |= ui
                .add(
                    egui::DragValue::new(&mut self.settings.width_mm)
                        .range(10.0..=500.0)
                        .suffix(" mm"),
                )
                .changed();
            ui.label(tr("Height"));
            size_changed |= ui
                .add(
                    egui::DragValue::new(&mut self.settings.height_mm)
                        .range(10.0..=500.0)
                        .suffix(" mm"),
                )
                .changed();
            if crate::designer_ui::icon_action(
                ui,
                "Rotate stock",
                "",
                "Swap label width and height",
                true,
                false,
            )
            .clicked()
            {
                std::mem::swap(&mut self.settings.width_mm, &mut self.settings.height_mm);
                size_changed = true;
            }
        });
        if size_changed {
            self.select_default_media();
            self.dirty = true;
        }
        ui.horizontal(|ui| {
            ui.label(tr("Rendering DPI"))
                .on_hover_text(crate::help::explanation("DPI"));
            egui::ComboBox::from_id_salt(format!("{id}-dpi"))
                .selected_text(self.settings.dpi.to_string())
                .show_ui(ui, |ui| {
                    for dpi in [203, 300, 600] {
                        ui.selectable_value(&mut self.settings.dpi, dpi, dpi.to_string());
                    }
                });
        });
        self.media_hints(ui);
    }

    /// Horizontal and vertical print offsets.
    pub(super) fn offset_controls(&mut self, ui: &mut egui::Ui) {
        use crate::i18n::tr;
        ui.horizontal(|ui| {
            ui.label(tr("Horizontal"));
            ui.add(
                egui::DragValue::new(&mut self.settings.offset_x_mm)
                    .range(-crate::settings::MAX_OFFSET_MM..=crate::settings::MAX_OFFSET_MM)
                    .speed(0.1)
                    .suffix(" mm"),
            )
            .accessible_name(tr("Horizontal print offset in millimeters"));
            ui.label(tr("Vertical"));
            ui.add(
                egui::DragValue::new(&mut self.settings.offset_y_mm)
                    .range(-crate::settings::MAX_OFFSET_MM..=crate::settings::MAX_OFFSET_MM)
                    .speed(0.1)
                    .suffix(" mm"),
            )
            .accessible_name(tr("Vertical print offset in millimeters"));
        });
    }

    /// One line that says what a stock profile holds.
    fn stock_summary(&self, stock: &crate::settings::StockProfile) -> String {
        let s = &stock.settings;
        let media = self
            .printers
            .iter()
            .find(|p| p.name == stock.printer)
            .map_or_else(|| s.media.clone(), |p| p.media_label(&s.media));
        format!(
            "{} · {} · {:.1} × {:.1} mm · {} DPI",
            stock.printer, media, s.width_mm, s.height_mm, s.dpi
        )
    }

    /// Save the current printer setup under a name. Returns whether it was saved.
    pub(super) fn save_stock_form(&mut self, ui: &mut egui::Ui) -> bool {
        use crate::i18n::tr;
        if self.settings.printer.is_empty() {
            ui.label(tr(
                "Choose a printer on the Printer tab first. Stock profiles belong to a printer.",
            ));
            return false;
        }
        let current = crate::settings::StockProfile {
            printer: self.settings.printer.clone(),
            settings: self.settings.current_profile(),
        };
        ui.label(
            RichText::new(format!("{} {}", tr("Saves:"), self.stock_summary(&current))).weak(),
        );
        let mut saved = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Stock profile name"));
            ui.add(egui::TextEdit::singleline(&mut self.stock_name).char_limit(80).hint_text(tr("For example: 4 × 6 shipping labels")))
                .accessible_name(tr("Stock profile name"));
            let name = self.stock_name.trim().to_owned();
            let key = crate::settings::stock_key(&self.settings.printer, &name);
            let exists = self.settings.stock_profiles.contains_key(&key);
            let can_save = !name.is_empty()
                && (exists || self.settings.stock_profiles.len() < crate::settings::MAX_PROFILES);
            let label = if exists { tr("Replace stock profile") } else { tr("Save stock profile") };
            if crate::designer_ui::icon_button_enabled(ui, can_save, "Save document", label).clicked() {
                self.settings.stock_profiles.insert(key.clone(), current.clone());
                self.settings.active_stock = Some(key.clone());
                self.stock_saved = Some(key);
                self.persist();
                self.stock_name.clear();
                self.notices.push(tr("Stock profile saved"), false, None);
                saved = true;
            }
            if exists && !saved {
                ui.colored_label(ui.visuals().warn_fg_color, tr("A profile with this name already exists for this printer. Saving replaces it."));
            }
        });
        if self.settings.stock_profiles.len() >= crate::settings::MAX_PROFILES {
            ui.label(tr(
                "You have reached the limit of saved stock profiles. Delete one to add another.",
            ));
        }
        saved
    }

    /// Every saved stock profile, for every printer: load, delete, or save a new one.
    pub(super) fn stock_profiles(&mut self, ui: &mut egui::Ui) {
        use crate::i18n::tr;
        Self::section_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            Self::center_heading(ui, tr("Saved label stock"), "Profile");
            ui.label(RichText::new(tr("A stock profile remembers a printer with its paper size, resolution, and alignment. Load one to switch to all of it at once.")).weak());
            ui.add_space(6.);
            let mut stocks: Vec<_> = self.settings.stock_profiles.iter()
                .map(|(key, stock)| (key.clone(), stock.clone()))
                .collect();
            // This printer's profiles first, then the others, each by name.
            stocks.sort_by_key(|(key, stock)| (stock.printer != self.settings.printer, key.to_lowercase()));
            if stocks.is_empty() {
                ui.label(tr("No saved stock yet. Set up the printer and paper, then save them below."));
            }
            let active = self.active_stock();
            let mut load = None;
            let mut update = false;
            for (key, stock) in &stocks {
                let available = self.printers.iter().any(|p| p.name == stock.printer);
                let is_active = active.as_ref().is_some_and(|(k, _)| k == key);
                let changed = is_active && active.as_ref().is_some_and(|(_, c)| *c);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(crate::settings::stock_display_name(key)).strong());
                    if is_active {
                        ui.label(RichText::new(format!("● {}", tr("Active"))).strong().color(theme::accent(ui.visuals())));
                    } else {
                        let response = crate::designer_ui::icon_button_enabled(ui, available, "Profile", tr("Load"));
                        let response = if available { response } else { response.on_disabled_hover_text(tr("This printer is not connected")) };
                        if response.clicked() {
                            load = Some(key.clone());
                        }
                    }
                    if is_active
                        && crate::designer_ui::icon_button_enabled(ui, changed, "Save document", tr("Update profile"))
                            .on_hover_text(tr("Save the current printer, paper, resolution, and alignment into this profile"))
                            .clicked()
                    {
                        update = true;
                    }
                    if crate::designer_ui::icon_button(ui, "Delete", tr("Delete")).clicked() {
                        self.stock_delete = Some(key.clone());
                    }
                });
                ui.label(RichText::new(self.stock_summary(stock)).small().weak());
                if changed {
                    ui.colored_label(ui.visuals().warn_fg_color, tr("Changed since it was loaded. Update profile saves the changes."));
                }
                ui.add_space(4.);
            }
            if let Some(key) = load {
                self.load_stock(&key);
            }
            if update {
                self.update_active_stock();
            }
            ui.separator();
            if let Some(key) = self.stock_saved.clone().filter(|_| self.stock_name.is_empty()) {
                ui.label(
                    RichText::new(format!(
                        "✔ {} {}",
                        tr("Saved and active:"),
                        crate::settings::stock_display_name(&key)
                    ))
                    .strong()
                    .color(theme::accent(ui.visuals())),
                );
            }
            ui.label(RichText::new(tr("Save the current setup as a new profile")).strong());
            self.save_stock_form(ui);
        });
        self.stock_delete_confirmation(ui.ctx());
    }

    fn stock_delete_confirmation(&mut self, ctx: &egui::Context) {
        use crate::i18n::tr;
        let Some(key) = self.stock_delete.clone() else {
            return;
        };
        let mut delete = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::unique("delete-stock-profile")).show(ctx, |ui| {
            ui.set_max_width(420.);
            ui.heading(tr("Delete stock profile?"));
            ui.label(crate::settings::stock_display_name(&key));
            ui.horizontal(|ui| {
                cancel = crate::designer_ui::icon_button(ui, "Close", tr("Cancel")).clicked();
                delete = crate::designer_ui::icon_button(ui, "Delete", tr("Delete")).clicked();
            });
        });
        if delete {
            self.settings.stock_profiles.remove(&key);
            if self.settings.active_stock.as_ref() == Some(&key) {
                self.settings.active_stock = None;
            }
            if self.stock_saved.as_ref() == Some(&key) {
                self.stock_saved = None;
            }
            self.persist();
            self.notices.push(tr("Stock profile deleted"), false, None);
        }
        if delete || cancel || response.should_close() {
            self.stock_delete = None;
        }
    }

    pub(super) fn reprint_confirmation(&mut self, ctx: &egui::Context) {
        let Some(entry) = self.reprint_entry.clone() else {
            return;
        };
        let mut print = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::unique("reprint-history")).show(ctx, |ui| {
            ui.set_max_width(480.);
            ui.heading(crate::i18n::tr("Reprint label?"));
            ui.label(&entry.title);
            if let Some(settings) = &entry.settings {
                ui.label(format!(
                    "{} · {} copies · {:.1} × {:.1} mm · {} DPI · {}",
                    entry.printer,
                    entry.copies,
                    settings.width_mm,
                    settings.height_mm,
                    settings.dpi,
                    settings.media
                ));
            }
            ui.label(crate::i18n::tr(
                "This sends another print job using the recorded settings.",
            ));
            ui.horizontal(|ui| {
                cancel = crate::designer_ui::icon_button(ui, "Close", crate::i18n::tr("Cancel"))
                    .clicked();
                print = crate::designer_ui::icon_button_enabled(
                    ui,
                    !self.busy,
                    "Print device",
                    crate::i18n::tr("Reprint"),
                )
                .clicked();
            });
        });
        if cancel || response.should_close() {
            self.reprint_entry = None;
        }
        if !print {
            return;
        }
        self.reprint_entry = None;
        let Some(settings) = entry.settings.clone() else {
            return;
        };
        let Some(printer) = self
            .printers
            .iter()
            .find(|p| p.name == entry.printer)
            .cloned()
        else {
            self.status =
                "The recorded printer is unavailable. Open the label and choose a printer.".into();
            self.error = true;
            return;
        };
        self.task("Reprinting label…", move || {
            if let Some(language) = entry.raw_language() {
                let doc = entry.design.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Open this saved design for review before sending raw {} again.",
                        language.name()
                    )
                })?;
                let detail = printing::submit_raw(
                    &language.encode_for_print(
                        doc,
                        settings.dpi,
                        &settings.footer,
                        settings.safe_margin_mm,
                    )?,
                    language,
                    &settings,
                    &printer,
                )?;
                let mut repeated = entry;
                repeated.time = crate::history::now();
                crate::history::record(repeated);
                return Ok(Message::Done(detail));
            }
            let output = match entry.kind {
                crate::history::Kind::Design => {
                    let doc = if let Some(doc) = &entry.design {
                        doc.clone()
                    } else {
                        let path = entry
                            .source
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("Saved design is unavailable"))?;
                        if entry.fingerprint.as_ref()
                            != Some(&crate::duplicates::fingerprint(path, entry.page)?)
                        {
                            anyhow::bail!(
                                "The saved design changed. Open it for review before printing."
                            );
                        }
                        crate::design::LabelDocument::load(path)?
                    };
                    crate::design_render::render_with_footer(
                        &doc,
                        settings.dpi,
                        &settings.footer,
                        settings.safe_margin_mm,
                    )?
                }
                crate::history::Kind::Import => {
                    let path = entry
                        .source
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Source label is unavailable"))?;
                    if entry.fingerprint.as_ref()
                        != Some(&crate::duplicates::fingerprint(path, entry.page)?)
                    {
                        anyhow::bail!(
                            "The source label changed. Open it for review before printing."
                        );
                    }
                    let page = document::load(path, entry.page)?;
                    layout::render(&page, entry.crop.unwrap_or(Crop::FULL), &settings)?
                }
            };
            let detail = printing::submit(&output, &settings, &printer)?;
            let mut repeated = entry;
            repeated.time = crate::history::now();
            crate::history::record(repeated);
            Ok(Message::Done(detail))
        });
    }
}
