//! Mensagens de status não modais, com "Desfazer" (feedback sem interromper).
use crate::app::App;
use crate::theme;
use egui::{Align2, Color32, Context, RichText};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
pub enum ToastKind {
    Info,
    Error,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    /// Profundidade esperada do histórico: se mudar, o "Desfazer" deixa de valer para esta ação.
    pub undo_at: Option<usize>,
    pub until: Instant,
}

impl App {
    /// Aviso que some sozinho depois de 10 s (para ao passar o mouse). Erros ficam até serem fechados.
    pub fn notify(&mut self, text: impl Into<String>, kind: ToastKind, undoable: bool) {
        let undo_at = undoable.then(|| self.undo.len() + 1);
        self.toast = Some(Toast { text: text.into(), kind, undo_at, until: Instant::now() + Duration::from_secs(10) });
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.notify(text, ToastKind::Info, false);
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.notify(text, ToastKind::Error, false);
    }

    pub fn toast_ui(&mut self, ctx: &Context) {
        let Some(t) = &self.toast else { return };
        let can_undo = t.undo_at == Some(self.undo.len());
        let (text, kind) = (t.text.clone(), t.kind);
        let mut undo = false;
        let mut close = false;
        let mut hovered = false;
        egui::Area::new(egui::Id::new("toast")).order(egui::Order::Foreground).anchor(Align2::CENTER_BOTTOM, [0.0, -40.0]).show(ctx, |ui| {
            let (fill, edge) = match kind {
                ToastKind::Info => (Color32::from_rgb(34, 40, 49), theme::pal().accent),
                ToastKind::Error => (Color32::from_rgb(58, 34, 34), Color32::from_rgb(232, 120, 110)),
            };
            let r = egui::Frame::popup(ui.style()).fill(fill).stroke(egui::Stroke::new(1.0, edge)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_max_width(560.0);
                    ui.label(RichText::new(&text).color(Color32::WHITE));
                    if can_undo && ui.button("Desfazer").clicked() {
                        undo = true;
                    }
                    if ui.add(egui::Button::new(RichText::new("×").color(theme::pal().dim)).frame(false)).on_hover_text("Fechar").clicked() {
                        close = true;
                    }
                });
            });
            hovered = r.response.hovered();
        });
        if undo {
            self.undo();
            self.toast = None;
            return;
        }
        if close {
            self.toast = None;
            return;
        }
        let now = Instant::now();
        if let Some(t) = &mut self.toast {
            if hovered {
                t.until = now + Duration::from_secs(10);
            }
            if kind == ToastKind::Info && now >= t.until {
                self.toast = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
    }
}
