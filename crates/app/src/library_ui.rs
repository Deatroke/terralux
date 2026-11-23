//! Painel esquerdo: abas da biblioteca, grade de miniaturas e inserção na cena.
use crate::app::{App, Sel};
use crate::library::{Item, ItemKind, Tab, THUMB};
use crate::theme;
use egui::{vec2, Color32, RichText, Sense, Stroke, StrokeKind, Ui};
use glam::Vec3;
use terralux_core::*;

impl App {
    /// Gaveta inferior da biblioteca: abas, busca e grade de miniaturas.
    pub fn library_drawer(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.thumbs.poll(&ctx);
        if self.simple && self.lib_tab == Tab::Gels {
            self.lib_tab = Tab::Basicos;
        }
        theme::sheen_after(egui::Panel::bottom("drawer")
            .exact_size((ui.ctx().content_rect().height() * 0.32).clamp(190.0, 256.0))
            .frame(theme::panel_frame().inner_margin(egui::Margin::symmetric(10, 8)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for t in Tab::ALL {
                        if self.simple && t == Tab::Gels {
                            continue;
                        }
                        let n = self.lib.items(t).len();
                        let label = if n > 0 { format!("{} ({})", t.name(), n) } else { t.name().to_string() };
                        if theme::small_button(ui, &label, self.lib_tab == t).clicked() {
                            self.lib_tab = t;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::small_button(ui, "Fechar", false).on_hover_text("Fechar a biblioteca (Esc)").clicked() {
                            self.drawer = false;
                        }
                        ui.add(egui::TextEdit::singleline(&mut self.lib_filter).hint_text("Buscar nesta aba").desired_width(170.0));
                    });
                });
                ui.add_space(4.0);
                let hint = match self.lib_tab {
                    Tab::Texturas => "Escolha um objeto na cena e clique numa textura para aplicá-la.",
                    Tab::Terrenos => "Clique num mapa para usá-lo como terreno.",
                    Tab::Gels => "Gels de luz: ainda sem luzes de ponto ou spot.",
                    _ => "Clique num item para colocá-lo na cena.",
                };
                ui.label(RichText::new(hint).small().color(theme::pal().dim));
                ui.add_space(2.0);
                let mut clicked: Option<Item> = None;
                let h = ui.available_height();
                theme::well(ui, |ui| {
                    ui.set_min_size(vec2(ui.available_width(), h - 14.0));
                    if !self.lib.found && !matches!(self.lib_tab, Tab::Basicos | Tab::Divertidos) {
                        ui.label(RichText::new("Não achei a pasta do CD").color(theme::pal().view_text).strong());
                        ui.label(RichText::new("Aponte a pasta que contém Terrain  Maps, OBJ Models e as outras. Os arquivos só são lidos, nunca copiados.").small().color(theme::pal().view_text));
                        ui.add_space(4.0);
                        if theme::button(ui, "Escolher pasta do CD", false, true, vec2(0.0, 24.0)).clicked() {
                            if let Some(p) = rfd::FileDialog::new().set_directory(assets::library_root()).pick_folder() {
                                assets::set_library_root(p);
                                self.lib = crate::library::Library::scan();
                                self.thumbs.tex.clear();
                            }
                        }
                        return;
                    }
                    let filter = self.lib_filter.to_lowercase();
                    let items: Vec<Item> = self.lib.items(self.lib_tab).iter().filter(|i| filter.is_empty() || i.name.to_lowercase().contains(&filter)).cloned().collect();
                    if items.is_empty() {
                        if self.lib_filter.is_empty() {
                            ui.label(RichText::new("Esta aba está vazia nesta pasta.").color(theme::pal().view_text));
                        } else {
                            ui.label(RichText::new(format!("Nada encontrado para “{}”.", self.lib_filter)).color(theme::pal().view_text));
                            if theme::small_button(ui, "Limpar busca", false).clicked() {
                                self.lib_filter.clear();
                            }
                        }
                        return;
                    }
                    let cell_w = THUMB as f32 + 14.0;
                    let cols = ((ui.available_width() - 4.0) / (cell_w + 6.0)).floor().max(1.0) as usize;
                    egui::ScrollArea::vertical().id_salt("libgrid").auto_shrink([false, false]).show(ui, |ui| {
                        egui::Grid::new("thumbs").spacing([6.0, 8.0]).show(ui, |ui| {
                            for (n, it) in items.iter().enumerate() {
                                self.thumbs.want(it);
                                ui.vertical(|ui| {
                                    ui.set_width(cell_w);
                                    let (rect, r) = ui.allocate_exact_size(vec2(cell_w, THUMB as f32 + 6.0), Sense::click());
                                    let hot = r.hovered();
                                    ui.painter().rect_filled(rect, 2.0, if hot { theme::mix(theme::pal().well, theme::pal().accent, 0.28) } else { theme::mix(theme::pal().well, theme::pal().ink, 0.10) });
                                    if hot {
                                        ui.painter().rect_stroke(rect, 2.0, Stroke::new(2.0, theme::pal().accent), StrokeKind::Inside);
                                    }
                                    let inner = egui::Rect::from_center_size(rect.center(), vec2(THUMB as f32, THUMB as f32));
                                    match self.thumbs.tex.get(&it.spec) {
                                        Some(t) => {
                                            ui.painter().image(t.id(), inner, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                                        }
                                        None => {
                                            ui.painter().text(inner.center(), egui::Align2::CENTER_CENTER, "…", egui::FontId::proportional(18.0), theme::pal().view_text);
                                        }
                                    }
                                    ui.add(egui::Label::new(RichText::new(&it.name).size(11.0).color(if hot { Color32::WHITE } else { theme::pal().view_text })).truncate());
                                    let tip = match it.kind {
                                        ItemKind::Shape(_) => format!("{}: clique para colocar na cena", it.name),
                                        ItemKind::Terrain => format!("{}: clique para usar como terreno", it.name),
                                        ItemKind::Texture => format!("{}: clique para aplicar ao objeto (ou ao terreno) selecionado", it.name),
                                        ItemKind::Picture => format!("{}: clique para colocar como imagem recortada", it.name),
                                        ItemKind::Gel => format!("{}: gel de luz (ainda sem luzes de ponto ou spot)", it.name),
                                    };
                                    if r.clone().on_hover_text(&tip).clicked() {
                                        clicked = Some(it.clone());
                                    }
                                    if hot {
                                        self.hint = tip;
                                    }
                                });
                                if (n + 1) % cols == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
                });
                if let Some(it) = clicked {
                    let keep = matches!(it.kind, ItemKind::Texture);
                    self.insert_item(&it);
                    if self.simple && !keep {
                        self.drawer = false;
                    }
                }
            }), ui);
    }

    /// Altura do chão sob o alvo da câmera (terreno, plano ou 0).
    fn ground_at(&mut self, x: f32, z: f32) -> f32 {
        let mut y = self.scene.objects.iter().filter(|o| matches!(o.shape, Shape::Plane)).map(|o| o.position.y).fold(f32::MIN, f32::max);
        if let Some(hf) = self.pick_heightfield_pub() {
            let w = self.scene.water.as_ref().map_or(f32::MIN, |w| w.level);
            y = y.max(hf.height(x, z).max(w));
        }
        if y == f32::MIN {
            0.0
        } else {
            y
        }
    }

    fn place(&mut self, shape: Shape, name: &str, size: f32, mat: Material) {
        let n = self.scene.objects.iter().filter(|o| o.name.starts_with(name)).count() + 1;
        let t = self.scene.camera.target;
        let plane = matches!(shape, Shape::Plane);
        let picture = matches!(shape, Shape::Picture(_));
        let rot = if picture || plane { Vec3::ZERO } else { Vec3::new(0.0, 20.0, 0.0) };
        // tudo normalizado tem meia-extensão 1: apoia o objeto no chão
        // Empurra para o lado se já houver um objeto no mesmo lugar (inserções em sequência).
        let (mut px, mut pz) = (t.x, t.z);
        if !plane {
            let step = size * 1.1;
            'busca: for ring in 0..12 {
                for k in 0..(if ring == 0 { 1 } else { 6 * ring }) {
                    let (cx, cz) = if ring == 0 {
                        (t.x, t.z)
                    } else {
                        let a = k as f32 / (6 * ring) as f32 * std::f32::consts::TAU;
                        (t.x + a.cos() * step * ring as f32, t.z + a.sin() * step * ring as f32)
                    };
                    let livre = self.scene.objects.iter().all(|o| matches!(o.shape, Shape::Plane) || (o.position.x - cx).hypot(o.position.z - cz) > (o.size + size) * 0.5);
                    if livre {
                        (px, pz) = (cx, cz);
                        break 'busca;
                    }
                }
            }
        }
        let ground = self.ground_at(px, pz);
        let y = if plane { ground } else { ground + size * if picture { 0.6 } else { 1.0 } };
        self.scene.objects.push(Object { name: if plane { name.into() } else { format!("{name} {n}") }, shape, position: Vec3::new(px, y, pz), rotation_deg: rot, size, material: mat });
        self.sel = Sel::Object(self.scene.objects.len() - 1);
        let nome = self.scene.objects.last().map(|o| o.name.clone()).unwrap_or_default();
        self.notify(format!("“{nome}” adicionado à cena."), crate::toast::ToastKind::Info, true);
    }

    pub fn insert_item(&mut self, it: &Item) {
        match &it.kind {
            ItemKind::Shape(sh) => {
                let size = match sh {
                    Shape::Sphere | Shape::Poly(_) => 5.0,
                    Shape::Plane => 1.0,
                    Shape::Prim(_) => 5.0,
                    _ => 6.0,
                };
                let mat = Material::matte(Vec3::splat(0.88));
                self.place(sh.clone(), &it.name, size, mat);
            }
            ItemKind::Picture => {
                let mut mat = Material::matte(Vec3::ONE);
                mat.specular = 0.0;
                self.place(Shape::Picture(it.spec.clone()), &it.name, 8.0, mat);
            }
            ItemKind::Terrain => {
                let mut t = TerrainDef::from_image(&it.spec);
                // um lago de baixa profundidade aparece no mapa: mantém o texture atual
                t.texture = self.scene.terrain.as_ref().and_then(|o| o.texture.clone());
                self.scene.terrain = Some(t);
                self.scene.water.get_or_insert_with(Water::sea);
                self.frame_terrain();
                self.sel = Sel::Terrain;
            }
            ItemKind::Texture => {
                let tref = |scale, m| TextureRef::new(&it.spec, scale, m);
                match self.sel {
                    Sel::Object(i) if i < self.scene.objects.len() => {
                        let o = &mut self.scene.objects[i];
                        let (scale, m) = if matches!(o.shape, Shape::Plane) { (10.0, Mapping::Planar) } else { (o.size.max(1.0), Mapping::Box) };
                        o.material.texture = Some(tref(scale, m));
                        o.material.color = Vec3::ONE;
                        o.material.reflectivity = o.material.reflectivity.min(0.1);
                        let msg = format!("Textura “{}” aplicada a “{}”.", it.name, o.name);
                        self.notify(msg, crate::toast::ToastKind::Info, true);
                    }
                    _ if self.scene.terrain.is_some() => {
                        self.scene.terrain.as_mut().unwrap().texture = Some(tref(40.0, Mapping::Planar));
                        self.sel = Sel::Terrain;
                        self.notify(format!("Textura “{}” aplicada ao terreno.", it.name), crate::toast::ToastKind::Info, true);
                    }
                    _ => self.info("Para aplicar uma textura, selecione um objeto na cena ou crie um terreno."),
                }
            }
            ItemKind::Gel => self.info("Gels precisam de luzes de ponto ou spot para projetar luz, e elas ainda não existem no Terralux."),
        }
    }

    /// Enquadra o terreno inteiro na câmera.
    pub fn frame_terrain(&mut self) {
        if let Some(t) = &self.scene.terrain {
            let h = t.half_size;
            self.scene.camera.target = Vec3::new(0.0, t.peak_height * 0.2, 0.0);
            self.scene.camera.position = Vec3::new(h * 0.15, t.peak_height * 1.2 + h * 0.35, -h * 1.45);
        }
    }
}
