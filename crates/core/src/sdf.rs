//! Escultura por campo de distância (SDF) → malha, com *surface nets*.
//! Serve para formas orgânicas (caveira, ossos) que primitivas simples não dão.
use crate::mesh::{Mesh, Tri};
use glam::Vec3;
use rayon::prelude::*;

pub fn sd_ellipsoid(p: Vec3, c: Vec3, r: Vec3) -> f32 {
    let q = (p - c) / r;
    (q.length() - 1.0) * r.min_element()
}

pub fn sd_capsule(p: Vec3, a: Vec3, b: Vec3, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.dot(ba)).clamp(0.0, 1.0);
    (pa - ba * h).length() - r
}

pub fn sd_round_box(p: Vec3, c: Vec3, h: Vec3, r: f32) -> f32 {
    let q = (p - c).abs() - h + Vec3::splat(r);
    q.max(Vec3::ZERO).length() + q.max_element().min(0.0) - r
}

/// União suave (k = raio da concordância).
pub fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

/// Subtração suave: `a` menos `b`.
pub fn ssub(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 - 0.5 * (a + b) / k).clamp(0.0, 1.0);
    a + (-b - a) * h + k * h * (1.0 - h)
}

/// Extrai a superfície f = 0 em `res` células no maior eixo. `mat` escolhe o material pelo centro do triângulo.
pub fn mesh_from_sdf(
    f: &(dyn Fn(Vec3) -> f32 + Sync),
    mat: &(dyn Fn(Vec3) -> u16 + Sync),
    lo: Vec3,
    hi: Vec3,
    res: usize,
    mats: Vec<Vec3>,
) -> Mesh {
    let ext = hi - lo;
    let cell = ext.max_element() / res as f32;
    let n = [(ext.x / cell).ceil() as usize, (ext.y / cell).ceil() as usize, (ext.z / cell).ceil() as usize];
    let (vx, vy) = (n[0] + 1, n[1] + 1);
    let vcount = vx * vy * (n[2] + 1);
    let pos = |i: usize, j: usize, k: usize| lo + Vec3::new(i as f32, j as f32, k as f32) * cell;
    let vals: Vec<f32> = (0..vcount).into_par_iter().map(|idx| f(pos(idx % vx, (idx / vx) % vy, idx / (vx * vy)))).collect();
    let at = |i: usize, j: usize, k: usize| vals[i + j * vx + k * vx * vy];

    // um vértice por célula que cruza a superfície
    let mut cell_vert = vec![u32::MAX; n[0] * n[1] * n[2]];
    let mut verts: Vec<Vec3> = vec![];
    const EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
    for k in 0..n[2] {
        for j in 0..n[1] {
            for i in 0..n[0] {
                let mut v = [0.0f32; 8];
                let mut p = [Vec3::ZERO; 8];
                for c in 0..8 {
                    let (di, dj, dk) = (c & 1, (c >> 1) & 1, (c >> 2) & 1);
                    v[c] = at(i + di, j + dj, k + dk);
                    p[c] = pos(i + di, j + dj, k + dk);
                }
                let neg = v.iter().filter(|x| **x < 0.0).count();
                if neg == 0 || neg == 8 {
                    continue;
                }
                let (mut acc, mut cnt) = (Vec3::ZERO, 0);
                for (a, b) in EDGES {
                    if (v[a] < 0.0) != (v[b] < 0.0) {
                        let t = v[a] / (v[a] - v[b]);
                        acc += p[a].lerp(p[b], t);
                        cnt += 1;
                    }
                }
                cell_vert[i + j * n[0] + k * n[0] * n[1]] = verts.len() as u32;
                verts.push(acc / cnt as f32);
            }
        }
    }
    let e = cell * 0.5;
    let grad = |p: Vec3| {
        Vec3::new(f(p + Vec3::X * e) - f(p - Vec3::X * e), f(p + Vec3::Y * e) - f(p - Vec3::Y * e), f(p + Vec3::Z * e) - f(p - Vec3::Z * e)).normalize_or_zero()
    };
    let normals: Vec<Vec3> = verts.par_iter().map(|p| grad(*p)).collect();
    let cv = |i: usize, j: usize, k: usize| cell_vert[i + j * n[0] + k * n[0] * n[1]];

    let mut idx: Vec<[u32; 3]> = vec![];
    let mut quad = |a: u32, b: u32, c: u32, d: u32| {
        if a != u32::MAX && b != u32::MAX && c != u32::MAX && d != u32::MAX {
            idx.push([a, b, c]);
            idx.push([a, c, d]);
        }
    };
    // uma face por aresta da grade que cruza a superfície
    for k in 1..n[2] {
        for j in 1..n[1] {
            for i in 0..n[0] {
                if (at(i, j, k) < 0.0) != (at(i + 1, j, k) < 0.0) {
                    quad(cv(i, j - 1, k - 1), cv(i, j, k - 1), cv(i, j, k), cv(i, j - 1, k));
                }
            }
        }
    }
    for k in 1..n[2] {
        for j in 0..n[1] {
            for i in 1..n[0] {
                if (at(i, j, k) < 0.0) != (at(i, j + 1, k) < 0.0) {
                    quad(cv(i - 1, j, k - 1), cv(i - 1, j, k), cv(i, j, k), cv(i, j, k - 1));
                }
            }
        }
    }
    for k in 0..n[2] {
        for j in 1..n[1] {
            for i in 1..n[0] {
                if (at(i, j, k) < 0.0) != (at(i, j, k + 1) < 0.0) {
                    quad(cv(i - 1, j - 1, k), cv(i, j - 1, k), cv(i, j, k), cv(i - 1, j, k));
                }
            }
        }
    }
    let tris = idx
        .into_iter()
        .map(|[a, b, c]| {
            let (mut ia, mut ib, mut ic) = (a as usize, b as usize, c as usize);
            let face = (verts[ib] - verts[ia]).cross(verts[ic] - verts[ia]);
            // orienta pela normal do campo (aponta para fora)
            if face.dot(normals[ia] + normals[ib] + normals[ic]) < 0.0 {
                std::mem::swap(&mut ib, &mut ic);
            }
            ia = ia.min(verts.len() - 1);
            let center = (verts[ia] + verts[ib] + verts[ic]) / 3.0;
            Tri { v: [verts[ia], verts[ib], verts[ic]], n: [normals[ia], normals[ib], normals[ic]], mat: mat(center) }
        })
        .collect();
    Mesh { tris, mats }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_sdf_gives_closed_outward_mesh() {
        let f = |p: Vec3| p.length() - 0.7;
        let m = mesh_from_sdf(&f, &|_| 0, Vec3::splat(-1.0), Vec3::splat(1.0), 32, vec![Vec3::ONE]);
        assert!(m.tris.len() > 500);
        let v = m.signed_volume();
        let exact = 4.0 / 3.0 * std::f32::consts::PI * 0.7f32.powi(3);
        assert!((v - exact).abs() / exact < 0.08, "volume {v} vs {exact}");
    }
}
