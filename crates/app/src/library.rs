//! Biblioteca de assets: varre a pasta do CD do usuário (somente leitura) e gera miniaturas em segundo plano.
use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use terralux_core::mesh::Primitive;
use terralux_core::{assets, Polyhedron, Scene, Shape};
use terralux_render::{tonemap_to_srgb8, RenderOptions, Renderer};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tab {
    Basicos,
    Divertidos,
    Terrenos,
    Modelos,
    Texturas,
    Imagens,
    Gels,
}

impl Tab {
    pub const ALL: [Tab; 7] = [Tab::Basicos, Tab::Divertidos, Tab::Terrenos, Tab::Modelos, Tab::Texturas, Tab::Imagens, Tab::Gels];
    pub fn name(self) -> &'static str {
        match self {
            Tab::Basicos => "Formas",
            Tab::Divertidos => "Divertidos",
            Tab::Terrenos => "Mapas",
            Tab::Modelos => "Modelos",
            Tab::Texturas => "Texturas",
            Tab::Imagens => "Imagens",
            Tab::Gels => "Gels",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Tab::Basicos => "formas",
            Tab::Divertidos => "divertidos",
            Tab::Terrenos => "terrenos",
            Tab::Modelos => "modelos",
            Tab::Texturas => "texturas",
            Tab::Imagens => "imagens",
            Tab::Gels => "gels",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Shape(Shape),
    Terrain,
    Texture,
    Picture,
    Gel,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub name: String,
    /// `lib:pasta/arquivo` para arquivos, `prim:Nome` para formas internas (também é a chave da miniatura).
    pub spec: String,
    pub kind: ItemKind,
}

#[derive(Default)]
pub struct Library {
    pub tabs: HashMap<Tab, Vec<Item>>,
    pub found: bool,
}

/// Modelos do CD escolhidos para a aba "Divertidos" (arquivo, nome).
const FUNNY: [(&str, &str); 14] = [
    ("Viewpoint DXF's/DXF Models/monster.dxf", "Monstro"),
    ("Viewpoint DXF's/DXF Models/shark.dxf", "Tubarão"),
    ("Viewpoint DXF's/DXF Models/ipigma1h.dxf", "Porquinho"),
    ("Viewpoint DXF's/DXF Models/iflamigm.dxf", "Flamingo"),
    ("Viewpoint DXF's/DXF Models/icrocodh.dxf", "Crocodilo"),
    ("Viewpoint DXF's/DXF Models/ideerh1h.dxf", "Cervo"),
    ("Viewpoint DXF's/DXF Models/idove1l.dxf", "Pomba"),
    ("OBJ Models/rockhorse.obj", "Cavalinho de balanço"),
    ("OBJ Models/sparrow.obj", "Pardal"),
    ("OBJ Models/sword.obj", "Espada"),
    ("OBJ Models/glasses.obj", "Óculos"),
    ("OBJ Models/bike2.obj", "Moto"),
    ("Viewpoint DXF's/DXF Models/aheart1l.dxf", "Coração"),
    ("OBJ Models/wheelbarl.obj", "Carrinho de mão"),
];

fn list(root: &Path, dir: &str, exts: &[&str]) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = std::fs::read_dir(root.join(dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_string_lossy().to_ascii_lowercase();
            exts.contains(&ext.as_str()).then(|| {
                let file = e.file_name().to_string_lossy().into_owned();
                (p.file_stem().unwrap().to_string_lossy().into_owned(), format!("{}{}/{}", assets::LIB_PREFIX, dir, file))
            })
        })
        .collect();
    v.sort_by_key(|a| a.0.to_lowercase());
    v
}

impl Library {
    pub fn scan() -> Library {
        let root = assets::library_root();
        let mut tabs: HashMap<Tab, Vec<Item>> = HashMap::new();
        let shapes = |sh: Shape, name: &str| Item { name: name.into(), spec: shape_key(&sh), kind: ItemKind::Shape(sh) };
        let mut basics = vec![shapes(Shape::Sphere, "Esfera")];
        basics.extend(Primitive::BASIC.iter().map(|p| shapes(Shape::Prim(*p), p.name())));
        basics.extend(Polyhedron::ALL.iter().map(|p| shapes(Shape::Poly(*p), p.name())));
        basics.push(shapes(Shape::Plane, "Plano (chão)"));
        tabs.insert(Tab::Basicos, basics);

        let mut fun: Vec<Item> = Primitive::FUN.iter().map(|p| shapes(Shape::Prim(*p), p.name())).collect();
        let found = root.is_dir();
        for (file, name) in FUNNY {
            if root.join(file).is_file() {
                let spec = format!("{}{}", assets::LIB_PREFIX, file);
                fun.push(Item { name: name.into(), spec: spec.clone(), kind: ItemKind::Shape(Shape::Model(spec)) });
            }
        }
        tabs.insert(Tab::Divertidos, fun);

        let mk = |v: Vec<(String, String)>, kind: ItemKind| -> Vec<Item> { v.into_iter().map(|(name, spec)| Item { name, spec, kind: kind.clone() }).collect() };
        tabs.insert(Tab::Terrenos, mk(list(&root, "Terrain  Maps", &["tif"]), ItemKind::Terrain));
        let mut models: Vec<Item> = vec![];
        for (dir, ext) in [("OBJ Models", "obj"), ("Viewpoint DXF's/DXF Models", "dxf"), ("ONYX", "dxf")] {
            for (name, spec) in list(&root, dir, &[ext]) {
                models.push(Item { name, spec: spec.clone(), kind: ItemKind::Shape(Shape::Model(spec)) });
            }
        }
        tabs.insert(Tab::Modelos, models);
        tabs.insert(Tab::Texturas, mk(list(&root, "Picture Textures", &["tif"]), ItemKind::Texture));
        tabs.insert(Tab::Imagens, mk(list(&root, "Picture Objects", &["tif"]), ItemKind::Picture));
        tabs.insert(Tab::Gels, mk(list(&root, "Light Gels", &["tif"]), ItemKind::Gel));
        Library { tabs, found }
    }

    pub fn items(&self, t: Tab) -> &[Item] {
        self.tabs.get(&t).map_or(&[], |v| v.as_slice())
    }
}

pub fn shape_key(s: &Shape) -> String {
    match s {
        Shape::Model(p) | Shape::Picture(p) => p.clone(),
        other => format!("prim:{other:?}"),
    }
}

// ---------- miniaturas ----------

pub const THUMB: usize = 80;

struct Req {
    key: String,
    shape: Option<Shape>,
}

pub struct Thumb {
    key: String,
    img: ColorImage,
}

pub struct Thumbs {
    tx: Sender<Req>,
    rx: Receiver<Thumb>,
    pending: HashSet<String>,
    pub tex: HashMap<String, TextureHandle>,
}

impl Thumbs {
    pub fn spawn(ctx: Context) -> Self {
        let (tx, rx_req) = channel::<Req>();
        let (tx_th, rx) = channel::<Thumb>();
        std::thread::spawn(move || {
            while let Ok(r) = rx_req.recv() {
                let img = match &r.shape {
                    Some(sh) => shape_thumb(sh),
                    None => image_thumb(&r.key),
                };
                let img = img.unwrap_or_else(|| ColorImage::filled([THUMB, THUMB], egui::Color32::from_rgb(70, 40, 40)));
                if tx_th.send(Thumb { key: r.key, img }).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Thumbs { tx, rx, pending: HashSet::new(), tex: HashMap::new() }
    }

    pub fn poll(&mut self, ctx: &Context) {
        while let Ok(t) = self.rx.try_recv() {
            self.pending.remove(&t.key);
            self.tex.insert(t.key.clone(), ctx.load_texture(format!("th:{}", t.key), t.img, TextureOptions::LINEAR));
        }
    }

    pub fn want(&mut self, it: &Item) {
        if self.tex.contains_key(&it.spec) || self.pending.contains(&it.spec) {
            return;
        }
        self.pending.insert(it.spec.clone());
        let shape = match &it.kind {
            ItemKind::Shape(s) => Some(s.clone()),
            _ => None,
        };
        let _ = self.tx.send(Req { key: it.spec.clone(), shape });
    }
}

fn shape_thumb(sh: &Shape) -> Option<ColorImage> {
    let mut s = Scene::empty();
    s.sky.clouds = None;
    s.sky.zenith = glam::Vec3::new(0.30, 0.38, 0.52);
    s.sky.horizon = glam::Vec3::new(0.52, 0.58, 0.66);
    s.sky.haze = 0.0;
    s.sun.azimuth_deg = -35.0;
    s.sun.elevation_deg = 38.0;
    let plane = matches!(sh, Shape::Plane);
    s.camera.position = glam::Vec3::new(0.0, if plane { 1.2 } else { 0.9 }, -3.5);
    s.camera.target = glam::Vec3::ZERO;
    s.camera.fov_deg = 40.0;
    s.objects.push(terralux_core::Object {
        name: String::new(),
        shape: sh.clone(),
        position: glam::Vec3::new(0.0, if plane { -0.6 } else { 0.0 }, 0.0),
        rotation_deg: if plane { glam::Vec3::ZERO } else { glam::Vec3::new(0.0, 28.0, 0.0) },
        size: 1.0,
        material: terralux_core::Material::matte(glam::Vec3::splat(0.9)),
    });
    let rgb = tonemap_to_srgb8(&Renderer::new(&s).render(&RenderOptions::new(THUMB, THUMB, 2)));
    Some(ColorImage::from_rgb([THUMB, THUMB], &rgb))
}

fn image_thumb(spec: &str) -> Option<ColorImage> {
    let img = assets::load_image(&assets::resolve(spec)).ok()?;
    // redução por média de caixa, mantendo a proporção dentro de THUMB × THUMB
    let k = (img.w.max(img.h) as f32 / THUMB as f32).max(1.0);
    let (tw, th) = (((img.w as f32 / k).round() as usize).max(1), ((img.h as f32 / k).round() as usize).max(1));
    let mut out = vec![egui::Color32::TRANSPARENT; THUMB * THUMB];
    let (ox, oy) = ((THUMB - tw) / 2, (THUMB - th) / 2);
    for y in 0..th {
        for x in 0..tw {
            let (x0, x1) = ((x as f32 * k) as usize, (((x + 1) as f32 * k) as usize).min(img.w).max((x as f32 * k) as usize + 1));
            let (y0, y1) = ((y as f32 * k) as usize, (((y + 1) as f32 * k) as usize).min(img.h).max((y as f32 * k) as usize + 1));
            let (mut acc, mut n) = ([0u32; 4], 0u32);
            for yy in y0..y1.min(img.h) {
                for xx in x0..x1.min(img.w) {
                    let p = &img.rgba[(yy * img.w + xx) * 4..][..4];
                    for c in 0..4 {
                        acc[c] += p[c] as u32;
                    }
                    n += 1;
                }
            }
            let n = n.max(1);
            let a = acc[3] / n;
            // sobre um xadrez escuro, para enxergar o recorte
            let chk = if ((x / 6) + (y / 6)) % 2 == 0 { 54u32 } else { 70 };
            let mix = |c: usize| ((acc[c] / n * a + chk * (255 - a)) / 255) as u8;
            out[(y + oy) * THUMB + x + ox] = egui::Color32::from_rgb(mix(0), mix(1), mix(2));
        }
    }
    Some(ColorImage { size: [THUMB, THUMB], source_size: egui::vec2(THUMB as f32, THUMB as f32), pixels: out })
}
