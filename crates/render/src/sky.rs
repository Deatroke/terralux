//! Céu procedural: gradiente atmosférico, sol, brilho e nuvens.
use glam::{Vec2, Vec3};
use terralux_core::noise::{hash3, perlin2};
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

pub fn sky_color(s: &Scene, o: Vec3, d: Vec3, sun: Vec3, fast: bool) -> Vec3 {
    let up = d.y.max(0.0);
    let k = up.powf(0.45);
    let mut col = s.sky.horizon.lerp(s.sky.zenith, k);
    let cs = d.dot(sun).max(0.0);
    col += s.sun.color * (cs.powf(8.0) * 0.12 + cs.powf(48.0) * 0.28 + cs.powf(512.0) * 1.2);
    if cs > 0.99985 && d.y > -0.02 {
        col += s.sun.color * (s.sun.intensity * 22.0);
    }
    if s.sky.stars > 0.0 && (d.y > -0.02 || (s.terrain.is_none() && s.water.is_none())) {
        col += stars(s.sky.stars, d, col);
    }
    if let Some(c) = &s.sky.clouds {
        if d.y > 0.012 {
            let t = (c.altitude - o.y) / d.y;
            let p = Vec2::new(o.x + d.x * t, o.z + d.z * t);
            let dens = cloud_density(s, p);
            if dens > 0.0 {
                // Sombreamento: olha a densidade na direção do sol para o auto-sombreamento.
                let sxz = Vec2::new(sun.x, sun.z).normalize_or_zero() * 90.0;
                let dl = if fast { dens } else { cloud_density(s, p + sxz) };
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

/// Campo de estrelas: células na esfera, uma estrela por célula sorteada, com brilho variável.
/// Some quando o céu é claro (o fundo já ofusca as estrelas).
fn stars(density: f32, d: Vec3, bg: Vec3) -> Vec3 {
    let lum = bg.dot(Vec3::new(0.3, 0.59, 0.11));
    let vis = (1.0 - lum * 6.0).clamp(0.0, 1.0);
    if vis <= 0.0 {
        return Vec3::ZERO;
    }
    let mut out = Vec3::ZERO;
    // Duas camadas: estrelas miúdas e densas, e poucas maiores.
    for (scale, thr, size, seed) in [(220.0f32, 0.10, 0.5f32, 17u32), (90.0, 0.035, 0.6, 91)] {
        let p = d * scale;
        let c = p.floor();
        let (cx, cy, cz) = (c.x as i32, c.y as i32, c.z as i32);
        let h = hash3(cx, cy, cz, seed);
        if (h & 0xffff) as f32 / 65535.0 > thr * density.min(3.0) {
            continue;
        }
        let j = |k: u32| (hash3(cx, cy, cz, seed ^ k) & 0xffff) as f32 / 65535.0;
        let center = c + Vec3::new(0.2 + 0.6 * j(1), 0.2 + 0.6 * j(2), 0.2 + 0.6 * j(3));
        let dist = (p - center).length() / size;
        if dist < 1.0 {
            let tint = Vec3::new(0.85 + 0.15 * j(4), 0.9, 1.0 - 0.2 * j(4));
            out += tint * ((1.0 - dist).powi(2) * (0.35 + 1.4 * j(5) * j(5)));
        }
    }
    out * vis
}
