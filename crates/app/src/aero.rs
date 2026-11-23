//! Ícones e botões brilhantes no estilo Frutiger Aero (pacote AeroMedia, CC BY 4.0; ver assets/aero/CREDITS.md).
//! Os PNG vão dentro do executável e viram texturas uma vez só.
use egui::{pos2, vec2, Color32, Context, Mesh, Painter, Rect, TextureHandle, TextureId, TextureOptions};
use std::collections::HashMap;
use std::sync::OnceLock;

macro_rules! png {
    ($n:literal) => {
        ($n, include_bytes!(concat!("../assets/aero/", $n, ".png")) as &[u8])
    };
}

const FILES: &[(&str, &[u8])] = &[
    png!("orb_blue"), png!("orb_green"), png!("orb_orange"), png!("orb_silver"), png!("orb_red"),
    png!("cap_blue"), png!("cap_green"), png!("cap_orange"), png!("cap_silver"),
    png!("g_cloud"), png!("g_allapps"), png!("g_flashlight"), png!("g_bin"), png!("g_home"), png!("g_compass"),
    png!("g_refresh_reset"), png!("g_camera"), png!("g_settings"), png!("g_plus"), png!("g_minus"), png!("g_uparrow"),
    png!("g_checkmark"), png!("g_information"), png!("g_mark"), png!("g_xmark"), png!("g_save"), png!("g_download"),
];

pub struct Aero {
    tex: HashMap<&'static str, (TextureHandle, [f32; 2])>,
}

static AERO: OnceLock<Aero> = OnceLock::new();

pub fn init(ctx: &Context) {
    AERO.get_or_init(|| {
        let mut tex = HashMap::new();
        for (name, bytes) in FILES {
            if let Ok(img) = image::load_from_memory(bytes) {
                let img = img.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                tex.insert(*name, (ctx.load_texture(*name, ci, TextureOptions::LINEAR), [size[0] as f32, size[1] as f32]));
            }
        }
        Aero { tex }
    });
}

fn get(name: &str) -> Option<(TextureId, [f32; 2])> {
    AERO.get()?.tex.get(name).map(|(t, s)| (t.id(), *s))
}

const FULL: Rect = Rect { min: pos2(0.0, 0.0), max: pos2(1.0, 1.0) };

/// Desenha uma textura inteira em `r`, tingida por `tint`.
pub fn image(p: &Painter, name: &str, r: Rect, tint: Color32) {
    if let Some((id, _)) = get(name) {
        p.image(id, r, FULL, tint);
    }
}

/// Desenha o ícone `name`, centrado em `r` (mantém a proporção), girado em `angle` radianos.
pub fn glyph(p: &Painter, name: &str, r: Rect, tint: Color32, angle: f32, flip_x: bool) -> bool {
    let Some((id, sz)) = get(name) else { return false };
    let k = (r.width() / sz[0]).min(r.height() / sz[1]);
    let (hw, hh) = (sz[0] * k * 0.5, sz[1] * k * 0.5);
    let c = r.center();
    let (s, co) = angle.sin_cos();
    let rot = |x: f32, y: f32| pos2(c.x + x * co - y * s, c.y + x * s + y * co);
    let (u0, u1) = if flip_x { (1.0, 0.0) } else { (0.0, 1.0) };
    let mut m = Mesh::with_texture(id);
    for (pt, uv) in [(rot(-hw, -hh), pos2(u0, 0.0)), (rot(hw, -hh), pos2(u1, 0.0)), (rot(hw, hh), pos2(u1, 1.0)), (rot(-hw, hh), pos2(u0, 1.0))] {
        m.vertices.push(egui::epaint::Vertex { pos: pt, uv, color: tint });
    }
    m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    p.add(egui::Shape::mesh(m));
    true
}

/// Cápsula brilhante esticada em três fatias (tampa esquerda, miolo, tampa direita).
pub fn capsule(p: &Painter, name: &str, r: Rect, tint: Color32) {
    let Some((id, sz)) = get(name) else { return };
    let cap_src = sz[1] * 0.46; // largura da tampa na imagem original
    let pad = r.height() * 0.10; // a imagem inclui uma sombra suave
    let r = r.expand2(vec2(pad, pad));
    let cap = (r.height() * (cap_src / sz[1])).min(r.width() * 0.5);
    let (u1, u2) = (cap_src / sz[0], 1.0 - cap_src / sz[0]);
    let xs = [r.left(), r.left() + cap, r.right() - cap, r.right()];
    let us = [0.0, u1, u2, 1.0];
    let mut m = Mesh::with_texture(id);
    for i in 0..4 {
        m.vertices.push(egui::epaint::Vertex { pos: pos2(xs[i], r.top()), uv: pos2(us[i], 0.0), color: tint });
        m.vertices.push(egui::epaint::Vertex { pos: pos2(xs[i], r.bottom()), uv: pos2(us[i], 1.0), color: tint });
    }
    for i in 0..3u32 {
        let b = i * 2;
        m.indices.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
    }
    p.add(egui::Shape::mesh(m));
}
