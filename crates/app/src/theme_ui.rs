//! Menu "Tema" e a janela de personalização (cores, estilo, arredondamento, brilho, neon).
use crate::app::App;
use crate::theme::{self, Palette, Style, PRESETS};
use egui::{Color32, Context, RichText, Ui};
use terralux_core::assets;

fn path() -> std::path::PathBuf {
    assets::config_dir().join("tema.txt")
}

/// Tema salvo da última vez; sem arquivo (ou inválido), o padrão cybercore.
pub fn load_saved() -> Palette {
    std::fs::read_to_string(path()).ok().and_then(|t| theme::from_text(&t)).unwrap_or_else(|| theme::preset(theme::DEFAULT_ID))
}

fn save(p: &Palette) {
    let _ = std::fs::create_dir_all(assets::config_dir());
    let _ = std::fs::write(path(), theme::to_text(p));
}

fn choose(ctx: &Context, id: &str) {
    let mut p = theme::preset(id);
    if id == "custom" {
        // "Personalizado" parte do tema que está na tela
        p = theme::pal();
        p.id = "custom";
    }
    theme::set(ctx, p);
    save(&theme::pal());
}

impl App {
    /// Submenu Exibir → Tema.
    pub fn theme_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.menu_button("Tema", |ui| {
            let cur = theme::pal().id;
            for (id, name) in PRESETS.iter().filter(|(id, _)| *id != "custom") {
                if ui.selectable_label(cur == *id, *name).clicked() {
                    choose(&ctx, id);
                    ui.close();
                }
            }
            ui.separator();
            if ui.selectable_label(cur == "custom", "Personalizado…").clicked() {
                if cur != "custom" {
                    choose(&ctx, "custom");
                }
                self.theme_open = true;
                ui.close();
            }
        });
    }

    pub fn theme_window(&mut self, ctx: &Context) {
        if !self.theme_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Personalizar tema").open(&mut open).collapsible(false).resizable(false).default_pos(egui::pos2(260.0, 90.0)).show(ctx, |ui| {
            let ctx = ui.ctx().clone();
            let mut p = theme::pal();
            let before = p;
            ui.label(RichText::new("Comece por uma paleta pronta e ajuste o que quiser. Qualquer mudança vira o tema “Personalizado”.").color(p.dim));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (id, name) in PRESETS.iter().filter(|(id, _)| *id != "custom") {
                    if theme::small_button(ui, name, p.id == *id).clicked() {
                        choose(&ctx, id);
                        p = theme::pal();
                    }
                }
            });
            let before = if p.id != before.id { p } else { before };
            ui.add_space(6.0);
            theme::section_title(ui, "Estilo");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.style, Style::Neon, "Neon (escuro, contornos brilhantes)");
                ui.selectable_value(&mut p.style, Style::Aero, "Vidro (Frutiger Aero)");
            });
            theme::section_title(ui, "Cores");
            egui::Grid::new("tema_cores").num_columns(4).spacing([14.0, 6.0]).show(ui, |ui| {
                let row = |ui: &mut Ui, name: &str, c: &mut Color32| {
                    ui.label(name);
                    ui.color_edit_button_srgba(c);
                };
                row(ui, "Fundo dos painéis", &mut p.stone);
                row(ui, "Botões", &mut p.light);
                ui.end_row();
                row(ui, "Texto", &mut p.ink);
                row(ui, "Texto suave", &mut p.dim);
                ui.end_row();
                row(ui, "Contorno", &mut p.dk);
                row(ui, "Destaque (ativo)", &mut p.accent);
                ui.end_row();
                row(ui, "Esferas e ícones", &mut p.orb);
                row(ui, "Neon (brilho)", &mut p.glow);
                ui.end_row();
                row(ui, "Título (topo)", &mut p.navy_hi);
                row(ui, "Título (base)", &mut p.navy);
                ui.end_row();
                row(ui, "Gaveta e campos escuros", &mut p.well);
                row(ui, "Fundo da cena", &mut p.view_bg);
                ui.end_row();
                row(ui, "Texto sobre a cena", &mut p.view_text);
                ui.end_row();
            });
            theme::section_title(ui, "Forma e brilho");
            egui::Grid::new("tema_formas").num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
                ui.label("Arredondamento");
                ui.add(egui::Slider::new(&mut p.radius, 0..=12));
                ui.end_row();
                ui.label("Brilho do vidro");
                ui.add(egui::Slider::new(&mut p.gloss, 0..=200));
                ui.end_row();
                ui.label("Intensidade do neon");
                ui.add(egui::Slider::new(&mut p.glow_amt, 0..=255));
                ui.end_row();
                ui.label("Opacidade do vidro");
                ui.add(egui::Slider::new(&mut p.glass, 60..=255));
                ui.end_row();
            });
            if p != before {
                p.id = "custom";
                theme::set(&ctx, p);
                save(&theme::pal());
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if theme::button(ui, "Exportar tema…", false, true, egui::vec2(0.0, 26.0)).clicked() {
                    if let Some(f) = rfd::FileDialog::new().add_filter("Tema Terralux", &["tema"]).set_file_name("meu-tema.tema").save_file() {
                        let mut q = theme::pal();
                        q.id = "custom";
                        let _ = std::fs::write(f, theme::to_text(&q));
                    }
                }
                if theme::button(ui, "Importar tema…", false, true, egui::vec2(0.0, 26.0)).clicked() {
                    if let Some(f) = rfd::FileDialog::new().add_filter("Tema Terralux", &["tema"]).pick_file() {
                        match std::fs::read_to_string(&f).ok().and_then(|t| theme::from_text(&t)) {
                            Some(q) => {
                                theme::set(&ctx, q);
                                save(&theme::pal());
                            }
                            None => self.error("Esse arquivo não parece um tema do Terralux."),
                        }
                    }
                }
                if theme::button(ui, "Voltar ao padrão", false, true, egui::vec2(0.0, 26.0)).clicked() {
                    choose(&ctx, theme::DEFAULT_ID);
                }
            });
        });
        self.theme_open = open;
    }
}
