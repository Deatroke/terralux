use crate::library::{Library, Tab, Thumbs};
use crate::shell::Palette;
use crate::theme;
use crate::view::Orbit;
use crate::worker::{FinalJob, Preview};
use egui::{Context, Key, Modifiers, Response, RichText, Ui};
use glam::Vec3;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use terralux_core::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sel {
    None,
    Sky,
    Terrain,
    Water,
    Object(usize),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    Move,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Display {
    Preview,
    Wire,
    Both,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Editor {
    Work,
    Terrain,
}

pub const RESOLUTIONS: [(&str, usize, usize); 5] = [
    ("640 × 480", 640, 480),
    ("1280 × 720", 1280, 720),
    ("1920 × 1080", 1920, 1080),
    ("2560 × 1440", 2560, 1440),
    ("3840 × 2160", 3840, 2160),
];
pub const QUALITIES: [(&str, usize); 4] = [("Rascunho", 1), ("Normal", 4), ("Alta", 9), ("Máxima", 25)];

#[derive(Default)]
pub struct Args {
    pub scene: Option<PathBuf>,
    pub shot: Option<(PathBuf, f32)>,
    pub terrain: bool,
    pub display: Option<String>,
    pub render: bool,
    pub select: Option<String>,
    pub library: Option<String>,
    /// Insere na cena os itens da biblioteca cujo nome contém o texto (repetível; para testes).
    pub insert: Vec<String>,
    /// Tamanho da janela (teste de layout): --size 1280x800
    pub size: Option<(f32, f32)>,
    pub mode: Option<String>,
    pub palette: Option<String>,
    /// Tema (teste): --theme ciano|matriz|sintetico|ambar|aero. Não grava nada.
    pub theme: Option<String>,
    /// Abre a janela de personalização (teste).
    pub theme_window: bool,
}

impl Args {
    pub fn parse() -> Self {
        let mut a = Args::default();
        let mut it = std::env::args().skip(1);
        while let Some(x) = it.next() {
            match x.as_str() {
                "--screenshot" => {
                    let p = it.next().unwrap_or_default();
                    let secs = it.next().and_then(|s| s.parse().ok()).unwrap_or(6.0);
                    a.shot = Some((p.into(), secs));
                }
                "--terrain" => a.terrain = true,
                "--render" => a.render = true,
                "--display" => a.display = it.next(),
                "--select" => a.select = it.next(),
                "--library" => a.library = it.next(),
                "--insert" => a.insert.extend(it.next()),
                "--size" => {
                    a.size = it.next().and_then(|s| {
                        let (w, h) = s.split_once('x')?;
                        Some((w.parse().ok()?, h.parse().ok()?))
                    })
                }
                "--mode" => a.mode = it.next(),
                "--palette" => a.palette = it.next(),
                "--theme" => a.theme = it.next(),
                "--theme-window" => a.theme_window = true,
                other => a.scene = Some(other.into()),
            }
        }
        a
    }
}

struct Shot {
    path: PathBuf,
    after: f32,
    requested: bool,
    done: Arc<AtomicBool>,
}

pub struct App {
    pub scene: Scene,
    pub committed: Scene,
    pub saved: Scene,
    pub undo: Vec<Scene>,
    pub redo: Vec<Scene>,
    pub path: Option<PathBuf>,
    pub sel: Sel,
    pub tool: Tool,
    pub display: Display,
    pub editor: Editor,
    pub terrain_backup: Option<TerrainDef>,
    pub preview: Preview,
    pub tex: Option<egui::TextureHandle>,
    pub stage_info: String,
    pub last_req: Option<(Scene, (usize, usize))>,
    pub req_id: u64,
    pub final_job: Option<FinalJob>,
    pub res_idx: usize,
    pub quality_idx: usize,
    pub cam_mem: [Option<(Vec3, Vec3)>; 5],
    pub hint: String,
    pub toast: Option<crate::toast::Toast>,
    pub map_hf: Option<(String, Heightfield)>,
    pub map_tex: Option<(String, egui::TextureHandle)>,
    pub pick_hf: Option<(String, Arc<Heightfield>)>,
    pub lib: Library,
    pub lib_tab: Tab,
    pub lib_filter: String,
    pub thumbs: Thumbs,
    pub simple: bool,
    pub palette: Palette,
    pub drawer: bool,
    pub welcome: bool,
    pub theme_open: bool,
    pub rendered_once: bool,
    pub view_rect: egui::Rect,
    last_title: String,
    shot: Option<Shot>,
    start: std::time::Instant,
    pub auto_render: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        let first = match args.theme.as_deref() {
            Some(id) => theme::preset(id),
            None => crate::theme_ui::load_saved(),
        };
        theme::set(&cc.egui_ctx, first);
        crate::aero::init(&cc.egui_ctx);
        let mut path = None;
        let scene = match &args.scene {
            Some(p) => match std::fs::read_to_string(p).ok().and_then(|t| Scene::from_ron(&t).ok()) {
                Some(s) => {
                    path = Some(p.clone());
                    s
                }
                None => Scene::demo(),
            },
            None => Scene::demo(),
        };
        let mut app = App {
            committed: scene.clone(),
            saved: scene.clone(),
            scene,
            undo: vec![],
            redo: vec![],
            path,
            sel: Sel::Sky,
            tool: Tool::Select,
            display: match args.display.as_deref() {
                Some("wire") => Display::Wire,
                Some("both") => Display::Both,
                _ => Display::Preview,
            },
            editor: Editor::Work,
            terrain_backup: None,
            preview: Preview::spawn(cc.egui_ctx.clone()),
            tex: None,
            stage_info: String::new(),
            last_req: None,
            req_id: 0,
            final_job: None,
            res_idx: 2,
            quality_idx: 2,
            cam_mem: [None; 5],
            hint: String::new(),
            toast: None,
            map_hf: None,
            map_tex: None,
            pick_hf: None,
            lib: Library::scan(),
            lib_tab: args.library.as_deref().and_then(|k| Tab::ALL.into_iter().find(|t| t.key() == k)).unwrap_or(Tab::Divertidos),
            lib_filter: String::new(),
            thumbs: Thumbs::spawn(cc.egui_ctx.clone()),
            simple: true,
            palette: args.palette.as_deref().and_then(Palette::from_key).unwrap_or(Palette::Criar),
            drawer: args.library.is_some(),
            welcome: true,
            theme_open: args.theme_window,
            rendered_once: false,
            view_rect: egui::Rect::NOTHING,
            last_title: String::new(),
            shot: args.shot.map(|(path, after)| Shot { path, after, requested: false, done: Arc::new(AtomicBool::new(false)) }),
            start: std::time::Instant::now(),
            auto_render: args.render,
        };
        app.load_ui();
        match args.mode.as_deref() {
            Some("avancado") => app.simple = false,
            Some("simples") => app.simple = true,
            _ => {}
        }
        if args.terrain {
            app.open_terrain_editor();
        }
        for q in &args.insert {
            let q = q.to_lowercase();
            let found = Tab::ALL.iter().flat_map(|t| app.lib.items(*t).to_vec()).find(|i| i.name.to_lowercase().contains(&q) || i.spec.to_lowercase().contains(&q));
            match found {
                Some(it) => app.insert_item(&it),
                None => eprintln!("--insert: nada chamado “{q}”"),
            }
        }
        if let Some(s) = &args.select {
            app.sel = match s.as_str() {
                "terrain" => Sel::Terrain,
                "water" => Sel::Water,
                "sky" => Sel::Sky,
                n => n.parse().map(Sel::Object).unwrap_or(Sel::None),
            };
        }
        app
    }

    // ---------- utilidades ----------

    pub fn tip(&mut self, r: Response, text: &str) -> Response {
        if r.hovered() {
            self.hint = text.to_string();
        }
        r.on_hover_text(text)
    }

    pub fn open_terrain_editor(&mut self) {
        if self.scene.terrain.is_none() {
            self.scene.terrain = Some(TerrainDef::island());
        }
        self.terrain_backup = self.scene.terrain.clone();
        self.sel = Sel::Terrain;
        self.editor = Editor::Terrain;
    }

    pub fn load_scene(&mut self, scene: Scene, path: Option<PathBuf>) {
        self.committed = scene.clone();
        self.saved = scene.clone();
        self.scene = scene;
        self.path = path;
        self.undo.clear();
        self.redo.clear();
        self.sel = Sel::Sky;
        self.last_req = None;
    }

    pub fn file_open(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("Cena Terralux", &["ron"]).pick_file() {
            match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| Scene::from_ron(&t).map_err(|e| e.to_string())) {
                Ok(s) => self.load_scene(s, Some(p)),
                Err(e) => self.error(format!("Não foi possível abrir a cena: {e}")),
            }
        }
    }

    pub fn file_save(&mut self, as_new: bool) {
        let path = if as_new || self.path.is_none() {
            rfd::FileDialog::new().add_filter("Cena Terralux", &["ron"]).set_file_name("cena.ron").save_file()
        } else {
            self.path.clone()
        };
        if let Some(p) = path {
            match std::fs::write(&p, self.scene.to_ron()) {
                Ok(()) => {
                    self.saved = self.scene.clone();
                    self.info(format!("Cena salva em {}", p.display()));
                    self.path = Some(p);
                }
                Err(e) => self.error(format!("Não foi possível salvar a cena: {e}")),
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.scene.clone());
            self.scene = prev.clone();
            self.committed = prev;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.scene.clone());
            self.scene = next.clone();
            self.committed = next;
        }
    }

    fn commit_history(&mut self, ctx: &Context) {
        if self.scene != self.committed && !ctx.input(|i| i.pointer.any_down()) {
            self.undo.push(std::mem::replace(&mut self.committed, self.scene.clone()));
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
        if let Sel::Object(i) = self.sel {
            if i >= self.scene.objects.len() {
                self.sel = Sel::None;
            }
        }
    }

    pub fn delete_selected(&mut self) {
        let nome = match self.sel {
            Sel::Object(i) if i < self.scene.objects.len() => Some(self.scene.objects.remove(i).name),
            Sel::Terrain => self.scene.terrain.take().map(|_| "Terreno".to_string()),
            Sel::Water => self.scene.water.take().map(|_| "Plano de água".to_string()),
            _ => None,
        };
        self.sel = Sel::None;
        if let Some(n) = nome {
            self.notify(format!("“{n}” excluído."), crate::toast::ToastKind::Info, true);
        }
    }

    pub fn duplicate_selected(&mut self) {
        if let Sel::Object(i) = self.sel {
            if let Some(o) = self.scene.objects.get(i).cloned() {
                let mut n = o;
                n.name = format!("{} cópia", n.name);
                n.position += Vec3::new(n.size * 2.2, 0.0, 0.0);
                self.scene.objects.push(n);
                self.sel = Sel::Object(self.scene.objects.len() - 1);
            }
        }
    }

    fn spawn_point(&self) -> Vec3 {
        let c = &self.scene.camera;
        Vec3::new(c.target.x, c.target.y.max(8.0) + 6.0, c.target.z)
    }

    pub fn create_shape(&mut self, shape: Shape, name: &str, size: f32) {
        let n = self.scene.objects.iter().filter(|o| o.name.starts_with(name)).count() + 1;
        self.scene.objects.push(Object {
            name: format!("{name} {n}"),
            shape,
            position: self.spawn_point(),
            rotation_deg: Vec3::new(15.0, 25.0, 0.0),
            size,
            material: Material::metal(Vec3::new(0.85, 0.85, 0.9)),
        });
        self.sel = Sel::Object(self.scene.objects.len() - 1);
    }

    pub fn start_render(&mut self, ctx: &Context) {
        self.rendered_once = true;
        let (_, w, h) = RESOLUTIONS[self.res_idx];
        if let Some(j) = &self.final_job {
            j.cancel();
        }
        self.final_job = Some(FinalJob::start(ctx.clone(), self.scene.clone(), w, h, QUALITIES[self.quality_idx].1));
    }

    pub fn apply_sky_preset(&mut self, name: &str) {
        let s = &mut self.scene;
        match name {
            "Dia claro" => {
                s.sun = Sun { azimuth_deg: -75.0, elevation_deg: 38.0, color: Vec3::new(1.0, 0.92, 0.78), intensity: 2.2 };
                s.sky.zenith = Vec3::new(0.10, 0.28, 0.70);
                s.sky.horizon = Vec3::new(0.78, 0.84, 0.92);
                s.sky.haze = 0.0011;
                s.sky.clouds = Some(Clouds { coverage: 0.4, ..Clouds::fair() });
            }
            "Fim de tarde" => {
                s.sun = Sun { azimuth_deg: -70.0, elevation_deg: 7.0, color: Vec3::new(1.0, 0.55, 0.28), intensity: 2.6 };
                s.sky.zenith = Vec3::new(0.10, 0.18, 0.42);
                s.sky.horizon = Vec3::new(0.95, 0.62, 0.42);
                s.sky.haze = 0.0022;
                s.sky.clouds = Some(Clouds { coverage: 0.5, ..Clouds::fair() });
            }
            "Nublado" => {
                s.sun = Sun { azimuth_deg: -75.0, elevation_deg: 30.0, color: Vec3::new(0.95, 0.95, 1.0), intensity: 0.9 };
                s.sky.zenith = Vec3::new(0.34, 0.41, 0.50);
                s.sky.horizon = Vec3::new(0.62, 0.66, 0.70);
                s.sky.haze = 0.0024;
                s.sky.clouds = Some(Clouds { coverage: 1.0, ..Clouds::fair() });
            }
            _ => {
                s.sun = Sun { azimuth_deg: -60.0, elevation_deg: 11.0, color: Vec3::new(1.0, 0.82, 0.66), intensity: 1.6 };
                s.sky.zenith = Vec3::new(0.30, 0.46, 0.70);
                s.sky.horizon = Vec3::new(0.85, 0.82, 0.85);
                s.sky.haze = 0.008;
                s.sky.clouds = Some(Clouds { coverage: 0.3, ..Clouds::fair() });
            }
        }
    }

    pub fn set_view(&mut self, yaw: f32, pitch: f32) {
        let mut o = Orbit::from_camera(&self.scene.camera);
        o.yaw = yaw;
        o.pitch = pitch;
        o.apply(&mut self.scene.camera);
    }

    fn final_window(&mut self, ctx: &Context) {
        let Some(job) = &mut self.final_job else { return };
        job.poll();
        if job.rgb.is_some() && job.tex.is_none() {
            let img = egui::ColorImage::from_rgb([job.w, job.h], job.rgb.as_ref().unwrap());
            job.tex = Some(ctx.load_texture("render", img, egui::TextureOptions::LINEAR));
        }
        let mut open = true;
        let mut saved_msg = None;
        egui::Window::new(format!("Render {}×{}", job.w, job.h)).open(&mut open).default_size([760.0, 470.0]).resizable(true).show(ctx, |ui| {
            match &job.tex {
                None => {
                    ui.add(egui::ProgressBar::new(job.progress()).show_percentage().text(format!("Renderizando… {} amostras/pixel", job.spp)));
                    ui.label(RichText::new(format!("{:.0} s", job.started.elapsed().as_secs_f32())).color(theme::pal().dim));
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
                Some(tex) => {
                    ui.horizontal(|ui| {
                        if theme::button(ui, "Salvar PNG…", true, true, egui::vec2(0.0, 26.0)).clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("PNG", &["png"]).set_file_name("render.png").save_file() {
                                saved_msg = Some(match job.save_png(&p) {
                                    Ok(()) => Ok(format!("Imagem salva em {}", p.display())),
                                    Err(e) => Err(format!("Não foi possível salvar a imagem: {e}")),
                                });
                            }
                        }
                        ui.label(RichText::new(format!("Concluído em {:.1} s", job.elapsed.unwrap_or(0.0))).color(theme::pal().dim));
                    });
                    let avail = ui.available_size();
                    let k = (avail.x / job.w as f32).min(avail.y / job.h as f32).min(1.0).max(0.05);
                    let size = egui::vec2(job.w as f32 * k, job.h as f32 * k);
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(tex.id(), size)));
                }
            }
        });
        if !open {
            job.cancel();
            self.final_job = None;
        }
        match saved_msg {
            Some(Ok(m)) => self.info(m),
            Some(Err(m)) => self.error(m),
            None => {}
        }
    }

    fn shortcuts(&mut self, ctx: &Context) {
        let (redo, undo, save, open, del, render, dup) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::NONE, Key::Delete),
                i.consume_key(Modifiers::NONE, Key::F9),
                i.consume_key(Modifiers::COMMAND, Key::D),
            )
        });
        let typing = ctx.egui_wants_keyboard_input();
        if self.drawer && self.editor == Editor::Work && !typing && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.drawer = false;
        }
        if redo {
            self.redo();
        } else if undo {
            self.undo();
        }
        if save {
            self.file_save(false);
        }
        if open {
            self.file_open();
        }
        if dup && !typing {
            self.duplicate_selected();
        }
        if del && !typing {
            self.delete_selected();
        }
        if render {
            self.start_render(ctx);
        }
    }

    fn poll_preview(&mut self, ctx: &Context) {
        while let Ok(r) = self.preview.rx.try_recv() {
            if r.id != self.req_id {
                continue;
            }
            let img = egui::ColorImage::from_rgb([r.w, r.h], &r.rgb);
            match &mut self.tex {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => self.tex = Some(ctx.load_texture("preview", img, egui::TextureOptions::LINEAR)),
            }
            self.stage_info = format!("preview {}/{} · {} ms", r.stage + 1, Preview::stages(), r.ms);
        }
    }

    fn screenshot_logic(&mut self, ctx: &Context) {
        let Some(s) = &mut self.shot else { return };
        if s.done.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if !s.requested && self.start.elapsed().as_secs_f32() > s.after {
            s.requested = true;
            let (path, done) = (s.path.clone(), s.done.clone());
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|p| [p.r(), p.g(), p.b()]).collect();
                let _ = image::save_buffer(&path, &bytes, img.size[0] as u32, img.size[1] as u32, image::ColorType::Rgb8);
                done.store(true, Ordering::Relaxed);
            })));
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.auto_render {
            self.auto_render = false;
            self.res_idx = 0;
            self.quality_idx = 1;
            self.start_render(&ctx);
        }
        let shown = std::mem::take(&mut self.hint);
        self.shortcuts(&ctx);
        self.poll_preview(&ctx);
        let title = format!(
            "{}{} — Terralux Studio",
            self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "sem título".into()),
            if self.scene != self.saved { " •" } else { "" }
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        theme::backdrop(ui);
        self.menu_bar(ui);
        if self.editor == Editor::Work {
            self.palette_bar(ui);
        }
        self.status_bar(ui, &shown);
        if self.editor == Editor::Work {
            if self.drawer {
                self.library_drawer(ui);
            }
            self.side_panel(ui);
            self.inspector(ui);
        }
        egui::CentralPanel::default().frame(theme::panel_frame().inner_margin(5.0)).show(ui, |ui| match self.editor {
            Editor::Work => {
                self.viewport(ui);
                theme::bevel(ui.painter(), self.view_rect.expand(2.0), false, egui::Color32::TRANSPARENT);
            }
            Editor::Terrain => self.terrain_editor(ui),
        });
        if self.editor == Editor::Work {
            self.viewport_overlays(&ctx, self.view_rect);
        }
        self.final_window(&ctx);
        self.theme_window(&ctx);
        self.toast_ui(&ctx);
        self.commit_history(&ctx);
        self.screenshot_logic(&ctx);
    }
}
