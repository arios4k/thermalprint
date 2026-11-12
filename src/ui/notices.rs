//! Short messages that stay visible long enough to read, a history of recent ones, and
//! an optional "Undo" for moving printed labels out of the watch folder.
use eframe::egui::{self, Color32, RichText};
use std::{
    collections::VecDeque,
    path::PathBuf,
    time::{Duration, Instant},
};

const HISTORY: usize = 30;

/// Files moved to the Processed folder: where each came from and where it went.
pub type Moves = Vec<(PathBuf, PathBuf)>;

struct Entry {
    text: String,
    error: bool,
    at: Instant,
}

struct Toast {
    text: String,
    error: bool,
    at: Instant,
    undo: Option<Moves>,
}

/// A label that is about to print by itself: what it is, when it prints, and how to stop it.
pub struct Countdown {
    pub id: u64,
    pub text: String,
    pub due: Instant,
}

#[derive(Default)]
pub struct Notices {
    log: VecDeque<Entry>,
    toast: Option<Toast>,
    history_open: bool,
    /// Labels waiting to print automatically, set by the app every frame.
    pub countdowns: Vec<Countdown>,
}

/// What the user asked for this frame.
pub enum Action {
    Undo(Moves),
    /// Stop an automatic print before it starts.
    CancelPrint(u64),
}

impl Notices {
    pub fn push(&mut self, text: impl Into<String>, error: bool, undo: Option<Moves>) {
        let text = text.into();
        if self.log.len() == HISTORY {
            self.log.pop_front();
        }
        self.log.push_back(Entry {
            text: text.clone(),
            error,
            at: Instant::now(),
        });
        self.toast = Some(Toast {
            text,
            error,
            at: Instant::now(),
            undo,
        });
    }

    pub fn toggle_history(&mut self) {
        self.history_open = !self.history_open;
    }

    fn lifetime(toast: &Toast) -> Duration {
        Duration::from_secs(if toast.undo.is_some() {
            12
        } else if toast.error {
            10
        } else {
            6
        })
    }

    /// Draw the toast and the history window. `can_act` is false while the app is busy.
    pub fn show(&mut self, ctx: &egui::Context, can_act: bool) -> Option<Action> {
        let mut action = None;
        let mut dismiss = false;
        if !self.countdowns.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
            let bottom = if self.toast.is_some() { -130. } else { -44. };
            egui::Area::new(egui::Id::NULL.with("auto-print-countdowns"))
                .anchor(egui::Align2::RIGHT_BOTTOM, [-16., bottom])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(360.);
                        for countdown in &self.countdowns {
                            ui.horizontal_wrapped(|ui| {
                                let left = countdown.due.saturating_duration_since(Instant::now());
                                let text = if left.is_zero() {
                                    format!("{} {} …", crate::i18n::tr("Printing"), countdown.text)
                                } else {
                                    format!(
                                        "{} {} · {} s",
                                        crate::i18n::tr("Printing"),
                                        countdown.text,
                                        left.as_secs() + 1
                                    )
                                };
                                ui.label(text);
                                if ui.button(crate::i18n::tr("Cancel")).clicked() {
                                    action = Some(Action::CancelPrint(countdown.id));
                                }
                            });
                        }
                    });
                });
        }
        if let Some(toast) = &self.toast {
            let left = Self::lifetime(toast).saturating_sub(toast.at.elapsed());
            if left.is_zero() {
                dismiss = true;
            } else {
                ctx.request_repaint_after(left.min(Duration::from_millis(500)));
                egui::Area::new(egui::Id::NULL.with("notice-toast"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, [-16., -44.])
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_max_width(360.);
                            ui.horizontal_wrapped(|ui| {
                                let text = RichText::new(&toast.text);
                                ui.label(if toast.error {
                                    text.color(Color32::from_rgb(200, 65, 55))
                                } else {
                                    text
                                });
                            });
                            ui.horizontal(|ui| {
                                if let Some(moves) = &toast.undo
                                    && ui
                                        .add_enabled(
                                            can_act,
                                            egui::Button::new(crate::i18n::tr("Undo")),
                                        )
                                        .clicked()
                                {
                                    action = Some(Action::Undo(moves.clone()));
                                    dismiss = true;
                                }
                                if ui.small_button(crate::i18n::tr("Dismiss")).clicked() {
                                    dismiss = true;
                                }
                            });
                        });
                    });
            }
        }
        if dismiss {
            self.toast = None;
        }
        let mut open = self.history_open;
        egui::Window::new(crate::i18n::tr("Recent messages"))
            .open(&mut open)
            .collapsible(false)
            .default_width(380.)
            .show(ctx, |ui| {
                if self.log.is_empty() {
                    ui.label(RichText::new(crate::i18n::tr("Nothing yet")).weak());
                }
                egui::ScrollArea::vertical()
                    .max_height(260.)
                    .show(ui, |ui| {
                        for entry in self.log.iter().rev() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(age(entry.at.elapsed())).small().weak());
                                let text = RichText::new(&entry.text);
                                ui.label(if entry.error {
                                    text.color(Color32::from_rgb(200, 65, 55))
                                } else {
                                    text
                                });
                            });
                        }
                    });
            });
        self.history_open = open;
        action
    }
}

fn age(elapsed: Duration) -> String {
    match elapsed.as_secs() {
        0..=4 => "now".into(),
        s @ 5..=59 => format!("{s}s ago"),
        s @ 60..=3599 => format!("{}m ago", s / 60),
        s => format!("{}h ago", s / 3600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_capped_and_the_latest_message_is_the_toast() {
        let mut notices = Notices::default();
        for i in 0..HISTORY + 5 {
            notices.push(format!("message {i}"), false, None);
        }
        assert_eq!(notices.log.len(), HISTORY);
        assert_eq!(notices.log.front().unwrap().text, "message 5");
        assert_eq!(notices.toast.as_ref().unwrap().text, "message 34");
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(age(Duration::from_secs(2)), "now");
        assert_eq!(age(Duration::from_secs(30)), "30s ago");
        assert_eq!(age(Duration::from_secs(150)), "2m ago");
        assert_eq!(age(Duration::from_secs(7300)), "2h ago");
    }
}
