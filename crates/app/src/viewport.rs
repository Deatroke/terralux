//! Janela de trabalho: preview ray-traced + wireframe + interação.
use crate::app::{App, Display, Editor, Sel, Tool};
use crate::theme;
use crate::view::{Basis, Orbit};
use egui::{pos2, vec2, Color32, Painter, PointerButton, Rect, Sense, Stroke, Ui};
use glam::{Quat, Vec3};
use std::collections::HashSet;
use std::sync::Arc;
use terralux_core::{Heightfield, Shape};
use terralux_render::{Pick, Renderer};

const MAX_PREVIEW_W: f32 = 1100.0;

impl App {
    /// Heightfield pequeno (256²) para wireframe e mapa do editor.
    pub fn ensure_map(&mut self) {
        let Some(def) = self.scene.terrain.clone() else {
            self.map_hf = None;
            return;
        };
        let mut d = def;
        d.resolution = 256;
        let key = format!("{:?}", d);
        if self.map_hf.as_ref().map(|m| m.0 != key).unwrap_or(true) {
            self.map_hf = Some((key, Heightfield::from_def(&d)));
        }
    }

    pub fn pick_heightfield_pub(&mut self) -> Option<Arc<Heightfield>> {
        self.pick_heightfield()
    }

    fn pick_heightfield(&mut self) -> Option<Arc<Heightfield>> {
        let mut d = self.scene.terrain.clone()?;
        d.resolution = 512;
        let key = format!("{:?}", d);
        if self.pick_hf.as_ref().map(|m| m.0 != key).unwrap_or(true) {
            self.pick_hf = Some((key, Arc::new(Heightfield::from_def(&d))));
        }
        self.pick_hf.as_ref().map(|m| m.1.clone())
    }

    pub fn viewport(&mut self, ui: &mut Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.view_rect = rect;
        painter.rect_filled(rect, 0.0, theme::pal().view_bg);

        // --- pedidos de preview ---
        if self.display != Display::Wire && rect.width() > 40.0 && rect.height() > 40.0 {
            let k = (MAX_PREVIEW_W / rect.width()).min(1.0);
            let (w, h) = ((rect.width() * k) as usize, (rect.height() * k) as usize);
            let need = match &self.last_req {
                Some((s, (lw, lh))) => *s != self.scene || (*lw as i64 - w as i64).abs() > 2 || (*lh as i64 - h as i64).abs() > 2,
                None => true,
            };
            if need {
                self.req_id = self.preview.request(self.scene.clone(), w, h);
                self.last_req = Some((self.scene.clone(), (w, h)));
            }
        }
        if self.display != Display::Wire {
            if let Some(t) = &self.tex {
                painter.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }

        // --- interação ---
        let mut orb = Orbit::from_camera(&self.scene.camera);
        let mut cam_changed = false;
        if resp.hovered() {
            let sc = ui.input(|i| i.smooth_scroll_delta.y);
            if sc != 0.0 {
                orb.dist = (orb.dist * (-sc * 0.0015).exp()).clamp(2.0, 1500.0);
                cam_changed = true;
            }
            self.hint = "Orbitar: botão direito · Deslocar: botão do meio · Zoom: roda · Clique: selecionar · Duplo clique no terreno: editar".into();
        }
        let d = resp.drag_delta();
        let basis = Basis::new(&self.scene.camera, rect);
        if resp.dragged_by(PointerButton::Secondary) {
            orb.yaw -= d.x * 0.006;
            orb.pitch += d.y * 0.006;
            cam_changed = true;
        } else if resp.dragged_by(PointerButton::Middle) {
            let k = orb.dist * 0.0016;
            orb.target += (-basis.right * d.x + basis.up * d.y) * k;
            cam_changed = true;
        } else if resp.dragged_by(PointerButton::Primary) {
            match (self.tool, self.sel) {
                (Tool::Move | Tool::Rotate | Tool::Scale, Sel::Object(i)) if i < self.scene.objects.len() => {
                    let shift = ui.input(|i| i.modifiers.shift);
                    let o = &mut self.scene.objects[i];
                    match self.tool {
                        Tool::Move => {
                            let k = (o.position - basis.pos).length() * 0.0016;
                            let fh = Vec3::new(basis.fwd.x, 0.0, basis.fwd.z).normalize_or_zero();
                            let rh = Vec3::new(basis.right.x, 0.0, basis.right.z).normalize_or_zero();
                            if shift {
                                o.position.y -= d.y * k;
                            } else {
                                o.position += rh * d.x * k - fh * d.y * k;
                            }
                        }
                        Tool::Rotate => {
                            o.rotation_deg.y += d.x * 0.6;
                            o.rotation_deg.x += d.y * 0.6;
                        }
                        _ => o.size = (o.size * (1.0 + (d.x - d.y) * 0.004)).clamp(0.2, 400.0),
                    }
                }
                _ => {
                    orb.yaw -= d.x * 0.006;
                    orb.pitch += d.y * 0.006;
                    cam_changed = true;
                }
            }
        }
        if cam_changed {
            orb.apply(&mut self.scene.camera);
        }
        if resp.clicked_by(PointerButton::Primary) || resp.double_clicked_by(PointerButton::Primary) {
            if let Some(p) = resp.interact_pointer_pos() {
                let dir = basis.ray(p);
                let hf = self.pick_heightfield();
                let r = Renderer::with_heightfield(&self.scene, hf);
                self.sel = match r.pick(basis.pos, dir) {
                    Pick::Object(i) => Sel::Object(i),
                    Pick::Terrain => Sel::Terrain,
                    Pick::Water => Sel::Water,
                    Pick::Sky => Sel::Sky,
                };
                if self.sel == Sel::Terrain && resp.double_clicked() {
                    self.open_terrain_editor();
                }
            }
        }

        // --- overlays ---
        let basis = Basis::new(&self.scene.camera, rect);
        if self.display != Display::Preview {
            self.draw_wire(&painter, &basis);
        } else if let Sel::Object(i) = self.sel {
            self.draw_object_wire(&painter, &basis, i, true);
        }
        self.draw_gizmos(&painter, &basis, rect);
        let busy = self.display != Display::Wire && self.stage_info.contains(&format!("{}/", crate::worker::Preview::stages() - 1)) ;
        let _ = busy;
        painter.text(rect.right_top() + vec2(-10.0, 10.0), egui::Align2::RIGHT_TOP, &self.stage_info, egui::FontId::proportional(11.0), theme::pal().view_text);
        if self.editor != Editor::Work {
            return;
        }
    }

    fn draw_gizmos(&self, p: &Painter, b: &Basis, rect: Rect) {
        // eixos
        let o = rect.left_bottom() + vec2(56.0, -56.0);
        for (axis, col, name) in [(Vec3::X, Color32::from_rgb(230, 90, 90), "X"), (Vec3::Y, Color32::from_rgb(110, 210, 110), "Y"), (Vec3::Z, Color32::from_rgb(90, 140, 240), "Z")] {
            let e = o + vec2(axis.dot(b.right), -axis.dot(b.up)) * 30.0;
            p.line_segment([o, e], Stroke::new(2.0, col));
            p.text(e + (e - o).normalized() * 8.0, egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(11.0), col);
        }
        // sol
        let sun = self.scene.sun.direction();
        if let Some(s) = b.project(b.pos + sun * 600.0) {
            if rect.contains(s) {
                let c = Color32::from_rgb(255, 214, 110);
                p.circle_stroke(s, 9.0, Stroke::new(1.5, c));
                for i in 0..8 {
                    let a = std::f32::consts::TAU * i as f32 / 8.0;
                    let (sn, cs) = a.sin_cos();
                    p.line_segment([s + vec2(cs, sn) * 13.0, s + vec2(cs, sn) * 18.0], Stroke::new(1.5, c));
                }
            }
        }
    }

    fn poly3(p: &Painter, b: &Basis, pts: &[Vec3], stroke: Stroke) {
        let mut prev = None;
        for &q in pts {
            let cur = b.project(q);
            if let (Some(a), Some(c)) = (prev, cur) {
                p.line_segment([a, c], stroke);
            }
            prev = cur;
        }
    }

    /// Arestas (espaço do objeto, esfera unitária) das primitivas, decimadas para o painter.
    fn prim_edges(p: terralux_core::mesh::Primitive) -> Arc<Vec<[Vec3; 2]>> {
        static C: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<terralux_core::mesh::Primitive, Arc<Vec<[Vec3; 2]>>>>> = std::sync::OnceLock::new();
        let c = C.get_or_init(Default::default);
        if let Some(e) = c.lock().unwrap().get(&p) {
            return e.clone();
        }
        let m = p.mesh();
        let key = |v: Vec3| ((v.x * 1e3).round() as i32, (v.y * 1e3).round() as i32, (v.z * 1e3).round() as i32);
        let mut seen = HashSet::new();
        let mut edges = vec![];
        for t in &m.tris {
            for k in 0..3 {
                let (a, b) = (t.v[k], t.v[(k + 1) % 3]);
                let (ka, kb) = (key(a), key(b));
                if seen.insert((ka.min(kb), ka.max(kb))) {
                    edges.push([a, b]);
                }
            }
        }
        let step = edges.len().div_ceil(700).max(1);
        let edges: Vec<[Vec3; 2]> = edges.into_iter().step_by(step).collect();
        let a = Arc::new(edges);
        c.lock().unwrap().insert(p, a.clone());
        a
    }

    fn object_rot(o: &terralux_core::Object) -> Quat {
        Quat::from_euler(glam::EulerRot::XYZ, o.rotation_deg.x.to_radians(), o.rotation_deg.y.to_radians(), o.rotation_deg.z.to_radians())
    }

    fn object_world_mesh(o: &terralux_core::Object) -> Option<(Vec<Vec3>, Vec<[usize; 3]>)> {
        match &o.shape {
            Shape::Poly(k) => {
                let (v, f) = k.mesh();
                let max_r = v.iter().map(|x| x.length()).fold(0.0, f32::max);
                let q = Self::object_rot(o);
                Some((v.iter().map(|x| o.position + q * (*x * (o.size / max_r))).collect(), f))
            }
            _ => None,
        }
    }

    fn draw_object_wire(&self, p: &Painter, b: &Basis, i: usize, selected: bool) {
        let Some(o) = self.scene.objects.get(i) else { return };
        let stroke = if selected { Stroke::new(2.0, theme::pal().accent) } else { Stroke::new(1.2, Color32::from_rgb(205, 212, 224)) };
        let q = Self::object_rot(o);
        match &o.shape {
            Shape::Poly(_) => {
                let (v, faces) = Self::object_world_mesh(o).unwrap();
                let mut seen = HashSet::new();
                for f in faces {
                    for k in 0..3 {
                        let (a, c) = (f[k], f[(k + 1) % 3]);
                        if seen.insert((a.min(c), a.max(c))) {
                            Self::poly3(p, b, &[v[a], v[c]], stroke);
                        }
                    }
                }
            }
            Shape::Prim(k) => {
                for [a, c] in Self::prim_edges(*k).iter() {
                    Self::poly3(p, b, &[o.position + q * (*a * o.size), o.position + q * (*c * o.size)], stroke);
                }
            }
            Shape::Model(_) => {
                // caixa envolvente orientada (o modelo é normalizado para meia-extensão 1)
                let c = |x: f32, y: f32, z: f32| o.position + q * (Vec3::new(x, y, z) * o.size);
                for (x, y, z) in [(-1.0, -1.0, -1.0), (1.0, -1.0, -1.0), (1.0, 1.0, -1.0), (-1.0, 1.0, -1.0)] {
                    Self::poly3(p, b, &[c(x, y, z), c(-x, y, z)], stroke);
                    Self::poly3(p, b, &[c(x, y, z), c(x, -y, z)], stroke);
                    Self::poly3(p, b, &[c(x, y, z), c(x, y, -z)], stroke);
                    Self::poly3(p, b, &[c(x, y, -z), c(-x, y, -z)], stroke);
                    Self::poly3(p, b, &[c(x, y, -z), c(x, -y, -z)], stroke);
                }
            }
            Shape::Plane => {
                let t = self.scene.camera.target;
                let step = 20.0;
                let (cx, cz) = ((t.x / step).round() * step, (t.z / step).round() * step);
                let st = Stroke::new(if selected { 1.6 } else { 1.0 }, if selected { theme::pal().accent } else { Color32::from_rgba_unmultiplied(190, 170, 120, 110) });
                for k in -8..=8 {
                    let u = k as f32 * step;
                    Self::poly3(p, b, &[Vec3::new(cx + u, o.position.y, cz - 160.0), Vec3::new(cx + u, o.position.y, cz + 160.0)], st);
                    Self::poly3(p, b, &[Vec3::new(cx - 160.0, o.position.y, cz + u), Vec3::new(cx + 160.0, o.position.y, cz + u)], st);
                }
            }
            _ => {
                for ax in 0..3 {
                    let pts: Vec<Vec3> = (0..=32).map(|k| {
                        let t = std::f32::consts::TAU * k as f32 / 32.0;
                        let (s, c) = t.sin_cos();
                        o.position + o.size * match ax { 0 => Vec3::new(c, s, 0.0), 1 => Vec3::new(c, 0.0, s), _ => Vec3::new(0.0, c, s) }
                    }).collect();
                    Self::poly3(p, b, &pts, stroke);
                }
            }
        }
    }

    fn draw_wire(&mut self, p: &Painter, b: &Basis) {
        self.ensure_map();
        let water_level = self.scene.water.as_ref().map(|w| w.level);
        if let Some(lv) = water_level {
            let st = Stroke::new(1.0, Color32::from_rgba_unmultiplied(70, 130, 200, 90));
            for i in -10..=10 {
                let k = i as f32 * 20.0;
                let a: Vec<Vec3> = (0..=20).map(|j| Vec3::new(-200.0 + j as f32 * 20.0, lv, k)).collect();
                let c: Vec<Vec3> = (0..=20).map(|j| Vec3::new(k, lv, -200.0 + j as f32 * 20.0)).collect();
                Self::poly3(p, b, &a, st);
                Self::poly3(p, b, &c, st);
            }
        }
        if let (Some(def), Some((_, hf))) = (&self.scene.terrain, &self.map_hf) {
            let r = def.view_radius();
            let (lines, samples) = (36, 72);
            let sel = self.sel == Sel::Terrain;
            let lv = water_level.unwrap_or(f32::NEG_INFINITY);
            let seg = |a: Vec3, c: Vec3| {
                if let (Some(pa), Some(pc)) = (b.project(a), b.project(c)) {
                    let above = (a.y + c.y) * 0.5 > lv;
                    let col = match (above, sel) {
                        (true, true) => Color32::from_rgb(120, 240, 220),
                        (true, false) => Color32::from_rgba_unmultiplied(90, 190, 160, 190),
                        (false, _) => Color32::from_rgba_unmultiplied(50, 90, 135, 70),
                    };
                    p.line_segment([pa, pc], Stroke::new(if sel { 1.4 } else { 1.0 }, col));
                }
            };
            for i in 0..=lines {
                let u = -r + 2.0 * r * i as f32 / lines as f32;
                let mut pa = Vec3::new(u, hf.height(u, -r), -r);
                let mut pc = Vec3::new(-r, hf.height(-r, u), u);
                for j in 1..=samples {
                    let v = -r + 2.0 * r * j as f32 / samples as f32;
                    let na = Vec3::new(u, hf.height(u, v), v);
                    let nc = Vec3::new(v, hf.height(v, u), u);
                    seg(pa, na);
                    seg(pc, nc);
                    pa = na;
                    pc = nc;
                }
            }
        }
        for i in 0..self.scene.objects.len() {
            self.draw_object_wire(p, b, i, self.sel == Sel::Object(i));
        }
    }
}
