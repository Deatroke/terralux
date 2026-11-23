//! Ícones brancos para as esferas de vidro. Alguns vêm do pacote AeroMedia (CC BY 4.0, ver assets/aero/CREDITS.md);
//! os outros são desenhados por código, no mesmo estilo.
use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum Icon {
    Terrain,
    Water,
    Cloud,
    Sphere,
    Tetra,
    Octa,
    Icosa,
    Select,
    Move,
    Rotate,
    Scale,
    Sun,
    Cube,
    Skull,
    Tree,
    Picture,
    Checker,
    Gel,
    Copy,
    Trash,
    Undo,
    Redo,
    Home,
    Render,
    Map,
}

fn texture_for(icon: Icon) -> Option<(&'static str, bool)> {
    Some(match icon {
        Icon::Cloud => ("g_cloud", false),
        Icon::Cube => ("g_allapps", false),
        Icon::Gel => ("g_flashlight", false),
        Icon::Trash => ("g_bin", false),
        Icon::Home => ("g_home", false),
        Icon::Map => ("g_compass", false),
        Icon::Redo => ("g_refresh_reset", false),
        Icon::Undo => ("g_refresh_reset", true),
        Icon::Render => ("g_camera", false),
        _ => return None,
    })
}

pub(crate) fn draw(p: &Painter, r: Rect, icon: Icon, col: Color32) {
    if let Some((name, flip)) = texture_for(icon) {
        if crate::aero::glyph(p, name, r.expand(r.width() * 0.22), col, 0.0, flip) {
            return;
        }
    }
    let c = r.center();
    let h = r.width() * 0.5;
    let pt = |x: f32, y: f32| pos2(c.x + x * h, c.y + y * h);
    let st = Stroke::new(1.6, col);
    let poly = |pts: Vec<Pos2>, fill: Color32| {
        p.add(Shape::convex_polygon(pts, fill, st));
    };
    let line = |a: Pos2, b: Pos2| {
        p.line_segment([a, b], st);
    };
    let faint = col.gamma_multiply(0.30);
    match icon {
        Icon::Terrain => poly(vec![pt(-1.0, 0.8), pt(-0.4, -0.55), pt(0.0, 0.1), pt(0.4, -0.85), pt(1.0, 0.8)], faint),
        Icon::Water => {
            for k in 0..3 {
                let y0 = -0.55 + 0.55 * k as f32;
                let pts: Vec<Pos2> = (0..=16).map(|i| {
                    let t = i as f32 / 16.0;
                    pt(-1.0 + 2.0 * t, y0 + 0.16 * (t * 12.5).sin())
                }).collect();
                p.add(Shape::line(pts, st));
            }
        }
        Icon::Cloud => {
            p.circle(pt(-0.45, 0.15), h * 0.38, faint, st);
            p.circle(pt(0.0, -0.18), h * 0.5, faint, st);
            p.circle(pt(0.5, 0.15), h * 0.36, faint, st);
            p.rect_filled(Rect::from_min_max(pt(-0.5, 0.2), pt(0.5, 0.52)), 3.0, faint);
        }
        Icon::Sphere => {
            p.circle(c, h * 0.92, faint, st);
            p.circle_filled(pt(-0.3, -0.3), h * 0.16, col.gamma_multiply(0.8));
        }
        Icon::Tetra => {
            poly(vec![pt(0.0, -0.95), pt(1.0, 0.8), pt(-1.0, 0.8)], faint);
            line(pt(0.0, -0.95), pt(0.15, 0.8));
        }
        Icon::Octa => {
            poly(vec![pt(0.0, -1.0), pt(0.9, 0.0), pt(0.0, 1.0), pt(-0.9, 0.0)], faint);
            line(pt(-0.9, 0.0), pt(0.9, 0.0));
        }
        Icon::Icosa => {
            let hex: Vec<Pos2> = (0..6).map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 6.0 - std::f32::consts::FRAC_PI_2;
                pt(a.cos() * 0.98, a.sin() * 0.98)
            }).collect();
            poly(hex.clone(), faint);
            for i in [0usize, 2, 4] {
                line(c, hex[i]);
            }
        }
        Icon::Select => poly(vec![pt(-0.6, -0.9), pt(-0.6, 0.7), pt(-0.2, 0.3), pt(0.1, 0.95), pt(0.4, 0.8), pt(0.12, 0.2), pt(0.7, 0.2)], col),
        Icon::Move => {
            line(pt(-1.0, 0.0), pt(1.0, 0.0));
            line(pt(0.0, -1.0), pt(0.0, 1.0));
            for (dx, dy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let (px, py) = (-dy, dx);
                p.add(Shape::convex_polygon(
                    vec![pt(dx, dy), pt(dx * 0.6 + px * 0.28, dy * 0.6 + py * 0.28), pt(dx * 0.6 - px * 0.28, dy * 0.6 - py * 0.28)],
                    col, Stroke::NONE));
            }
        }
        Icon::Rotate => {
            let pts: Vec<Pos2> = (0..=18).map(|i| {
                let a = -0.4 + 4.9 * i as f32 / 18.0;
                pt(a.cos() * 0.8, a.sin() * 0.8)
            }).collect();
            let last = *pts.last().unwrap();
            p.add(Shape::line(pts, st));
            p.add(Shape::convex_polygon(vec![last + vec2(-5.0, -2.0), last + vec2(5.0, -3.0), last + vec2(1.0, 6.0)], col, Stroke::NONE));
        }
        Icon::Scale => {
            p.rect_stroke(Rect::from_min_max(pt(-1.0, -0.2), pt(0.2, 1.0)), 2.0, st, StrokeKind::Middle);
            line(pt(-0.1, 0.1), pt(0.95, -0.95));
            p.add(Shape::convex_polygon(vec![pt(1.0, -1.0), pt(0.35, -0.95), pt(0.95, -0.35)], col, Stroke::NONE));
        }
        Icon::Cube => {
            poly(vec![pt(0.0, -1.0), pt(0.9, -0.5), pt(0.0, 0.0), pt(-0.9, -0.5)], col.gamma_multiply(0.15));
            poly(vec![pt(-0.9, -0.5), pt(0.0, 0.0), pt(0.0, 1.0), pt(-0.9, 0.5)], faint);
            poly(vec![pt(0.9, -0.5), pt(0.0, 0.0), pt(0.0, 1.0), pt(0.9, 0.5)], col.gamma_multiply(0.5));
        }
        Icon::Skull => {
            p.circle(pt(0.0, -0.15), h * 0.78, faint, st);
            p.rect(Rect::from_min_max(pt(-0.42, 0.45), pt(0.42, 0.98)), 2.0, faint, st, StrokeKind::Middle);
            p.circle_filled(pt(-0.3, -0.2), h * 0.2, col);
            p.circle_filled(pt(0.3, -0.2), h * 0.2, col);
            line(pt(-0.14, 0.45), pt(-0.14, 0.95));
            line(pt(0.14, 0.45), pt(0.14, 0.95));
        }
        Icon::Tree => {
            p.rect_filled(Rect::from_min_max(pt(-0.14, 0.4), pt(0.14, 1.0)), 0.0, col);
            poly(vec![pt(0.0, -1.0), pt(0.7, 0.0), pt(-0.7, 0.0)], faint);
            poly(vec![pt(0.0, -0.5), pt(0.95, 0.55), pt(-0.95, 0.55)], faint);
        }
        Icon::Picture => {
            p.rect(Rect::from_min_max(pt(-1.0, -0.8), pt(1.0, 0.8)), 2.0, Color32::TRANSPARENT, st, StrokeKind::Middle);
            poly(vec![pt(-0.85, 0.65), pt(-0.3, -0.1), pt(0.1, 0.4), pt(0.4, 0.05), pt(0.85, 0.65)], faint);
            p.circle_filled(pt(0.5, -0.4), h * 0.15, col);
        }
        Icon::Checker => {
            let cell = |x: f32, y: f32, f: Color32| p.rect_filled(Rect::from_min_max(pt(x, y), pt(x + 0.5, y + 0.5)), 0.0, f);
            for i in 0..4 {
                for j in 0..4 {
                    if (i + j) % 2 == 0 {
                        cell(-1.0 + 0.5 * i as f32, -1.0 + 0.5 * j as f32, col.gamma_multiply(0.75));
                    }
                }
            }
            p.rect_stroke(Rect::from_min_max(pt(-1.0, -1.0), pt(1.0, 1.0)), 0.0, st, StrokeKind::Middle);
        }
        Icon::Gel => {
            poly(vec![pt(-0.35, -1.0), pt(0.35, -1.0), pt(0.2, -0.3), pt(-0.2, -0.3)], faint);
            for k in [-0.7f32, 0.0, 0.7] {
                line(pt(k * 0.25, -0.2), pt(k * 1.1, 0.95));
            }
        }
        Icon::Copy => {
            p.rect(Rect::from_min_max(pt(-0.95, -0.5), pt(0.35, 0.95)), 1.0, faint, st, StrokeKind::Middle);
            p.rect(Rect::from_min_max(pt(-0.35, -0.95), pt(0.95, 0.5)), 1.0, Color32::TRANSPARENT, st, StrokeKind::Middle);
        }
        Icon::Trash => {
            line(pt(-0.9, -0.55), pt(0.9, -0.55));
            line(pt(-0.3, -0.55), pt(-0.3, -0.85));
            line(pt(0.3, -0.55), pt(0.3, -0.85));
            line(pt(-0.3, -0.85), pt(0.3, -0.85));
            poly(vec![pt(-0.7, -0.4), pt(0.7, -0.4), pt(0.55, 0.95), pt(-0.55, 0.95)], faint);
            line(pt(-0.2, -0.1), pt(-0.15, 0.7));
            line(pt(0.2, -0.1), pt(0.15, 0.7));
        }
        Icon::Undo | Icon::Redo => {
            let s = if matches!(icon, Icon::Undo) { 1.0f32 } else { -1.0 };
            let pts: Vec<Pos2> = (0..=14).map(|i| {
                let a = -1.6 + 3.3 * i as f32 / 14.0;
                pt(s * (a.cos() * 0.7 - 0.05), a.sin() * 0.55 + 0.25)
            }).collect();
            p.add(Shape::line(pts, st));
            p.add(Shape::convex_polygon(vec![pt(s * -0.95, -0.3), pt(s * -0.1, -0.85), pt(s * -0.1, 0.25)], col, Stroke::NONE));
        }
        Icon::Home => {
            poly(vec![pt(-1.0, 0.0), pt(0.0, -0.95), pt(1.0, 0.0)], faint);
            poly(vec![pt(-0.7, 0.0), pt(0.7, 0.0), pt(0.7, 0.9), pt(-0.7, 0.9)], faint);
        }
        Icon::Render => {
            p.rect(Rect::from_min_max(pt(-1.0, -0.55), pt(1.0, 0.75)), 3.0, faint, st, StrokeKind::Middle);
            p.circle(pt(0.0, 0.1), h * 0.38, Color32::TRANSPARENT, st);
            p.rect_filled(Rect::from_min_max(pt(-0.45, -0.85), pt(0.0, -0.55)), 1.0, col);
        }
        Icon::Map => {
            poly(vec![pt(-1.0, -0.7), pt(-0.35, -0.9), pt(0.35, -0.7), pt(1.0, -0.9), pt(1.0, 0.8), pt(0.35, 1.0), pt(-0.35, 0.8), pt(-1.0, 1.0)], faint);
            line(pt(-0.35, -0.9), pt(-0.35, 0.8));
            line(pt(0.35, -0.7), pt(0.35, 1.0));
        }
        Icon::Sun => {
            p.circle(c, h * 0.42, faint, st);
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                line(pt(a.cos() * 0.68, a.sin() * 0.68), pt(a.cos() * 0.98, a.sin() * 0.98));
            }
        }
    }
}
