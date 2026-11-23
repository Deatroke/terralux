//! Threads de renderização: preview progressivo e render final.
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use terralux_core::{Heightfield, Scene};
use terralux_render::{tonemap_to_srgb8, RenderOptions, Renderer};

/// Resolução máxima do terreno no preview (o render final usa a da cena).
pub const PREVIEW_TERRAIN_RES: usize = 512;
/// (divisor da resolução, amostras por pixel) de cada estágio do preview.
const STAGES: [(usize, usize); 4] = [(8, 1), (4, 1), (2, 2), (1, 4)];

pub struct PreviewReq {
    pub id: u64,
    pub scene: Scene,
    pub w: usize,
    pub h: usize,
}

pub struct PreviewResp {
    pub id: u64,
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
    pub stage: usize,
    pub ms: u128,
}

pub struct Preview {
    tx: Sender<PreviewReq>,
    pub rx: Receiver<PreviewResp>,
    latest: Arc<AtomicU64>,
    next_id: u64,
}

impl Preview {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (tx, rx_req) = channel::<PreviewReq>();
        let (tx_resp, rx) = channel::<PreviewResp>();
        let latest = Arc::new(AtomicU64::new(0));
        let lat = latest.clone();
        thread::spawn(move || {
            let mut cache: Option<(String, Arc<Heightfield>)> = None;
            while let Ok(mut req) = rx_req.recv() {
                while let Ok(r) = rx_req.try_recv() {
                    req = r;
                }
                if lat.load(Ordering::Relaxed) != req.id {
                    continue;
                }
                let mut sc = req.scene;
                let hf = sc.terrain.as_mut().map(|t| {
                    t.resolution = t.resolution.min(PREVIEW_TERRAIN_RES);
                    let key = format!("{:?}", t);
                    match &cache {
                        Some((k, h)) if *k == key => h.clone(),
                        _ => {
                            let h = Arc::new(Heightfield::from_def(t));
                            cache = Some((key, h.clone()));
                            h
                        }
                    }
                });
                let r = Renderer::with_heightfield(&sc, hf);
                for (i, (div, spp)) in STAGES.iter().enumerate() {
                    let (w, h) = ((req.w / div).max(16), (req.h / div).max(16));
                    let mut o = RenderOptions::new(w, h, *spp);
                    o.cancel = Some((lat.clone(), req.id));
                    let t0 = std::time::Instant::now();
                    let img = r.render(&o);
                    if lat.load(Ordering::Relaxed) != req.id {
                        break;
                    }
                    let rgb = tonemap_to_srgb8(&img);
                    let _ = tx_resp.send(PreviewResp { id: req.id, w, h, rgb, stage: i, ms: t0.elapsed().as_millis() });
                    ctx.request_repaint();
                }
            }
        });
        Preview { tx, rx, latest, next_id: 0 }
    }

    pub fn request(&mut self, scene: Scene, w: usize, h: usize) -> u64 {
        self.next_id += 1;
        self.latest.store(self.next_id, Ordering::Relaxed);
        let _ = self.tx.send(PreviewReq { id: self.next_id, scene, w, h });
        self.next_id
    }

    pub fn stages() -> usize {
        STAGES.len()
    }
}

/// Render final em segundo plano.
pub struct FinalJob {
    pub w: usize,
    pub h: usize,
    pub spp: usize,
    pub rows: Arc<AtomicUsize>,
    pub cancel: Arc<AtomicU64>,
    pub result: Arc<Mutex<Option<Vec<u8>>>>,
    pub started: std::time::Instant,
    pub elapsed: Option<f32>,
    pub rgb: Option<Vec<u8>>,
    pub tex: Option<egui::TextureHandle>,
}

impl FinalJob {
    pub fn start(ctx: egui::Context, scene: Scene, w: usize, h: usize, spp: usize) -> Self {
        let rows = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicU64::new(1));
        let result = Arc::new(Mutex::new(None));
        let (r2, c2, res2) = (rows.clone(), cancel.clone(), result.clone());
        thread::spawn(move || {
            let r = Renderer::new(&scene);
            let mut o = RenderOptions::new(w, h, spp);
            o.cancel = Some((c2.clone(), 1));
            o.rows_done = Some(r2);
            let img = r.render(&o);
            if c2.load(Ordering::Relaxed) == 1 {
                *res2.lock().unwrap() = Some(tonemap_to_srgb8(&img));
            }
            ctx.request_repaint();
        });
        FinalJob { w, h, spp, rows, cancel, result, started: std::time::Instant::now(), elapsed: None, rgb: None, tex: None }
    }

    pub fn progress(&self) -> f32 {
        self.rows.load(Ordering::Relaxed) as f32 / self.h as f32
    }

    pub fn cancel(&self) {
        self.cancel.store(0, Ordering::Relaxed);
    }

    /// Coleta o resultado quando termina.
    pub fn poll(&mut self) {
        if self.rgb.is_none() {
            if let Some(v) = self.result.lock().unwrap().take() {
                self.elapsed = Some(self.started.elapsed().as_secs_f32());
                self.rgb = Some(v);
            }
        }
    }

    pub fn save_png(&self, path: &std::path::Path) -> Result<(), image::ImageError> {
        match &self.rgb {
            Some(b) => image::save_buffer(path, b, self.w as u32, self.h as u32, image::ColorType::Rgb8),
            None => Ok(()),
        }
    }
}
