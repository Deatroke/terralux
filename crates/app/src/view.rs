//! Câmera orbital e projeção (mesma geometria do renderizador).
use egui::{pos2, Pos2, Rect};
use glam::Vec3;
use terralux_core::Camera;

pub struct Orbit {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
}

impl Orbit {
    pub fn from_camera(c: &Camera) -> Self {
        let off = c.position - c.target;
        let dist = off.length().max(0.5);
        Orbit { target: c.target, yaw: off.x.atan2(off.z), pitch: (off.y / dist).clamp(-1.0, 1.0).asin(), dist }
    }

    pub fn apply(&self, c: &mut Camera) {
        let p = self.pitch.clamp(-1.52, 1.52);
        c.position = self.target + self.dist * Vec3::new(p.cos() * self.yaw.sin(), p.sin(), p.cos() * self.yaw.cos());
        c.target = self.target;
    }
}

pub struct Basis {
    pub pos: Vec3,
    pub fwd: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    half_w: f32,
    half_h: f32,
    rect: Rect,
}

impl Basis {
    pub fn new(c: &Camera, rect: Rect) -> Self {
        let fwd = (c.target - c.position).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let half_h = (c.fov_deg.to_radians() * 0.5).tan();
        let half_w = half_h * rect.width() / rect.height().max(1.0);
        Basis { pos: c.position, fwd, right, up, half_w, half_h, rect }
    }

    pub fn ray(&self, p: Pos2) -> Vec3 {
        let u = ((p.x - self.rect.min.x) / self.rect.width() * 2.0 - 1.0) * self.half_w;
        let v = (1.0 - (p.y - self.rect.min.y) / self.rect.height() * 2.0) * self.half_h;
        (self.fwd + self.right * u + self.up * v).normalize()
    }

    pub fn project(&self, w: Vec3) -> Option<Pos2> {
        let v = w - self.pos;
        let z = v.dot(self.fwd);
        if z < 0.05 {
            return None;
        }
        let x = v.dot(self.right) / (z * self.half_w);
        let y = v.dot(self.up) / (z * self.half_h);
        Some(pos2(self.rect.min.x + (x + 1.0) * 0.5 * self.rect.width(), self.rect.min.y + (1.0 - y) * 0.5 * self.rect.height()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_and_ray_are_inverse() {
        let cam = Scene::demo().camera;
        let rect = Rect::from_min_size(pos2(10.0, 20.0), egui::vec2(800.0, 450.0));
        let b = Basis::new(&cam, rect);
        let w = cam.position + Vec3::new(3.0, 4.0, 60.0);
        let p = b.project(w).expect("na frente da câmera");
        let dir = b.ray(p);
        let expect = (w - cam.position).normalize();
        assert!((dir - expect).length() < 1e-3, "{dir:?} vs {expect:?}");
    }

    #[test]
    fn orbit_roundtrip_keeps_camera() {
        let mut c = Scene::demo().camera;
        let before = c.position;
        Orbit::from_camera(&c).apply(&mut c);
        assert!((c.position - before).length() < 1e-3);
    }

    use terralux_core::Scene;
}
