//! Guided setup and review actions share the existing Print Center and print pipeline.
use super::*;

impl ThermalPrint {
    pub(super) fn show_window(&mut self, ctx: &egui::Context) {
        self.background_hidden = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    pub(super) fn background_controls(&mut self, ui: &mut egui::Ui) {
        ui.collapsing(crate::i18n::tr("Background printing"), |ui| {
            let mut enabled = self.settings.background_printing;
            let label = crate::i18n::tr("Keep running when the window is closed");
            if ui.checkbox(&mut enabled, label).changed() {
                if enabled {
                    match crate::background::Tray::new() {
                        Ok(tray) => {
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
            let clear = egui::Button::new(crate::i18n::tr("Clear duplicate protection…"));
            if ui.add_enabled(!self.busy, clear).clicked() {
                self.duplicate_clear = true;
            }
        });
        if !self.duplicate_clear {
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("clear-automatic-duplicates"))
            .show(ui.ctx(), |ui| {
                ui.heading(crate::i18n::tr("Clear duplicate protection?"));
                ui.label(crate::i18n::tr("Previously printed files can print automatically again if they arrive in the watch folder."));
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("Cancel")).clicked() {
                        self.duplicate_clear = false;
                    }
                    if ui.button(crate::i18n::tr("Clear duplicate protection")).clicked() {
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

    pub(super) fn stock_setup(&mut self, ui: &mut egui::Ui) {
        Self::section_frame(ui).show(ui, |ui| {
            Self::center_heading(ui, crate::i18n::tr("Printer setup assistant"), "Status");
            let start = ui.button(crate::i18n::tr("Set up printer and label stock"));
            if self.setup_step.is_none() && start.clicked() {
                self.setup_step = Some(0);
            }
            if let Some(step) = self.setup_step {
                let instructions = [
                    "Choose your printer above. Refresh printers if it is missing.",
                    "Measure the loaded label. Set its width, height, orientation, media size, and DPI above.",
                    "Print one alignment test. Compare its 1 mm border to the label edges, then adjust the offsets above.",
                    "Give this stock profile a name and save it below. Load it whenever you use this printer and label stock."
                ];
                ui.label(crate::i18n::tr(instructions[step]));
                ui.horizontal_wrapped(|ui| {
                    if step > 0 && ui.button(crate::i18n::tr("Back")).clicked() {
                        self.setup_step = Some(step - 1);
                    }
                    let next = egui::Button::new(crate::i18n::tr("Next"));
                    if step < 3 && ui.add_enabled(!self.settings.printer.is_empty(), next).clicked() {
                        self.setup_step = Some(step + 1);
                    }
                    let test = egui::Button::new(crate::i18n::tr("Print one test label"));
                    if step == 2 && ui.add_enabled(!self.busy, test).clicked() {
                        self.print_alignment_test();
                    }
                    if ui.button(crate::i18n::tr("Close assistant")).clicked() {
                        self.setup_step = None;
                    }
                });
            }
            let printer = self.printers.iter()
                .find(|printer| printer.name == self.settings.printer).cloned();
            if let Some(printer) = printer {
                let matching = printer.matching_media(
                    self.settings.width_mm,
                    self.settings.height_mm,
                );
                if let Some(media) = matching {
                    if ui.button(crate::i18n::tr("Match media to label size")).clicked() {
                        self.settings.media = media;
                        self.dirty = true;
                        self.persist();
                    }
                } else {
                    ui.label(crate::i18n::tr("This printer does not advertise a matching media size. Configure the stock in its driver."));
                }
                if let Some(reason) = printer.media_mismatch(
                    &self.settings.media,
                    self.settings.width_mm,
                    self.settings.height_mm,
                ) {
                    ui.colored_label(ui.visuals().warn_fg_color, reason);
                }
            }
            ui.separator();
            ui.label(crate::i18n::tr("Saved label stock"));
            let stocks: Vec<_> = self.settings.stock_profiles.iter()
                .filter(|(_, stock)| stock.printer == self.settings.printer)
                .map(|(name, stock)| (name.clone(), stock.settings.clone())).collect();
            for (name, profile) in stocks {
                ui.horizontal_wrapped(|ui| {
                    ui.label(&name);
                    if ui.button(crate::i18n::tr("Load profile")).clicked() {
                        self.settings.apply_profile(&profile);
                        self.stock_name = name.split_once(" · ")
                            .map(|(_, name)| name).unwrap_or(&name).to_owned();
                        self.dirty = true;
                        self.persist();
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::i18n::tr("Stock profile name"));
                ui.add(egui::TextEdit::singleline(&mut self.stock_name).char_limit(80))
                    .accessible_name(crate::i18n::tr("Stock profile name"));
                let key = format!("{} · {}", self.settings.printer, self.stock_name.trim());
                let can_save = !self.settings.printer.is_empty()
                    && !self.stock_name.trim().is_empty()
                    && (self.settings.stock_profiles.len() < 50
                        || self.settings.stock_profiles.contains_key(&key));
                let save = egui::Button::new(crate::i18n::tr("Save stock profile"));
                if ui.add_enabled(can_save, save).clicked() {
                    let profile = crate::settings::StockProfile {
                        printer: self.settings.printer.clone(),
                        settings: self.settings.current_profile(),
                    };
                    self.settings.stock_profiles.insert(key, profile);
                    self.persist();
                    self.setup_step = None;
                    self.notices.push("Stock profile saved", false, None);
                }
            });
        });
    }

    pub(super) fn reprint_confirmation(&mut self, ctx: &egui::Context) {
        let Some(entry) = self.reprint_entry.clone() else {
            return;
        };
        let mut print = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::unique("reprint-history")).show(ctx, |ui| {
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
                cancel = ui.button(crate::i18n::tr("Cancel")).clicked();
                print = ui
                    .add_enabled(!self.busy, egui::Button::new(crate::i18n::tr("Reprint")))
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
            if entry.raw_zpl {
                let doc = entry.design.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Open this saved design for review before sending RAW ZPL again."
                    )
                })?;
                let detail = printing::submit_raw_zpl(
                    &crate::zpl::encode_design(doc, settings.dpi)?,
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
                    crate::design_render::render_for_print(&doc, settings.dpi)?
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
