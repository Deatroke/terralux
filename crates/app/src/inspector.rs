//! Painel direito: o que está selecionado e o que dá para mudar nele.
//! Modo simples: poucos controles grandes. Modo avançado: tudo, em seções.
use crate::app::{App, Sel};
use crate::theme;
use egui::{vec2, Margin, RichText, Ui};
use glam::Vec3;
use std::ops::RangeInclusive;
use terralux_core::*;

fn vec3_row(ui: &mut Ui, label: &str, v: &mut Vec3, speed: f32, suffix: &str) {
    ui.label(RichText::new(label).color(theme::pal().dim));
    ui.horizontal(|ui| {
        for c in [&mut v.x, &mut v.y, &mut v.z] {
            ui.add(egui::DragValue::new(c).speed(speed).suffix(suffix));
        }
    });
}

fn color_row(ui: &mut Ui, label: &str, v: &mut Vec3) {
    ui.horizontal(|ui| {
        let mut a = v.to_array();
        if ui.color_edit_button_rgb(&mut a).changed() {
            *v = Vec3::from(a);
        }
        ui.label(label);
    });
}

/// Slider com a caixa de valor de largura FIXA ao lado. A largura do slider é calculada a partir do que
/// sobra na linha, então o conteúdo nunca pede mais do que o painel tem (sem crescer sozinho).
fn slider_row(ui: &mut Ui, v: &mut f32, r: RangeInclusive<f32>, log: bool) {
    const BOX: f32 = 58.0;
    ui.horizontal(|ui| {
        let w = (ui.available_width() - BOX - ui.spacing().item_spacing.x - 6.0).clamp(50.0, 360.0);
        ui.spacing_mut().slider_width = w;
        let mut s = egui::Slider::new(v, r.clone()).show_value(false).clamping(egui::SliderClamping::Always);
        if log {
            s = s.logarithmic(true);
        }
        ui.add(s);
        let span = r.end() - r.start();
        let dec = if span < 0.1 { 4 } else if span < 10.0 { 3 } else { 1 };
        ui.allocate_ui_with_layout(vec2(BOX, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_max_width(BOX);
            ui.style_mut().spacing.interact_size.x = BOX;
            ui.add(egui::DragValue::new(v).speed(span * if log { 0.0005 } else { 0.002 }).range(r).max_decimals(dec));
        });
    });
}

fn slider(ui: &mut Ui, label: &str, v: &mut f32, r: RangeInclusive<f32>) {
    ui.label(RichText::new(label).color(theme::pal().dim));
    slider_row(ui, v, r, false);
}

fn big_button(ui: &mut Ui, label: &str, primary: bool) -> bool {
    theme::button(ui, label, primary, true, vec2(ui.available_width().min(140.0), 28.0)).clicked()
}

impl App {
    pub fn inspector(&mut self, ui: &mut Ui) {
        theme::sheen_after(egui::Panel::right("inspector").default_size(300.0).min_size(250.0).max_size(440.0).resizable(true).frame(theme::panel_frame().inner_margin(Margin::same(8))).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            let title = match self.sel {
                Sel::None => "Nada selecionado".to_string(),
                Sel::Sky => "Céu e luz".to_string(),
                Sel::Terrain => "Terreno".to_string(),
                Sel::Water => "Água".to_string(),
                Sel::Object(i) => self.scene.objects.get(i).map(|o| o.name.clone()).unwrap_or_default(),
            };
            theme::header(ui, &title);
            egui::ScrollArea::vertical().id_salt("props").auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(2.0);
                match self.sel {
                    Sel::None => self.empty_panel(ui),
                    Sel::Sky => self.sky_panel(ui),
                    Sel::Terrain => self.terrain_panel(ui),
                    Sel::Water => self.water_panel(ui),
                    Sel::Object(i) => self.object_panel(ui, i),
                }
                ui.add_space(8.0);
                self.scene_list(ui);
            });
        }), ui);
    }

    fn empty_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Clique em algo na cena ou escolha um item na lista abaixo para ver e mudar suas opções.").color(theme::pal().dim));
        ui.add_space(4.0);
        if big_button(ui, "Ajustar o céu", false) {
            self.sel = Sel::Sky;
        }
    }

    fn scene_list(&mut self, ui: &mut Ui) {
        let n = self.scene.objects.len() + 1 + self.scene.terrain.is_some() as usize + self.scene.water.is_some() as usize;
        egui::CollapsingHeader::new(RichText::new(format!("Tudo na cena ({n})")).strong().color(theme::pal().heading)).id_salt(("scene", self.simple)).default_open(!self.simple).show(ui, |ui| {
            ui.selectable_value(&mut self.sel, Sel::Sky, "Céu e luz");
            if self.scene.terrain.is_some() {
                ui.selectable_value(&mut self.sel, Sel::Terrain, "Terreno");
            }
            if self.scene.water.is_some() {
                ui.selectable_value(&mut self.sel, Sel::Water, "Água");
            }
            for i in 0..self.scene.objects.len() {
                let name = self.scene.objects[i].name.clone();
                ui.selectable_value(&mut self.sel, Sel::Object(i), name);
            }
        });
    }

    fn object_panel(&mut self, ui: &mut Ui, i: usize) {
        let simple = self.simple;
        let mut act = 0; // 1 duplicar, 2 excluir
        if let Some(o) = self.scene.objects.get_mut(i) {
            ui.add(egui::TextEdit::singleline(&mut o.name).desired_width(f32::INFINITY));
            ui.label(RichText::new(o.shape.label()).small().color(theme::pal().dim));
            if simple {
                slider(ui, "Tamanho (m)", &mut o.size, 0.5..=80.0);
                slider(ui, "Girar (graus)", &mut o.rotation_deg.y, -180.0..=180.0);
                ui.add_space(2.0);
                color_row(ui, "Cor", &mut o.material.color);
                slider(ui, "Reflexo", &mut o.material.reflectivity, 0.0..=1.0);
                egui::CollapsingHeader::new(RichText::new("Mais opções").strong().color(theme::pal().heading)).default_open(false).show(ui, |ui| {
                    vec3_row(ui, "Posição (m)", &mut o.position, 0.2, "");
                    vec3_row(ui, "Rotação", &mut o.rotation_deg, 0.5, "°");
                    slider(ui, "Brilho do reflexo", &mut o.material.specular, 0.0..=2.0);
                    texture_section(ui, &mut o.material.texture, true);
                });
            } else {
                theme::section_title(ui, "Posição e forma");
                vec3_row(ui, "Posição (m)", &mut o.position, 0.2, "");
                vec3_row(ui, "Rotação", &mut o.rotation_deg, 0.5, "°");
                ui.label(RichText::new("Tamanho (m)").color(theme::pal().dim));
                ui.add(egui::DragValue::new(&mut o.size).speed(0.05).range(0.2..=400.0).suffix(" m"));
                theme::section_title(ui, "Material");
                color_row(ui, "Cor", &mut o.material.color);
                slider(ui, "Reflexo", &mut o.material.reflectivity, 0.0..=1.0);
                slider(ui, "Brilho do reflexo", &mut o.material.specular, 0.0..=2.0);
                slider(ui, "Dureza do brilho", &mut o.material.shininess, 2.0..=600.0);
                texture_section(ui, &mut o.material.texture, true);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if big_button(ui, "Duplicar", false) {
                    act = 1;
                }
                if big_button(ui, "Excluir", false) {
                    act = 2;
                }
            });
        }
        match act {
            1 => self.duplicate_selected(),
            2 => self.delete_selected(),
            _ => {}
        }
    }

    fn terrain_panel(&mut self, ui: &mut Ui) {
        let simple = self.simple;
        let mut edit = false;
        let mut del = false;
        if let Some(t) = &mut self.scene.terrain {
            let src = match &t.source {
                TerrainSource::Procedural => format!("Gerado pelo programa (semente {})", t.seed),
                TerrainSource::Image(p) => format!("Mapa: {}", p.rsplit('/').next().unwrap_or(p)),
            };
            ui.label(RichText::new(src).small().color(theme::pal().dim));
            slider(ui, "Altura das montanhas (m)", &mut t.peak_height, 2.0..=120.0);
            if !simple {
                slider(ui, "Profundidade do mar (m)", &mut t.sea_floor, 1.0..=40.0);
                slider(ui, "Largura do terreno (m, metade)", &mut t.half_size, 40.0..=600.0);
                ui.label(RichText::new(format!("grade {0}² · {1:.0} × {1:.0} m", t.resolution, t.half_size * 2.0)).small().color(theme::pal().dim));
            }
            ui.add_space(2.0);
            if simple {
                egui::CollapsingHeader::new(RichText::new("Mais opções").strong().color(theme::pal().heading)).default_open(false).show(ui, |ui| {
                    slider(ui, "Profundidade do mar (m)", &mut t.sea_floor, 1.0..=40.0);
                    slider(ui, "Largura do terreno (m, metade)", &mut t.half_size, 40.0..=600.0);
                    texture_section(ui, &mut t.texture, false);
                });
            } else {
                texture_section(ui, &mut t.texture, false);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                edit = theme::button(ui, "Editar terreno", true, true, vec2(130.0, 28.0)).clicked();
                del = big_button(ui, "Excluir", false);
            });
        }
        if edit {
            self.open_terrain_editor();
        }
        if del {
            self.delete_selected();
        }
    }

    fn water_panel(&mut self, ui: &mut Ui) {
        let simple = self.simple;
        let mut remove = false;
        if let Some(w) = &mut self.scene.water {
            slider(ui, "Nível da água (m)", &mut w.level, -20.0..=40.0);
            slider(ui, "Altura das ondas", &mut w.wave_height, 0.0..=2.0);
            if !simple {
                slider(ui, "Tamanho das ondas", &mut w.wave_scale, 0.2..=5.0);
                vec3_row(ui, "Absorção (vermelho, verde, azul)", &mut w.absorption, 0.01, "");
                color_row(ui, "Cor do fundo", &mut w.deep_color);
            }
            ui.add_space(6.0);
            remove = big_button(ui, "Excluir água", false);
        }
        if remove {
            self.delete_selected();
        }
    }

    fn sky_panel(&mut self, ui: &mut Ui) {
        let simple = self.simple;
        let mut preset: Option<&str> = None;
        ui.label(RichText::new("Modelos prontos").color(theme::pal().dim));
        ui.horizontal_wrapped(|ui| {
            for n in ["Dia claro", "Fim de tarde", "Nublado", "Névoa da manhã"] {
                if theme::small_button(ui, n, false).clicked() {
                    preset = Some(n);
                }
            }
        });
        let s = &mut self.scene;
        theme::section_title(ui, "Sol");
        slider(ui, "Altura do sol (graus)", &mut s.sun.elevation_deg, -5.0..=90.0);
        slider(ui, "Direção do sol (graus)", &mut s.sun.azimuth_deg, -180.0..=180.0);
        if !simple {
            slider(ui, "Intensidade", &mut s.sun.intensity, 0.0..=6.0);
            color_row(ui, "Cor da luz", &mut s.sun.color);
        }
        theme::section_title(ui, "Nuvens");
        let mut on = s.sky.clouds.is_some();
        if ui.checkbox(&mut on, "Mostrar nuvens").changed() {
            s.sky.clouds = if on { Some(Clouds::fair()) } else { None };
        }
        if let Some(c) = &mut s.sky.clouds {
            slider(ui, "Quantidade", &mut c.coverage, 0.0..=1.0);
            if !simple {
                slider(ui, "Altitude (m)", &mut c.altitude, 150.0..=3000.0);
                slider(ui, "Tamanho", &mut c.scale, 0.2..=4.0);
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut c.seed));
                    ui.label("Semente");
                });
            }
        }
        if simple {
            egui::CollapsingHeader::new(RichText::new("Mais opções").strong().color(theme::pal().heading)).default_open(false).show(ui, |ui| {
                slider(ui, "Névoa", &mut s.sky.haze, 0.0..=0.02);
                slider(ui, "Estrelas", &mut s.sky.stars, 0.0..=3.0);
                slider(ui, "Campo de visão", &mut s.camera.fov_deg, 15.0..=110.0);
                slider(ui, "Exposição", &mut s.camera.exposure, 0.2..=3.0);
            });
        } else {
            theme::section_title(ui, "Atmosfera");
            color_row(ui, "Cor do alto do céu", &mut s.sky.zenith);
            color_row(ui, "Cor do horizonte", &mut s.sky.horizon);
            ui.label(RichText::new("Névoa").color(theme::pal().dim));
            slider_row(ui, &mut s.sky.haze, 0.0..=0.02, false);
            ui.label(RichText::new("Estrelas (aparecem no céu escuro)").color(theme::pal().dim));
            slider_row(ui, &mut s.sky.stars, 0.0..=3.0, false);
            theme::section_title(ui, "Câmera");
            slider(ui, "Campo de visão", &mut s.camera.fov_deg, 15.0..=110.0);
            slider(ui, "Exposição", &mut s.camera.exposure, 0.2..=3.0);
            vec3_row(ui, "Posição", &mut s.camera.position, 0.5, "");
            vec3_row(ui, "Olhando para", &mut s.camera.target, 0.5, "");
        }
        if let Some(n) = preset {
            self.apply_sky_preset(n);
        }
    }
}

/// Seção de textura: mostra o arquivo, a escala e o mapeamento; permite remover.
fn texture_section(ui: &mut Ui, tex: &mut Option<TextureRef>, mapping: bool) {
    theme::section_title(ui, "Textura");
    let mut remove = false;
    match tex {
        Some(t) => {
            ui.label(RichText::new(t.path.rsplit('/').next().unwrap_or(&t.path)).color(theme::pal().ink));
            slider(ui, if mapping { "Escala" } else { "Metros por repetição" }, &mut t.scale, 0.1..=200.0);
            if mapping {
                ui.label(RichText::new("Mapeamento").color(theme::pal().dim));
                egui::ComboBox::from_id_salt("texmap").selected_text(t.mapping.name()).show_ui(ui, |ui| {
                    for m in Mapping::ALL {
                        ui.selectable_value(&mut t.mapping, m, m.name());
                    }
                });
            }
            remove = theme::small_button(ui, "Remover textura", false).clicked();
        }
        None => {
            ui.label(RichText::new("Nenhuma. Abra a biblioteca de texturas (Criar › Texturas) e clique numa.").small().color(theme::pal().dim));
        }
    }
    if remove {
        *tex = None;
    }
}
