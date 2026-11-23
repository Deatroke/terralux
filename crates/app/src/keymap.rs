//! Atalhos no estilo do Blender e o modo modal G/R/S (mover, girar, escalar com eixo e número).
//! Tudo aqui é função pura (testável sem janela); a ligação com a interface fica em `app.rs`.
use egui::{Key, Modifiers, Vec2};
use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, PI};
use terralux_core::Object;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn vec(self) -> Vec3 {
        match self {
            Axis::X => Vec3::X,
            Axis::Y => Vec3::Y,
            Axis::Z => Vec3::Z,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Axis::X => "X",
            Axis::Y => "Y",
            Axis::Z => "Z",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModalKind {
    Grab,
    Rotate,
    Scale,
}

impl ModalKind {
    pub fn name(self) -> &'static str {
        match self {
            ModalKind::Grab => "Mover",
            ModalKind::Rotate => "Girar",
            ModalKind::Scale => "Escalar",
        }
    }
}

/// Ações de teclado fora do modo modal.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    Modal(ModalKind),
    Delete,
    Duplicate,
    DuplicateGrab,
    Deselect,
    AddMenu,
    ToggleInspector,
    ToggleSide,
    ToggleMode,
    CycleDisplay,
    Render,
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    /// (yaw, pitch) da câmera orbital.
    View(f32, f32),
    FrameSelected,
    FrameAll,
    Help,
}

/// Tecla (com modificadores) → ação. `Ctrl` é `command` (Cmd no macOS).
pub fn map_key(key: Key, m: Modifiers) -> Option<Action> {
    use Action::*;
    let ctrl = m.command;
    let (shift, alt) = (m.shift, m.alt);
    Some(match (key, ctrl, shift, alt) {
        (Key::N, true, false, false) => New,
        (Key::O, true, false, false) => Open,
        (Key::S, true, false, false) => Save,
        (Key::S, true, true, false) => SaveAs,
        (Key::Z, true, false, false) => Undo,
        (Key::Z, true, true, false) | (Key::Y, true, false, false) => Redo,
        (Key::D, true, false, false) => Duplicate,
        (Key::D, false, true, false) => DuplicateGrab,
        (Key::A, false, false, true) => Deselect,
        (Key::A, false, true, false) => AddMenu,
        (Key::G, false, false, false) => Modal(ModalKind::Grab),
        (Key::R, false, false, false) => Modal(ModalKind::Rotate),
        (Key::S, false, false, false) => Modal(ModalKind::Scale),
        (Key::X, false, false, false) | (Key::Delete, false, false, false) => Delete,
        (Key::N, false, false, false) => ToggleInspector,
        (Key::T, false, false, false) => ToggleSide,
        (Key::Tab, false, false, false) => ToggleMode,
        (Key::Z, false, false, false) => CycleDisplay,
        (Key::F12, ..) | (Key::F9, ..) => Render,
        (Key::F1, ..) => Help,
        (Key::Period, false, false, false) => FrameSelected,
        (Key::Home, false, false, false) => FrameAll,
        // Vistas: 1 frente, 3 direita, 7 topo; com Ctrl, o lado oposto (como no teclado numérico do Blender).
        (Key::Num1, false, false, false) => View(PI, 0.0),
        (Key::Num1, true, false, false) => View(0.0, 0.0),
        (Key::Num3, false, false, false) => View(FRAC_PI_2, 0.0),
        (Key::Num3, true, false, false) => View(-FRAC_PI_2, 0.0),
        (Key::Num7, false, false, false) => View(PI, 1.52),
        (Key::Num7, true, false, false) => View(PI, -1.52),
        (Key::Num0, false, false, false) => View(PI + 0.14, 0.18),
        _ => return None,
    })
}

/// O que uma tecla significa enquanto G/R/S está ativo.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ModalInput {
    Axis(Axis),
    Char(char),
    Backspace,
    Confirm,
    Cancel,
}

pub fn modal_key(key: Key) -> Option<ModalInput> {
    Some(match key {
        Key::X => ModalInput::Axis(Axis::X),
        Key::Y => ModalInput::Axis(Axis::Y),
        Key::Z => ModalInput::Axis(Axis::Z),
        Key::Enter => ModalInput::Confirm,
        Key::Escape => ModalInput::Cancel,
        Key::Backspace => ModalInput::Backspace,
        Key::Period => ModalInput::Char('.'),
        Key::Minus => ModalInput::Char('-'),
        Key::Num0 => ModalInput::Char('0'),
        Key::Num1 => ModalInput::Char('1'),
        Key::Num2 => ModalInput::Char('2'),
        Key::Num3 => ModalInput::Char('3'),
        Key::Num4 => ModalInput::Char('4'),
        Key::Num5 => ModalInput::Char('5'),
        Key::Num6 => ModalInput::Char('6'),
        Key::Num7 => ModalInput::Char('7'),
        Key::Num8 => ModalInput::Char('8'),
        Key::Num9 => ModalInput::Char('9'),
        _ => return None,
    })
}

/// Aplica uma tecla ao texto numérico digitado (aceita um único '.', e '-' só no começo; '-' de novo inverte o sinal).
pub fn push_char(typed: &mut String, c: char) {
    match c {
        '-' => {
            if typed.starts_with('-') {
                typed.remove(0);
            } else {
                typed.insert(0, '-');
            }
        }
        '.' => {
            if !typed.contains('.') {
                if typed.is_empty() || typed == "-" {
                    typed.push('0');
                }
                typed.push('.');
            }
        }
        d if d.is_ascii_digit() => typed.push(d),
        _ => {}
    }
}

pub fn parse_typed(typed: &str) -> Option<f32> {
    typed.trim_end_matches('.').parse().ok()
}

/// Entrada do cálculo de transformação.
pub struct TransformIn {
    pub kind: ModalKind,
    pub axis: Option<Axis>,
    pub typed: Option<f32>,
    /// Deslocamento do mouse desde o início, em pixels.
    pub mouse: Vec2,
    /// Direção do eixo da câmera (mundo) para a tela: (right, up) e metros por pixel.
    pub right: Vec3,
    pub up: Vec3,
    pub m_per_px: f32,
}

/// Direção do eixo na tela (x para a direita, y para baixo), normalizada; `None` se ele aponta para a câmera.
fn screen_dir(axis: Axis, right: Vec3, up: Vec3) -> Option<Vec2> {
    let v = Vec2::new(axis.vec().dot(right), -axis.vec().dot(up));
    (v.length() > 0.12).then(|| v.normalized())
}

/// Posição/rotação/tamanho resultantes a partir do objeto original `base`.
pub fn transform(t: &TransformIn, base: &Object) -> Object {
    let mut o = base.clone();
    match t.kind {
        ModalKind::Grab => {
            let delta = if let Some(n) = t.typed {
                t.axis.unwrap_or(Axis::X).vec() * n
            } else if let Some(a) = t.axis {
                match screen_dir(a, t.right, t.up) {
                    Some(d) => a.vec() * (t.mouse.x * d.x + t.mouse.y * d.y) * t.m_per_px,
                    None => Vec3::ZERO,
                }
            } else {
                (t.right * t.mouse.x - t.up * t.mouse.y) * t.m_per_px
            };
            o.position += delta;
        }
        ModalKind::Rotate => {
            let deg = t.typed.unwrap_or(t.mouse.x * 0.5);
            match t.axis.unwrap_or(Axis::Y) {
                Axis::X => o.rotation_deg.x += deg,
                Axis::Y => o.rotation_deg.y += deg,
                Axis::Z => o.rotation_deg.z += deg,
            }
        }
        ModalKind::Scale => {
            let f = t.typed.unwrap_or_else(|| (t.mouse.x * 0.005).exp());
            o.size = (base.size * f).clamp(0.05, 4000.0);
        }
    }
    o
}

/// Linha de status do modo modal (como o cabeçalho do Blender).
pub fn modal_status(kind: ModalKind, axis: Option<Axis>, typed: &str) -> String {
    let eixo = axis.map_or(String::new(), |a| format!("  ·  eixo {}", a.name()));
    let valor = if typed.is_empty() { String::new() } else { format!("  ·  valor {typed}") };
    format!("{}{}{}   —   Enter/clique confirma · Esc/botão direito cancela · X Y Z fixa o eixo · digite um número", kind.name(), eixo, valor)
}

/// Texto da cola de atalhos (F1).
pub const CHEATSHEET: &[(&str, &[(&str, &str)])] = &[
    ("Navegação", &[
        ("Botão do meio", "Orbitar"),
        ("Shift + botão do meio", "Deslocar"),
        ("Ctrl + botão do meio · roda", "Zoom"),
        ("Alt + botão esquerdo", "Orbitar (touchpad)"),
        ("1 / 3 / 7", "Frente / Direita / Topo"),
        ("Ctrl + 1 / 3 / 7", "Trás / Esquerda / Baixo"),
        ("0", "Vista em perspectiva"),
        (".", "Enquadrar a seleção"),
        ("Home", "Enquadrar a cena toda"),
    ]),
    ("Objetos", &[
        ("G", "Mover (X Y Z fixa o eixo; digite o valor)"),
        ("R", "Girar"),
        ("S", "Escalar"),
        ("Shift + A", "Adicionar objeto"),
        ("Shift + D", "Duplicar e mover"),
        ("Ctrl + D", "Duplicar"),
        ("X · Delete", "Excluir"),
        ("Alt + A", "Desselecionar"),
        ("Clique", "Selecionar"),
    ]),
    ("Janela", &[
        ("N", "Mostrar/ocultar o painel de propriedades"),
        ("T", "Mostrar/ocultar a coluna de vistas"),
        ("Z", "Alternar Pré-visualização / Wireframe / Ambos"),
        ("Tab", "Alternar modo Simples / Avançado"),
        ("F1", "Esta ajuda"),
    ]),
    ("Arquivo e render", &[
        ("Ctrl + N / O / S", "Novo / Abrir / Salvar"),
        ("Ctrl + Shift + S", "Salvar como"),
        ("Ctrl + Z / Ctrl + Shift + Z", "Desfazer / Refazer"),
        ("F12 · F9", "Renderizar imagem"),
    ]),
];

#[cfg(test)]
mod tests {
    use super::*;
    use terralux_core::{Material, Shape};

    fn obj() -> Object {
        Object { name: "o".into(), shape: Shape::Sphere, position: Vec3::new(1.0, 2.0, 3.0), rotation_deg: Vec3::ZERO, size: 2.0, material: Material::matte(Vec3::ONE) }
    }

    fn base_in(kind: ModalKind) -> TransformIn {
        TransformIn { kind, axis: None, typed: None, mouse: Vec2::ZERO, right: Vec3::X, up: Vec3::Y, m_per_px: 0.1 }
    }

    #[test]
    fn keys_map_like_blender() {
        let none = Modifiers::NONE;
        assert_eq!(map_key(Key::G, none), Some(Action::Modal(ModalKind::Grab)));
        assert_eq!(map_key(Key::R, none), Some(Action::Modal(ModalKind::Rotate)));
        assert_eq!(map_key(Key::S, none), Some(Action::Modal(ModalKind::Scale)));
        assert_eq!(map_key(Key::S, Modifiers::COMMAND), Some(Action::Save));
        assert_eq!(map_key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT), Some(Action::SaveAs));
        assert_eq!(map_key(Key::D, Modifiers::SHIFT), Some(Action::DuplicateGrab));
        assert_eq!(map_key(Key::A, Modifiers::ALT), Some(Action::Deselect));
        assert_eq!(map_key(Key::A, Modifiers::SHIFT), Some(Action::AddMenu));
        assert_eq!(map_key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT), Some(Action::Redo));
        assert_eq!(map_key(Key::X, none), Some(Action::Delete));
        assert_eq!(map_key(Key::F12, none), Some(Action::Render));
        assert_eq!(map_key(Key::Num7, none), Some(Action::View(PI, 1.52)));
        assert_eq!(map_key(Key::Num7, Modifiers::COMMAND), Some(Action::View(PI, -1.52)));
        assert_eq!(map_key(Key::Q, none), None);
    }

    #[test]
    fn typed_number_building() {
        let mut s = String::new();
        for c in ['-', '.', '5', '.', '2'] {
            push_char(&mut s, c);
        }
        assert_eq!(s, "-0.52");
        assert_eq!(parse_typed(&s), Some(-0.52));
        push_char(&mut s, '-');
        assert_eq!(s, "0.52");
        assert_eq!(parse_typed("3."), Some(3.0));
        assert_eq!(parse_typed(""), None);
    }

    #[test]
    fn grab_typed_moves_along_axis() {
        let mut t = base_in(ModalKind::Grab);
        t.axis = Some(Axis::Z);
        t.typed = Some(5.0);
        assert_eq!(transform(&t, &obj()).position, Vec3::new(1.0, 2.0, 8.0));
        t.axis = None;
        assert_eq!(transform(&t, &obj()).position, Vec3::new(6.0, 2.0, 3.0), "sem eixo, o número vale para X");
    }

    #[test]
    fn grab_mouse_follows_screen_axis() {
        let mut t = base_in(ModalKind::Grab);
        t.mouse = Vec2::new(50.0, 30.0);
        // livre: plano da câmera (direita +x, cima -y na tela)
        let p = transform(&t, &obj()).position;
        assert!((p - Vec3::new(1.0 + 5.0, 2.0 - 3.0, 3.0)).length() < 1e-4, "{p:?}");
        // preso a X: só a componente do mouse ao longo do eixo na tela
        t.axis = Some(Axis::X);
        let p = transform(&t, &obj()).position;
        assert!((p - Vec3::new(6.0, 2.0, 3.0)).length() < 1e-4, "{p:?}");
        // eixo Z aponta para a câmera: não move
        t.axis = Some(Axis::Z);
        assert_eq!(transform(&t, &obj()).position, obj().position);
    }

    #[test]
    fn rotate_and_scale() {
        let mut t = base_in(ModalKind::Rotate);
        t.typed = Some(90.0);
        assert_eq!(transform(&t, &obj()).rotation_deg, Vec3::new(0.0, 90.0, 0.0));
        t.axis = Some(Axis::X);
        assert_eq!(transform(&t, &obj()).rotation_deg, Vec3::new(90.0, 0.0, 0.0));
        let mut s = base_in(ModalKind::Scale);
        s.typed = Some(2.5);
        assert_eq!(transform(&s, &obj()).size, 5.0);
        s.typed = None;
        s.mouse = Vec2::ZERO;
        assert_eq!(transform(&s, &obj()).size, 2.0, "sem movimento, o tamanho não muda");
    }
}
