//! Exportação de malhas para engines e DCCs: glTF binário (.glb), OBJ (+MTL) e STL binário.
//! O terreno vira uma grade de triângulos com cores nos vértices (a paleta do bioma); os objetos
//! viram malhas com material; a água, a câmera e o sol vão no glTF.
use crate::Renderer;
use glam::{Mat3, Quat, Vec3};
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;
use terralux_core::mesh::Mesh;
use terralux_core::{Heightfield, Scene, Shape};

#[derive(Clone, Debug)]
pub struct ExportOptions {
    /// Vértices por lado da malha do terreno (2 a 2049).
    pub terrain_res: usize,
    pub include_terrain: bool,
    pub include_objects: bool,
    /// Água como plano translúcido (glTF/OBJ).
    pub include_water: bool,
    /// Câmera e sol (só glTF).
    pub include_camera: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions { terrain_res: 257, include_terrain: true, include_objects: true, include_water: false, include_camera: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExportReport {
    pub triangles: usize,
    pub vertices: usize,
    /// Itens que o formato não consegue levar (ex.: imagens recortadas).
    pub skipped: Vec<String>,
    pub bytes: u64,
}

struct Mat {
    color: Vec3,
    metallic: f32,
    roughness: f32,
    alpha: f32,
}

struct Prim {
    pos: Vec<Vec3>,
    nrm: Vec<Vec3>,
    col: Option<Vec<Vec3>>,
    uv: Option<Vec<[f32; 2]>>,
    idx: Vec<u32>,
    mat: Mat,
}

struct Item {
    name: String,
    t: Vec3,
    q: Quat,
    s: f32,
    prims: Vec<Prim>,
}

impl Item {
    fn world(&self, p: Vec3) -> Vec3 {
        self.q * (p * self.s) + self.t
    }
    fn world_n(&self, n: Vec3) -> Vec3 {
        (self.q * n).normalize_or_zero()
    }
}

fn mesh_prims(m: &Mesh, base: &terralux_core::Material) -> Vec<Prim> {
    let mut out = vec![];
    for (mi, mc) in m.mats.iter().enumerate() {
        let tris: Vec<_> = m.tris.iter().filter(|t| t.mat as usize == mi).collect();
        if tris.is_empty() {
            continue;
        }
        let mut p = Prim {
            pos: Vec::with_capacity(tris.len() * 3),
            nrm: Vec::with_capacity(tris.len() * 3),
            col: None,
            uv: None,
            idx: Vec::with_capacity(tris.len() * 3),
            mat: Mat { color: base.color * *mc, metallic: base.reflectivity.clamp(0.0, 1.0), roughness: (1.0 - base.reflectivity * 0.85).clamp(0.12, 1.0), alpha: 1.0 },
        };
        for t in tris {
            for k in 0..3 {
                p.idx.push(p.pos.len() as u32);
                p.pos.push(t.v[k]);
                p.nrm.push(t.n[k]);
            }
        }
        out.push(p);
    }
    out
}

fn uv_sphere() -> Mesh {
    let (nu, nv) = (32usize, 16usize);
    let pt = |i: usize, j: usize| {
        let (a, b) = (i as f32 / nu as f32 * std::f32::consts::TAU, j as f32 / nv as f32 * std::f32::consts::PI);
        Vec3::new(b.sin() * a.cos(), b.cos(), b.sin() * a.sin())
    };
    let mut tris = vec![];
    for j in 0..nv {
        for i in 0..nu {
            let (a, b, c, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
            // sentido anti-horário visto de fora
            if j > 0 {
                tris.push(terralux_core::mesh::Tri { v: [a, d, b], n: [a, d, b], mat: 0 });
            }
            if j + 1 < nv {
                tris.push(terralux_core::mesh::Tri { v: [b, d, c], n: [b, d, c], mat: 0 });
            }
        }
    }
    Mesh { tris, mats: vec![Vec3::ONE] }
}

fn poly_mesh(p: terralux_core::Polyhedron) -> Mesh {
    let (v, f) = p.mesh();
    let tris = f
        .iter()
        .map(|&[a, b, c]| {
            let (a, b, c) = (v[a].normalize(), v[b].normalize(), v[c].normalize());
            let n = (b - a).cross(c - a).normalize_or_zero();
            terralux_core::mesh::Tri { v: [a, b, c], n: [n, n, n], mat: 0 }
        })
        .collect();
    Mesh { tris, mats: vec![Vec3::ONE] }
}

fn quad_prim(y: f32, half: f32, mat: Mat) -> Prim {
    let pos = vec![Vec3::new(-half, y, -half), Vec3::new(-half, y, half), Vec3::new(half, y, half), Vec3::new(half, y, -half)];
    Prim { pos, nrm: vec![Vec3::Y; 4], col: None, uv: None, idx: vec![0, 1, 2, 0, 2, 3], mat }
}

fn build(scene: &Scene, o: &ExportOptions) -> (Vec<Item>, ExportReport) {
    let mut items = vec![];
    let mut rep = ExportReport::default();
    let hf: Option<Arc<Heightfield>> = scene.terrain.as_ref().map(|d| Arc::new(Heightfield::from_def(d)));
    let half = hf.as_ref().map_or(500.0, |h| h.half);
    if let (true, Some(hf)) = (o.include_terrain, &hf) {
        let m = o.terrain_res.clamp(2, 2049);
        let r = Renderer::with_heightfield(scene, Some(hf.clone()));
        let mut p = Prim { pos: vec![], nrm: vec![], col: Some(vec![]), uv: Some(vec![]), idx: vec![], mat: Mat { color: Vec3::ONE, metallic: 0.0, roughness: 0.95, alpha: 1.0 } };
        for iz in 0..m {
            let z = -hf.half + 2.0 * hf.half * iz as f32 / (m - 1) as f32;
            for ix in 0..m {
                let x = -hf.half + 2.0 * hf.half * ix as f32 / (m - 1) as f32;
                let y = hf.height(x, z);
                let n = hf.normal(x, z);
                p.pos.push(Vec3::new(x, y, z));
                p.nrm.push(n);
                p.col.as_mut().unwrap().push(r.terrain_color_at(Vec3::new(x, y, z), n).clamp(Vec3::ZERO, Vec3::ONE));
                p.uv.as_mut().unwrap().push([ix as f32 / (m - 1) as f32, iz as f32 / (m - 1) as f32]);
            }
        }
        for iz in 0..m - 1 {
            for ix in 0..m - 1 {
                let i00 = (iz * m + ix) as u32;
                let (i10, i01, i11) = (i00 + 1, i00 + m as u32, i00 + m as u32 + 1);
                p.idx.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
            }
        }
        items.push(Item { name: "terreno".into(), t: Vec3::ZERO, q: Quat::IDENTITY, s: 1.0, prims: vec![p] });
    }
    if o.include_water {
        if let Some(w) = &scene.water {
            let mat = Mat { color: Vec3::new(0.10, 0.35, 0.50), metallic: 0.0, roughness: 0.05, alpha: 0.72 };
            items.push(Item { name: "agua".into(), t: Vec3::ZERO, q: Quat::IDENTITY, s: 1.0, prims: vec![quad_prim(w.level, half, mat)] });
        }
    }
    if o.include_objects {
        for ob in &scene.objects {
            let q = Quat::from_euler(glam::EulerRot::XYZ, ob.rotation_deg.x.to_radians(), ob.rotation_deg.y.to_radians(), ob.rotation_deg.z.to_radians());
            let (mesh, s): (Option<Mesh>, f32) = match &ob.shape {
                Shape::Sphere => (Some(uv_sphere()), ob.size),
                Shape::Poly(p) => (Some(poly_mesh(*p)), ob.size),
                Shape::Prim(p) => (Some(p.mesh()), ob.size),
                Shape::Model(path) => match terralux_core::assets::load_mesh(path) {
                    Ok(m) => (Some(m), ob.size),
                    Err(e) => {
                        rep.skipped.push(format!("{}: {e}", ob.name));
                        (None, 1.0)
                    }
                },
                Shape::Plane => {
                    let m = Mat { color: ob.material.color, metallic: ob.material.reflectivity, roughness: 0.8, alpha: 1.0 };
                    items.push(Item { name: ob.name.clone(), t: Vec3::ZERO, q: Quat::IDENTITY, s: 1.0, prims: vec![quad_prim(ob.position.y, 1000.0, m)] });
                    (None, 1.0)
                }
                Shape::Picture(_) => {
                    rep.skipped.push(format!("{}: imagem recortada não é exportada", ob.name));
                    (None, 1.0)
                }
            };
            if let Some(m) = mesh {
                items.push(Item { name: ob.name.clone(), t: ob.position, q, s, prims: mesh_prims(&m, &ob.material) });
            }
        }
    }
    for it in &items {
        for p in &it.prims {
            rep.triangles += p.idx.len() / 3;
            rep.vertices += p.pos.len();
        }
    }
    (items, rep)
}

// ---------- glTF binário ----------

struct Bin {
    data: Vec<u8>,
    views: Vec<Value>,
    accs: Vec<Value>,
}

impl Bin {
    fn view(&mut self, bytes: &[u8], target: u32) -> usize {
        while self.data.len() % 4 != 0 {
            self.data.push(0);
        }
        self.views.push(json!({"buffer": 0, "byteOffset": self.data.len(), "byteLength": bytes.len(), "target": target}));
        self.data.extend_from_slice(bytes);
        self.views.len() - 1
    }

    fn vec3(&mut self, v: &[Vec3], minmax: bool) -> usize {
        let bytes: Vec<u8> = v.iter().flat_map(|p| [p.x, p.y, p.z]).flat_map(|f| f.to_le_bytes()).collect();
        let view = self.view(&bytes, 34962);
        let mut a = json!({"bufferView": view, "componentType": 5126, "count": v.len(), "type": "VEC3"});
        if minmax {
            let (lo, hi) = v.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(l, h), p| (l.min(*p), h.max(*p)));
            a["min"] = json!([lo.x, lo.y, lo.z]);
            a["max"] = json!([hi.x, hi.y, hi.z]);
        }
        self.accs.push(a);
        self.accs.len() - 1
    }

    fn vec2(&mut self, v: &[[f32; 2]]) -> usize {
        let bytes: Vec<u8> = v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect();
        let view = self.view(&bytes, 34962);
        self.accs.push(json!({"bufferView": view, "componentType": 5126, "count": v.len(), "type": "VEC2"}));
        self.accs.len() - 1
    }

    fn indices(&mut self, v: &[u32]) -> usize {
        let bytes: Vec<u8> = v.iter().flat_map(|i| i.to_le_bytes()).collect();
        let view = self.view(&bytes, 34963);
        self.accs.push(json!({"bufferView": view, "componentType": 5125, "count": v.len(), "type": "SCALAR"}));
        self.accs.len() - 1
    }
}

/// Rotação que leva o eixo local −Z até `forward`, com +Y local o mais próximo possível de `up`.
fn look_rotation(forward: Vec3, up: Vec3) -> Quat {
    let f = forward.normalize();
    let up = if f.cross(up).length() < 1e-4 { Vec3::X } else { up };
    let r = f.cross(up).normalize();
    let u = r.cross(f);
    Quat::from_mat3(&Mat3::from_cols(r, u, -f)).normalize()
}

fn write_glb(items: &[Item], scene: &Scene, o: &ExportOptions) -> Vec<u8> {
    let mut bin = Bin { data: vec![], views: vec![], accs: vec![] };
    let mut materials: Vec<Value> = vec![];
    let mut meshes: Vec<Value> = vec![];
    let mut nodes: Vec<Value> = vec![];
    for it in items {
        let mut prims = vec![];
        for p in &it.prims {
            let mut attrs = serde_json::Map::new();
            attrs.insert("POSITION".into(), json!(bin.vec3(&p.pos, true)));
            attrs.insert("NORMAL".into(), json!(bin.vec3(&p.nrm, false)));
            if let Some(c) = &p.col {
                attrs.insert("COLOR_0".into(), json!(bin.vec3(c, false)));
            }
            if let Some(uv) = &p.uv {
                attrs.insert("TEXCOORD_0".into(), json!(bin.vec2(uv)));
            }
            let indices = bin.indices(&p.idx);
            let mut m = json!({
                "pbrMetallicRoughness": {
                    "baseColorFactor": [p.mat.color.x, p.mat.color.y, p.mat.color.z, p.mat.alpha],
                    "metallicFactor": p.mat.metallic,
                    "roughnessFactor": p.mat.roughness
                },
                "doubleSided": p.mat.alpha < 1.0
            });
            if p.mat.alpha < 1.0 {
                m["alphaMode"] = json!("BLEND");
            }
            materials.push(m);
            prims.push(json!({"attributes": attrs, "indices": indices, "material": materials.len() - 1, "mode": 4}));
        }
        meshes.push(json!({"name": it.name, "primitives": prims}));
        let mut node = json!({"name": it.name, "mesh": meshes.len() - 1});
        if it.t != Vec3::ZERO {
            node["translation"] = json!([it.t.x, it.t.y, it.t.z]);
        }
        if it.q != Quat::IDENTITY {
            node["rotation"] = json!([it.q.x, it.q.y, it.q.z, it.q.w]);
        }
        if (it.s - 1.0).abs() > 1e-6 {
            node["scale"] = json!([it.s, it.s, it.s]);
        }
        nodes.push(node);
    }
    let mut root = json!({
        "asset": {"version": "2.0", "generator": "Terralux Studio"},
        "scene": 0,
        "buffers": [{"byteLength": bin.data.len()}],
        "bufferViews": bin.views,
        "accessors": bin.accs,
        "materials": materials,
        "meshes": meshes,
    });
    let mut cameras: Vec<Value> = vec![];
    let mut lights: Vec<Value> = vec![];
    if o.include_camera {
        let c = &scene.camera;
        cameras.push(json!({"name": "camera", "type": "perspective", "perspective": {"yfov": c.fov_deg.to_radians(), "aspectRatio": 16.0 / 9.0, "znear": 0.1, "zfar": 20000.0}}));
        let q = look_rotation(c.target - c.position, Vec3::Y);
        nodes.push(json!({"name": "camera", "camera": 0, "translation": [c.position.x, c.position.y, c.position.z], "rotation": [q.x, q.y, q.z, q.w]}));
        let s = &scene.sun;
        lights.push(json!({"name": "sol", "type": "directional", "color": [s.color.x, s.color.y, s.color.z], "intensity": s.intensity}));
        let q = look_rotation(-s.direction(), Vec3::Y);
        nodes.push(json!({"name": "sol", "rotation": [q.x, q.y, q.z, q.w], "extensions": {"KHR_lights_punctual": {"light": 0}}}));
        root["cameras"] = json!(cameras);
        root["extensionsUsed"] = json!(["KHR_lights_punctual"]);
        root["extensions"] = json!({"KHR_lights_punctual": {"lights": lights}});
    }
    root["scenes"] = json!([{"name": "Terralux", "nodes": (0..nodes.len()).collect::<Vec<_>>()}]);
    root["nodes"] = json!(nodes);

    let mut js = serde_json::to_vec(&root).expect("json");
    while js.len() % 4 != 0 {
        js.push(b' ');
    }
    let mut data = bin.data;
    while data.len() % 4 != 0 {
        data.push(0);
    }
    let total = 12 + 8 + js.len() + 8 + data.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&data);
    out
}

// ---------- OBJ + MTL ----------

fn write_obj(items: &[Item], mtl_name: &str) -> (String, String) {
    let mut obj = String::new();
    let mut mtl = String::new();
    let _ = writeln!(obj, "# Terralux Studio\nmtllib {mtl_name}");
    let (mut vo, mut no, mut to) = (1usize, 1usize, 1usize);
    let mut mi = 0;
    for it in items {
        let _ = writeln!(obj, "o {}", it.name.replace(' ', "_"));
        for p in &it.prims {
            for (i, v) in p.pos.iter().enumerate() {
                let w = it.world(*v);
                match &p.col {
                    Some(c) => {
                        let _ = writeln!(obj, "v {:.4} {:.4} {:.4} {:.4} {:.4} {:.4}", w.x, w.y, w.z, c[i].x, c[i].y, c[i].z);
                    }
                    None => {
                        let _ = writeln!(obj, "v {:.4} {:.4} {:.4}", w.x, w.y, w.z);
                    }
                }
            }
            for n in &p.nrm {
                let n = it.world_n(*n);
                let _ = writeln!(obj, "vn {:.4} {:.4} {:.4}", n.x, n.y, n.z);
            }
            if let Some(uv) = &p.uv {
                for t in uv {
                    let _ = writeln!(obj, "vt {:.5} {:.5}", t[0], t[1]);
                }
            }
            let _ = writeln!(obj, "usemtl m{mi}");
            let _ = writeln!(mtl, "newmtl m{mi}\nKd {:.4} {:.4} {:.4}\nKa 0 0 0\nd {:.2}\nNs {:.1}\n", p.mat.color.x, p.mat.color.y, p.mat.color.z, p.mat.alpha, (1.0 - p.mat.roughness) * 200.0);
            mi += 1;
            for t in p.idx.chunks_exact(3) {
                let f = |k: u32| {
                    let v = vo + k as usize;
                    let n = no + k as usize;
                    if p.uv.is_some() { format!("{v}/{}/{n}", to + k as usize) } else { format!("{v}//{n}") }
                };
                let _ = writeln!(obj, "f {} {} {}", f(t[0]), f(t[1]), f(t[2]));
            }
            vo += p.pos.len();
            no += p.nrm.len();
            if let Some(uv) = &p.uv {
                to += uv.len();
            }
        }
    }
    (obj, mtl)
}

// ---------- STL binário ----------

fn write_stl(items: &[Item]) -> Vec<u8> {
    let n: usize = items.iter().flat_map(|i| i.prims.iter()).map(|p| p.idx.len() / 3).sum();
    let mut out = vec![0u8; 80];
    out[..16].copy_from_slice(b"Terralux Studio ");
    out.extend_from_slice(&(n as u32).to_le_bytes());
    for it in items {
        for p in &it.prims {
            for t in p.idx.chunks_exact(3) {
                let v: Vec<Vec3> = t.iter().map(|&i| it.world(p.pos[i as usize])).collect();
                let nn = (v[1] - v[0]).cross(v[2] - v[0]).normalize_or_zero();
                for c in [nn, v[0], v[1], v[2]] {
                    for f in [c.x, c.y, c.z] {
                        out.extend_from_slice(&f.to_le_bytes());
                    }
                }
                out.extend_from_slice(&[0, 0]);
            }
        }
    }
    out
}

/// Exporta uma malha avulsa (conversão entre formatos), no tamanho original.
pub fn export_mesh(mesh: &Mesh, path: &Path) -> Result<ExportReport, String> {
    let item = Item { name: "malha".into(), t: Vec3::ZERO, q: Quat::IDENTITY, s: 1.0, prims: mesh_prims(mesh, &terralux_core::Material::matte(Vec3::splat(0.8))) };
    write_items(vec![item], &Scene::empty(), path, &ExportOptions { include_camera: false, ..Default::default() })
}

/// Exporta a cena (ou só o terreno, conforme as opções). O formato vem da extensão:
/// `.glb` (glTF binário), `.obj` (com `.mtl` ao lado) ou `.stl` (binário).
pub fn export_scene(scene: &Scene, path: &Path, o: &ExportOptions) -> Result<ExportReport, String> {
    let (items, rep) = build(scene, o);
    let mut r = write_items(items, scene, path, o)?;
    r.skipped = rep.skipped;
    Ok(r)
}

fn write_items(items: Vec<Item>, scene: &Scene, path: &Path, o: &ExportOptions) -> Result<ExportReport, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let mut rep = ExportReport::default();
    for it in &items {
        for p in &it.prims {
            rep.triangles += p.idx.len() / 3;
            rep.vertices += p.pos.len();
        }
    }
    if items.is_empty() {
        return Err("nada para exportar (a cena não tem terreno nem objetos)".into());
    }
    let werr = |e: std::io::Error| format!("{}: {e}", path.display());
    match ext.as_str() {
        "glb" => {
            let bytes = write_glb(&items, scene, o);
            rep.bytes = bytes.len() as u64;
            std::fs::write(path, bytes).map_err(werr)?;
        }
        "obj" => {
            let mtl_path = path.with_extension("mtl");
            let mtl_name = mtl_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let (obj, mtl) = write_obj(&items, &mtl_name);
            rep.bytes = obj.len() as u64;
            std::fs::write(path, obj).map_err(werr)?;
            std::fs::write(&mtl_path, mtl).map_err(werr)?;
        }
        "stl" => {
            let bytes = write_stl(&items);
            rep.bytes = bytes.len() as u64;
            std::fs::write(path, bytes).map_err(werr)?;
        }
        e => return Err(format!("formato de exportação não suportado: .{e} (use .glb, .obj ou .stl)")),
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terralux_core::{Material, Object};

    fn scene() -> Scene {
        let mut s = Scene::template("jogo-colinas").unwrap();
        s.terrain.as_mut().unwrap().resolution = 65;
        s.objects.push(Object {
            name: "bola".into(),
            shape: Shape::Sphere,
            position: Vec3::new(5.0, 30.0, 5.0),
            rotation_deg: Vec3::new(10.0, 20.0, 30.0),
            size: 3.0,
            material: Material::matte(Vec3::new(0.9, 0.2, 0.2)),
        });
        s.objects.push(Object {
            name: "cubo".into(),
            shape: Shape::Prim(terralux_core::mesh::Primitive::Cube),
            position: Vec3::new(-5.0, 30.0, 5.0),
            rotation_deg: Vec3::ZERO,
            size: 2.0,
            material: Material::metal(Vec3::new(0.8, 0.8, 0.2)),
        });
        s
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join("terralux-test-export");
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn glb_reimports_with_the_same_triangle_count() {
        let s = scene();
        let o = ExportOptions { terrain_res: 17, ..Default::default() };
        let p = tmp("a.glb");
        let rep = export_scene(&s, &p, &o).unwrap();
        assert_eq!(rep.triangles, 2 * 16 * 16 + uv_sphere().tris.len() + 12);
        let m = Mesh::from_gltf(&p).unwrap();
        assert_eq!(m.tris.len(), rep.triangles);
        // o terreno cobre a caixa inteira (meia largura 256 m)
        let (lo, hi) = m.bounds();
        assert!(lo.x < -250.0 && hi.x > 250.0);
    }

    #[test]
    fn terrain_triangles_face_up() {
        let s = scene();
        let (items, _) = build(&s, &ExportOptions { terrain_res: 9, include_objects: false, ..Default::default() });
        let p = &items[0].prims[0];
        for t in p.idx.chunks_exact(3) {
            let n = (p.pos[t[1] as usize] - p.pos[t[0] as usize]).cross(p.pos[t[2] as usize] - p.pos[t[0] as usize]);
            assert!(n.y > 0.0, "triângulo virado para baixo");
        }
    }

    #[test]
    fn obj_and_stl_counts_match() {
        let s = scene();
        let o = ExportOptions { terrain_res: 17, include_camera: false, ..Default::default() };
        let rep = export_scene(&s, &tmp("a.stl"), &o).unwrap();
        let bytes = std::fs::read(tmp("a.stl")).unwrap();
        assert_eq!(bytes.len(), 84 + 50 * rep.triangles);
        assert_eq!(Mesh::from_stl(&bytes).unwrap().tris.len(), rep.triangles);

        export_scene(&s, &tmp("a.obj"), &o).unwrap();
        let text = std::fs::read_to_string(tmp("a.obj")).unwrap();
        let m = Mesh::from_obj(&text, &|_| None);
        assert_eq!(m.tris.len(), rep.triangles);
        assert!(std::fs::metadata(tmp("a.mtl")).is_ok());
    }

    #[test]
    fn empty_scene_and_bad_extension_are_errors() {
        assert!(export_scene(&Scene::empty(), &tmp("x.glb"), &ExportOptions::default()).is_err());
        assert!(export_scene(&scene(), &tmp("x.fbx"), &ExportOptions { terrain_res: 9, ..Default::default() }).is_err());
    }
}
