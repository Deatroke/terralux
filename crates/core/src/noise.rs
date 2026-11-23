//! Ruído procedural (Perlin 2D com gradientes, ruído de valor 3D, fBm).
//! Escrito do zero; é a base do Terrain Editor e do futuro Deep Texture Editor.
use glam::{Vec2, Vec3};

#[inline]
fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d)
        ^ (y as u32).wrapping_mul(0x1656_67b1)
        ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

#[inline]
pub fn hash3(x: i32, y: i32, z: i32, seed: u32) -> u32 {
    hash2(x.wrapping_add(hash2(z, 7, seed) as i32), y, seed ^ 0x51ed_270b)
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Ruído de gradiente 2D, aproximadamente em [-1, 1].
pub fn perlin2(p: Vec2, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let (ix, iy) = (i.x as i32, i.y as i32);
    let g = |dx: i32, dy: i32| -> f32 {
        let h = hash2(ix + dx, iy + dy, seed);
        let a = (h as f32) * (std::f32::consts::TAU / 4_294_967_296.0);
        let (s, c) = a.sin_cos();
        c * (f.x - dx as f32) + s * (f.y - dy as f32)
    };
    let (u, v) = (fade(f.x), fade(f.y));
    let a = g(0, 0) + u * (g(1, 0) - g(0, 0));
    let b = g(0, 1) + u * (g(1, 1) - g(0, 1));
    (a + v * (b - a)) * 1.4
}

/// Ruído de valor 3D em [0, 1].
pub fn value3(p: Vec3, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let (ix, iy, iz) = (i.x as i32, i.y as i32, i.z as i32);
    let v = |dx: i32, dy: i32, dz: i32| hash3(ix + dx, iy + dy, iz + dz, seed) as f32 / 4_294_967_296.0;
    let (u, w, t) = (fade(f.x), fade(f.y), fade(f.z));
    let l = |a: f32, b: f32, k: f32| a + (b - a) * k;
    let x00 = l(v(0, 0, 0), v(1, 0, 0), u);
    let x10 = l(v(0, 1, 0), v(1, 1, 0), u);
    let x01 = l(v(0, 0, 1), v(1, 0, 1), u);
    let x11 = l(v(0, 1, 1), v(1, 1, 1), u);
    l(l(x00, x10, w), l(x01, x11, w), t)
}

/// fBm de Perlin 2D, normalizado para ~[-1, 1].
pub fn fbm2(mut p: Vec2, octaves: u32, lacunarity: f32, gain: f32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * perlin2(p, seed.wrapping_add(o * 131));
        norm += amp;
        amp *= gain;
        p *= lacunarity;
    }
    sum / norm
}

/// Multifractal "ridged": cristas afiadas, típico de rocha. Retorna ~[0, 1].
pub fn ridged2(mut p: Vec2, octaves: u32, lacunarity: f32, gain: f32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut weight) = (0.0, 1.0, 0.0, 1.0f32);
    for o in 0..octaves {
        let n = 1.0 - perlin2(p, seed.wrapping_add(o * 977)).abs().min(1.0);
        let n = n * n * weight;
        weight = (n * 1.6).clamp(0.0, 1.0);
        sum += amp * n;
        norm += amp;
        amp *= gain;
        p *= lacunarity;
    }
    sum / norm
}

/// fBm de ruído de valor 3D em [0, 1] (nuvens, texturas de volume).
pub fn fbm3(mut p: Vec3, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * value3(p, seed.wrapping_add(o * 53));
        norm += amp;
        amp *= 0.5;
        p *= 2.03;
    }
    sum / norm
}
