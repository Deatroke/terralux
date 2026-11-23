//! Sistema de temas: paletas prontas (cybercore em várias cores, Frutiger Aero) e um tema personalizado.
//! Tudo é lido de uma `Palette` global; mudar a paleta refaz o visual na hora.
//! Esferas e cápsulas do estilo Aero vêm do pacote AeroMedia (CC BY 4.0, ver assets/aero/CREDITS.md);
//! o estilo Neon é desenhado por código.
use crate::aero;
use crate::icons::{self, Icon};
use egui::{pos2, vec2, Color32, Context, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, Visuals};
use std::sync::{OnceLock, RwLock};

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Como esferas, cápsulas e molduras são desenhadas.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Vidro brilhante com as imagens do pacote Aero.
    Aero,
    /// Escuro com contornos e brilho de neon, tudo por código.
    Neon,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Palette {
    pub id: &'static str,
    pub style: Style,
    // --- editáveis ---
    pub stone: Color32,     // fundo dos painéis
    pub light: Color32,     // botões
    pub ink: Color32,       // texto
    pub dim: Color32,       // texto suave
    pub dk: Color32,        // contorno
    pub navy_hi: Color32,   // barra de título (topo)
    pub navy: Color32,      // barra de título (base)
    pub accent: Color32,    // destaque (ativo, botão principal)
    pub orb: Color32,       // esferas e ícones
    pub glow: Color32,      // cor do neon
    pub well: Color32,      // gaveta e campos escuros
    pub view_bg: Color32,   // fundo da cena
    pub view_text: Color32, // texto sobre a cena
    pub radius: u8,         // arredondamento
    pub gloss: u8,          // brilho do vidro (0 a 200)
    pub glow_amt: u8,       // intensidade do neon (0 a 255)
    pub glass: u8,          // opacidade dos painéis (255 = sólido; menos = vidro transparente com papel de parede)
    // --- derivadas (ver `derive`) ---
    pub lo: Color32,
    pub hover: Color32,
    pub press: Color32,
    pub tab_idle: Color32,
    pub field: Color32,
    pub accent_dim: Color32,
    pub accent_dk: Color32,
    pub heading: Color32,
    pub dark: bool,
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn lum(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

impl Palette {
    #[allow(clippy::too_many_arguments)]
    const fn base(id: &'static str, style: Style, c: [Color32; 13], radius: u8, gloss: u8, glow_amt: u8, glass: u8) -> Palette {
        Palette {
            id, style,
            stone: c[0], light: c[1], ink: c[2], dim: c[3], dk: c[4], navy_hi: c[5], navy: c[6], accent: c[7], orb: c[8], glow: c[9], well: c[10], view_bg: c[11], view_text: c[12],
            radius, gloss, glow_amt, glass,
            lo: c[0], hover: c[0], press: c[0], tab_idle: c[0], field: c[0], accent_dim: c[0], accent_dk: c[0], heading: c[0], dark: false,
        }
    }

    /// Calcula as cores derivadas das editáveis.
    pub fn derive(mut self) -> Palette {
        self.dark = lum(self.stone) < 0.45;
        self.lo = mix(self.stone, self.dk, 0.5);
        self.hover = mix(self.light, if self.style == Style::Neon { self.glow } else { self.accent }, if self.dark { 0.20 } else { 0.14 });
        self.press = mix(self.light, self.dk, 0.28);
        self.tab_idle = mix(self.stone, self.dk, 0.18);
        self.field = if self.dark { mix(self.stone, Color32::BLACK, 0.45) } else { mix(self.light, Color32::WHITE, 0.6) };
        self.accent_dim = mix(self.light, self.accent, if self.dark { 0.38 } else { 0.5 });
        self.accent_dk = if self.dark { self.accent } else { mix(self.accent, Color32::BLACK, 0.45) };
        self.heading = if self.dark { mix(self.accent, Color32::WHITE, 0.2) } else { self.navy };
        self
    }

    /// Aplica a transparência do vidro a uma cor sólida (`extra` deixa botões mais opacos que painéis).
    pub fn glassy(&self, c: Color32, extra: u8) -> Color32 {
        if self.glass == 255 || c.a() != 255 {
            return c;
        }
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), self.glass.saturating_add(extra))
    }

    pub fn radius_f(&self) -> f32 {
        self.radius as f32
    }
}

pub const PRESETS: [(&str, &str); 7] = [
    ("ciano", "Cyber ciano e magenta"),
    ("matriz", "Terminal verde"),
    ("sintetico", "Synthwave"),
    ("ambar", "Âmbar e turquesa"),
    ("aero", "Frutiger Aero (claro)"),
    ("aeroescuro", "Frutiger Aero (escuro)"),
    ("custom", "Personalizado"),
];

pub const DEFAULT_ID: &str = "ciano";

pub fn preset(id: &str) -> Palette {
    let p = match id {
        "matriz" => Palette::base("matriz", Style::Neon,
            [rgb(6, 16, 10), rgb(12, 30, 18), rgb(196, 255, 214), rgb(100, 166, 126), rgb(0, 150, 76), rgb(0, 96, 44), rgb(4, 26, 12),
             rgb(0, 255, 120), rgb(0, 230, 110), rgb(0, 255, 120), rgb(3, 10, 6), rgb(2, 8, 4), rgb(150, 255, 190)], 2, 20, 150, 255),
        "sintetico" => Palette::base("sintetico", Style::Neon,
            [rgb(24, 10, 44), rgb(42, 18, 72), rgb(255, 228, 246), rgb(192, 144, 204), rgb(206, 52, 164), rgb(255, 112, 44), rgb(112, 20, 124),
             rgb(255, 150, 30), rgb(255, 64, 172), rgb(255, 64, 172), rgb(14, 6, 28), rgb(10, 4, 22), rgb(255, 196, 232)], 2, 24, 150, 255),
        "ambar" => Palette::base("ambar", Style::Neon,
            [rgb(20, 14, 10), rgb(40, 29, 18), rgb(255, 232, 194), rgb(196, 166, 124), rgb(190, 122, 22), rgb(206, 112, 10), rgb(64, 32, 6),
             rgb(255, 176, 0), rgb(255, 172, 32), rgb(34, 232, 212), rgb(12, 8, 5), rgb(10, 7, 4), rgb(255, 218, 156)], 2, 20, 140, 255),
        "aero" => Palette::base("aero", Style::Aero,
            [rgb(222, 235, 248), rgb(244, 249, 254), rgb(18, 38, 66), rgb(74, 96, 124), rgb(86, 120, 164), rgb(84, 168, 236), rgb(14, 70, 140),
             rgb(70, 178, 60), rgb(40, 130, 230), rgb(255, 255, 255), rgb(22, 42, 70), rgb(14, 24, 40), rgb(200, 220, 242)], 6, 130, 0, 108),
        "aeroescuro" => Palette::base("aeroescuro", Style::Aero,
            [rgb(10, 26, 52), rgb(34, 62, 104), rgb(228, 242, 255), rgb(150, 178, 214), rgb(96, 156, 216), rgb(58, 140, 232), rgb(8, 34, 88),
             rgb(64, 204, 96), rgb(40, 130, 230), rgb(190, 230, 255), rgb(8, 18, 36), rgb(8, 14, 26), rgb(200, 224, 248)], 6, 100, 0, 110),
        // "ciano" e, por segurança, qualquer id desconhecido
        _ => Palette::base("ciano", Style::Neon,
            [rgb(10, 12, 28), rgb(21, 27, 52), rgb(216, 236, 255), rgb(130, 154, 196), rgb(0, 156, 196), rgb(100, 26, 146), rgb(14, 10, 42),
             rgb(255, 48, 192), rgb(0, 226, 255), rgb(0, 226, 255), rgb(6, 8, 20), rgb(4, 6, 14), rgb(170, 230, 255)], 2, 22, 150, 255),
    };
    p.derive()
}

static PAL: OnceLock<RwLock<Palette>> = OnceLock::new();

fn cell() -> &'static RwLock<Palette> {
    PAL.get_or_init(|| RwLock::new(preset(DEFAULT_ID)))
}

/// Paleta atual (cópia barata).
pub fn pal() -> Palette {
    *cell().read().unwrap_or_else(|e| e.into_inner())
}

/// Troca a paleta e refaz o estilo do egui.
pub fn set(ctx: &Context, p: Palette) {
    *cell().write().unwrap_or_else(|e| e.into_inner()) = p.derive();
    apply(ctx);
}

pub fn apply(ctx: &Context) {
    let p = pal();
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.stone;
    v.window_fill = p.glassy(p.stone, 60);
    v.window_stroke = Stroke::new(1.0, p.dk);
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.tab_idle;
    v.selection.bg_fill = p.accent_dim;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.heading;
    let r = CornerRadius::same(p.radius.min(8));
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = r;
        w.fg_stroke = Stroke::new(1.0, p.ink);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.lo);
    v.widgets.inactive.bg_fill = p.light;
    v.widgets.inactive.weak_bg_fill = p.light;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.dk);
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, if p.style == Style::Neon { p.glow } else { p.ink });
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.ink);
    v.widgets.active.bg_fill = p.press;
    v.widgets.active.weak_bg_fill = p.press;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.open.bg_fill = p.press;
    v.widgets.open.weak_bg_fill = p.press;
    let shadow = if p.style == Style::Neon { p.glow.gamma_multiply(p.glow_amt as f32 / 255.0 * 0.35) } else { Color32::from_rgba_unmultiplied(20, 50, 100, 70) };
    v.popup_shadow = egui::Shadow { offset: [2, 4], blur: 10, spread: 0, color: shadow };
    v.window_shadow = v.popup_shadow;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 7.0);
        s.spacing.button_padding = vec2(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
        s.spacing.slider_width = 130.0;
    });
}

// --- salvar e carregar (linhas `chave=valor`, tolerante a erros) ---

fn hex(c: Color32) -> String {
    format!("{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some(rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

pub fn to_text(p: &Palette) -> String {
    if p.id != "custom" {
        return format!("preset={}\n", p.id);
    }
    let mut s = String::from("preset=custom\n");
    s += &format!("estilo={}\n", if p.style == Style::Aero { "aero" } else { "neon" });
    for (k, c) in fields(p) {
        s += &format!("{k}={}\n", hex(c));
    }
    s += &format!("arredondamento={}\nbrilho={}\nneon={}\nvidro={}\n", p.radius, p.gloss, p.glow_amt, p.glass);
    s
}

fn fields(p: &Palette) -> [(&'static str, Color32); 13] {
    [("paineis", p.stone), ("botoes", p.light), ("texto", p.ink), ("texto_suave", p.dim), ("contorno", p.dk), ("titulo_topo", p.navy_hi), ("titulo_base", p.navy),
     ("destaque", p.accent), ("esferas", p.orb), ("neon", p.glow), ("campos_escuros", p.well), ("cena", p.view_bg), ("texto_cena", p.view_text)]
}

/// Lê um tema; linhas inválidas são ignoradas. Sem `preset=`, devolve `None`.
pub fn from_text(t: &str) -> Option<Palette> {
    let mut preset_id: Option<String> = None;
    let mut kv: Vec<(String, String)> = Vec::new();
    for l in t.lines() {
        if let Some((k, v)) = l.split_once('=') {
            if k.trim() == "preset" {
                preset_id = Some(v.trim().to_string());
            } else {
                kv.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    let id = preset_id?;
    if id != "custom" {
        return Some(preset(&id));
    }
    let mut p = preset(DEFAULT_ID);
    p.id = "custom";
    for (k, v) in kv {
        let slot: Option<&mut Color32> = match k.as_str() {
            "paineis" => Some(&mut p.stone), "botoes" => Some(&mut p.light), "texto" => Some(&mut p.ink), "texto_suave" => Some(&mut p.dim),
            "contorno" => Some(&mut p.dk), "titulo_topo" => Some(&mut p.navy_hi), "titulo_base" => Some(&mut p.navy), "destaque" => Some(&mut p.accent),
            "esferas" => Some(&mut p.orb), "neon" => Some(&mut p.glow), "campos_escuros" => Some(&mut p.well), "cena" => Some(&mut p.view_bg),
            "texto_cena" => Some(&mut p.view_text),
            _ => None,
        };
        if let Some(slot) = slot {
            if let Some(c) = parse_hex(&v) {
                *slot = c;
            }
            continue;
        }
        match k.as_str() {
            "estilo" => p.style = if v == "aero" { Style::Aero } else { Style::Neon },
            "arredondamento" => p.radius = v.parse::<u8>().unwrap_or(p.radius).min(12),
            "brilho" => p.gloss = v.parse::<u8>().unwrap_or(p.gloss).min(200),
            "neon_forca" | "intensidade" => p.glow_amt = v.parse().unwrap_or(p.glow_amt),
            "vidro" => p.glass = v.parse().unwrap_or(p.glass),
            _ => {}
        }
    }
    Some(p.derive())
}

// --- desenho ---

fn a(c: Color32, k: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), k)
}

/// Brilho de neon em volta de um retângulo (anéis com transparência decrescente).
pub fn glow_rect(p: &Painter, r: Rect, rad: f32, col: Color32, amt: u8) {
    if amt == 0 {
        return;
    }
    for i in 1..=4 {
        let k = (amt as f32 * 0.30 / i as f32) as u8;
        p.rect_stroke(r.expand(i as f32 * 1.6), rad + i as f32 * 1.6, Stroke::new(1.8, a(col, k)), StrokeKind::Outside);
    }
}

pub fn glow_circle(p: &Painter, c: Pos2, rad: f32, col: Color32, amt: u8) {
    if amt == 0 {
        return;
    }
    for i in 1..=4 {
        let k = (amt as f32 * 0.32 / i as f32) as u8;
        p.circle_stroke(c, rad + i as f32 * 1.6, Stroke::new(1.8, a(col, k)));
    }
}

/// Moldura arredondada. `raised`: botão com contorno e brilho; senão campo afundado.
/// Com `fill` transparente só desenha o contorno (usado como moldura por cima do conteúdo).
pub fn bevel(p: &Painter, r: Rect, raised: bool, fill: Color32) {
    let pl = pal();
    let rad = pl.radius_f();
    if fill != Color32::TRANSPARENT {
        let fill = pl.glassy(fill, 40);
        p.rect_filled(r, rad, fill);
        if raised && pl.gloss > 0 {
            let top = Rect::from_min_max(r.min + vec2(2.0, 2.0), pos2(r.max.x - 2.0, r.min.y + r.height() * 0.48));
            if top.height() > 2.0 {
                let rr = pl.radius.saturating_sub(1);
                p.rect_filled(top, CornerRadius { nw: rr, ne: rr, sw: 0, se: 0 }, Color32::from_white_alpha(pl.gloss));
            }
        }
    }
    let edge = if raised { pl.dk } else { pl.lo };
    p.rect_stroke(r, rad, Stroke::new(1.0, edge), StrokeKind::Inside);
    let inner = match (pl.style, raised) {
        (Style::Aero, true) => Color32::from_white_alpha(170),
        (Style::Aero, false) => Color32::from_white_alpha(60),
        (Style::Neon, true) => a(pl.glow, (pl.glow_amt as f32 * 0.22) as u8),
        (Style::Neon, false) => Color32::TRANSPARENT,
    };
    if inner.a() > 0 {
        p.rect_stroke(r.shrink(1.0), (rad - 1.0).max(0.0), Stroke::new(1.0, inner), StrokeKind::Inside);
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum OrbState {
    Normal,
    Active,
    Off,
}

/// Esfera de ícone. Devolve a cor com que o ícone deve ser desenhado em cima dela.
pub fn orb(p: &Painter, c: Pos2, d: f32, state: OrbState, hot: bool) -> Color32 {
    let pl = pal();
    match pl.style {
        Style::Aero => {
            let name = match state {
                OrbState::Off => "orb_silver",
                OrbState::Active => "orb_green",
                OrbState::Normal => "orb_blue",
            };
            let k = if hot { 1.05 } else { 1.0 };
            aero::image(p, name, Rect::from_center_size(c, vec2(d * 1.08 * k, d * 1.08 * k)), Color32::WHITE);
            if state == OrbState::Off { Color32::from_white_alpha(200) } else { Color32::WHITE }
        }
        Style::Neon => {
            let col = match state {
                OrbState::Normal => pl.orb,
                OrbState::Active => pl.accent,
                OrbState::Off => pl.dim,
            };
            let r = d * 0.5;
            let glow = if state == OrbState::Active || hot { pl.glow_amt } else { pl.glow_amt / 4 };
            if state != OrbState::Off {
                glow_circle(p, c, r, col, glow);
            }
            p.circle_filled(c, r, mix(pl.stone, col, if state == OrbState::Off { 0.08 } else { 0.20 }));
            p.circle_stroke(c, r, Stroke::new(1.6, col));
            if pl.gloss > 0 {
                p.circle_filled(c - vec2(r * 0.18, r * 0.42), r * 0.34, Color32::from_white_alpha(pl.gloss / 2));
            }
            mix(col, Color32::WHITE, if state == OrbState::Off { 0.0 } else { 0.40 })
        }
    }
}

/// Gradiente vertical por malha de vértices coloridos.
pub fn vgrad(p: &Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

/// Barra de título, na largura disponível. Devolve o retângulo da barra.
pub fn header(ui: &mut Ui, text: &str) -> Rect {
    let pl = pal();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    vgrad(ui.painter(), rect, pl.navy_hi, pl.navy);
    if pl.gloss > 0 {
        let gloss = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.min.y + rect.height() * 0.5));
        ui.painter().rect_filled(gloss, 0.0, Color32::from_white_alpha((pl.gloss / 2).max(8)));
    }
    let edge = if pl.style == Style::Neon { pl.glow } else { pl.dk };
    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(if pl.style == Style::Neon { 2.0 } else { 1.0 }, edge));
    let shadow = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(14.0), Color32::from_black_alpha(110));
    ui.painter().galley(rect.left_center() + vec2(10.0, 1.0 - shadow.size().y * 0.5), shadow, Color32::BLACK);
    ui.painter().text(rect.left_center() + vec2(9.0, 0.0), egui::Align2::LEFT_CENTER, text, FontId::proportional(14.0), Color32::WHITE);
    rect
}

/// Moldura afundada (campo): conteúdo sobre fundo escuro.
pub fn well<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let pl = pal();
    let r = egui::Frame::new().fill(pl.well).corner_radius(pl.radius_f()).inner_margin(6.0).show(ui, add);
    bevel(ui.painter(), r.response.rect, false, Color32::TRANSPARENT);
    r.inner
}

/// Painel com conteúdo (cartão).
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let pl = pal();
    let r = egui::Frame::new().fill(pl.light).corner_radius(pl.radius_f()).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, add);
    bevel(ui.painter(), r.response.rect, true, Color32::TRANSPARENT);
    r.inner
}

// --- widgets ---

/// Botão de ferramenta: esfera com o ícone dentro e o rótulo embaixo.
pub fn tile(ui: &mut Ui, icon: Icon, label: &str, active: bool, enabled: bool) -> Response {
    let pl = pal();
    let size = vec2(78.0, 66.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let down = enabled && resp.is_pointer_button_down_on();
    let hot = enabled && resp.hovered();
    if active {
        bevel(ui.painter(), rect, false, mix(pl.light, pl.accent, 0.30));
        if pl.style == Style::Neon {
            glow_rect(ui.painter(), rect, pl.radius_f(), pl.accent, pl.glow_amt);
        }
    } else if hot || down {
        bevel(ui.painter(), rect, !down, if down { pl.press } else { pl.hover });
    }
    let c = rect.center() - vec2(0.0, 11.0) + if down { vec2(0.0, 1.0) } else { Vec2::ZERO };
    let state = if !enabled { OrbState::Off } else if active { OrbState::Active } else { OrbState::Normal };
    let tint = orb(ui.painter(), c, 36.0, state, hot && !down);
    icons::draw(ui.painter(), Rect::from_center_size(c, vec2(20.0, 20.0)), icon, tint);
    let col = if enabled { pl.ink } else { pl.lo };
    ui.painter().text(rect.center_bottom() - vec2(0.0, 11.0), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.0), col);
    resp
}

/// Aba de paleta: a ativa é mais alta e funde-se com a faixa abaixo.
pub fn tab(ui: &mut Ui, label: &str, active: bool) -> Response {
    let pl = pal();
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(14.0), pl.ink);
    let size = vec2(galley.size().x + 34.0, 30.0);
    let (full, resp) = ui.allocate_exact_size(size, Sense::click());
    let rect = if active { full } else { Rect::from_min_max(full.min + vec2(0.0, 5.0), full.max - vec2(0.0, 1.0)) };
    let fill = if active { pl.light } else if resp.hovered() { pl.hover } else { pl.tab_idle };
    bevel(ui.painter(), rect, true, fill);
    if active {
        ui.painter().rect_filled(Rect::from_min_max(rect.left_bottom() + vec2(2.0, -2.0), rect.right_bottom() + vec2(-2.0, 1.0)), 0.0, pl.stone);
        if pl.style == Style::Neon {
            ui.painter().line_segment([rect.left_top() + vec2(3.0, 1.5), rect.right_top() + vec2(-3.0, 1.5)], Stroke::new(2.0, pl.accent));
        }
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(14.0), if active { pl.heading } else { pl.ink });
    resp
}

/// Separador vertical (entre grupos de ferramentas).
pub fn vsep(ui: &mut Ui, h: f32) {
    let pl = pal();
    ui.add_space(6.0);
    let (r, _) = ui.allocate_exact_size(vec2(2.0, h), Sense::hover());
    ui.painter().line_segment([r.center_top(), r.center_bottom()], Stroke::new(1.0, pl.lo));
    let hi = if pl.dark { a(pl.glow, 40) } else { Color32::WHITE };
    ui.painter().line_segment([r.center_top() + vec2(1.0, 0.0), r.center_bottom() + vec2(1.0, 0.0)], Stroke::new(1.0, hi));
    ui.add_space(6.0);
}

/// Botão de texto. `primary` = botão principal (cápsula brilhante), maior; os demais, vidro claro.
pub fn button(ui: &mut Ui, label: &str, primary: bool, enabled: bool, min: Vec2) -> Response {
    let pl = pal();
    let font = if primary { FontId::proportional(16.0) } else { FontId::proportional(13.0) };
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), pl.ink);
    let size = vec2((galley.size().x + if primary { 34.0 } else { 22.0 }).max(min.x), (galley.size().y + 10.0).max(min.y).max(if primary { 36.0 } else { 0.0 }));
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let down = enabled && resp.is_pointer_button_down_on();
    let off = if down { vec2(0.0, 1.0) } else { Vec2::ZERO };
    if primary {
        match pl.style {
            Style::Aero => {
                let tint = if !enabled { Color32::from_white_alpha(120) } else if down { rgb(200, 200, 200) } else if resp.hovered() { Color32::WHITE } else { rgb(236, 236, 236) };
                aero::capsule(ui.painter(), if enabled { "cap_green" } else { "cap_silver" }, rect.translate(off), tint);
            }
            Style::Neon => {
                let r = rect.translate(off);
                let rad = (pl.radius_f() + 2.0).min(r.height() * 0.5);
                let col = if enabled { pl.accent } else { pl.dim };
                if enabled {
                    glow_rect(ui.painter(), r, rad, col, if resp.hovered() { pl.glow_amt } else { (pl.glow_amt as f32 * 0.6) as u8 });
                }
                ui.painter().rect_filled(r, rad, mix(pl.stone, col, if down { 0.62 } else if resp.hovered() { 0.52 } else { 0.42 }));
                if pl.gloss > 0 {
                    let rr = rad as u8;
                    ui.painter().rect_filled(Rect::from_min_max(r.min + vec2(2.0, 2.0), pos2(r.max.x - 2.0, r.center().y)), CornerRadius { nw: rr, ne: rr, sw: 0, se: 0 }, Color32::from_white_alpha(pl.gloss));
                }
                ui.painter().rect_stroke(r, rad, Stroke::new(1.6, col), StrokeKind::Inside);
            }
        }
        ui.painter().text(rect.center() + off + vec2(0.0, 1.0), egui::Align2::CENTER_CENTER, label, font.clone(), Color32::from_black_alpha(120));
        ui.painter().text(rect.center() + off, egui::Align2::CENTER_CENTER, label, font, if enabled { Color32::WHITE } else { pl.lo });
    } else {
        let fill = if down { pl.press } else if resp.hovered() && enabled { pl.hover } else { pl.light };
        bevel(ui.painter(), rect, !down, fill);
        ui.painter().text(rect.center() + off, egui::Align2::CENTER_CENTER, label, font, if enabled { pl.ink } else { pl.lo });
    }
    resp
}

pub fn small_button(ui: &mut Ui, label: &str, active: bool) -> Response {
    small_button_w(ui, label, active, 0.0)
}

pub fn small_button_w(ui: &mut Ui, label: &str, active: bool, min_w: f32) -> Response {
    let pl = pal();
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(12.0), pl.ink);
    let size = vec2((galley.size().x + 14.0).max(min_w), 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let down = resp.is_pointer_button_down_on();
    let fill = if active { pl.accent_dim } else if resp.hovered() { pl.hover } else { pl.light };
    bevel(ui.painter(), rect, !(active || down), fill);
    if active && pl.style == Style::Neon {
        ui.painter().rect_stroke(rect, pl.radius_f(), Stroke::new(1.4, pl.accent), StrokeKind::Inside);
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.0), pl.ink);
    resp
}

/// Título de seção dentro de um painel.
pub fn section_title(ui: &mut Ui, text: &str) {
    let pl = pal();
    ui.add_space(4.0);
    ui.label(RichText::new(text).strong().size(13.0).color(pl.heading));
    let r = ui.available_rect_before_wrap();
    let y = ui.cursor().top();
    ui.painter().line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0, pl.lo));
    let hi = if pl.dark { a(pl.glow, 36) } else { Color32::WHITE };
    ui.painter().line_segment([pos2(r.left(), y + 1.0), pos2(r.right(), y + 1.0)], Stroke::new(1.0, hi));
    ui.add_space(4.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_round_trip_and_tolerance() {
        let mut p = preset("matriz");
        p.id = "custom";
        p.accent = rgb(1, 2, 3);
        p.radius = 7;
        let q = from_text(&to_text(&p)).expect("tema");
        assert_eq!(q.accent, rgb(1, 2, 3));
        assert_eq!(q.radius, 7);
        assert_eq!(q.id, "custom");
        // linhas inválidas são ignoradas
        let r = from_text("preset=custom\nlixo\ndestaque=ZZZZZZ\nbrilho=abc\ntexto=FFFFFF\n").expect("tema");
        assert_eq!(r.ink, rgb(255, 255, 255));
        assert!(from_text("sem preset").is_none());
        assert_eq!(from_text("preset=inexistente").unwrap().id, "ciano");
    }
}

// --- vidro: papel de parede e molduras translúcidas ---

/// Moldura de painel: cor de fundo com a transparência do vidro e contorno fino escuro.
pub fn panel_frame() -> egui::Frame {
    let pl = pal();
    let f = egui::Frame::new().fill(pl.glassy(pl.stone, 0));
    if pl.glass < 255 {
        f.stroke(Stroke::new(1.0, Color32::from_black_alpha(if pl.dark { 150 } else { 90 })))
    } else {
        f
    }
}

type Wall = ((Color32, Color32, Color32, Color32), egui::TextureHandle);
static WALL: OnceLock<std::sync::Mutex<Option<Wall>>> = OnceLock::new();

fn wallpaper_image(pl: &Palette) -> egui::ColorImage {
    const W: usize = 320;
    const H: usize = 200;
    let white = Color32::WHITE;
    let top = mix(pl.navy, pl.navy_hi, 0.45);
    let midc = if pl.dark { mix(pl.navy_hi, pl.stone, 0.5) } else { mix(pl.navy_hi, white, 0.25) };
    let bot = if pl.dark { mix(pl.accent, pl.stone, 0.7) } else { mix(pl.accent, white, 0.5) };
    let hill = if pl.dark { mix(pl.accent, pl.stone, 0.55) } else { mix(pl.accent, white, 0.15) };
    let hash = |i: u32| -> f32 {
        let mut x = i.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
        x ^= x >> 15;
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^= x >> 12;
        (x & 0xFFFF) as f32 / 65535.0
    };
    // bolhas (marca registrada do estilo): posição, raio em fração da altura
    let bubbles: Vec<(f32, f32, f32)> = (0..18u32).map(|i| (hash(i * 3 + 1), hash(i * 3 + 2) * 0.9, 0.025 + hash(i * 3 + 3).powi(2) * 0.11)).collect();
    let mut px = Vec::with_capacity(W * H);
    for y in 0..H {
        for x in 0..W {
            let (u, v) = (x as f32 / W as f32, y as f32 / H as f32);
            let mut c = if v < 0.55 { mix(top, midc, v / 0.55) } else { mix(midc, bot, (v - 0.55) / 0.45) };
            let lighten = |c: &mut Color32, a: f32| *c = mix(*c, white, a.clamp(0.0, 1.0));
            // brilho do alto, à direita
            let (dx, dy) = ((u - 0.78) * 1.6, v - 0.12);
            lighten(&mut c, 0.55 * (-(dx * dx + dy * dy) / 0.05).exp());
            // feixes de luz diagonais
            for (off, w, k) in [(0.25f32, 0.045f32, 0.20f32), (0.55, 0.07, 0.15), (0.85, 0.03, 0.12)] {
                let t = (u * 0.8 + v * 0.6) - off - 0.0;
                lighten(&mut c, k * (-(t / w).powi(2)).exp() * (1.0 - v * 0.6));
            }
            // colina suave embaixo
            let h = 0.80 + 0.06 * (u * 3.2 + 1.0).sin() + 0.03 * (u * 7.0).cos();
            let s = ((v - (h - 0.03)) / 0.07).clamp(0.0, 1.0);
            c = mix(c, hill, s * s * (3.0 - 2.0 * s));
            for &(bx, by, br) in &bubbles {
                let (ddx, ddy) = ((u - bx) * W as f32 / H as f32, v - by);
                let d = (ddx * ddx + ddy * ddy).sqrt();
                if d < br + 0.02 {
                    if d < br {
                        lighten(&mut c, 0.07 + 0.07 * (d / br));
                    }
                    lighten(&mut c, 0.30 * (-((d - br) / 0.006).powi(2)).exp());
                    let (hx, hy) = (ddx + br * 0.35, ddy + br * 0.40);
                    lighten(&mut c, 0.22 * (-(hx * hx + hy * hy) / (br * br * 0.05 + 1e-5)).exp());
                }
            }
            px.push(c);
        }
    }
    egui::ColorImage { size: [W, H], source_size: egui::vec2(W as f32, H as f32), pixels: px }
}

/// Pinta o papel de parede atrás de tudo (só quando os painéis são translúcidos).
pub fn backdrop(ui: &Ui) {
    let pl = pal();
    if pl.glass == 255 {
        return;
    }
    let key = (pl.navy, pl.navy_hi, pl.accent, if pl.dark { Color32::BLACK } else { Color32::WHITE });
    let mut g = WALL.get_or_init(|| std::sync::Mutex::new(None)).lock().unwrap_or_else(|e| e.into_inner());
    if g.as_ref().map(|(k, _)| *k != key).unwrap_or(true) {
        let tex = ui.ctx().load_texture("papel_de_parede", wallpaper_image(&pl), egui::TextureOptions::LINEAR);
        *g = Some((key, tex));
    }
    if let Some((_, tex)) = g.as_ref() {
        ui.painter().image(tex.id(), ui.ctx().content_rect(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

/// Reflexo diagonal de vidro sobre um painel (só quando há transparência).
pub fn sheen(p: &Painter, r: Rect) {
    let pl = pal();
    if pl.glass == 255 || pl.gloss == 0 {
        return;
    }
    let p = p.with_clip_rect(r);
    let k = (pl.gloss as f32 * 0.42) as u8;
    let (w, h) = (r.width(), r.height());
    let lean = (h * 0.7).min(w * 0.6);
    for (x0, x1, alpha) in [(0.06f32, 0.40f32, k), (0.46, 0.60, k / 2)] {
        let pts = vec![
            pos2(r.left() + x0 * w + lean * 0.5, r.top()),
            pos2(r.left() + x1 * w + lean * 0.5, r.top()),
            pos2(r.left() + x1 * w - lean * 0.5, r.bottom()),
            pos2(r.left() + x0 * w - lean * 0.5, r.bottom()),
        ];
        p.add(Shape::convex_polygon(pts, Color32::from_white_alpha(alpha), Stroke::NONE));
    }
    if h <= 90.0 {
        p.rect_filled(Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + h * 0.5)), 0.0, Color32::from_white_alpha(k / 2));
    }
    p.line_segment([r.left_top() + vec2(0.0, 1.5), r.right_top() + vec2(0.0, 1.5)], Stroke::new(1.0, Color32::from_white_alpha(90)));
}

/// Mostra o painel e aplica o reflexo de vidro por cima.
pub fn sheen_after<R>(r: egui::InnerResponse<R>, ui: &Ui) -> egui::InnerResponse<R> {
    sheen(ui.painter(), r.response.rect);
    r
}
