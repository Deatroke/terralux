//! Tela inicial: começar de um modelo, abrir um arquivo ou voltar a um projeto recente.
use crate::app::{App, Editor};
use crate::theme;
use egui::{vec2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use terralux_core::{Heightfield, Scene, TemplateGroup, TemplateInfo, TEMPLATES};
use terralux_render::{tonemap_to_srgb8, RenderOptions, Renderer};

const THUMB_W: usize = 200;
const THUMB_H: usize = 112;
const MAX_RECENT: usize = 12;

#[derive(Default)]
pub struct StartState {
    pub thumbs: HashMap<String, egui::TextureHandle>,
    rx: Option<Receiver<(String, egui::ColorImage)>>,
    started: bool,
    pub recent: Vec<PathBuf>,
    /// Veio de uma cena em andamento (Ctrl+N): pode voltar a ela.
    pub back_to: bool,
}

// ---------- arquivos recentes ----------

fn recent_file() -> PathBuf {
    terralux_core::assets::config_dir().join("recent.txt")
}

/// Lê a lista, descartando o que não existe mais.
pub fn load_recent() -> Vec<PathBuf> {
    let t = std::fs::read_to_string(recent_file()).unwrap_or_default();
    t.lines().map(PathBuf::from).filter(|p| p.is_file()).take(MAX_RECENT).collect()
}

/// Põe o arquivo no topo da lista (e grava).
pub fn push_recent(list: &mut Vec<PathBuf>, p: &Path) {
    let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    list.retain(|x| *x != p);
    list.insert(0, p);
    list.truncate(MAX_RECENT);
    let _ = std::fs::create_dir_all(terralux_core::assets::config_dir());
    let text: String = list.iter().map(|p| format!("{}\n", p.display())).collect();
    let _ = std::fs::write(recent_file(), text);
}

// ---------- miniaturas dos modelos ----------

fn spawn_thumbs(ctx: egui::Context) -> Receiver<(String, egui::ColorImage)> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for info in TEMPLATES {
            let Some(mut sc) = Scene::template(info.id) else { continue };
            let hf = sc.terrain.as_mut().map(|t| {
                t.resolution = t.resolution.min(192);
                Heightfield::from_def(t)
            });
            let r = Renderer::with_heightfield(&sc, hf.map(std::sync::Arc::new)).fast(true);
            let img = r.render(&RenderOptions::new(THUMB_W * 2, THUMB_H * 2, 1));
            let rgb = tonemap_to_srgb8(&img);
            let ci = egui::ColorImage::from_rgb([THUMB_W * 2, THUMB_H * 2], &rgb);
            if tx.send((info.id.to_string(), ci)).is_err() {
                return;
            }
            ctx.request_repaint();
        }
    });
    rx
}

fn template_card(ui: &mut Ui, info: &TemplateInfo, tex: Option<&egui::TextureHandle>) -> egui::Response {
    let pl = theme::pal();
    let (rect, resp) = ui.allocate_exact_size(vec2(THUMB_W as f32 + 8.0, THUMB_H as f32 + 62.0), Sense::click());
    let down = resp.is_pointer_button_down_on();
    let fill = if resp.hovered() { pl.hover } else { pl.light };
    theme::bevel(ui.painter(), rect, !down, fill);
    let thumb = Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(THUMB_W as f32, THUMB_H as f32));
    match tex {
        Some(t) => {
            ui.painter().image(t.id(), thumb, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            ui.painter().rect_filled(thumb, 0.0, pl.well);
            ui.painter().text(thumb.center(), egui::Align2::CENTER_CENTER, "gerando…", FontId::proportional(11.0), pl.dim);
        }
    }
    theme::bevel(ui.painter(), thumb.expand(1.0), false, Color32::TRANSPARENT);
    if resp.hovered() {
        ui.painter().rect_stroke(thumb.expand(1.0), 0.0, Stroke::new(1.5, pl.accent), egui::StrokeKind::Outside);
    }
    ui.painter().text(rect.min + vec2(8.0, THUMB_H as f32 + 14.0), egui::Align2::LEFT_CENTER, info.name, FontId::proportional(13.0), pl.ink);
    let g = ui.painter().layout(info.desc.to_string(), FontId::proportional(11.0), pl.dim, THUMB_W as f32 - 8.0);
    ui.painter().galley(rect.min + vec2(8.0, THUMB_H as f32 + 26.0), g, pl.dim);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl App {
    /// Abre a tela inicial. `back_to`: há uma cena em andamento para voltar.
    pub fn open_start(&mut self, back_to: bool) {
        self.home.back_to = back_to;
        self.home.recent = load_recent();
        self.editor = Editor::Start;
    }

    pub fn note_recent(&mut self, p: &Path) {
        push_recent(&mut self.home.recent, p);
    }

    pub fn open_path(&mut self, p: &Path) {
        match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| Scene::from_ron(&t).map_err(|e| e.to_string())) {
            Ok(s) => {
                self.load_scene(s, Some(p.to_path_buf()));
                self.note_recent(p);
            }
            Err(e) => self.error(format!("Não foi possível abrir {}: {e}", p.display())),
        }
    }

    pub fn start_ui(&mut self, ui: &mut Ui) {
        if !self.home.started {
            self.home.started = true;
            self.home.rx = Some(spawn_thumbs(ui.ctx().clone()));
        }
        if let Some(rx) = &self.home.rx {
            while let Ok((id, ci)) = rx.try_recv() {
                let t = ui.ctx().load_texture(format!("tpl-{id}"), ci, egui::TextureOptions::LINEAR);
                self.home.thumbs.insert(id, t);
            }
        }
        let pl = theme::pal();
        let mut pick: Option<&'static str> = None;
        let mut open_file = false;
        let mut open_recent: Option<PathBuf> = None;
        let mut back = false;
        let dirty = self.scene != self.saved;

        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("Terralux Studio").size(26.0).strong().color(pl.heading));
            ui.label(RichText::new("Crie terrenos e cenas 3D para jogos e renders. Comece de um modelo ou abra um projeto.").color(pl.dim));
            ui.add_space(10.0);
        });

        let right_w = 290.0;
        let total = ui.available_width();
        let h = ui.available_height() - 6.0;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2((total - right_w - 34.0).max(300.0), h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for (group, title) in [(TemplateGroup::Scenery, "Cenários para render"), (TemplateGroup::Game, "Terrenos para jogos (heightmap, OBJ, glTF)")] {
                        theme::header(ui, title);
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                            for info in TEMPLATES.iter().filter(|i| i.group == group) {
                                if template_card(ui, info, self.home.thumbs.get(info.id)).clicked() {
                                    pick = Some(info.id);
                                }
                            }
                        });
                        ui.add_space(14.0);
                    }
                });
            });
            ui.add_space(14.0);
            ui.allocate_ui_with_layout(vec2(right_w, h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                theme::header(ui, "Projetos");
                ui.add_space(6.0);
                if theme::button(ui, "Abrir arquivo…", true, true, vec2(right_w, 0.0)).clicked() {
                    open_file = true;
                }
                if self.home.back_to {
                    ui.add_space(4.0);
                    if theme::button(ui, "Voltar à cena atual", false, true, vec2(right_w, 28.0)).clicked() {
                        back = true;
                    }
                    if dirty {
                        ui.label(RichText::new("A cena atual tem alterações não salvas; começar outra a descarta.").small().color(pl.dim));
                    }
                }
                ui.add_space(10.0);
                theme::section_title(ui, "Recentes");
                if self.home.recent.is_empty() {
                    ui.label(RichText::new("Nenhum projeto salvo ainda. Os que você abrir ou salvar aparecem aqui.").small().color(pl.dim));
                }
                egui::ScrollArea::vertical().id_salt("recent").auto_shrink([false, false]).show(ui, |ui| {
                    for p in &self.home.recent {
                        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
                        let (rect, resp) = ui.allocate_exact_size(vec2(right_w - 6.0, 42.0), Sense::click());
                        theme::bevel(ui.painter(), rect, !resp.is_pointer_button_down_on(), if resp.hovered() { pl.hover } else { pl.light });
                        ui.painter().text(rect.min + vec2(8.0, 14.0), egui::Align2::LEFT_CENTER, &name, FontId::proportional(13.0), pl.ink);
                        let short = if dir.chars().count() > 38 { format!("…{}", dir.chars().skip(dir.chars().count() - 37).collect::<String>()) } else { dir };
                        ui.painter().text(rect.min + vec2(8.0, 30.0), egui::Align2::LEFT_CENTER, short, FontId::proportional(10.5), pl.dim);
                        if resp.on_hover_text(p.display().to_string()).clicked() {
                            open_recent = Some(p.clone());
                        }
                        ui.add_space(3.0);
                    }
                });
            });
        });

        if let Some(id) = pick {
            if let Some(s) = Scene::template(id) {
                self.load_scene(s, None);
                self.home.back_to = false;
            }
        }
        if open_file {
            self.file_open();
        }
        if let Some(p) = open_recent {
            self.open_path(&p);
        }
        if back {
            self.editor = Editor::Work;
        }
        if self.home.back_to && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.editor = Editor::Work;
        }
    }
}
