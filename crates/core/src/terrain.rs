//! Heightfield gerado proceduralmente (equivalente funcional do "Terrain Editor").
use crate::noise::{fbm2, ridged2};
use crate::scene::{TerrainDef, TerrainSource};
use glam::{Vec2, Vec3};
use rayon::prelude::*;

pub struct Heightfield {
    pub n: usize,
    pub half: f32,
    pub cell: f32,
    pub data: Vec<f32>,
    pub min: f32,
    pub max: f32,
    /// Limite superior do gradiente (para o ray marching seguro).
    pub max_slope: f32,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Amostragem bicúbica (Catmull-Rom) de uma imagem em tons de cinza; u, v em 0..1.
fn catmull_sample(v: &[f32], w: usize, h: usize, u: f32, t: f32) -> f32 {
    let fx = u * (w - 1) as f32;
    let fy = t * (h - 1) as f32;
    let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
    let (a, b) = (fx - x0 as f32, fy - y0 as f32);
    let px = |x: i64, y: i64| v[(y.clamp(0, h as i64 - 1) as usize) * w + x.clamp(0, w as i64 - 1) as usize];
    let cr = |p0: f32, p1: f32, p2: f32, p3: f32, x: f32| {
        p1 + 0.5 * x * (p2 - p0 + x * (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3 + x * (3.0 * (p1 - p2) + p3 - p0)))
    };
    let row = |dy: i64| cr(px(x0 - 1, y0 + dy), px(x0, y0 + dy), px(x0 + 1, y0 + dy), px(x0 + 2, y0 + dy), a);
    cr(row(-1), row(0), row(1), row(2), b)
}

impl Heightfield {
    pub fn from_def(d: &TerrainDef) -> Self {
        let n = d.resolution.max(8);
        let cell = 2.0 * d.half_size / (n - 1) as f32;
        let src = match &d.source {
            TerrainSource::Procedural => None,
            TerrainSource::Image(p) => match crate::assets::cached_image(p) {
                Ok(img) => Some(img),
                Err(e) => {
                    eprintln!("terreno por imagem: {e}");
                    None
                }
            },
        };
        let lum: Option<(usize, usize, Vec<f32>)> = src.as_ref().map(|im| {
            let v = im.rgba.chunks_exact(4).map(|p| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0).collect();
            (im.w, im.h, v)
        });
        let flat_image = matches!(d.source, TerrainSource::Image(_)) && lum.is_none();
        let data: Vec<f32> = (0..n * n)
            .into_par_iter()
            .map(|i| {
                let (ix, iz) = (i % n, i / n);
                if let Some((w, h, v)) = &lum {
                    let l = catmull_sample(v, *w, *h, ix as f32 / (n - 1) as f32, iz as f32 / (n - 1) as f32);
                    return -d.sea_floor + l.clamp(0.0, 1.0) * (d.peak_height + d.sea_floor);
                }
                if flat_image {
                    return -d.sea_floor;
                }
                let x = -d.half_size + ix as f32 * cell;
                let z = -d.half_size + iz as f32 * cell;
                Self::sample_def(d, x, z)
            })
            .collect();
        let (mut min, mut max) = (f32::MAX, f32::MIN);
        for &h in &data {
            min = min.min(h);
            max = max.max(h);
        }
        let mut g = 0.0f32;
        for z in 0..n - 1 {
            for x in 0..n - 1 {
                let h = data[z * n + x];
                let gx = (data[z * n + x + 1] - h).abs();
                let gz = (data[(z + 1) * n + x] - h).abs();
                g = g.max((gx * gx + gz * gz).sqrt());
            }
        }
        Heightfield { n, half: d.half_size, cell, data, min, max, max_slope: g / cell * 1.1 + 0.01 }
    }

    fn sample_def(d: &TerrainDef, x: f32, z: f32) -> f32 {
        let p = Vec2::new(x, z);
        // Deformação de domínio: costa irregular.
        let warp = Vec2::new(
            fbm2(p * 0.018 + Vec2::splat(31.7), 4, 2.0, 0.5, d.seed ^ 0xa1),
            fbm2(p * 0.018 - Vec2::splat(12.3), 4, 2.0, 0.5, d.seed ^ 0xb2),
        );
        let q = p + warp * d.island_radius * 0.55;
        let r = q.length() / d.island_radius;
        let mask = smoothstep(1.25, 0.15, r);
        let f = d.frequency;
        let base = fbm2(p * f, d.octaves, 2.05, 0.5, d.seed) * 0.5 + 0.5;
        let ridge = ridged2(p * f * 0.8 + Vec2::splat(5.0), d.octaves, 2.1, 0.5, d.seed ^ 0x77);
        // Mais cristas no centro (picos), mais suave perto da costa.
        let shape = base * 0.35 + ridge * (0.45 + 0.4 * mask) + 0.1;
        let land = d.peak_height * mask * mask.sqrt() * shape.clamp(0.0, 1.2);
        // Sobe a base para a ilha emergir; o fundo do mar desce ao longe.
        let shelf = -d.sea_floor * (1.0 - smoothstep(0.0, 0.9, mask)) + 2.2 * smoothstep(0.1, 0.45, mask);
        shelf + land
    }

    #[inline]
    fn at(&self, ix: usize, iz: usize) -> f32 {
        self.data[iz * self.n + ix]
    }

    /// Altura com interpolação bilinear (fora do terreno: fundo do mar).
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let fx = ((x + self.half) / self.cell).clamp(0.0, (self.n - 1) as f32 - 1e-3);
        let fz = ((z + self.half) / self.cell).clamp(0.0, (self.n - 1) as f32 - 1e-3);
        let (ix, iz) = (fx as usize, fz as usize);
        let (u, v) = (fx - ix as f32, fz - iz as f32);
        let a = self.at(ix, iz) + u * (self.at(ix + 1, iz) - self.at(ix, iz));
        let b = self.at(ix, iz + 1) + u * (self.at(ix + 1, iz + 1) - self.at(ix, iz + 1));
        a + v * (b - a)
    }

    pub fn normal(&self, x: f32, z: f32) -> Vec3 {
        let e = self.cell;
        let dx = self.height(x + e, z) - self.height(x - e, z);
        let dz = self.height(x, z + e) - self.height(x, z - e);
        Vec3::new(-dx, 2.0 * e, -dz).normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_terrain_maps_luminance_to_height() {
        let dir = std::env::temp_dir().join("terralux-test-hm");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ramp.png");
        let img = image::GrayImage::from_fn(16, 16, |x, _| image::Luma([(x * 17) as u8]));
        img.save(&path).unwrap();
        let mut d = TerrainDef::from_image(path.to_str().unwrap());
        d.resolution = 64;
        let hf = Heightfield::from_def(&d);
        assert!((hf.min + d.sea_floor).abs() < 0.2, "min {}", hf.min);
        assert!((hf.max - d.peak_height).abs() < 0.5, "max {}", hf.max);
        // rampa em X: esquerda baixa, direita alta
        assert!(hf.height(-100.0, 0.0) < hf.height(100.0, 0.0));
    }
}
