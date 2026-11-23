//! Casca da janela: barra de menus, paleta por abas, coluna de visão/render,
//! barra de status, guia "Comece aqui" e controle de órbita sobre a cena.
use crate::app::{App, Display, Editor, Sel, Tool, QUALITIES, RESOLUTIONS};
use crate::aero;
use crate::icons::{self, Icon};
use crate::library::Tab;
use crate::theme;
use egui::{pos2, vec2, Color32, Context, Margin, Rect, RichText, Sense, Stroke, Ui};
use std::f32::consts::{FRAC_PI_2, PI};
use terralux_core::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Palette {
    Criar,
    Editar,
    Ceu,
}

impl Palette {
    pub fn from_key(k: &str) -> Option<Self> {
        match k {
            "criar" => Some(Palette::Criar),
            "editar" => Some(Palette::Editar),
            "ceu" => Some(Palette::Ceu),
            _ => None,
        }
    }
}

/// Dica de uma linha para cada aba da paleta (ajuda no contexto).
fn palette_hint(p: Palette) -> &'static str {
    match p {
        Palette::Criar => "Escolha o que colocar na cena. Os itens da biblioteca abrem na gaveta de baixo.",
        Palette::Editar => "Selecione um objeto na cena e use as ferramentas para movê-lo, girá-lo ou mudar o tamanho.",
        Palette::Ceu => "Um clique muda a luz e o clima da cena inteira. Dá para desfazer.",
    }
}

fn srgb(c: [f32; 3]) -> Color32 {
    Color32::from_rgb((c[0].clamp(0.0, 1.0) * 255.0) as u8, (c[1].clamp(0.0, 1.0) * 255.0) as u8, (c[2].clamp(0.0, 1.0) * 255.0) as u8)
}

/// Botão com amostra de céu pintada (gradiente zênite → horizonte).
fn sky_tile(ui: &mut Ui, label: &str, top: [f32; 3], bottom: [f32; 3]) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(104.0, 66.0), Sense::click());
    let down = resp.is_pointer_button_down_on();
    theme::bevel(ui.painter(), rect, !down, if resp.hovered() { theme::pal().hover } else { theme::pal().light });
    let off = if down { vec2(1.0, 1.0) } else { vec2(0.0, 0.0) };
    let sw = Rect::from_min_size(rect.min + vec2(9.0, 8.0) + off, vec2(rect.width() - 18.0, 30.0));
    theme::vgrad(ui.painter(), sw, srgb(top), srgb(bottom));
    ui.painter().rect_stroke(sw, 0.0, Stroke::new(1.0, theme::pal().dk), egui::StrokeKind::Outside);
    ui.painter().text(rect.center_bottom() - vec2(0.0, 11.0) + off, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.0), theme::pal().ink);
    resp
}

#[derive(Clone, Copy)]
enum Glyph {
    Left,
    Right,
    Up,
    Down,
    Plus,
    Minus,
    Home,
}

/// Botão redondo (esfera de vidro azul) com um desenho branco; devolve a resposta (para clique e para "segurar").
fn glyph_button(ui: &mut Ui, g: Glyph) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    let down = resp.is_pointer_button_down_on();
    let c = rect.center() + if down { vec2(0.0, 1.0) } else { vec2(0.0, 0.0) };
    let p = ui.painter();
    let w = theme::orb(p, c, 27.0, theme::OrbState::Normal, resp.hovered() && !down);
    let r = Rect::from_center_size(c, vec2(15.0, 15.0));
    let pi = std::f32::consts::PI;
    match g {
        Glyph::Up => { aero::glyph(p, "g_uparrow", r, w, 0.0, false); }
        Glyph::Down => { aero::glyph(p, "g_uparrow", r, w, pi, false); }
        Glyph::Left => { aero::glyph(p, "g_uparrow", r, w, -pi / 2.0, false); }
        Glyph::Right => { aero::glyph(p, "g_uparrow", r, w, pi / 2.0, false); }
        Glyph::Plus => { aero::glyph(p, "g_plus", r, w, 0.0, false); }
        Glyph::Minus => { aero::glyph(p, "g_minus", r, w, 0.0, false); }
        Glyph::Home => icons::draw(p, r.expand(1.0), Icon::Home, w),
    }
    resp
}

impl App {
    // ---------- barra de menus ----------

    pub fn menu_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        theme::sheen_after(egui::Panel::top("menu").frame(theme::panel_frame().inner_margin(Margin::symmetric(8, 3))).show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Arquivo", |ui| {
                    if ui.button("Novo / modelos…   Ctrl+N").clicked() {
                        self.open_start(true);
                        ui.close();
                    }
                    if ui.button("Abrir…   Ctrl+O").clicked() {
                        ui.close();
                        self.file_open();
                    }
                    ui.menu_button("Abrir recente", |ui| {
                        if self.home.recent.is_empty() {
                            ui.label("(nenhum ainda)");
                        }
                        let mut go = None;
                        for p in &self.home.recent {
                            let n = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            if ui.button(n).on_hover_text(p.display().to_string()).clicked() {
                                go = Some(p.clone());
                            }
                        }
                        if let Some(p) = go {
                            ui.close();
                            self.open_path(&p);
                        }
                    });
                    if ui.button("Salvar   Ctrl+S").clicked() {
                        ui.close();
                        self.file_save(false);
                    }
                    if ui.button("Salvar como…   Ctrl+Shift+S").clicked() {
                        ui.close();
                        self.file_save(true);
                    }
                    ui.separator();
                    if ui.button("Importar modelo 3D…   (glb, gltf, obj, stl, ply)").clicked() {
                        ui.close();
                        self.file_import_model();
                    }
                    ui.menu_button("Exportar", |ui| {
                        if ui.button("Cena 3D (.glb) — Unity, Unreal, Godot, Blender").clicked() {
                            ui.close();
                            self.file_export_scene("glb");
                        }
                        if ui.button("Cena 3D (.obj + .mtl)").clicked() {
                            ui.close();
                            self.file_export_scene("obj");
                        }
                        if ui.button("Cena 3D (.stl) — impressão 3D").clicked() {
                            ui.close();
                            self.file_export_scene("stl");
                        }
                        ui.separator();
                        let has_t = self.scene.terrain.is_some();
                        if ui.add_enabled(has_t, egui::Button::new("Mapa de altura 16 bits (.png)")).clicked() {
                            ui.close();
                            self.file_export_heightmap(false);
                        }
                        if ui.add_enabled(has_t, egui::Button::new("Mapa de altura RAW 16 bits (.raw) — Unity/Unreal")).clicked() {
                            ui.close();
                            self.file_export_heightmap(true);
                        }
                    });
                    ui.separator();
                    if ui.button("Renderizar imagem   F12").clicked() {
                        ui.close();
                        self.start_render(&ctx);
                    }
                    ui.separator();
                    if ui.button("Sair").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Editar", |ui| {
                    if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Desfazer   Ctrl+Z")).clicked() {
                        self.undo();
                        ui.close();
                    }
                    if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Refazer   Ctrl+Shift+Z")).clicked() {
                        self.redo();
                        ui.close();
                    }
                    ui.separator();
                    let has_obj = matches!(self.sel, Sel::Object(_));
                    if ui.add_enabled(has_obj, egui::Button::new("Duplicar objeto   Shift+D")).on_disabled_hover_text("Selecione um objeto na cena para duplicá-lo").clicked() {
                        self.duplicate_selected();
                        ui.close();
                    }
                    if ui.add_enabled(!matches!(self.sel, Sel::None | Sel::Sky), egui::Button::new("Excluir seleção   X")).on_disabled_hover_text("Selecione um objeto, o terreno ou a água para excluir").clicked() {
                        self.delete_selected();
                        ui.close();
                    }
                    if ui.button("Desselecionar   Alt+A").clicked() {
                        self.sel = Sel::None;
                        ui.close();
                    }
                });
                ui.menu_button("Objetos", |ui| {
                    if ui.button("Terreno…").clicked() {
                        self.open_terrain_editor();
                        ui.close();
                    }
                    if ui.button("Plano de água").clicked() {
                        self.scene.water.get_or_insert_with(Water::sea);
                        self.sel = Sel::Water;
                        ui.close();
                    }
                    if ui.button("Esfera").clicked() {
                        self.create_shape(Shape::Sphere, "Esfera", 5.0);
                        ui.close();
                    }
                    for p in Polyhedron::ALL {
                        if ui.button(p.name()).clicked() {
                            self.create_shape(Shape::Poly(p), p.name(), 5.0);
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Céu", |ui| {
                    for n in ["Dia claro", "Fim de tarde", "Nublado", "Névoa da manhã"] {
                        if ui.button(n).clicked() {
                            self.apply_sky_preset(n);
                            self.sel = Sel::Sky;
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Exibir", |ui| {
                    ui.selectable_value(&mut self.display, Display::Preview, "Pré-visualização");
                    ui.selectable_value(&mut self.display, Display::Wire, "Wireframe");
                    ui.selectable_value(&mut self.display, Display::Both, "Ambos");
                    ui.separator();
                    ui.checkbox(&mut self.show_side, "Coluna de vistas   T");
                    ui.checkbox(&mut self.show_inspector, "Propriedades   N");
                    if ui.button("Biblioteca").clicked() {
                        self.drawer = !self.drawer;
                        ui.close();
                    }
                    self.theme_menu(ui);
                    if ui.button("Voltar à janela de trabalho").clicked() {
                        self.editor = Editor::Work;
                        ui.close();
                    }
                });
                ui.menu_button("Ajuda", |ui| {
                    if ui.button("Mostrar o guia “Comece aqui”").clicked() {
                        self.welcome = true;
                        self.save_ui();
                        ui.close();
                    }
                    if ui.button("Atalhos de teclado   F1").clicked() {
                        self.help_open = true;
                        ui.close();
                    }
                    ui.separator();
                    ui.label(RichText::new("Orbitar: botão do meio · Deslocar: Shift + meio · Zoom: roda").color(theme::pal().dim));
                    ui.label(RichText::new("G mover · R girar · S escalar · Shift+A adicionar").color(theme::pal().dim));
                    ui.separator();
                    ui.label(RichText::new("Créditos dos ícones").strong());
                    ui.label(RichText::new("Esferas, cápsulas e ícones brancos: “AeroMedia: Frutiger Aero UI”,").color(theme::pal().dim));
                    ui.label(RichText::new("de OnNOff (onnoff.itch.io/aero-media), licença CC BY 4.0.").color(theme::pal().dim));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let r = theme::small_button_w(ui, "Avançado", !self.simple, 74.0);
                    if self.tip(r.clone(), "Mostra todas as opções: posição exata, material, gels e memórias de câmera").clicked() {
                        self.simple = false;
                        self.save_ui();
                    }
                    let r = theme::small_button_w(ui, "Simples", self.simple, 74.0);
                    if self.tip(r.clone(), "Mostra só o essencial, com botões grandes").clicked() {
                        self.simple = true;
                        self.save_ui();
                    }
                    ui.label(RichText::new("Modo").color(theme::pal().dim));
                });
            });
        }), ui);
    }

    // ---------- paleta por abas ----------

    pub fn palette_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        theme::sheen_after(egui::Panel::top("palette").frame(theme::panel_frame().inner_margin(Margin { left: 10, right: 10, top: 6, bottom: 8 })).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
            let mut active_tab: Option<Rect> = None;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (p, n) in [(Palette::Criar, "Criar"), (Palette::Editar, "Editar"), (Palette::Ceu, "Céu")] {
                    let r = theme::tab(ui, n, self.palette == p);
                    if self.palette == p {
                        active_tab = Some(r.rect);
                    }
                    if r.clicked() {
                        self.palette = p;
                    }
                }
                ui.add_space(14.0);
                ui.label(RichText::new(palette_hint(self.palette)).color(theme::pal().dim));
            });
            let card_top = ui.cursor().top();
            theme::card(ui, |ui| {
                ui.set_min_width(ui.available_width() - 16.0);
                ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                ui.horizontal(|ui| match self.palette {
                    Palette::Criar => self.tiles_criar(ui),
                    Palette::Editar => self.tiles_editar(ui, &ctx),
                    Palette::Ceu => self.tiles_ceu(ui),
                });
            });
            if let Some(t) = active_tab {
                // a aba ativa "abre" na faixa: apaga a linha de cima da faixa sob ela
                ui.painter().line_segment([pos2(t.left() + 2.0, card_top + 0.5), pos2(t.right() - 2.0, card_top + 0.5)], Stroke::new(2.0, theme::pal().stone));
            }
        }), ui);
    }

    fn lib_tile(&mut self, ui: &mut Ui, icon: Icon, label: &str, tab: Tab, tip: &str) {
        let active = self.drawer && self.lib_tab == tab;
        let r = theme::tile(ui, icon, label, active, true);
        if self.tip(r, tip).clicked() {
            if active {
                self.drawer = false;
            } else {
                self.lib_tab = tab;
                self.drawer = true;
            }
        }
    }

    fn tiles_criar(&mut self, ui: &mut Ui) {
        let r = theme::tile(ui, Icon::Terrain, "Terreno", self.editor == Editor::Terrain, true);
        if self.tip(r, "Cria o terreno, ou abre o editor para esculpir montanhas e vales").clicked() {
            self.open_terrain_editor();
        }
        let r = theme::tile(ui, Icon::Water, "Água", self.sel == Sel::Water, true);
        if self.tip(r, "Coloca um mar ou lago que se estende até o horizonte").clicked() {
            self.scene.water.get_or_insert_with(Water::sea);
            self.sel = Sel::Water;
        }
        let r = theme::tile(ui, Icon::Cloud, "Nuvens", false, true);
        if self.tip(r, "Liga a camada de nuvens do céu").clicked() {
            self.scene.sky.clouds.get_or_insert_with(Clouds::fair);
            self.sel = Sel::Sky;
        }
        theme::vsep(ui, 66.0);
        self.lib_tile(ui, Icon::Cube, "Formas", Tab::Basicos, "Esferas, cubos e outras formas simples");
        self.lib_tile(ui, Icon::Skull, "Divertidos", Tab::Divertidos, "Caveira, ossos e outras peças de brincadeira");
        self.lib_tile(ui, Icon::Tree, "Modelos", Tab::Modelos, "Árvores, rochas e outros modelos 3D do CD");
        self.lib_tile(ui, Icon::Picture, "Imagens", Tab::Imagens, "Figuras recortadas para colocar na cena");
        self.lib_tile(ui, Icon::Checker, "Texturas", Tab::Texturas, "Superfícies para pintar objetos e terreno");
        self.lib_tile(ui, Icon::Map, "Mapas", Tab::Terrenos, "Terrenos prontos (mapas de altura)");
        if !self.simple {
            self.lib_tile(ui, Icon::Gel, "Gels", Tab::Gels, "Gels de luz (ainda sem luzes de ponto ou spot)");
        }
    }

    fn tiles_editar(&mut self, ui: &mut Ui, ctx: &Context) {
        let _ = ctx;
        for (t, ic, n, tx) in [
            (Tool::Select, Icon::Select, "Selecionar", "Clique num objeto para selecioná-lo; arraste para orbitar"),
            (Tool::Move, Icon::Move, "Mover", "Arraste o objeto selecionado (Shift = subir e descer)"),
            (Tool::Rotate, Icon::Rotate, "Girar", "Arraste para girar o objeto selecionado"),
            (Tool::Scale, Icon::Scale, "Tamanho", "Arraste para aumentar ou diminuir o objeto selecionado"),
        ] {
            let r = theme::tile(ui, ic, n, self.tool == t, true);
            if self.tip(r, tx).clicked() {
                self.tool = t;
            }
        }
        theme::vsep(ui, 66.0);
        let has_obj = matches!(self.sel, Sel::Object(_));
        let r = theme::tile(ui, Icon::Copy, "Duplicar", false, has_obj);
        if self.tip(r, if has_obj { "Cria uma cópia do objeto selecionado (Ctrl+D)" } else { "Selecione um objeto na cena para duplicá-lo" }).clicked() && has_obj {
            self.duplicate_selected();
        }
        let can_del = !matches!(self.sel, Sel::None | Sel::Sky);
        let r = theme::tile(ui, Icon::Trash, "Excluir", false, can_del);
        if self.tip(r, if can_del { "Exclui a seleção. Dá para desfazer (Del)" } else { "Selecione um objeto, o terreno ou a água para excluir" }).clicked() && can_del {
            self.delete_selected();
        }
        theme::vsep(ui, 66.0);
        let can = !self.undo.is_empty();
        let r = theme::tile(ui, Icon::Undo, "Desfazer", false, can);
        if self.tip(r, if can { "Desfaz a última mudança (Ctrl+Z)" } else { "Nada para desfazer ainda" }).clicked() && can {
            self.undo();
        }
        let can = !self.redo.is_empty();
        let r = theme::tile(ui, Icon::Redo, "Refazer", false, can);
        if self.tip(r, if can { "Refaz o que foi desfeito (Ctrl+Shift+Z)" } else { "Nada para refazer" }).clicked() && can {
            self.redo();
        }
    }

    fn tiles_ceu(&mut self, ui: &mut Ui) {
        for (n, top, bottom, tx) in [
            ("Dia claro", [0.10, 0.28, 0.70], [0.78, 0.84, 0.92], "Sol alto e céu azul"),
            ("Fim de tarde", [0.10, 0.18, 0.42], [0.95, 0.62, 0.42], "Sol baixo, luz alaranjada"),
            ("Nublado", [0.34, 0.41, 0.50], [0.62, 0.66, 0.70], "Céu coberto, luz suave"),
            ("Névoa da manhã", [0.30, 0.46, 0.70], [0.85, 0.82, 0.85], "Neblina e sol rasante"),
        ] {
            let r = sky_tile(ui, n, top, bottom);
            if self.tip(r, tx).clicked() {
                self.apply_sky_preset(n);
                self.sel = Sel::Sky;
            }
        }
        theme::vsep(ui, 66.0);
        let r = theme::tile(ui, Icon::Sun, "Ajustar", self.sel == Sel::Sky, true);
        if self.tip(r, "Abre os ajustes do sol, das nuvens e da névoa").clicked() {
            self.sel = Sel::Sky;
        }
    }

    // ---------- coluna esquerda: visão e render ----------

    pub fn side_panel(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        theme::sheen_after(egui::Panel::left("side").exact_size(186.0).frame(theme::panel_frame().inner_margin(Margin::symmetric(10, 8))).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            // O bloco de render fica preso embaixo, para o botão principal nunca sair da tela.
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.label(RichText::new("Atalho: F12").small().color(theme::pal().dim));
                let r = theme::button(ui, "Renderizar cena", true, true, vec2(162.0, 40.0));
                if self.tip(r, "Gera a imagem final no tamanho e na qualidade escolhidos (F12)").clicked() {
                    self.start_render(&ctx);
                }
                egui::ComboBox::from_id_salt("qual").width(162.0).selected_text(QUALITIES[self.quality_idx].0).show_ui(ui, |ui| {
                    for (i, q) in QUALITIES.iter().enumerate() {
                        ui.selectable_value(&mut self.quality_idx, i, q.0);
                    }
                });
                ui.label(RichText::new("Qualidade").color(theme::pal().dim).small());
                egui::ComboBox::from_id_salt("res").width(162.0).selected_text(RESOLUTIONS[self.res_idx].0).show_ui(ui, |ui| {
                    for (i, r) in RESOLUTIONS.iter().enumerate() {
                        ui.selectable_value(&mut self.res_idx, i, r.0);
                    }
                });
                ui.label(RichText::new("Tamanho").color(theme::pal().dim).small());
                ui.label(RichText::new("Imagem final").strong().size(13.0).color(theme::pal().heading));
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    egui::ScrollArea::vertical().id_salt("sidescroll").auto_shrink([false, false]).show(ui, |ui| self.side_views(ui));
                });
            });
        }), ui);
    }

    fn side_views(&mut self, ui: &mut Ui) {
        theme::section_title(ui, "Visão");
        let views = [("Perspectiva", PI + 0.14, 0.18), ("Topo", PI, 1.52), ("Frente", PI, 0.12), ("Trás", 0.0, 0.12), ("Esquerda", -FRAC_PI_2, 0.12), ("Direita", FRAC_PI_2, 0.12)];
        egui::Grid::new("views").spacing([6.0, 6.0]).show(ui, |ui| {
            for (k, (n, yaw, pitch)) in views.into_iter().enumerate() {
                let r = theme::small_button_w(ui, n, false, 78.0);
                if self.tip(r, &format!("Câmera: vista {}", n.to_lowercase())).clicked() {
                    self.set_view(yaw, pitch);
                }
                if k % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        theme::section_title(ui, "Prévia");
        ui.horizontal(|ui| {
            for (fast, n, tx) in [(true, "Rápida", "Prévia leve: sem sombras nem reflexos. Ideal para mexer na cena."), (false, "Completa", "Prévia com sombras, reflexos e refração da água (mais lenta)")] {
                let r = theme::small_button_w(ui, n, self.preview_fast == fast, 78.0);
                if self.tip(r, tx).clicked() {
                    self.preview_fast = fast;
                    self.save_ui();
                }
            }
        });
        if self.simple {
            return;
        }
        ui.add_space(2.0);
        ui.label(RichText::new("Memórias de câmera").small().color(theme::pal().dim));
        ui.horizontal(|ui| {
            for i in 0..5 {
                let stored = self.cam_mem[i].is_some();
                let (rect, r) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
                theme::bevel(ui.painter(), rect, !r.is_pointer_button_down_on(), if stored { theme::pal().accent_dim } else { theme::pal().light });
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, (i + 1).to_string(), egui::FontId::proportional(12.0), theme::pal().ink);
                let r = self.tip(r, if stored { "Clique para voltar a esta câmera; clique direito grava de novo" } else { "Clique grava a câmera atual neste espaço" });
                if r.secondary_clicked() || (r.clicked() && !stored) {
                    self.cam_mem[i] = Some((self.scene.camera.position, self.scene.camera.target));
                } else if r.clicked() {
                    if let Some((p, t)) = self.cam_mem[i] {
                        self.scene.camera.position = p;
                        self.scene.camera.target = t;
                    }
                }
            }
        });
        theme::section_title(ui, "Exibição");
        for (d, n, tx) in [
            (Display::Preview, "Pré-visualização", "Mostra a cena renderizada"),
            (Display::Wire, "Wireframe", "Mostra só a estrutura em linhas"),
            (Display::Both, "Ambos", "Pré-visualização com as linhas por cima"),
        ] {
            let r = theme::small_button_w(ui, n, self.display == d, 162.0);
            if self.tip(r, tx).clicked() {
                self.display = d;
            }
        }
    }

    // ---------- barra de status ----------

    pub fn status_bar(&mut self, ui: &mut Ui, shown: &str) {
        theme::sheen_after(egui::Panel::bottom("status").frame(theme::panel_frame().inner_margin(Margin::symmetric(8, 4))).show(ui, |ui| {
            ui.horizontal(|ui| {
                let t = if shown.is_empty() { "Pronto." } else { shown };
                ui.label(RichText::new(t).color(theme::pal().ink));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let n = self.scene.objects.len();
                    ui.label(RichText::new(format!("{} {}", n, if n == 1 { "objeto" } else { "objetos" })).color(theme::pal().ink));
                    ui.label(RichText::new(&self.stage_info).small().color(theme::pal().dim));
                });
            });
        }), ui);
    }

    // ---------- sobre a cena ----------

    /// Controles flutuantes: setas de órbita, zoom e guia inicial. `rect` é a cena.
    pub fn viewport_overlays(&mut self, ctx: &Context, rect: Rect) {
        if self.editor != Editor::Work {
            return;
        }
        // --- controle de órbita (canto inferior direito) ---
        egui::Area::new(egui::Id::new("orbit_pad")).order(egui::Order::Middle).pivot(egui::Align2::RIGHT_BOTTOM).fixed_pos(rect.right_bottom() - vec2(14.0, 14.0)).show(ctx, |ui| {
            theme::card(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                let dt = ui.input(|i| i.stable_dt).min(0.05);
                let mut yaw = 0.0f32;
                let mut pitch = 0.0f32;
                let mut zoom = 0.0f32;
                let mut reset = false;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.add_space(31.0);
                        let r = glyph_button(ui, Glyph::Left).on_hover_text("Girar a câmera para a esquerda (segure)");
                        if r.is_pointer_button_down_on() {
                            yaw += 1.0;
                        }
                    });
                    ui.vertical(|ui| {
                        if glyph_button(ui, Glyph::Up).on_hover_text("Subir a câmera (segure)").is_pointer_button_down_on() {
                            pitch -= 1.0;
                        }
                        if glyph_button(ui, Glyph::Home).on_hover_text("Voltar à vista em perspectiva").clicked() {
                            reset = true;
                        }
                        if glyph_button(ui, Glyph::Down).on_hover_text("Descer a câmera (segure)").is_pointer_button_down_on() {
                            pitch += 1.0;
                        }
                    });
                    ui.vertical(|ui| {
                        ui.add_space(31.0);
                        if glyph_button(ui, Glyph::Right).on_hover_text("Girar a câmera para a direita (segure)").is_pointer_button_down_on() {
                            yaw -= 1.0;
                        }
                    });
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.add_space(31.0);
                        if glyph_button(ui, Glyph::Plus).on_hover_text("Aproximar (segure)").is_pointer_button_down_on() {
                            zoom -= 1.0;
                        }
                        if glyph_button(ui, Glyph::Minus).on_hover_text("Afastar (segure)").is_pointer_button_down_on() {
                            zoom += 1.0;
                        }
                    });
                });
                if reset {
                    self.set_view(PI + 0.14, 0.18);
                } else if yaw != 0.0 || pitch != 0.0 || zoom != 0.0 {
                    let mut o = crate::view::Orbit::from_camera(&self.scene.camera);
                    o.yaw += yaw * 1.2 * dt;
                    o.pitch = (o.pitch + pitch * 0.9 * dt).clamp(-1.5, 1.52);
                    o.dist = (o.dist * (zoom * 0.9 * dt).exp()).clamp(2.0, 1500.0);
                    o.apply(&mut self.scene.camera);
                    ui.ctx().request_repaint();
                }
            });
        });

        // --- guia "Comece aqui" ---
        if self.welcome && self.path.is_none() && rect.height() > 420.0 {
            let steps = [
                ("Terreno na cena", self.scene.terrain.is_some(), "Criar terreno"),
                ("Um objeto na cena", !self.scene.objects.is_empty(), "Abrir formas"),
                ("Imagem renderizada", self.rendered_once, "Renderizar"),
            ];
            let mut go = None;
            let mut dismiss = false;
            egui::Area::new(egui::Id::new("welcome")).order(egui::Order::Middle).fixed_pos(rect.left_top() + vec2(14.0, 14.0)).show(ctx, |ui| {
                ui.set_width(250.0);
                theme::header(ui, "Comece aqui");
                theme::card(ui, |ui| {
                    ui.set_min_width(232.0);
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for (i, (text, done, action)) in steps.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                            theme::bevel(ui.painter(), r, false, theme::pal().field);
                            if *done {
                                ui.painter().line_segment([r.left_center() + vec2(4.0, 1.0), r.center_bottom() + vec2(-1.0, -4.0)], Stroke::new(2.0, theme::pal().accent_dk));
                                ui.painter().line_segment([r.center_bottom() + vec2(-1.0, -4.0), r.right_top() + vec2(-3.0, 4.0)], Stroke::new(2.0, theme::pal().accent_dk));
                            }
                            ui.label(RichText::new(format!("{}. {}", i + 1, text)).color(if *done { theme::pal().dim } else { theme::pal().ink }));
                        });
                        if !*done && steps[..i].iter().all(|s| s.1) {
                            ui.horizontal(|ui| {
                                ui.add_space(24.0);
                                if theme::small_button(ui, action, false).clicked() {
                                    go = Some(i);
                                }
                            });
                        }
                    }
                    ui.add_space(2.0);
                    if theme::small_button(ui, "Fechar guia", false).on_hover_text("Você pode reabri-lo em Ajuda").clicked() {
                        dismiss = true;
                    }
                });
            });
            match go {
                Some(0) => self.open_terrain_editor(),
                Some(1) => {
                    self.palette = Palette::Criar;
                    self.lib_tab = Tab::Basicos;
                    self.drawer = true;
                }
                Some(2) => self.start_render(ctx),
                _ => {}
            }
            if dismiss {
                self.welcome = false;
                self.save_ui();
            }
        }
    }

    // ---------- preferências da interface ----------

    pub fn load_ui(&mut self) {
        if let Ok(t) = std::fs::read_to_string(assets::config_dir().join("ui.txt")) {
            for l in t.lines() {
                match l.split_once('=') {
                    Some(("modo", "avancado")) => self.simple = false,
                    Some(("modo", "simples")) => self.simple = true,
                    Some(("guia", "0")) => self.welcome = false,
                    Some(("previa", "completa")) => self.preview_fast = false,
                    Some(("previa", "rapida")) => self.preview_fast = true,
                    _ => {}
                }
            }
        }
    }

    pub fn save_ui(&self) {
        let _ = std::fs::create_dir_all(assets::config_dir());
        let t = format!("modo={}\nguia={}\nprevia={}\n", if self.simple { "simples" } else { "avancado" }, if self.welcome { 1 } else { 0 }, if self.preview_fast { "rapida" } else { "completa" });
        let _ = std::fs::write(assets::config_dir().join("ui.txt"), t);
    }
}

