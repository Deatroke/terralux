//! Renderizador offline em CPU (ray tracer estilo Whitted + céu/água/terreno procedurais).
mod bvh;
mod poly;
mod sky;

use glam::{Quat, Vec2, Vec3};
use bvh::MeshBvh;
use poly::{Convex, Hit as PolyHit};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use terralux_core::noise::{fbm2, perlin2, ridged2};
use terralux_core::assets::Img;
use terralux_core::terrain::smoothstep;
use terralux_core::{Biome, Heightfield, Mapping, Scene, Shape, TextureRef, Water};

pub struct RenderOptions {
    pub width: usize,
    pub height: usize,
    /// Amostras por pixel (antialiasing por jitter estratificado).
    pub spp: usize,
    pub progress: bool,
    /// Cancela quando o contador atual difere do id do trabalho.
    pub cancel: Option<(Arc<AtomicU64>, u64)>,
    /// Linhas concluídas (para barras de progresso).
    pub rows_done: Option<Arc<AtomicUsize>>,
}

impl RenderOptions {
    pub fn new(width: usize, height: usize, spp: usize) -> Self {
        RenderOptions { width, height, spp, progress: false, cancel: None, rows_done: None }
    }

    fn cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(|(c, id)| c.load(Ordering::Relaxed) != *id)
    }
}

/// O que há sob um raio (para seleção por clique).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Object(usize),
    Terrain,
    Water,
    Sky,
}

enum Body {
    Sphere { c: Vec3, r: f32 },
    Convex(Convex),
    /// Malha no espaço do objeto: raio entra por `inv`, normal sai por `q`.
    Mesh { bvh: Arc<MeshBvh>, pos: Vec3, q: Quat, size: f32 },
    Plane { y: f32 },
    /// Imagem em pé (plano local z=0), recortada pelo alfa.
    Picture { pos: Vec3, q: Quat, hw: f32, hh: f32, img: Arc<Img> },
}

/// sRGB 8 bits → linear (tabela, evita `powf` por texel).
fn srgb_lut() -> &'static [f32; 256] {
    static L: OnceLock<[f32; 256]> = OnceLock::new();
    L.get_or_init(|| {
        let mut t = [0.0; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *v = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        }
        t
    })
}

/// Amostra bilinear com repetição; devolve cor linear e alfa. `u`,`v` em repetições (0..1 = uma imagem).
fn tex_sample(img: &Img, u: f32, v: f32) -> (Vec3, f32) {
    let lut = srgb_lut();
    let (fx, fy) = (u.rem_euclid(1.0) * img.w as f32 - 0.5, v.rem_euclid(1.0) * img.h as f32 - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (a, b) = (fx - x0, fy - y0);
    let px = |x: f32, y: f32| {
        let (xi, yi) = ((x as i64).rem_euclid(img.w as i64) as usize, (y as i64).rem_euclid(img.h as i64) as usize);
        let p = &img.rgba[(yi * img.w + xi) * 4..][..4];
        (Vec3::new(lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize]), p[3] as f32 / 255.0)
    };
    let (c00, a00) = px(x0, y0);
    let (c10, a10) = px(x0 + 1.0, y0);
    let (c01, a01) = px(x0, y0 + 1.0);
    let (c11, a11) = px(x0 + 1.0, y0 + 1.0);
    let c = (c00 * (1.0 - a) + c10 * a) * (1.0 - b) + (c01 * (1.0 - a) + c11 * a) * b;
    let al = (a00 * (1.0 - a) + a10 * a) * (1.0 - b) + (a01 * (1.0 - a) + a11 * a) * b;
    (c, al)
}

/// Cache global de BVHs (por primitiva ou caminho): órbitas na prévia não reprocessam OBJ.
fn mesh_bvh(shape: &Shape) -> Option<Arc<MeshBvh>> {
    static C: OnceLock<Mutex<HashMap<String, Option<Arc<MeshBvh>>>>> = OnceLock::new();
    let key = match shape {
        Shape::Prim(p) => format!("prim:{p:?}"),
        Shape::Model(m) => format!("model:{m}"),
        _ => return None,
    };
    let c = C.get_or_init(Default::default);
    if let Some(v) = c.lock().unwrap().get(&key) {
        return v.clone();
    }
    let mesh = match shape {
        Shape::Prim(p) => Some(p.mesh()),
        Shape::Model(m) => match terralux_core::assets::load_mesh(m) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("modelo: {e}");
                None
            }
        },
        _ => None,
    };
    let v = mesh.map(|m| Arc::new(MeshBvh::new(&m)));
    c.lock().unwrap().insert(key, v.clone());
    v
}

struct Obj {
    body: Body,
    mat: terralux_core::Material,
    pos: Vec3,
    q: Quat,
    tex: Option<(Arc<Img>, TextureRef)>,
    plane: bool,
    /// Imagens recortadas: sombreamento "de papel", sem brilho.
    flat: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Terrain,
    Water,
    Object(usize),
}

struct Hit {
    t: f32,
    p: Vec3,
    n: Vec3,
    kind: Kind,
    /// Cor do material da malha (Kd), multiplicada pela cor do objeto.
    tint: Vec3,
}

pub struct Renderer<'a> {
    scene: &'a Scene,
    hf: Option<Arc<Heightfield>>,
    terrain_tex: Option<(Arc<Img>, TextureRef)>,
    objs: Vec<Obj>,
    sun: Vec3,
    water_level: f32,
    /// Modo de prévia: sem sombras, sem reflexos recursivos e sem raio de refração na água.
    fast: bool,
}

fn rng_next(s: &mut u32) -> f32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    (*s >> 8) as f32 / 16_777_216.0
}

impl<'a> Renderer<'a> {
    pub fn new(scene: &'a Scene) -> Self {
        let hf = scene.terrain.as_ref().map(|d| Arc::new(Heightfield::from_def(d)));
        Self::with_heightfield(scene, hf)
    }

    /// Usa um heightfield já gerado (cache do editor).
    pub fn with_heightfield(scene: &'a Scene, hf: Option<Arc<Heightfield>>) -> Self {
        let objs = scene
            .objects
            .iter()
            .map(|o| {
                let q = Quat::from_euler(
                    glam::EulerRot::XYZ,
                    o.rotation_deg.x.to_radians(),
                    o.rotation_deg.y.to_radians(),
                    o.rotation_deg.z.to_radians(),
                );
                let body = match &o.shape {
                    Shape::Sphere => Body::Sphere { c: o.position, r: o.size },
                    Shape::Poly(k) => Body::Convex(Convex::new(*k, o.position, q, o.size)),
                    Shape::Plane => Body::Plane { y: o.position.y },
                    Shape::Picture(path) => match terralux_core::assets::cached_image(path) {
                        Ok(img) => {
                            let hw = o.size.max(1e-3);
                            let hh = hw * img.h as f32 / img.w as f32;
                            Body::Picture { pos: o.position, q, hw, hh, img }
                        }
                        Err(e) => {
                            eprintln!("imagem: {e}");
                            Body::Sphere { c: o.position, r: 0.5 }
                        }
                    },
                    sh => match mesh_bvh(sh) {
                        Some(bvh) => Body::Mesh { bvh, pos: o.position, q, size: o.size.max(1e-4) },
                        // modelo ausente: marcador esférico pequeno em vez de sumir
                        None => Body::Sphere { c: o.position, r: 0.5 },
                    },
                };
                let tex = o.material.texture.as_ref().and_then(|t| terralux_core::assets::cached_image(&t.path).ok().map(|i| (i, t.clone())));
                let flat = matches!(body, Body::Picture { .. });
                Obj { body, mat: o.material.clone(), pos: o.position, q, tex, plane: matches!(o.shape, Shape::Plane), flat }
            })
            .collect();
        let terrain_tex = scene.terrain.as_ref().and_then(|d| d.texture.as_ref()).and_then(|t| terralux_core::assets::cached_image(&t.path).ok().map(|i| (i, t.clone())));
        Renderer { scene, hf, terrain_tex, objs, sun: scene.sun.direction(), water_level: scene.water.as_ref().map_or(f32::NEG_INFINITY, |w| w.level), fast: false }
    }

    /// Liga o modo rápido (prévia do editor): bem mais leve, com menos fidelidade.
    pub fn fast(mut self, on: bool) -> Self {
        self.fast = on;
        self
    }

    /// Renderiza e devolve RGB linear (f32), linha a linha de cima para baixo.
    pub fn render(&self, o: &RenderOptions) -> Vec<f32> {
        let cam = &self.scene.camera;
        let fwd = (cam.target - cam.position).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let half_h = (cam.fov_deg.to_radians() * 0.5).tan();
        let half_w = half_h * o.width as f32 / o.height as f32;
        let spp = o.spp.max(1);
        let grid = (spp as f32).sqrt().ceil() as usize;
        let done = AtomicUsize::new(0);
        let mut img = vec![0.0f32; o.width * o.height * 3];
        img.par_chunks_mut(o.width * 3).enumerate().for_each(|(y, row)| {
            if o.cancelled() {
                return;
            }
            for x in 0..o.width {
                let mut seed = (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663) ^ 0x1234_5679;
                if seed == 0 {
                    seed = 1;
                }
                let mut acc = Vec3::ZERO;
                for s in 0..spp {
                    let (sx, sy) = ((s % grid) as f32, (s / grid) as f32);
                    let jx = (sx + rng_next(&mut seed)) / grid as f32;
                    let jy = (sy + rng_next(&mut seed)) / grid as f32;
                    let u = ((x as f32 + jx) / o.width as f32 * 2.0 - 1.0) * half_w;
                    let v = (1.0 - (y as f32 + jy) / o.height as f32 * 2.0) * half_h;
                    let dir = (fwd + right * u + up * v).normalize();
                    let c = self.trace(cam.position, dir, 0, &mut seed);
                    // Limita fireflies de brilhos muito altos antes de média.
                    let l = c.max_element();
                    acc += if l > 16.0 { c * (16.0 / l) } else { c };
                }
                let c = acc / spp as f32;
                row[x * 3] = c.x;
                row[x * 3 + 1] = c.y;
                row[x * 3 + 2] = c.z;
            }
            if let Some(r) = &o.rows_done {
                r.fetch_add(1, Ordering::Relaxed);
            }
            if o.progress {
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if d % 60 == 0 || d == o.height {
                    eprint!("\rrenderizando: {:>3}%", d * 100 / o.height);
                }
            }
        });
        if o.progress {
            eprintln!();
        }
        let e = cam.exposure;
        for v in img.iter_mut() {
            *v *= e;
        }
        img
    }

    /// Identifica o que está sob o raio (seleção por clique).
    pub fn pick(&self, o: Vec3, d: Vec3) -> Pick {
        match self.closest(o, d, true) {
            None => Pick::Sky,
            Some(h) => match h.kind {
                Kind::Terrain => Pick::Terrain,
                Kind::Water => Pick::Water,
                Kind::Object(i) => Pick::Object(i),
            },
        }
    }

    // ---------- geometria ----------

    fn hit_terrain(&self, o: Vec3, d: Vec3, tmax: f32) -> Option<f32> {
        let hf = self.hf.as_ref()?;
        // Recorte pela caixa do terreno.
        let (mut t0, mut t1) = (0.0f32, tmax);
        for a in 0..3 {
            let (lo, hi) = match a {
                0 => (-hf.half, hf.half),
                1 => (hf.min - 0.01, hf.max + 0.01),
                _ => (-hf.half, hf.half),
            };
            let (oa, da) = (o[a], d[a]);
            if da.abs() < 1e-9 {
                if oa < lo || oa > hi {
                    return None;
                }
            } else {
                let (mut ta, mut tb) = ((lo - oa) / da, (hi - oa) / da);
                if ta > tb {
                    std::mem::swap(&mut ta, &mut tb);
                }
                t0 = t0.max(ta);
                t1 = t1.min(tb);
                if t0 > t1 {
                    return None;
                }
            }
        }
        let dxz = Vec2::new(d.x, d.z).length();
        let min_step = hf.cell * 0.25;
        let mut t = t0;
        let mut prev = t0;
        let mut iters = 0;
        while t <= t1 && iters < 3000 {
            iters += 1;
            let p = o + d * t;
            let f = p.y - hf.height(p.x, p.z);
            if f < 0.0 {
                // refina por bissecção entre prev e t
                let (mut a, mut b) = (prev, t);
                for _ in 0..8 {
                    let m = 0.5 * (a + b);
                    let pm = o + d * m;
                    if pm.y - hf.height(pm.x, pm.z) < 0.0 {
                        b = m;
                    } else {
                        a = m;
                    }
                }
                return Some(b);
            }
            prev = t;
            let denom = hf.max_slope * dxz - d.y;
            let step = if denom > 1e-4 { f / denom * 0.9 } else { 8.0 };
            t += step.clamp(min_step, 12.0);
        }
        None
    }

    fn hit_objects(&self, o: Vec3, d: Vec3, tmax: f32) -> Option<(f32, Vec3, usize, Vec3)> {
        let mut best: Option<(f32, Vec3, usize, Vec3)> = None;
        for (i, ob) in self.objs.iter().enumerate() {
            let h = match &ob.body {
                Body::Sphere { c, r } => {
                    let oc = o - *c;
                    let b = oc.dot(d);
                    let disc = b * b - (oc.dot(oc) - r * r);
                    if disc < 0.0 {
                        None
                    } else {
                        let t = -b - disc.sqrt();
                        if t > 1e-3 {
                            Some((t, (o + d * t - *c).normalize(), Vec3::ONE))
                        } else {
                            None
                        }
                    }
                }
                Body::Convex(cv) => cv.intersect(o, d).map(|PolyHit { t, n }| (t, n, Vec3::ONE)),
                Body::Plane { y } => {
                    if d.y.abs() < 1e-6 {
                        None
                    } else {
                        let t = (y - o.y) / d.y;
                        (t > 1e-3).then_some((t, if d.y < 0.0 { Vec3::Y } else { -Vec3::Y }, Vec3::ONE))
                    }
                }
                Body::Picture { pos, q, hw, hh, img } => {
                    let inv = q.inverse();
                    let (ol, dl) = (inv * (o - *pos), inv * d);
                    if dl.z.abs() < 1e-6 {
                        None
                    } else {
                        let t = -ol.z / dl.z;
                        let (x, y) = (ol.x + dl.x * t, ol.y + dl.y * t);
                        if t > 1e-3 && x.abs() < *hw && y.abs() < *hh {
                            let (_, al) = tex_sample(img, (x / hw + 1.0) * 0.5, 1.0 - (y / hh + 1.0) * 0.5);
                            let n = *q * if dl.z > 0.0 { -Vec3::Z } else { Vec3::Z };
                            (al > 0.5).then_some((t, n, Vec3::ONE))
                        } else {
                            None
                        }
                    }
                }
                Body::Mesh { bvh, pos, q, size } => {
                    let inv = q.inverse();
                    let (ol, dl) = (inv * (o - *pos) / *size, inv * d);
                    bvh.intersect(ol, dl, tmax / *size, false).map(|h| {
                        let mut n = *q * h.n;
                        if n.dot(d) > 0.0 {
                            n = -n;
                        }
                        (h.t * *size, n, bvh.mats.get(h.mat as usize).copied().unwrap_or(Vec3::ONE))
                    })
                }
            };
            if let Some((t, n, tint)) = h {
                if t < tmax && best.map_or(true, |b| t < b.0) {
                    best = Some((t, n, i, tint));
                }
            }
        }
        best
    }

    /// Cor extra (textura) no ponto de impacto de um objeto.
    fn texture_tint(&self, i: usize, p: Vec3, n: Vec3) -> Vec3 {
        let ob = &self.objs[i];
        if let Body::Picture { pos, q, hw, hh, img } = &ob.body {
            let l = q.inverse() * (p - *pos);
            return tex_sample(img, (l.x / hw + 1.0) * 0.5, 1.0 - (l.y / hh + 1.0) * 0.5).0;
        }
        let Some((img, t)) = &ob.tex else { return Vec3::ONE };
        let sc = t.scale.max(1e-3);
        let inv = ob.q.inverse();
        let local = inv * (p - ob.pos); // metros no espaço do objeto
        let (u, v) = if ob.plane {
            (p.x / sc, p.z / sc)
        } else {
            match t.mapping {
                Mapping::Planar => (local.x / sc, local.z / sc),
                Mapping::Box => {
                    let ln = (inv * n).abs();
                    if ln.x >= ln.y && ln.x >= ln.z {
                        (local.z / sc, local.y / sc)
                    } else if ln.y >= ln.z {
                        (local.x / sc, local.z / sc)
                    } else {
                        (local.x / sc, local.y / sc)
                    }
                }
                Mapping::Spherical => {
                    let r = local.normalize_or_zero();
                    (r.z.atan2(r.x) / std::f32::consts::TAU + 0.5, r.y.clamp(-1.0, 1.0).acos() / std::f32::consts::PI)
                }
            }
        };
        tex_sample(img, u, v).0
    }

    fn closest(&self, o: Vec3, d: Vec3, include_water: bool) -> Option<Hit> {
        let mut tmax = 1e5f32;
        let mut res: Option<Hit> = None;
        if include_water && self.scene.water.is_some() && d.y < -1e-5 && o.y > self.water_level {
            let t = (self.water_level - o.y) / d.y;
            if t > 0.0 {
                tmax = t;
                let p = o + d * t;
                res = Some(Hit { t, p, n: Vec3::Y, kind: Kind::Water, tint: Vec3::ONE });
            }
        }
        if let Some(t) = self.hit_terrain(o, d, tmax) {
            let p = o + d * t;
            let n = self.hf.as_ref().unwrap().normal(p.x, p.z);
            tmax = t;
            res = Some(Hit { t, p, n, kind: Kind::Terrain, tint: Vec3::ONE });
        }
        if let Some((t, n, i, tint)) = self.hit_objects(o, d, tmax) {
            let p = o + d * t;
            let tint = tint * self.texture_tint(i, p, n);
            res = Some(Hit { t, p, n, kind: Kind::Object(i), tint });
        }
        res
    }

    fn shadowed(&self, p: Vec3) -> bool {
        let l = self.sun;
        self.hit_terrain(p, l, 400.0).is_some() || self.hit_objects(p, l, 1e5).is_some()
    }

    // ---------- shading ----------

    fn fog(&self, col: Vec3, t: f32, dir: Vec3) -> Vec3 {
        let f = 1.0 - (-t * self.scene.sky.haze).exp();
        let haze = sky::horizon_color(self.scene, dir, self.sun);
        col * (1.0 - f) + haze * f
    }

    fn ambient(&self, n: Vec3) -> Vec3 {
        let s = &self.scene.sky;
        let k = 0.5 + 0.5 * n.y;
        (s.horizon * 0.55 * (1.0 - k) + s.zenith * 0.9 * k) * 0.55 + Vec3::splat(0.02)
    }

    fn terrain_color(&self, p: Vec3, n: Vec3) -> Vec3 {
        if let Some((img, t)) = &self.terrain_tex {
            let sc = t.scale.max(1e-3);
            return tex_sample(img, p.x / sc, p.z / sc).0;
        }
        self.terrain_albedo(p, n)
    }

    fn terrain_albedo(&self, p: Vec3, n: Vec3) -> Vec3 {
        let (biome, peak) = self.scene.terrain.as_ref().map_or((Biome::Tropical, 21.0), |t| (t.biome, t.peak_height.max(1.0)));
        let q = Vec2::new(p.x, p.z);
        let slope = 1.0 - n.y;
        let rockn = fbm2(q * 0.35, 4, 2.0, 0.5, 3) * 0.5 + 0.5;
        let fine = perlin2(q * 2.3, 9) * 0.5 + 0.5;
        let strata = (perlin2(Vec2::new(q.x * 0.04 + q.y * 0.03, p.y * 0.9), 21) * 0.5 + 0.5) * 0.35;
        let c3 = |r: f32, g: f32, b: f32| Vec3::new(r, g, b);
        match biome {
            Biome::Tropical => {
                let rock_a = c3(0.34, 0.29, 0.25);
                let rock_b = c3(0.52, 0.44, 0.35);
                let mut c = rock_a.lerp(rock_b, (rockn * 0.7 + strata + fine * 0.25).clamp(0.0, 1.0));
                // Grama em declives suaves acima da areia.
                let grass = c3(0.20, 0.30, 0.10).lerp(c3(0.30, 0.38, 0.12), fine);
                let gmask = ((0.28 - slope) / 0.2).clamp(0.0, 1.0) * ((p.y - 1.0) / 3.0).clamp(0.0, 1.0)
                    * (1.0 - ((p.y - 18.0) / 10.0).clamp(0.0, 1.0));
                c = c.lerp(grass, gmask * 0.85);
                // Areia na linha d'água e abaixo.
                let sand = c3(0.76, 0.68, 0.50).lerp(c3(0.66, 0.58, 0.42), fine);
                let smask = (1.0 - ((p.y - 1.4) / 2.2).clamp(0.0, 1.0)).clamp(0.0, 1.0) * (1.0 - (slope * 2.2).clamp(0.0, 1.0));
                c = c.lerp(sand, smask);
                // Neve em picos.
                let snow = ((p.y - 29.0) / 5.0).clamp(0.0, 1.0) * (1.0 - (slope * 1.6).clamp(0.0, 1.0));
                c.lerp(Vec3::splat(0.9), snow)
            }
            Biome::Alpine => {
                let rock = c3(0.30, 0.29, 0.28).lerp(c3(0.50, 0.48, 0.45), (rockn * 0.7 + strata + fine * 0.2).clamp(0.0, 1.0));
                let grass = c3(0.16, 0.26, 0.09).lerp(c3(0.26, 0.36, 0.12), fine);
                let low = 1.0 - ((p.y - peak * 0.22) / (peak * 0.22)).clamp(0.0, 1.0);
                let gmask = ((0.30 - slope) / 0.2).clamp(0.0, 1.0) * low.max(0.0) * ((p.y + 0.5) / 2.0).clamp(0.0, 1.0);
                let mut c = rock.lerp(grass, gmask * 0.9);
                // Margem de terra úmida perto da água.
                let shore = (1.0 - ((p.y - 0.2) / 1.6).clamp(0.0, 1.0)) * (1.0 - (slope * 2.0).clamp(0.0, 1.0));
                c = c.lerp(c3(0.30, 0.25, 0.18), shore);
                let snow = ((p.y - peak * 0.58) / (peak * 0.14)).clamp(0.0, 1.0) * (1.0 - (slope * 1.4 - 0.1).clamp(0.0, 1.0));
                c.lerp(c3(0.93, 0.95, 0.98), snow)
            }
            Biome::Desert => {
                let sand_a = c3(0.78, 0.56, 0.30);
                let sand_b = c3(0.62, 0.40, 0.21);
                let ripple = perlin2(Vec2::new(q.x * 0.9 + q.y * 0.6, q.y * 0.5 - q.x * 0.3), 33) * 0.5 + 0.5;
                let mut c = sand_a.lerp(sand_b, (rockn * 0.55 + ripple * 0.25 + strata * 0.5).clamp(0.0, 1.0));
                // Sotavento das dunas fica mais escuro e avermelhado.
                c *= 1.0 - (slope * 0.9).clamp(0.0, 0.35);
                let rock = c3(0.46, 0.30, 0.22).lerp(c3(0.58, 0.40, 0.28), fine);
                c.lerp(rock, ((slope - 0.45) / 0.3).clamp(0.0, 1.0))
            }
            Biome::Snow => {
                let snow = c3(0.93, 0.95, 1.0).lerp(c3(0.80, 0.86, 0.96), (fine * 0.5 + rockn * 0.4).clamp(0.0, 1.0));
                let rock = c3(0.30, 0.29, 0.30).lerp(c3(0.46, 0.44, 0.43), rockn);
                let bare = ((slope - 0.38) / 0.28).clamp(0.0, 1.0);
                snow.lerp(rock, bare)
            }
            Biome::RedRock => {
                // Faixas de rocha sedimentar: a cor muda com a altura.
                let band = (p.y * 0.42 + perlin2(q * 0.02, 5) * 1.3 + fine * 0.15).rem_euclid(1.0);
                let pal = [c3(0.62, 0.28, 0.15), c3(0.77, 0.46, 0.26), c3(0.50, 0.21, 0.13), c3(0.82, 0.62, 0.42), c3(0.66, 0.34, 0.20)];
                let k = band * pal.len() as f32;
                let i = (k as usize).min(pal.len() - 1);
                let mut c = pal[i].lerp(pal[(i + 1) % pal.len()], smoothstep(0.7, 1.0, k - i as f32));
                c *= 0.85 + 0.25 * rockn;
                // Poeira e areia nos patamares planos e no fundo do vale.
                let dust = c3(0.82, 0.58, 0.38);
                c.lerp(dust, ((0.16 - slope) / 0.14).clamp(0.0, 1.0) * 0.8)
            }
            Biome::Volcanic => {
                let basalt = c3(0.045, 0.042, 0.045).lerp(c3(0.17, 0.15, 0.14), (rockn * 0.7 + fine * 0.3).clamp(0.0, 1.0));
                let ash = c3(0.30, 0.28, 0.27);
                let mut c = basalt.lerp(ash, ((0.2 - slope) / 0.18).clamp(0.0, 1.0) * 0.7);
                // Brasas nas rachaduras perto da cratera.
                let crack = (1.0 - perlin2(q * 0.5, 12).abs() * 4.0).clamp(0.0, 1.0);
                let hot = ((p.y - peak * 0.72) / (peak * 0.15)).clamp(0.0, 1.0);
                c += c3(0.55, 0.12, 0.02) * (crack * crack * hot * 1.2);
                let sand = c3(0.14, 0.13, 0.12);
                c.lerp(sand, (1.0 - ((p.y - 1.0) / 2.0).clamp(0.0, 1.0)) * 0.9)
            }
            Biome::Grassland => {
                let grass = c3(0.18, 0.32, 0.09).lerp(c3(0.34, 0.45, 0.14), (fine * 0.6 + rockn * 0.4).clamp(0.0, 1.0));
                let patch = c3(0.40, 0.33, 0.16);
                let mut c = grass.lerp(patch, (rockn * rockn * 0.35).clamp(0.0, 0.35));
                let dirt = c3(0.36, 0.28, 0.20).lerp(c3(0.46, 0.38, 0.28), fine);
                c = c.lerp(dirt, ((slope - 0.22) / 0.25).clamp(0.0, 1.0));
                let sand = c3(0.72, 0.65, 0.46);
                c.lerp(sand, (1.0 - ((p.y - 0.4) / 1.4).clamp(0.0, 1.0)) * (1.0 - (slope * 2.0).clamp(0.0, 1.0)))
            }
        }
    }

    fn shade_surface(&self, albedo: Vec3, n: Vec3, p: Vec3, spec: f32, shin: f32, view: Vec3) -> Vec3 {
        let sun = &self.scene.sun;
        let ndl = n.dot(self.sun).max(0.0);
        let lit = if ndl > 0.0 && (self.fast || !self.shadowed(p + n * 0.05)) { ndl } else { 0.0 };
        let sun_col = sun.color * sun.intensity;
        let h = (self.sun - view).normalize();
        let s = if lit > 0.0 { spec * n.dot(h).max(0.0).powf(shin) * lit } else { 0.0 };
        albedo * (sun_col * lit + self.ambient(n)) + sun_col * s
    }

    fn trace(&self, o: Vec3, d: Vec3, depth: u32, seed: &mut u32) -> Vec3 {
        match self.closest(o, d, true) {
            None => sky::sky_color(self.scene, o, d, self.sun, self.fast),
            Some(h) => {
                let col = match h.kind {
                    Kind::Terrain => {
                        let alb = self.terrain_color(h.p, h.n);
                        self.shade_surface(alb, h.n, h.p, 0.04, 20.0, d)
                    }
                    Kind::Object(i) => {
                        let m = &self.objs[i].mat;
                        let albedo = m.color * h.tint;
                        let base = if self.objs[i].flat {
                            let ndl = h.n.dot(self.sun).abs().max(0.35);
                            albedo * (self.scene.sun.color * self.scene.sun.intensity * ndl * 0.55 + self.ambient(h.n) * 1.3)
                        } else {
                            self.shade_surface(albedo, h.n, h.p, m.specular, m.shininess, d)
                        };
                        if depth < 3 && m.reflectivity > 0.0 {
                            let r = d - 2.0 * d.dot(h.n) * h.n;
                            let rc = if self.fast { sky::sky_color(self.scene, h.p, r, self.sun, true) } else { self.trace(h.p + h.n * 0.02, r, depth + 1, seed) };
                            base * (1.0 - m.reflectivity) + rc * m.color.lerp(Vec3::ONE, 0.25) * m.reflectivity
                        } else {
                            base
                        }
                    }
                    Kind::Water => self.shade_water(&h, d, depth, seed),
                };
                self.fog(col, h.t, d)
            }
        }
    }

    fn water_normal(&self, w: &Water, p: Vec3, t: f32) -> Vec3 {
        // Ondas perdem amplitude com a distância (evita ruído/aliasing no horizonte).
        let fade = 1.0 / (1.0 + t * 0.012);
        let amp = w.wave_height * fade;
        let s = 1.0 / w.wave_scale.max(0.05);
        let h = |x: f32, z: f32| -> f32 {
            let q = Vec2::new(x, z) * s;
            ridged2(q * 0.11, 3, 2.0, 0.5, 5) * 1.4 + perlin2(q * 0.55 + Vec2::splat(3.0), 8) * 0.35 + perlin2(q * 1.7, 2) * 0.08
        };
        let e = 0.15;
        let dx = (h(p.x + e, p.z) - h(p.x - e, p.z)) / (2.0 * e);
        let dz = (h(p.x, p.z + e) - h(p.x, p.z - e)) / (2.0 * e);
        Vec3::new(-dx * amp, 1.0, -dz * amp).normalize()
    }

    fn shade_water(&self, h: &Hit, d: Vec3, depth: u32, seed: &mut u32) -> Vec3 {
        let w = self.scene.water.as_ref().unwrap();
        let mut n = self.water_normal(w, h.p, h.t);
        if n.dot(d) > 0.0 {
            n = Vec3::Y;
        }
        let cos_i = (-d.dot(n)).clamp(0.0, 1.0);
        let fres = 0.02 + 0.98 * (1.0 - cos_i).powi(5);
        // Reflexo (sem passar para baixo da superfície).
        let mut r = d - 2.0 * d.dot(n) * n;
        if r.y < 0.01 {
            r.y = 0.01;
            r = r.normalize();
        }
        let refl = if self.fast {
            sky::sky_color(self.scene, h.p, r, self.sun, true)
        } else if depth < 3 {
            self.trace(h.p + Vec3::Y * 0.02, r, depth + 1, seed)
        } else {
            self.scene.sky.horizon
        };
        // Brilho do sol na água (lóbulo especular).
        let sun = &self.scene.sun;
        let hv = (self.sun - d).normalize();
        let glint = n.dot(hv).max(0.0).powf(900.0) * 60.0 + n.dot(hv).max(0.0).powf(60.0) * 0.4;
        let glint_c = sun.color * sun.intensity * glint * fres.max(0.15);
        // Refração: segue sob a superfície até o fundo.
        let eta = 1.0 / 1.333;
        let k = 1.0 - eta * eta * (1.0 - cos_i * cos_i);
        let rd = (d * eta + n * (eta * cos_i - k.max(0.0).sqrt())).normalize();
        let under = if self.fast {
            // Prévia: só olha o fundo logo abaixo do ponto (sem marchar o raio refratado).
            self.hf.as_ref().and_then(|hf| {
                let depth_below = self.water_level - hf.height(h.p.x, h.p.z);
                (depth_below > 0.0).then_some(depth_below / rd.y.abs().max(0.2))
            })
        } else {
            self.hit_terrain(h.p - Vec3::Y * 0.02, rd, 400.0)
        };
        let scatter = w.deep_color * (self.ambient(Vec3::Y) * 2.2 + Vec3::splat(0.5) * sun.color * (sun.intensity * 0.12));
        let refr = match under {
            Some(t) => {
                let p = h.p + rd * t;
                let hf = self.hf.as_ref().unwrap();
                let nn = hf.normal(p.x, p.z);
                // Luz que chega ao fundo é atenuada pelo caminho do sol até a superfície.
                let alb = self.terrain_color(p, nn);
                let depth_sun = ((self.water_level - p.y).max(0.0)) / self.sun.y.max(0.1);
                let sun_t = (-w.absorption * depth_sun).exp();
                let ndl = nn.dot(self.sun).max(0.0);
                let lit = alb * (sun.color * sun.intensity * ndl * sun_t + self.ambient(nn) * (-w.absorption * (self.water_level - p.y).max(0.0)).exp());
                let trans = (-w.absorption * t).exp();
                lit * trans + scatter * (Vec3::ONE - trans)
            }
            None => scatter,
        };
        refr * (1.0 - fres) + refl * fres + glint_c
    }
}

/// Mapeamento de tons (ACES aproximado) + sRGB, em bytes RGB8.
pub fn tonemap_to_srgb8(linear: &[f32]) -> Vec<u8> {
    linear
        .iter()
        .map(|&x| {
            let x = x.max(0.0);
            let a = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
            let a = a.clamp(0.0, 1.0);
            let s = if a <= 0.003_130_8 { 12.92 * a } else { 1.055 * a.powf(1.0 / 2.4) - 0.055 };
            (s * 255.0 + 0.5) as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terralux_core::Scene;

    fn small_scene() -> Scene {
        let mut s = Scene::demo();
        s.terrain.as_mut().unwrap().resolution = 192;
        s
    }

    #[test]
    fn pick_finds_object_water_and_sky() {
        let s = small_scene();
        let r = Renderer::new(&s);
        let o = s.camera.position;
        let obj = s.objects[0].position;
        assert_eq!(r.pick(o, (obj - o).normalize()), Pick::Object(0));
        assert_eq!(r.pick(o, Vec3::Y), Pick::Sky);
        assert_eq!(r.pick(Vec3::new(150.0, 5.0, -150.0), Vec3::new(0.0, -1.0, 0.1).normalize()), Pick::Water);
    }

    #[test]
    fn render_is_finite_and_cancellable() {
        let s = small_scene();
        let r = Renderer::new(&s);
        let img = r.render(&RenderOptions::new(64, 36, 1));
        assert!(img.iter().all(|v| v.is_finite() && *v >= 0.0));
        assert!(img.iter().sum::<f32>() > 1.0, "imagem não pode ser preta");
        let c = Arc::new(AtomicU64::new(2));
        let mut o = RenderOptions::new(64, 36, 1);
        o.cancel = Some((c, 1)); // id diferente -> cancela antes de renderizar
        assert!(r.render(&o).iter().all(|v| *v == 0.0));
    }
}
