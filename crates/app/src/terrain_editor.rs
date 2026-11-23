//! Editor de Terreno: ocupa a janela de trabalho (como os "editores" do original).
use crate::app::{App, Editor};
use crate::theme;
use egui::{vec2, Color32, ColorImage, RichText, Stroke, Ui};
use terralux_core::Heightfield;

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Mapa de elevação com relevo sombreado, visto de cima (norte = +Z no topo).
fn map_image(hf: &Heightfield, water: f32) -> ColorImage {
    let n = hf.n;
    let mut px = vec![Color32::BLACK; n * n];
    for iz in 0..n {
        for ix in 0..n {
            let h = hf.data[iz * n + ix];
            let xm = hf.data[iz * n + ix.saturating_sub(1)];
            let xp = hf.data[iz * n + (ix + 1).min(n - 1)];
            let zm = hf.data[iz.saturating_sub(1) * n + ix];
            let zp = hf.data[(iz + 1).min(n - 1) * n + ix];
            let shade = (1.0 + ((xm - xp) * 0.7 + (zm - zp) * 0.45) / hf.cell * 0.22).clamp(0.55, 1.45);
            let rel = h - water;
            let c = if rel < 0.0 {
                lerp3([40.0, 125.0, 150.0], [10.0, 32.0, 66.0], -rel / 10.0)
            } else if rel < 1.4 {
                [205.0, 190.0, 140.0]
            } else if rel < 9.0 {
                lerp3([88.0, 128.0, 66.0], [118.0, 112.0, 82.0], (rel - 1.4) / 7.6)
            } else {
                lerp3([125.0, 110.0, 96.0], [238.0, 238.0, 242.0], (rel - 9.0) / 14.0)
            };
            let s = if rel < 0.0 { 1.0 + (shade - 1.0) * 0.3 } else { shade };
            px[(n - 1 - iz) * n + ix] = Color32::from_rgb((c[0] * s).min(255.0) as u8, (c[1] * s).min(255.0) as u8, (c[2] * s).min(255.0) as u8);
        }
    }
    ColorImage { size: [n, n], source_size: vec2(n as f32, n as f32), pixels: px }
}

impl App {
    pub fn terrain_editor(&mut self, ui: &mut Ui) {
        let Some(orig) = self.scene.terrain.clone() else {
            self.editor = Editor::Work;
            return;
        };
        let mut def = orig.clone();
        let mut close: Option<bool> = None; // Some(true)=aceitar, Some(false)=cancelar

        theme::header(ui, "Editor de terreno");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Escolha o tipo de relevo e veja o mapa de cima. Nada vale até você aplicar.").color(theme::pal().dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::button(ui, "Aplicar terreno", true, true, vec2(150.0, 28.0)).clicked() {
                    close = Some(true);
                }
                if theme::button(ui, "Descartar alterações", false, true, vec2(0.0, 28.0)).clicked() {
                    close = Some(false);
                }
            });
        });
        ui.add_space(4.0);

        self.ensure_map();
        let water = self.scene.water.as_ref().map(|w| w.level).unwrap_or(0.0);
        let key = self.map_hf.as_ref().map(|m| format!("{}|{}", m.0, water)).unwrap_or_default();
        if self.map_tex.as_ref().map(|t| t.0 != key).unwrap_or(true) {
            if let Some((_, hf)) = &self.map_hf {
                let img = map_image(hf, water);
                self.map_tex = Some((key, ui.ctx().load_texture("terrainmap", img, egui::TextureOptions::LINEAR)));
            }
        }

        ui.horizontal_top(|ui| {
            // mapa
            let side = (ui.available_width() * 0.62).min(ui.available_height() - 8.0).max(200.0);
            let (rect, r) = ui.allocate_exact_size(vec2(side, side), egui::Sense::hover());
            // Enquadra na ilha: mostra só a região central do terreno.
            let procedural = matches!(def.source, terralux_core::TerrainSource::Procedural);
            let view_half = if procedural && def.kind.has_sea_edge() { (def.island_radius * 2.3).min(def.half_size).max(20.0) } else { def.half_size };
            let f = view_half / def.half_size;
            let uv = egui::Rect::from_min_max(egui::pos2(0.5 - f * 0.5, 0.5 - f * 0.5), egui::pos2(0.5 + f * 0.5, 0.5 + f * 0.5));
            if let Some((_, t)) = &self.map_tex {
                ui.painter().image(t.id(), rect, uv, Color32::WHITE);
            }
            theme::bevel(ui.painter(), rect.expand(2.0), false, Color32::TRANSPARENT);
            let rad = if procedural && def.kind.has_sea_edge() { def.island_radius / view_half * side * 0.5 } else { 0.0 };
            let _unused = def.island_radius / (2.0 * view_half) * side;
            ui.painter().circle_stroke(rect.center(), rad, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 70)));
            ui.painter().text(rect.left_bottom() + vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, format!("região de {:.0} × {:.0} m   ·   norte (+Z) para cima", view_half * 2.0, view_half * 2.0), egui::FontId::proportional(11.0), Color32::from_rgb(235, 240, 250));
            if r.hovered() {
                self.hint = "Mapa de elevação: azul = mar, bege = praia, verde/marrom = terra, branco = pico".into();
            }

            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.set_min_width(300.0);
                egui::Grid::new("terrain_params").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    if !procedural {
                        ui.label("Fonte");
                        if theme::small_button(ui, "Voltar ao procedural", false).clicked() {
                            def.source = terralux_core::TerrainSource::Procedural;
                        }
                        ui.end_row();
                    }
                    if procedural {
                    ui.label("Tipo de relevo");
                    egui::ComboBox::from_id_salt("tkind").selected_text(def.kind.name()).show_ui(ui, |ui| {
                        for k in terralux_core::TerrainKind::ALL {
                            ui.selectable_value(&mut def.kind, k, k.name());
                        }
                    });
                    ui.end_row();
                    ui.label("Cores");
                    egui::ComboBox::from_id_salt("tbiome").selected_text(def.biome.name()).show_ui(ui, |ui| {
                        for b in terralux_core::Biome::ALL {
                            ui.selectable_value(&mut def.biome, b, b.name());
                        }
                    });
                    ui.end_row();
                    ui.label("Semente");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut def.seed));
                        if theme::small_button(ui, "Sortear", false).on_hover_text("Gera um novo terreno").clicked() {
                            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u32).unwrap_or(1);
                            def.seed = t % 100_000;
                        }
                    });
                    ui.end_row();
                    ui.label(def.kind.radius_label());
                    ui.add(egui::Slider::new(&mut def.island_radius, 10.0..=200.0).suffix(" m"));
                    ui.end_row();
                    }
                    ui.label("Altura do pico");
                    ui.add(egui::Slider::new(&mut def.peak_height, 2.0..=200.0).suffix(" m"));
                    ui.end_row();
                    ui.label(if def.kind.has_sea_edge() { "Profundidade do mar" } else { "Profundidade do lago/rio" });
                    ui.add(egui::Slider::new(&mut def.sea_floor, 2.0..=40.0).suffix(" m"));
                    ui.end_row();
                    if procedural {
                        ui.label("Frequência");
                        ui.add(egui::Slider::new(&mut def.frequency, 0.006..=0.08).logarithmic(true));
                        ui.end_row();
                        ui.label("Oitavas (detalhe)");
                        ui.add(egui::Slider::new(&mut def.octaves, 1..=10));
                        ui.end_row();
                    }
                    ui.label("Tamanho do terreno");
                    ui.add(egui::Slider::new(&mut def.half_size, 80.0..=1000.0).suffix(" m (meia largura)"));
                    ui.end_row();
                    ui.label("Resolução final");
                    egui::ComboBox::from_id_salt("tres").selected_text(format!("{0} × {0}", def.resolution)).show_ui(ui, |ui| {
                        for r in [512usize, 513, 1024, 1025, 2048, 2049, 4096, 4097] {
                            ui.selectable_value(&mut def.resolution, r, format!("{r} × {r}"));
                        }
                    });
                    ui.end_row();
                });
                ui.add_space(10.0);
                ui.label(RichText::new("O preview usa resolução reduzida (512²). O render final usa a resolução escolhida aqui.").small().color(theme::pal().dim));
                ui.add_space(6.0);
                ui.label(RichText::new("Esc descarta as alterações e volta ao terreno de antes de abrir o editor.").small().color(theme::pal().dim));
            });
        });

        if close.is_none() && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) && !ui.ctx().egui_wants_keyboard_input() {
            close = Some(false);
        }
        if def != orig {
            self.scene.terrain = Some(def);
        }
        match close {
            Some(true) => self.editor = Editor::Work,
            Some(false) => {
                self.scene.terrain = self.terrain_backup.take();
                self.editor = Editor::Work;
            }
            None => {}
        }
    }
}
