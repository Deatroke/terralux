//! Despeja o mapa de altura de um modelo em PNG (depuração): hm <id> <saida.png> [resolucao]
use terralux_core::{Heightfield, Scene};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut s = Scene::template(&a[1]).expect("modelo");
    let mut d = s.terrain.take().expect("sem terreno");
    d.resolution = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(512);
    let h = Heightfield::from_def(&d);
    let (lo, hi) = (h.min, h.max);
    eprintln!("min {lo:.1} max {hi:.1} slope {:.2}", h.max_slope);
    let img = image::GrayImage::from_fn(h.n as u32, h.n as u32, |x, y| image::Luma([(((h.data[y as usize * h.n + x as usize] - lo) / (hi - lo)) * 255.0) as u8]));
    img.save(&a[2]).unwrap();
}
