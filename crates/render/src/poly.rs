//! Poliedros convexos por interseção de semiespaços (face plana).
use glam::{Quat, Vec3};
use terralux_core::Polyhedron;

pub struct Hit {
    pub t: f32,
    pub n: Vec3,
}

pub struct Convex {
    planes: Vec<(Vec3, f32)>, // n·p <= d
}

impl Convex {
    pub fn new(k: Polyhedron, pos: Vec3, rot: Quat, radius: f32) -> Self {
        let (verts, faces) = k.mesh();
        let max_r = verts.iter().map(|v| v.length()).fold(0.0, f32::max);
        let w: Vec<Vec3> = verts.iter().map(|v| pos + rot * (*v * (radius / max_r))).collect();
        let planes = faces
            .iter()
            .map(|f| {
                let (a, b, c) = (w[f[0]], w[f[1]], w[f[2]]);
                let mut n = (b - a).cross(c - a).normalize();
                if n.dot(a - pos) < 0.0 {
                    n = -n;
                }
                (n, n.dot(a))
            })
            .collect();
        Convex { planes }
    }

    pub fn intersect(&self, o: Vec3, d: Vec3) -> Option<Hit> {
        let (mut tn, mut tf, mut nn) = (f32::NEG_INFINITY, f32::INFINITY, Vec3::Y);
        for &(n, dist) in &self.planes {
            let denom = n.dot(d);
            let num = dist - n.dot(o);
            if denom.abs() < 1e-8 {
                if num < 0.0 {
                    return None;
                }
            } else {
                let t = num / denom;
                if denom < 0.0 {
                    if t > tn {
                        tn = t;
                        nn = n;
                    }
                } else if t < tf {
                    tf = t;
                }
            }
        }
        if tn > tf || tn < 1e-3 {
            return None;
        }
        Some(Hit { t: tn, n: nn })
    }
}
