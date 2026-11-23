//! BVH de triângulos (divisão por SAH em bins) para malhas importadas e primitivas.
use glam::Vec3;
use terralux_core::mesh::{Mesh, Tri};

#[derive(Clone, Copy)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    /// Folha: primeiro triângulo. Interno: índice do filho esquerdo (o direito é +1).
    first: u32,
    /// >0: folha com essa quantidade de triângulos.
    count: u32,
}

pub struct MeshBvh {
    tris: Vec<Tri>,
    nodes: Vec<Node>,
    pub mats: Vec<Vec3>,
}

pub struct MeshHit {
    pub t: f32,
    pub n: Vec3,
    pub mat: u16,
}

const LEAF: usize = 4;
const BINS: usize = 12;

fn tri_box(t: &Tri) -> (Vec3, Vec3) {
    (t.v[0].min(t.v[1]).min(t.v[2]), t.v[0].max(t.v[1]).max(t.v[2]))
}

fn area(lo: Vec3, hi: Vec3) -> f32 {
    let d = (hi - lo).max(Vec3::ZERO);
    d.x * d.y + d.y * d.z + d.z * d.x
}

impl MeshBvh {
    pub fn new(mesh: &Mesh) -> Self {
        let mut tris = mesh.tris.clone();
        let mut nodes = Vec::with_capacity(tris.len() / 2 + 1);
        nodes.push(Node { lo: Vec3::ZERO, hi: Vec3::ZERO, first: 0, count: 0 });
        let n = tris.len();
        Self::build(&mut tris, &mut nodes, 0, 0, n);
        MeshBvh { tris, nodes, mats: mesh.mats.clone() }
    }

    fn build(tris: &mut [Tri], nodes: &mut Vec<Node>, ni: usize, start: usize, end: usize) {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let (mut clo, mut chi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for t in &tris[start..end] {
            let (a, b) = tri_box(t);
            lo = lo.min(a);
            hi = hi.max(b);
            let c = (a + b) * 0.5;
            clo = clo.min(c);
            chi = chi.max(c);
        }
        nodes[ni].lo = lo;
        nodes[ni].hi = hi;
        let count = end - start;
        let make_leaf = |nodes: &mut Vec<Node>| {
            nodes[ni].first = start as u32;
            nodes[ni].count = count as u32;
        };
        if count <= LEAF {
            return make_leaf(nodes);
        }
        let ext = chi - clo;
        let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
        if ext[axis] < 1e-9 {
            return make_leaf(nodes);
        }
        // SAH em bins
        let bin_of = |t: &Tri| {
            let (a, b) = tri_box(t);
            let c = (a + b) * 0.5;
            (((c[axis] - clo[axis]) / ext[axis] * BINS as f32) as usize).min(BINS - 1)
        };
        let mut bcount = [0usize; BINS];
        let mut blo = [Vec3::splat(f32::MAX); BINS];
        let mut bhi = [Vec3::splat(f32::MIN); BINS];
        for t in &tris[start..end] {
            let b = bin_of(t);
            let (a, c) = tri_box(t);
            bcount[b] += 1;
            blo[b] = blo[b].min(a);
            bhi[b] = bhi[b].max(c);
        }
        let (mut best, mut best_cost) = (0usize, f32::MAX);
        for split in 1..BINS {
            let (mut l0, mut h0, mut c0) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN), 0usize);
            let (mut l1, mut h1, mut c1) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN), 0usize);
            for b in 0..BINS {
                if b < split {
                    l0 = l0.min(blo[b]);
                    h0 = h0.max(bhi[b]);
                    c0 += bcount[b];
                } else {
                    l1 = l1.min(blo[b]);
                    h1 = h1.max(bhi[b]);
                    c1 += bcount[b];
                }
            }
            if c0 == 0 || c1 == 0 {
                continue;
            }
            let cost = area(l0, h0) * c0 as f32 + area(l1, h1) * c1 as f32;
            if cost < best_cost {
                best_cost = cost;
                best = split;
            }
        }
        // partição
        let slice = &mut tris[start..end];
        let mut i = 0;
        let mut j = slice.len();
        if best != 0 {
            while i < j {
                if bin_of(&slice[i]) < best {
                    i += 1;
                } else {
                    j -= 1;
                    slice.swap(i, j);
                }
            }
        }
        let mut mid = start + i;
        if best == 0 || mid == start || mid == end {
            // degenerado: mediana por ordenação
            slice.sort_by(|a, b| {
                let (a0, a1) = tri_box(a);
                let (b0, b1) = tri_box(b);
                ((a0 + a1)[axis]).partial_cmp(&(b0 + b1)[axis]).unwrap_or(std::cmp::Ordering::Equal)
            });
            mid = start + count / 2;
        }
        let left = nodes.len();
        nodes.push(Node { lo: Vec3::ZERO, hi: Vec3::ZERO, first: 0, count: 0 });
        nodes.push(Node { lo: Vec3::ZERO, hi: Vec3::ZERO, first: 0, count: 0 });
        nodes[ni].first = left as u32;
        nodes[ni].count = 0;
        Self::build(tris, nodes, left, start, mid);
        Self::build(tris, nodes, left + 1, mid, end);
    }

    #[inline]
    fn box_hit(n: &Node, o: Vec3, inv: Vec3, tmax: f32) -> Option<f32> {
        let t0 = (n.lo - o) * inv;
        let t1 = (n.hi - o) * inv;
        let tmin = t0.min(t1).max_element().max(0.0);
        let tmx = t0.max(t1).min_element().min(tmax);
        (tmin <= tmx).then_some(tmin)
    }

    #[inline]
    fn tri_hit(t: &Tri, o: Vec3, d: Vec3) -> Option<(f32, f32, f32)> {
        let e1 = t.v[1] - t.v[0];
        let e2 = t.v[2] - t.v[0];
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        let s = o - t.v[0];
        let u = s.dot(p) * inv;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(e1);
        let v = d.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let tt = e2.dot(q) * inv;
        (tt > 1e-5).then_some((tt, u, v))
    }

    /// Interseção mais próxima (espaço do objeto). `any` para sombras.
    pub fn intersect(&self, o: Vec3, d: Vec3, tmax: f32, any: bool) -> Option<MeshHit> {
        let inv = Vec3::new(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
        let mut best: Option<(f32, usize, f32, f32)> = None;
        let mut tm = tmax;
        let mut stack = [0u32; 64];
        let mut sp = 0;
        stack[0] = 0;
        sp += 1;
        if Self::box_hit(&self.nodes[0], o, inv, tm).is_none() {
            return None;
        }
        while sp > 0 {
            sp -= 1;
            let n = self.nodes[stack[sp] as usize];
            if n.count > 0 {
                for i in n.first as usize..(n.first + n.count) as usize {
                    if let Some((t, u, v)) = Self::tri_hit(&self.tris[i], o, d) {
                        if t < tm {
                            tm = t;
                            best = Some((t, i, u, v));
                            if any {
                                break;
                            }
                        }
                    }
                }
                if any && best.is_some() {
                    break;
                }
            } else {
                let (a, b) = (n.first, n.first + 1);
                let ta = Self::box_hit(&self.nodes[a as usize], o, inv, tm);
                let tb = Self::box_hit(&self.nodes[b as usize], o, inv, tm);
                match (ta, tb) {
                    (Some(x), Some(y)) => {
                        // empilha o mais distante primeiro: o mais próximo sai antes
                        let (far, near) = if x <= y { (b, a) } else { (a, b) };
                        stack[sp] = far;
                        stack[sp + 1] = near;
                        sp += 2;
                    }
                    (Some(_), None) => {
                        stack[sp] = a;
                        sp += 1;
                    }
                    (None, Some(_)) => {
                        stack[sp] = b;
                        sp += 1;
                    }
                    _ => {}
                }
            }
        }
        best.map(|(t, i, u, v)| {
            let tr = &self.tris[i];
            let n = (tr.n[0] * (1.0 - u - v) + tr.n[1] * u + tr.n[2] * v).normalize_or_zero();
            MeshHit { t, n, mat: tr.mat }
        })
    }

    /// Força bruta, só para os testes.
    #[cfg(test)]
    pub fn intersect_brute(&self, o: Vec3, d: Vec3) -> Option<f32> {
        self.tris.iter().filter_map(|t| Self::tri_hit(t, o, d).map(|h| h.0)).fold(None, |a: Option<f32>, t| Some(a.map_or(t, |x| x.min(t))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terralux_core::mesh::Primitive;

    #[test]
    fn bvh_matches_brute_force() {
        let mut seed = 12345u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 8) as f32 / 16_777_216.0
        };
        for p in Primitive::ALL {
            let bvh = MeshBvh::new(&p.mesh());
            for _ in 0..400 {
                let o = Vec3::new(rnd() * 6.0 - 3.0, rnd() * 6.0 - 3.0, rnd() * 6.0 - 3.0);
                let target = Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5);
                let d = (target - o).normalize();
                let a = bvh.intersect(o, d, 1e9, false).map(|h| h.t);
                let b = bvh.intersect_brute(o, d);
                match (a, b) {
                    (None, None) => {}
                    (Some(x), Some(y)) => assert!((x - y).abs() < 1e-4, "{p:?}: {x} vs {y}"),
                    other => panic!("{p:?}: divergiu {other:?}"),
                }
            }
        }
    }
}
