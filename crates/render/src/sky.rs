//! Céu procedural: gradiente atmosférico, sol, brilho e nuvens.
use glam::{Vec2, Vec3};
use terralux_core::noise::perlin2;
use terralux_core::Scene;

pub fn horizon_color(s: &Scene, dir: Vec3, sun: Vec3) -> Vec3 {
    let glow = dir.dot(sun).max(0.0).powf(6.0);
    s.sky.horizon * (1.0 + 0.25 * glow) + s.sun.color * (0.18 * glow)
}

fn cloud_density(s: &Scene, p: Vec2) -> f32 {
    let c = s.sky.clouds.as_ref().unwrap();
    let q = p * 0.0016 / c.scale.max(0.05);
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, 1.0);
    for o in 0..6 {
        sum += amp * (perlin2(q * f, c.seed + o) * 0.5 + 0.5);
        norm += amp;
        amp *= 0.5;
        f *= 2.1;
    }
    let v = sum / norm;
    let thr = 1.0 - c.coverage * 0.62 - 0.08;
    ((v - thr) / 0.22).clamp(0.0, 1.0)
}

pub fn sky_color(s: &Scene, o: Vec3, d: Vec3, sun: Vec3) -> Vec3 {
    let up = d.y.max(0.0);
    let k = up.powf(0.45);
    let mut col = s.sky.horizon.lerp(s.sky.zenith, k);
    let cs = d.dot(sun).max(0.0);
    col += s.sun.color * (cs.powf(8.0) * 0.12 + cs.powf(48.0) * 0.28 + cs.powf(512.0) * 1.2);
    if cs > 0.99985 && d.y > -0.02 {
        col += s.sun.color * (s.sun.intensity * 22.0);
    }
    if let Some(c) = &s.sky.clouds {
        if d.y > 0.012 {
            let t = (c.altitude - o.y) / d.y;
            let p = Vec2::new(o.x + d.x * t, o.z + d.z * t);
            let dens = cloud_density(s, p);
            if dens > 0.0 {
                // Sombreamento: olha a densidade na direção do sol para o auto-sombreamento.
                let sxz = Vec2::new(sun.x, sun.z).normalize_or_zero() * 90.0;
                let dl = cloud_density(s, p + sxz);
                let shade = (1.0 - (dl - dens * 0.6).max(0.0) * 1.1).clamp(0.2, 1.0);
                let lit = s.sun.color * (s.sun.intensity * 0.30) * shade + s.sky.zenith * 0.35 + Vec3::splat(0.25);
                let ccol = lit.lerp(s.sky.horizon, (1.0 - d.y).powf(3.0) * 0.5);
                let alpha = dens * dens.sqrt() * ((d.y - 0.012) / 0.12).clamp(0.0, 1.0) * 0.95;
                col = col.lerp(ccol, alpha);
            }
        }
    }
    col
}
