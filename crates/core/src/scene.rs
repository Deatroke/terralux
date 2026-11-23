//! Modelo de cena (serializável em RON). Convenção: Y para cima, metros.
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    /// Campo de visão vertical, em graus.
    pub fov_deg: f32,
    pub exposure: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sun {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub color: Vec3,
    pub intensity: f32,
}

impl Sun {
    /// Direção PARA o sol.
    pub fn direction(&self) -> Vec3 {
        let (az, el) = (self.azimuth_deg.to_radians(), self.elevation_deg.to_radians());
        Vec3::new(el.cos() * az.sin(), el.sin(), el.cos() * az.cos()).normalize()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clouds {
    pub altitude: f32,
    /// 0 = céu limpo, 1 = encoberto.
    pub coverage: f32,
    pub scale: f32,
    pub seed: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sky {
    pub zenith: Vec3,
    pub horizon: Vec3,
    /// Densidade da névoa de distância (1/m).
    pub haze: f32,
    pub clouds: Option<Clouds>,
    /// Densidade de estrelas (0 = nenhuma). Só aparecem quando o céu está escuro.
    #[serde(default)]
    pub stars: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Water {
    pub level: f32,
    pub wave_height: f32,
    pub wave_scale: f32,
    /// Absorção por canal (R,G,B) em 1/m: o vermelho some primeiro.
    pub absorption: Vec3,
    pub deep_color: Vec3,
}

/// De onde vem a forma do terreno.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum TerrainSource {
    /// Ruído procedural (ilha).
    #[default]
    Procedural,
    /// Mapa de altura em imagem (preto = baixo, branco = alto). Caminho `lib:...` ou absoluto.
    Image(String),
}

/// Tipo de relevo gerado (só vale para terreno procedural).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainKind {
    /// Ilha cercada de mar.
    #[default]
    Island,
    /// Cordilheira de cristas afiadas, com um lago ao centro.
    Mountains,
    /// Colinas suaves e planícies (bom para terrenos de jogo).
    Hills,
    /// Mar de dunas.
    Dunes,
    /// Planalto em degraus cortado por um cânion.
    Canyon,
    /// Cone vulcânico com cratera, cercado de mar.
    Volcano,
}

impl TerrainKind {
    pub const ALL: [TerrainKind; 6] =
        [TerrainKind::Island, TerrainKind::Mountains, TerrainKind::Hills, TerrainKind::Dunes, TerrainKind::Canyon, TerrainKind::Volcano];

    pub fn name(self) -> &'static str {
        match self {
            TerrainKind::Island => "Ilha",
            TerrainKind::Mountains => "Montanhas",
            TerrainKind::Hills => "Colinas e planícies",
            TerrainKind::Dunes => "Dunas",
            TerrainKind::Canyon => "Cânion",
            TerrainKind::Volcano => "Vulcão",
        }
    }

    /// Identificador estável para a linha de comando.
    pub fn id(self) -> &'static str {
        match self {
            TerrainKind::Island => "island",
            TerrainKind::Mountains => "mountains",
            TerrainKind::Hills => "hills",
            TerrainKind::Dunes => "dunes",
            TerrainKind::Canyon => "canyon",
            TerrainKind::Volcano => "volcano",
        }
    }

    /// Nome do controle de "raio" para este relevo.
    pub fn radius_label(self) -> &'static str {
        match self {
            TerrainKind::Island => "Raio da ilha",
            TerrainKind::Mountains => "Raio do lago",
            TerrainKind::Volcano => "Raio do vulcão",
            TerrainKind::Canyon => "Largura do cânion",
            TerrainKind::Hills | TerrainKind::Dunes => "Escala das formas",
        }
    }

    /// O relevo termina em mar (a ilha e o vulcão) ou tem bordas de terra (os demais).
    pub fn has_sea_edge(self) -> bool {
        matches!(self, TerrainKind::Island | TerrainKind::Volcano)
    }
}

/// Paleta de cores do terreno procedural.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    /// Rocha, grama e areia (a paleta original da ilha).
    #[default]
    Tropical,
    /// Rocha cinza, grama baixa e neve nos cumes.
    Alpine,
    Desert,
    Snow,
    /// Rocha avermelhada em camadas.
    RedRock,
    /// Basalto escuro e cinza.
    Volcanic,
    Grassland,
}

impl Biome {
    pub const ALL: [Biome; 7] = [Biome::Tropical, Biome::Alpine, Biome::Desert, Biome::Snow, Biome::RedRock, Biome::Volcanic, Biome::Grassland];

    pub fn name(self) -> &'static str {
        match self {
            Biome::Tropical => "Tropical",
            Biome::Alpine => "Alpino",
            Biome::Desert => "Deserto",
            Biome::Snow => "Neve",
            Biome::RedRock => "Rocha vermelha",
            Biome::Volcanic => "Vulcânico",
            Biome::Grassland => "Pradaria",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Biome::Tropical => "tropical",
            Biome::Alpine => "alpine",
            Biome::Desert => "desert",
            Biome::Snow => "snow",
            Biome::RedRock => "redrock",
            Biome::Volcanic => "volcanic",
            Biome::Grassland => "grassland",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainDef {
    #[serde(default)]
    pub source: TerrainSource,
    /// Tipo de relevo procedural.
    #[serde(default)]
    pub kind: TerrainKind,
    /// Paleta de cores.
    #[serde(default)]
    pub biome: Biome,
    /// Meia largura do terreno quadrado (m).
    pub half_size: f32,
    /// Resolução da grade (n x n).
    pub resolution: usize,
    pub seed: u32,
    /// Textura que substitui as cores procedurais do terreno.
    #[serde(default)]
    pub texture: Option<TextureRef>,
    /// Altura máxima do pico acima do nível zero.
    pub peak_height: f32,
    /// Profundidade do fundo do mar nas bordas.
    pub sea_floor: f32,
    /// Raio aproximado da ilha.
    pub island_radius: f32,
    /// Frequência base do ruído (1/m).
    pub frequency: f32,
    pub octaves: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Polyhedron {
    Tetrahedron,
    Octahedron,
    Icosahedron,
}

impl Polyhedron {
    pub const ALL: [Polyhedron; 3] = [Polyhedron::Tetrahedron, Polyhedron::Octahedron, Polyhedron::Icosahedron];

    pub fn name(self) -> &'static str {
        match self {
            Polyhedron::Tetrahedron => "Tetraedro",
            Polyhedron::Octahedron => "Octaedro",
            Polyhedron::Icosahedron => "Icosaedro",
        }
    }

    /// Vértices (não normalizados) e faces triangulares do poliedro.
    pub fn mesh(self) -> (Vec<Vec3>, Vec<[usize; 3]>) {
        match self {
            Polyhedron::Tetrahedron => (
                vec![Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, -1.0, -1.0), Vec3::new(-1.0, 1.0, -1.0), Vec3::new(-1.0, -1.0, 1.0)],
                vec![[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
            ),
            Polyhedron::Octahedron => (
                vec![Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z],
                vec![[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]],
            ),
            Polyhedron::Icosahedron => {
                let t = (1.0 + 5.0f32.sqrt()) / 2.0;
                let v = vec![
                    Vec3::new(-1.0, t, 0.0), Vec3::new(1.0, t, 0.0), Vec3::new(-1.0, -t, 0.0), Vec3::new(1.0, -t, 0.0),
                    Vec3::new(0.0, -1.0, t), Vec3::new(0.0, 1.0, t), Vec3::new(0.0, -1.0, -t), Vec3::new(0.0, 1.0, -t),
                    Vec3::new(t, 0.0, -1.0), Vec3::new(t, 0.0, 1.0), Vec3::new(-t, 0.0, -1.0), Vec3::new(-t, 0.0, 1.0),
                ];
                let f = vec![
                    [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
                    [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
                ];
                (v, f)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Sphere,
    Poly(Polyhedron),
    Prim(crate::mesh::Primitive),
    /// Modelo importado (OBJ/DXF): caminho `lib:...` ou absoluto.
    Model(String),
    /// Plano infinito horizontal na altura `position.y`.
    Plane,
    /// Imagem com recorte por alfa, em pé (largura = 2 × size). Caminho `lib:...` ou absoluto.
    Picture(String),
}

impl Shape {
    pub fn label(&self) -> String {
        match self {
            Shape::Sphere => "Esfera".into(),
            Shape::Poly(p) => p.name().into(),
            Shape::Prim(p) => p.name().into(),
            Shape::Model(p) => std::path::Path::new(p).file_stem().map_or("Modelo".into(), |s| s.to_string_lossy().into_owned()),
            Shape::Plane => "Plano".into(),
            Shape::Picture(p) => std::path::Path::new(p).file_stem().map_or("Imagem".into(), |s| s.to_string_lossy().into_owned()),
        }
    }
}

/// Como a imagem é projetada sobre a superfície.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mapping {
    /// De cima para baixo (XZ do mundo/objeto).
    #[default]
    Planar,
    /// Caixa: projeta pelo eixo dominante da normal local.
    Box,
    /// Longitude/latitude em torno do centro do objeto.
    Spherical,
}

impl Mapping {
    pub const ALL: [Mapping; 3] = [Mapping::Planar, Mapping::Box, Mapping::Spherical];
    pub fn name(self) -> &'static str {
        match self {
            Mapping::Planar => "Planar",
            Mapping::Box => "Caixa",
            Mapping::Spherical => "Esférico",
        }
    }
}

/// Textura de imagem (arquivo da biblioteca ou caminho absoluto).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextureRef {
    pub path: String,
    /// Metros por repetição da imagem (terreno/plano/planar) ou escala relativa ao objeto.
    pub scale: f32,
    pub mapping: Mapping,
}

impl TextureRef {
    pub fn new(path: &str, scale: f32, mapping: Mapping) -> Self {
        TextureRef { path: path.into(), scale, mapping }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    #[serde(default)]
    pub texture: Option<TextureRef>,
    pub color: Vec3,
    pub reflectivity: f32,
    pub specular: f32,
    pub shininess: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub name: String,
    pub shape: Shape,
    pub position: Vec3,
    /// Rotação em graus (XYZ).
    pub rotation_deg: Vec3,
    /// Raio do envelope esférico (m).
    pub size: f32,
    pub material: Material,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub camera: Camera,
    pub sun: Sun,
    pub sky: Sky,
    pub water: Option<Water>,
    pub terrain: Option<TerrainDef>,
    pub objects: Vec<Object>,
}

impl TerrainDef {
    /// Meia-largura da região que vale a pena mostrar (editor e wireframe).
    pub fn view_radius(&self) -> f32 {
        match self.source {
            TerrainSource::Procedural if self.kind.has_sea_edge() => (self.island_radius * 1.9).min(self.half_size),
            TerrainSource::Procedural => self.half_size,
            TerrainSource::Image(_) => self.half_size,
        }
    }

    /// Terreno a partir de um mapa de altura em imagem.
    pub fn from_image(path: &str) -> Self {
        TerrainDef { source: TerrainSource::Image(path.into()), half_size: 120.0, resolution: 1024, peak_height: 40.0, sea_floor: 3.0, ..Self::island() }
    }

    pub fn island() -> Self {
        TerrainDef { source: TerrainSource::Procedural, kind: TerrainKind::Island, biome: Biome::Tropical, texture: None, half_size: 220.0, resolution: 2048, seed: 11, peak_height: 21.0, sea_floor: 14.0, island_radius: 56.0, frequency: 0.022, octaves: 8 }
    }
}

impl Water {
    pub fn sea() -> Self {
        Water { level: 0.0, wave_height: 0.35, wave_scale: 1.0, absorption: Vec3::new(0.50, 0.11, 0.06), deep_color: Vec3::new(0.005, 0.045, 0.10) }
    }
}

impl Clouds {
    pub fn fair() -> Self {
        Clouds { altitude: 700.0, coverage: 0.55, scale: 1.0, seed: 7 }
    }
}

impl Material {
    /// Plástico fosco: o padrão para tudo que vem da biblioteca.
    pub fn matte(color: Vec3) -> Self {
        Material { texture: None, color, reflectivity: 0.0, specular: 0.12, shininess: 20.0 }
    }

    pub fn metal(color: Vec3) -> Self {
        Material { texture: None, color, reflectivity: 0.75, specular: 1.0, shininess: 120.0 }
    }
}

impl Scene {
    /// Cena vazia: só céu.
    pub fn empty() -> Self {
        let mut s = Scene::demo();
        s.water = None;
        s.terrain = None;
        s.objects.clear();
        s.sky.clouds = None;
        s
    }

    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).expect("serializar cena")
    }

    /// Cena de demonstração: céu com sol e névoa, água refletiva,
    /// ilha rochosa (fBm + ridged) e um poliedro flutuando.
    pub fn demo() -> Self {
        Scene {
            camera: Camera {
                position: Vec3::new(-16.0, 8.0, -125.0),
                target: Vec3::new(2.0, 17.0, 0.0),
                fov_deg: 42.0,
                exposure: 1.0,
            },
            sun: Sun { azimuth_deg: -75.0, elevation_deg: 23.0, color: Vec3::new(1.0, 0.86, 0.68), intensity: 2.2 },
            sky: Sky {
                zenith: Vec3::new(0.10, 0.28, 0.70),
                horizon: Vec3::new(0.78, 0.84, 0.92),
                haze: 0.0011,
                clouds: Some(Clouds { altitude: 700.0, coverage: 0.55, scale: 1.0, seed: 7 }),
                stars: 0.0,
            },
            water: Some(Water {
                level: 0.0,
                wave_height: 0.35,
                wave_scale: 1.0,
                absorption: Vec3::new(0.50, 0.11, 0.06),
                deep_color: Vec3::new(0.005, 0.045, 0.10),
            }),
            terrain: Some(TerrainDef {
                source: TerrainSource::Procedural,
                kind: TerrainKind::Island,
                biome: Biome::Tropical,
                texture: None,
                half_size: 220.0,
                resolution: 2048,
                seed: 11,
                peak_height: 21.0,
                sea_floor: 14.0,
                island_radius: 56.0,
                frequency: 0.022,
                octaves: 8,
            }),
            objects: vec![Object {
                name: "poliedro".into(),
                shape: Shape::Poly(Polyhedron::Icosahedron),
                position: Vec3::new(10.0, 40.0, -12.0),
                rotation_deg: Vec3::new(20.0, 35.0, 12.0),
                size: 4.5,
                material: Material {
                    texture: None,
                    color: Vec3::new(1.0, 0.78, 0.36),
                    reflectivity: 0.75,
                    specular: 1.0,
                    shininess: 120.0,
                },
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_roundtrip() {
        let s = Scene::demo();
        assert_eq!(Scene::from_ron(&s.to_ron()).unwrap(), s);
        let e = Scene::empty();
        assert_eq!(Scene::from_ron(&e.to_ron()).unwrap(), e);
    }

    #[test]
    fn polyhedra_are_closed() {
        for p in Polyhedron::ALL {
            let (v, f) = p.mesh();
            // Euler: V - E + F = 2 (E = 3F/2 em malhas triangulares fechadas)
            assert_eq!(v.len() as i32 - (3 * f.len() / 2) as i32 + f.len() as i32, 2, "{}", p.name());
        }
    }
}
