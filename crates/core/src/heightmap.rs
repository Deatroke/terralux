//! Mapas de altura para engines: exportação PNG 16 bits e RAW 16 bits (Unity, Unreal, Godot, World Machine)
//! e leitura com precisão total na importação.
use crate::terrain::Heightfield;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// Altura amostrada numa grade n×n que cobre o terreno inteiro, em 16 bits.
/// Linha 0 = borda z = −meia largura; coluna 0 = x = −meia largura (a mesma convenção da importação).
#[derive(Clone, Debug)]
pub struct Heightmap {
    pub n: usize,
    /// Altura (m) que corresponde ao valor 0.
    pub min: f32,
    /// Altura (m) que corresponde ao valor 65535.
    pub max: f32,
    /// Lado do terreno (m).
    pub extent: f32,
    pub data: Vec<u16>,
}

impl Heightmap {
    /// Reamostra o terreno em `n`×`n` (use 2ⁿ+1: 513, 1025, 2049, 4097 para a Unity), normalizando de min a max.
    pub fn from_heightfield(hf: &Heightfield, n: usize) -> Heightmap {
        Self::with_range(hf, n, hf.min, hf.max)
    }

    /// Como `from_heightfield`, mas com a faixa [min, max] escolhida (alturas fora dela são cortadas).
    pub fn with_range(hf: &Heightfield, n: usize, min: f32, max: f32) -> Heightmap {
        let n = n.max(2);
        let span = (max - min).max(1e-6);
        let extent = 2.0 * hf.half;
        let mut data = Vec::with_capacity(n * n);
        for iz in 0..n {
            let z = -hf.half + extent * iz as f32 / (n - 1) as f32;
            for ix in 0..n {
                let x = -hf.half + extent * ix as f32 / (n - 1) as f32;
                let t = ((hf.height(x, z) - min) / span).clamp(0.0, 1.0);
                data.push((t * 65535.0).round() as u16);
            }
        }
        Heightmap { n, min, max, extent, data }
    }

    /// Altura total representada (m): é o valor de "Terrain Height" a usar na engine.
    pub fn height_range(&self) -> f32 {
        self.max - self.min
    }

    /// PNG em tons de cinza de 16 bits.
    pub fn write_png16(&self, path: &Path) -> Result<(), String> {
        let img: image::ImageBuffer<image::Luma<u16>, Vec<u16>> =
            image::ImageBuffer::from_raw(self.n as u32, self.n as u32, self.data.clone()).ok_or("tamanho inválido")?;
        img.save(path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// RAW 16 bits, little-endian (ordem de bytes "Windows" da Unity), sem cabeçalho: n·n·2 bytes.
    pub fn write_raw16(&self, path: &Path) -> Result<(), String> {
        let mut bytes = Vec::with_capacity(self.data.len() * 2);
        for v in &self.data {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Mapa de altura em tons de cinza: valores 0..1, linhas de cima para baixo.
pub type Gray = (usize, usize, Vec<f32>);

/// Lê um mapa de altura com a maior precisão possível: PNG/TIFF de 16 bits, RAW/R16 quadrado
/// (little-endian) ou qualquer imagem comum (luminância).
pub fn load_gray(spec: &str) -> Result<Arc<Gray>, String> {
    static C: OnceLock<Mutex<HashMap<String, Arc<Gray>>>> = OnceLock::new();
    let c = C.get_or_init(Default::default);
    if let Some(g) = c.lock().unwrap().get(spec) {
        return Ok(g.clone());
    }
    let path = crate::assets::resolve(spec);
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let g: Gray = if ext == "raw" || ext == "r16" {
        let b = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let n = ((b.len() / 2) as f64).sqrt().round() as usize;
        if n < 2 || n * n * 2 != b.len() {
            return Err(format!("{}: RAW 16 bits precisa ser quadrado (n·n·2 bytes); tem {} bytes", path.display(), b.len()));
        }
        (n, n, b.chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]]) as f32 / 65535.0).collect())
    } else {
        match image::open(&path) {
            Ok(im) => {
                let g = im.into_luma16();
                (g.width() as usize, g.height() as usize, g.into_raw().into_iter().map(|v| v as f32 / 65535.0).collect())
            }
            Err(_) => {
                // TIFF com paleta e afins: o leitor de imagens do projeto.
                let im = crate::assets::cached_image(spec)?;
                (im.w, im.h, im.rgba.chunks_exact(4).map(|p| (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) / 255.0).collect())
            }
        }
    };
    let g = Arc::new(g);
    c.lock().unwrap().insert(spec.to_string(), g.clone());
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::TerrainDef;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join("terralux-test-hmap");
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn export_and_reimport_keep_16_bits() {
        let mut def = TerrainDef::island();
        def.resolution = 129;
        let hf = Heightfield::from_def(&def);
        let hm = Heightmap::from_heightfield(&hf, 65);
        assert_eq!(hm.data.len(), 65 * 65);
        // A reamostragem pode não cair exatamente no ponto mais alto, mas chega perto dos extremos.
        assert!(*hm.data.iter().min().unwrap() < 3000);
        assert!(*hm.data.iter().max().unwrap() > 62000);

        let raw = tmp("a.raw");
        hm.write_raw16(&raw).unwrap();
        assert_eq!(std::fs::metadata(&raw).unwrap().len(), 65 * 65 * 2);
        let g = load_gray(raw.to_str().unwrap()).unwrap();
        assert_eq!((g.0, g.1), (65, 65));

        let png = tmp("a.png");
        hm.write_png16(&png).unwrap();
        let g2 = load_gray(png.to_str().unwrap()).unwrap();
        for i in 0..hm.data.len() {
            let want = hm.data[i] as f32 / 65535.0;
            assert!((g.2[i] - want).abs() < 1e-6 && (g2.2[i] - want).abs() < 1e-6, "pixel {i}");
        }
    }

    #[test]
    fn sixteen_bit_png_is_not_quantized_to_8_bits() {
        let img: image::ImageBuffer<image::Luma<u16>, Vec<u16>> = image::ImageBuffer::from_fn(8, 8, |x, _| image::Luma([1000 + x as u16]));
        let p = tmp("ramp16.png");
        img.save(&p).unwrap();
        let g = load_gray(p.to_str().unwrap()).unwrap();
        // 1 degrau de 16 bits (1/65535) entre colunas vizinhas.
        assert!((g.2[1] - g.2[0] - 1.0 / 65535.0).abs() < 1e-7);
    }

    #[test]
    fn raw_must_be_square() {
        let p = tmp("bad.raw");
        std::fs::write(&p, vec![0u8; 10]).unwrap();
        assert!(load_gray(p.to_str().unwrap()).is_err());
    }
}
