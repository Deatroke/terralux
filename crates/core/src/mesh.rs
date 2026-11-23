//! Malhas triangulares: leitores de OBJ+MTL e DXF (3DFACE) e geradores das primitivas.
use glam::Vec3;
use std::collections::HashMap;
use std::f32::consts::TAU;

/// Triângulo com normais por canto (suavização já resolvida na carga).
#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub v: [Vec3; 3],
    pub n: [Vec3; 3],
    pub mat: u16,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub tris: Vec<Tri>,
    /// Cor base de cada material (Kd).
    pub mats: Vec<Vec3>,
}

const DEFAULT_GRAY: Vec3 = Vec3::splat(0.8);
const CREASE_COS: f32 = 0.64; // ~50 graus: arestas mais vivas ficam duras

impl Mesh {
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for t in &self.tris {
            for p in t.v {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        (lo, hi)
    }

    /// Centraliza a caixa na origem e escala para que a maior meia-extensão valha 1
    /// (assim `size` significa o mesmo para todo objeto).
    pub fn normalize(&mut self) {
        if self.tris.is_empty() {
            return;
        }
        let (lo, hi) = self.bounds();
        let c = (lo + hi) * 0.5;
        let h = ((hi - lo) * 0.5).max_element().max(1e-6);
        for t in &mut self.tris {
            for p in &mut t.v {
                *p = (*p - c) / h;
            }
        }
    }

    /// Normais suaves por canto, respeitando um ângulo de quina.
    pub fn smooth_from_faces(tris: Vec<([Vec3; 3], u16)>, mats: Vec<Vec3>) -> Mesh {
        let fnorm: Vec<Vec3> = tris.iter().map(|(v, _)| (v[1] - v[0]).cross(v[2] - v[0])).collect();
        let key = |p: Vec3| ((p.x * 1e4).round() as i64, (p.y * 1e4).round() as i64, (p.z * 1e4).round() as i64);
        let mut by_pos: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, (v, _)) in tris.iter().enumerate() {
            for p in v {
                by_pos.entry(key(*p)).or_default().push(i);
            }
        }
        let out = tris
            .iter()
            .enumerate()
            .map(|(i, (v, m))| {
                let fl = fnorm[i].length();
                let fnn = if fl > 1e-12 { fnorm[i] / fl } else { Vec3::Y };
                let mut n = [fnn; 3];
                for k in 0..3 {
                    let mut acc = fnorm[i];
                    for &j in &by_pos[&key(v[k])] {
                        if j != i {
                            let l = fnorm[j].length();
                            if l > 1e-12 && (fnorm[j] / l).dot(fnn) > CREASE_COS {
                                acc += fnorm[j];
                            }
                        }
                    }
                    n[k] = acc.try_normalize().unwrap_or(fnn);
                }
                Tri { v: *v, n, mat: *m }
            })
            .collect();
        Mesh { tris: out, mats }
    }

    // ---------- OBJ ----------

    /// Lê OBJ; `mtl_lookup` devolve o texto do arquivo MTL citado (ou None).
    pub fn from_obj(text: &str, mtl_lookup: &dyn Fn(&str) -> Option<String>) -> Mesh {
        let mut vs: Vec<Vec3> = vec![];
        let mut ns: Vec<Vec3> = vec![];
        let mut mat_names: HashMap<String, u16> = HashMap::new();
        let mut mats: Vec<Vec3> = vec![DEFAULT_GRAY];
        let mut kd: HashMap<String, Vec3> = HashMap::new();
        let mut cur: u16 = 0;
        let mut faces: Vec<([Vec3; 3], [Option<Vec3>; 3], u16)> = vec![];
        let mut have_normals = true;
        for line in text.lines() {
            let line = line.trim();
            let mut it = line.split_whitespace();
            let Some(tag) = it.next() else { continue };
            let f3 = |it: &mut std::str::SplitWhitespace| -> Option<Vec3> {
                let x = it.next()?.parse().ok()?;
                let y = it.next()?.parse().ok()?;
                let z = it.next()?.parse().ok()?;
                Some(Vec3::new(x, y, z))
            };
            match tag {
                "v" => vs.extend(f3(&mut it)),
                "vn" => ns.extend(f3(&mut it)),
                "mtllib" => {
                    let name = line[6..].trim();
                    if let Some(t) = mtl_lookup(name) {
                        kd.extend(parse_mtl(&t));
                    }
                }
                "usemtl" => {
                    let name = line[6..].trim().to_string();
                    cur = *mat_names.entry(name.clone()).or_insert_with(|| {
                        mats.push(kd.get(&name).copied().unwrap_or(DEFAULT_GRAY));
                        (mats.len() - 1) as u16
                    });
                }
                "f" => {
                    let idx = |s: &str, len: usize| -> Option<usize> {
                        let i: i64 = s.parse().ok()?;
                        let r = if i < 0 { len as i64 + i } else { i - 1 };
                        (r >= 0 && (r as usize) < len).then_some(r as usize)
                    };
                    let mut corners: Vec<(Vec3, Option<Vec3>)> = vec![];
                    for tok in it {
                        let mut p = tok.split('/');
                        let Some(vi) = p.next().and_then(|s| idx(s, vs.len())) else { continue };
                        let _ = p.next();
                        let ni = p.next().filter(|s| !s.is_empty()).and_then(|s| idx(s, ns.len()));
                        corners.push((vs[vi], ni.map(|i| ns[i])));
                    }
                    for k in 1..corners.len().saturating_sub(1) {
                        let (a, b, c) = (corners[0], corners[k], corners[k + 1]);
                        if a.1.is_none() || b.1.is_none() || c.1.is_none() {
                            have_normals = false;
                        }
                        faces.push(([a.0, b.0, c.0], [a.1, b.1, c.1], cur));
                    }
                }
                _ => {}
            }
        }
        if have_normals && !faces.is_empty() {
            let tris = faces
                .iter()
                .map(|(v, n, m)| Tri { v: *v, n: [n[0].unwrap().normalize_or_zero(), n[1].unwrap().normalize_or_zero(), n[2].unwrap().normalize_or_zero()], mat: *m })
                .collect();
            Mesh { tris, mats }
        } else {
            Mesh::smooth_from_faces(faces.into_iter().map(|(v, _, m)| (v, m)).collect(), mats)
        }
    }

    // ---------- DXF ----------

    /// Lê as entidades 3DFACE de um DXF (a cor vem da camada).
    pub fn from_dxf(text: &str) -> Mesh {
        // Alguns DXF antigos (ONYX) terminam linhas só com \r.
        let mut lines = text.split(|c| c == '\n' || c == '\r').filter(|l| !l.trim().is_empty()).map(str::trim);
        let mut faces: Vec<([Vec3; 3], u16)> = vec![];
        let mut layers: HashMap<String, u16> = HashMap::new();
        let mut in_face = false;
        let mut pts = [Vec3::ZERO; 4];
        let mut layer = 0u16;
        let flush = |in_face: bool, pts: &[Vec3; 4], layer: u16, faces: &mut Vec<([Vec3; 3], u16)>| {
            if in_face {
                faces.push(([pts[0], pts[1], pts[2]], layer));
                if pts[3] != pts[2] {
                    faces.push(([pts[0], pts[2], pts[3]], layer));
                }
            }
        };
        while let (Some(code), Some(val)) = (lines.next(), lines.next()) {
            let Ok(code) = code.parse::<i32>() else { continue };
            if code == 0 {
                flush(in_face, &pts, layer, &mut faces);
                in_face = val == "3DFACE";
                pts = [Vec3::ZERO; 4];
                layer = 0;
            } else if in_face {
                match code {
                    8 => {
                        let n = layers.len() as u16;
                        layer = *layers.entry(val.to_string()).or_insert(n);
                    }
                    10..=13 | 20..=23 | 30..=33 => {
                        if let Ok(x) = val.parse::<f32>() {
                            let (k, axis) = ((code % 10) as usize, (code / 10 - 1) as usize);
                            pts[k][axis] = x;
                        }
                    }
                    _ => {}
                }
            }
        }
        flush(in_face, &pts, layer, &mut faces);
        let palette = [Vec3::new(0.80, 0.80, 0.82), Vec3::new(0.80, 0.35, 0.30), Vec3::new(0.35, 0.60, 0.80), Vec3::new(0.85, 0.75, 0.35), Vec3::new(0.40, 0.70, 0.45), Vec3::new(0.70, 0.50, 0.80)];
        let mats = (0..layers.len().max(1)).map(|i| palette[i % palette.len()]).collect();
        // DXF costuma ter Z para cima; o Terralux usa Y para cima.
        let faces = faces.into_iter().map(|(v, m)| (v.map(|p| Vec3::new(p.x, p.z, -p.y)), m)).collect();
        Mesh::smooth_from_faces(faces, mats)
    }
}

fn parse_mtl(text: &str) -> HashMap<String, Vec3> {
    let mut out = HashMap::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(n) = line.strip_prefix("newmtl") {
            cur = Some(n.trim().to_string());
        } else if let Some(k) = line.strip_prefix("Kd") {
            let v: Vec<f32> = k.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if let (Some(n), 3..) = (&cur, v.len()) {
                out.insert(n.clone(), Vec3::new(v[0], v[1], v[2]));
            }
        }
    }
    out
}

// ---------- primitivas ----------

/// Primitivas geradas como malha (todas cabem na esfera unitária).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Primitive {
    Cube,
    Pyramid,
    Cylinder,
    Cone,
    Torus,
    Disc,
    /// Caveira (osso + órbitas escuras), feita de elipsoides.
    Skull,
    Bone,
    CrossedBones,
}

impl Primitive {
    pub const ALL: [Primitive; 9] = [
        Primitive::Cube, Primitive::Pyramid, Primitive::Cylinder, Primitive::Cone, Primitive::Torus, Primitive::Disc,
        Primitive::Skull, Primitive::Bone, Primitive::CrossedBones,
    ];
    pub const BASIC: [Primitive; 6] = [Primitive::Cube, Primitive::Pyramid, Primitive::Cylinder, Primitive::Cone, Primitive::Torus, Primitive::Disc];
    pub const FUN: [Primitive; 3] = [Primitive::Skull, Primitive::Bone, Primitive::CrossedBones];

    pub fn name(self) -> &'static str {
        match self {
            Primitive::Cube => "Cubo",
            Primitive::Pyramid => "Pirâmide",
            Primitive::Cylinder => "Cilindro",
            Primitive::Cone => "Cone",
            Primitive::Torus => "Toro",
            Primitive::Disc => "Disco",
            Primitive::Skull => "Caveira",
            Primitive::Bone => "Osso",
            Primitive::CrossedBones => "Ossos cruzados",
        }
    }

    pub fn mesh(self) -> Mesh {
        if let Some(m) = self.fun_mesh() {
            return m;
        }
        let mut q: Vec<([Vec3; 3], u16)> = vec![];
        let quad = |q: &mut Vec<([Vec3; 3], u16)>, a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
            q.push(([a, b, c], 0));
            q.push(([a, c, d], 0));
        };
        match self {
            Primitive::Cube => {
                let s = 0.577_35;
                let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * s;
                quad(&mut q, p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.));
                quad(&mut q, p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.));
                quad(&mut q, p(1., -1., 1.), p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.));
                quad(&mut q, p(-1., -1., -1.), p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.));
                quad(&mut q, p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.), p(-1., 1., -1.));
                quad(&mut q, p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.), p(-1., -1., 1.));
            }
            Primitive::Pyramid => {
                let top = Vec3::new(0.0, 1.0, 0.0);
                let b = |x: f32, z: f32| Vec3::new(x, -0.6, z) * Vec3::new(0.9, 1.0, 0.9);
                let (a, bb, c, d) = (b(-1., 1.), b(1., 1.), b(1., -1.), b(-1., -1.));
                for (u, v) in [(a, bb), (bb, c), (c, d), (d, a)] {
                    q.push(([u, v, top], 0));
                }
                quad(&mut q, d, c, bb, a);
            }
            Primitive::Cylinder | Primitive::Cone | Primitive::Disc => {
                let (hh, r0, r1) = match self {
                    Primitive::Cylinder => (0.8, 0.6, 0.6),
                    Primitive::Cone => (0.8, 0.7, 0.0),
                    _ => (0.06, 1.0, 1.0),
                };
                let seg = 48;
                let ring = |i: usize, r: f32, y: f32| {
                    let a = i as f32 / seg as f32 * TAU;
                    Vec3::new(r * a.cos(), y, r * a.sin())
                };
                for i in 0..seg {
                    let (a0, a1) = (ring(i, r0, -hh), ring(i + 1, r0, -hh));
                    let (b0, b1) = (ring(i, r1, hh), ring(i + 1, r1, hh));
                    q.push(([a1, a0, b0], 0));
                    if r1 > 0.0 {
                        q.push(([a1, b0, b1], 0));
                    }
                    q.push(([Vec3::new(0.0, -hh, 0.0), a0, a1], 0));
                    if r1 > 0.0 {
                        q.push(([Vec3::new(0.0, hh, 0.0), b1, b0], 0));
                    }
                }
            }
            Primitive::Skull | Primitive::Bone | Primitive::CrossedBones => unreachable!("tratadas em fun_mesh"),
            Primitive::Torus => {
                let (big, small) = (0.72, 0.28);
                let (u_n, v_n) = (48, 20);
                let pt = |i: usize, j: usize| {
                    let (u, v) = (i as f32 / u_n as f32 * TAU, j as f32 / v_n as f32 * TAU);
                    let r = big + small * v.cos();
                    Vec3::new(r * u.cos(), small * v.sin(), r * u.sin())
                };
                for i in 0..u_n {
                    for j in 0..v_n {
                        quad(&mut q, pt(i, j), pt(i, j + 1), pt(i + 1, j + 1), pt(i + 1, j));
                    }
                }
            }
        }
        Mesh::smooth_from_faces(q, vec![DEFAULT_GRAY])
    }
}

/// Campo de distância da caveira (rosto para -Z, Y para cima).
fn skull_sdf(p: Vec3) -> f32 {
    use crate::sdf::*;
    // mantém a simetria: tudo é calculado em p com x espelhado
    let q = Vec3::new(p.x.abs(), p.y, p.z);
    let cranium = sd_ellipsoid(p, Vec3::new(0.0, 0.30, 0.10), Vec3::new(0.54, 0.50, 0.60));
    let temple = sd_ellipsoid(q, Vec3::new(0.30, 0.05, -0.12), Vec3::new(0.25, 0.28, 0.32));
    let face = sd_ellipsoid(p, Vec3::new(0.0, -0.20, -0.30), Vec3::new(0.34, 0.31, 0.27));
    let cheek = sd_ellipsoid(q, Vec3::new(0.34, -0.09, -0.27), Vec3::new(0.13, 0.085, 0.17));
    let brow = sd_ellipsoid(q, Vec3::new(0.19, 0.115, -0.50), Vec3::new(0.19, 0.05, 0.12));
    let mut d = smin(cranium, face, 0.12);
    d = smin(d, temple, 0.10);
    d = smin(d, cheek, 0.06);
    d = smin(d, brow, 0.05);
    // mandíbula: ramos laterais + corpo em U
    let ramus = sd_capsule(q, Vec3::new(0.32, -0.52, -0.02), Vec3::new(0.34, -0.14, 0.04), 0.055);
    let body = sd_capsule(q, Vec3::new(0.30, -0.52, -0.05), Vec3::new(0.12, -0.54, -0.33), 0.06);
    let chin = sd_capsule(q, Vec3::new(0.12, -0.54, -0.33), Vec3::new(0.0, -0.54, -0.35), 0.06);
    let jaw = smin(smin(ramus, body, 0.05), chin, 0.04);
    d = smin(d, jaw, 0.015);
    // cavidades: órbitas, nariz, abertura da boca
    let orbit = sd_ellipsoid(q, Vec3::new(0.175, 0.0, -0.46), Vec3::new(0.125, 0.115, 0.25));
    let nose = smin(sd_ellipsoid(q, Vec3::new(0.035, -0.20, -0.52), Vec3::new(0.05, 0.10, 0.2)), sd_ellipsoid(q, Vec3::new(0.0, -0.27, -0.55), Vec3::new(0.05, 0.05, 0.2)), 0.03);
    let mouth = sd_round_box(p, Vec3::new(0.0, -0.405, -0.30), Vec3::new(0.30, 0.03, 0.22), 0.01);
    d = ssub(d, orbit, 0.03);
    d = ssub(d, nose, 0.02);
    d = ssub(d, mouth, 0.01);
    d.min(skull_teeth(p))
}

/// Dentes em arco: fileira superior e inferior.
fn skull_teeth(p: Vec3) -> f32 {
    use crate::sdf::*;
    let mut teeth = f32::MAX;
    for k in 0..8 {
        let t = (k as f32 - 3.5) / 3.5; // -1..1
        let x = t * 0.215;
        let z = -0.535 + 0.10 * t * t;
        let w = 0.0255 - 0.004 * t.abs();
        let up = sd_round_box(p, Vec3::new(x, -0.368, z), Vec3::new(w, 0.066, 0.026), 0.012);
        let lo = sd_round_box(p, Vec3::new(x, -0.442, z + 0.008), Vec3::new(w * 0.93, 0.06, 0.024), 0.012);
        teeth = teeth.min(up).min(lo);
    }
    teeth
}

/// Negativo dentro das regiões que devem ser escuras (órbitas, nariz, boca).
fn skull_dark(p: Vec3) -> f32 {
    use crate::sdf::*;
    let q = Vec3::new(p.x.abs(), p.y, p.z);
    let orbit = sd_ellipsoid(q, Vec3::new(0.175, 0.0, -0.46), Vec3::new(0.125, 0.115, 0.25));
    let nose = sd_ellipsoid(q, Vec3::new(0.03, -0.22, -0.52), Vec3::new(0.06, 0.13, 0.2));
    let mouth = sd_round_box(p, Vec3::new(0.0, -0.405, -0.30), Vec3::new(0.30, 0.03, 0.22), 0.01);
    orbit.min(nose).min(mouth)
}

fn bone_sdf(p: Vec3) -> f32 {
    use crate::sdf::*;
    let q = Vec3::new(p.x.abs(), p.y, p.z.abs());
    let shaft = sd_capsule(Vec3::new(q.x, p.y, p.z), Vec3::new(-0.45, 0.0, 0.0), Vec3::new(0.45, 0.0, 0.0), 0.085);
    // nós nas pontas: duas esferas por ponta, com um sulco no meio
    let knob = sd_ellipsoid(q, Vec3::new(0.66, 0.0, 0.1), Vec3::new(0.15, 0.15, 0.14));
    let d = smin(shaft, knob, 0.07);
    let notch = sd_ellipsoid(Vec3::new(q.x, p.y, p.z), Vec3::new(0.82, 0.0, 0.0), Vec3::new(0.08, 0.1, 0.045));
    ssub(d, notch, 0.02)
}

impl Primitive {
    fn fun_mesh(self) -> Option<Mesh> {
        use crate::sdf::*;
        let ivory = Vec3::new(0.88, 0.82, 0.68);
        let m = match self {
            Primitive::Skull => {
                let mut m = mesh_from_sdf(
                    &skull_sdf,
                    &|c| (skull_dark(c) < 0.012 && skull_teeth(c) > 0.006) as u16,
                    Vec3::new(-0.75, -0.75, -0.75),
                    Vec3::new(0.75, 0.95, 0.80),
                    150,
                    vec![ivory, Vec3::new(0.03, 0.025, 0.025)],
                );
                m.normalize();
                m
            }
            Primitive::Bone => {
                let mut m = mesh_from_sdf(&bone_sdf, &|_| 0, Vec3::new(-1.0, -0.3, -0.35), Vec3::new(1.0, 0.3, 0.35), 150, vec![ivory]);
                m.normalize();
                m
            }
            Primitive::CrossedBones => {
                let rot = |a: f32| glam::Quat::from_rotation_z(a);
                let (r1, r2) = (rot(0.62).inverse(), rot(-0.62).inverse());
                let f = move |p: Vec3| smin(bone_sdf(r1 * p), bone_sdf(r2 * p + Vec3::new(0.0, 0.0, 0.0)), 0.04);
                let mut m = mesh_from_sdf(&f, &|_| 0, Vec3::new(-1.0, -1.0, -0.3), Vec3::new(1.0, 1.0, 0.3), 170, vec![ivory]);
                m.normalize();
                m
            }
            _ => return None,
        };
        Some(m)
    }
}

impl Mesh {
    /// Volume com sinal (positivo quando as faces apontam para fora).
    pub fn signed_volume(&self) -> f32 {
        self.tris.iter().map(|t| t.v[0].dot(t.v[1].cross(t.v[2])) / 6.0).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_obj_with_materials_and_polygons() {
        let obj = "mtllib a.mtl\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nusemtl red\nf 1 2 3 4\n";
        let m = Mesh::from_obj(obj, &|_| Some("newmtl red\nKd 1 0 0\n".into()));
        assert_eq!(m.tris.len(), 2);
        assert_eq!(m.mats[m.tris[0].mat as usize], Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn parses_dxf_3dface() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\n3DFACE\n8\nL\n10\n0\n20\n0\n30\n0\n11\n1\n21\n0\n31\n0\n12\n1\n22\n1\n32\n0\n13\n1\n23\n1\n33\n0\n0\nENDSEC\n";
        assert_eq!(Mesh::from_dxf(dxf).tris.len(), 1); // quarto vértice == terceiro: triângulo
    }

    #[test]
    fn normalize_gives_unit_half_extent() {
        let mut m = Mesh::from_obj("v 0 0 0\nv 10 0 0\nv 0 4 0\nf 1 2 3\n", &|_| None);
        m.normalize();
        let (lo, hi) = m.bounds();
        assert!(((hi - lo) * 0.5).max_element() - 1.0 < 1e-5);
    }

    #[test]
    fn primitives_point_outward_and_fit_unit_sphere() {
        for p in Primitive::ALL {
            let m = p.mesh();
            assert!(m.signed_volume() > 0.0, "{} aponta para dentro ({})", p.name(), m.signed_volume());
            assert!(!m.tris.is_empty(), "{}", p.name());
            let (lo, hi) = m.bounds();
            assert!(lo.min_element() >= -1.01 && hi.max_element() <= 1.01, "{} {lo} {hi}", p.name());
        }
    }
}
