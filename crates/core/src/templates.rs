//! Modelos prontos de cena: cenários para render e terrenos para jogos.
use crate::scene::*;
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateGroup {
    /// Cenas completas (céu, água, relevo) para renderizar.
    Scenery,
    /// Terrenos sem água nem enfeites, em resolução amigável para engines (2ⁿ+1).
    Game,
}

#[derive(Clone, Copy, Debug)]
pub struct TemplateInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub group: TemplateGroup,
}

const fn t(id: &'static str, name: &'static str, desc: &'static str, group: TemplateGroup) -> TemplateInfo {
    TemplateInfo { id, name, desc, group }
}

use TemplateGroup::{Game, Scenery};

pub const TEMPLATES: &[TemplateInfo] = &[
    t("vazio", "Cena vazia", "Só o céu. Comece do zero.", Scenery),
    t("ilha-tropical", "Ilha tropical", "Ilha rochosa com praia, mar e nuvens.", Scenery),
    t("montanhas-lago", "Montanhas e lago", "Cordilheira alpina com um lago no vale.", Scenery),
    t("deserto-dunas", "Deserto de dunas", "Mar de areia sob sol forte.", Scenery),
    t("vale-nevado", "Vale nevado", "Picos cobertos de neve em dia encoberto.", Scenery),
    t("canion-por-do-sol", "Cânion ao pôr do sol", "Planalto vermelho em degraus cortado por um rio.", Scenery),
    t("vulcao", "Vulcão", "Cone vulcânico com cratera numa ilha negra.", Scenery),
    t("planeta-espaco", "Planeta no espaço", "Planeta e lua num céu de estrelas.", Scenery),
    t("noite-estrelada", "Noite estrelada", "Colinas e lagos sob a lua.", Scenery),
    t("oceano-aberto", "Oceano aberto", "Só mar e céu, com ondas altas.", Scenery),
    t("planicie", "Planície verde", "Campo aberto com colinas suaves.", Scenery),
    t("jogo-colinas", "Colinas (jogo)", "Terreno de colinas, 1025², sem água.", Game),
    t("jogo-montanhas", "Montanhas (jogo)", "Cordilheira alpina, 1025², sem água.", Game),
    t("jogo-deserto", "Deserto (jogo)", "Dunas, 1025², sem água.", Game),
    t("jogo-canion", "Cânion (jogo)", "Planalto em degraus com rio seco, 1025².", Game),
    t("jogo-ilha", "Ilha (jogo)", "Ilha cercada de mar, 1025².", Game),
    t("jogo-arena", "Arena plana (jogo)", "Terreno quase plano para arenas e mapas pequenos, 513².", Game),
];

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

#[allow(clippy::too_many_arguments)]
fn terrain(kind: TerrainKind, biome: Biome, half: f32, res: usize, seed: u32, peak: f32, sea: f32, radius: f32, freq: f32) -> TerrainDef {
    TerrainDef {
        source: TerrainSource::Procedural,
        kind,
        biome,
        texture: None,
        half_size: half,
        resolution: res,
        seed,
        peak_height: peak,
        sea_floor: sea,
        island_radius: radius,
        frequency: freq,
        octaves: 8,
    }
}

fn clouds(coverage: f32, seed: u32) -> Option<Clouds> {
    Some(Clouds { altitude: 700.0, coverage, scale: 1.0, seed })
}

fn day_sky(cov: f32) -> Sky {
    Sky { zenith: v(0.10, 0.28, 0.70), horizon: v(0.78, 0.84, 0.92), haze: 0.0011, clouds: clouds(cov, 7), stars: 0.0 }
}

fn cam(pos: Vec3, target: Vec3, fov: f32) -> Camera {
    Camera { position: pos, target, fov_deg: fov, exposure: 1.0 }
}

fn sun(az: f32, el: f32, color: Vec3, intensity: f32) -> Sun {
    Sun { azimuth_deg: az, elevation_deg: el, color, intensity }
}

fn plain(camera: Camera, sun: Sun, sky: Sky, water: Option<Water>, terrain: Option<TerrainDef>) -> Scene {
    Scene { camera, sun, sky, water, terrain, objects: vec![] }
}

/// Câmera aérea oblíqua que enquadra um terreno de jogo.
fn aerial(half: f32, peak: f32) -> Camera {
    cam(v(0.0, peak * 0.9 + half * 0.42, -half * 0.95), v(0.0, peak * 0.1, half * 0.02), 46.0)
}

fn game_scene(def: TerrainDef) -> Scene {
    let camera = aerial(def.half_size, def.peak_height);
    plain(camera, sun(-50.0, 38.0, v(1.0, 0.95, 0.85), 2.3), Sky { haze: 0.00012, ..day_sky(0.25) }, None, Some(def))
}

impl Scene {
    /// Cena pronta pelo identificador (veja [`TEMPLATES`]).
    pub fn template(id: &str) -> Option<Scene> {
        Some(match id {
            "vazio" => Scene::empty(),
            "ilha-tropical" => {
                let mut s = Scene::demo();
                s.objects.clear();
                s
            }
            "montanhas-lago" => plain(
                cam(v(0.0, 150.0, -330.0), v(0.0, 40.0, 0.0), 44.0),
                sun(-55.0, 26.0, v(1.0, 0.90, 0.76), 2.3),
                Sky { haze: 0.0004, ..day_sky(0.45) },
                Some(Water { level: 0.0, wave_height: 0.12, wave_scale: 1.0, absorption: v(0.40, 0.12, 0.08), deep_color: v(0.01, 0.07, 0.10) }),
                Some(terrain(TerrainKind::Mountains, Biome::Alpine, 420.0, 2048, 23, 120.0, 12.0, 110.0, 0.011)),
            ),
            "deserto-dunas" => plain(
                cam(v(0.0, 24.0, -170.0), v(0.0, 6.0, 0.0), 48.0),
                sun(35.0, 24.0, v(1.0, 0.92, 0.78), 2.0),
                Sky { zenith: v(0.16, 0.38, 0.80), horizon: v(0.94, 0.84, 0.70), haze: 0.0009, clouds: clouds(0.12, 31), stars: 0.0 },
                None,
                Some(terrain(TerrainKind::Dunes, Biome::Desert, 300.0, 2048, 5, 22.0, 6.0, 56.0, 0.022)),
            ),
            "vale-nevado" => plain(
                cam(v(0.0, 170.0, -330.0), v(0.0, 50.0, 0.0), 44.0),
                sun(-30.0, 15.0, v(1.0, 0.95, 0.90), 1.9),
                Sky { zenith: v(0.34, 0.46, 0.64), horizon: v(0.80, 0.85, 0.90), haze: 0.0007, clouds: clouds(0.8, 19), stars: 0.0 },
                Some(Water { level: 0.0, wave_height: 0.04, wave_scale: 1.0, absorption: v(0.40, 0.12, 0.08), deep_color: v(0.02, 0.06, 0.09) }),
                Some(terrain(TerrainKind::Mountains, Biome::Snow, 420.0, 2048, 41, 130.0, 12.0, 100.0, 0.011)),
            ),
            "canion-por-do-sol" => plain(
                cam(v(-60.0, 70.0, -300.0), v(0.0, 25.0, 0.0), 44.0),
                sun(15.0, 8.0, v(1.0, 0.50, 0.28), 2.3),
                Sky { zenith: v(0.12, 0.10, 0.40), horizon: v(1.0, 0.50, 0.42), haze: 0.0006, clouds: clouds(0.4, 12), stars: 0.0 },
                Some(Water { level: 0.0, wave_height: 0.06, wave_scale: 1.0, absorption: v(0.30, 0.14, 0.10), deep_color: v(0.08, 0.05, 0.05) }),
                Some(terrain(TerrainKind::Canyon, Biome::RedRock, 400.0, 2048, 8, 70.0, 14.0, 60.0, 0.007)),
            ),
            "vulcao" => plain(
                cam(v(0.0, 75.0, -190.0), v(0.0, 38.0, 0.0), 46.0),
                sun(-50.0, 14.0, v(1.0, 0.55, 0.30), 2.0),
                Sky { zenith: v(0.14, 0.12, 0.20), horizon: v(0.82, 0.46, 0.34), haze: 0.0009, clouds: clouds(0.65, 44), stars: 0.0 },
                Some(Water::sea()),
                Some(terrain(TerrainKind::Volcano, Biome::Volcanic, 300.0, 2048, 3, 80.0, 16.0, 78.0, 0.02)),
            ),
            "planeta-espaco" => Scene {
                camera: cam(v(0.0, 0.0, 0.0), v(-20.0, 8.0, 100.0), 55.0),
                sun: sun(125.0, 22.0, v(1.0, 0.97, 0.92), 3.0),
                sky: Sky { zenith: v(0.0, 0.0, 0.004), horizon: v(0.0, 0.0, 0.006), haze: 0.0, clouds: None, stars: 1.6 },
                water: None,
                terrain: None,
                objects: vec![
                    Object {
                        name: "planeta".into(),
                        shape: Shape::Sphere,
                        position: v(-35.0, 14.0, 330.0),
                        rotation_deg: Vec3::ZERO,
                        size: 130.0,
                        material: Material { texture: None, color: v(0.10, 0.30, 0.62), reflectivity: 0.0, specular: 0.5, shininess: 40.0 },
                    },
                    Object {
                        name: "lua".into(),
                        shape: Shape::Sphere,
                        position: v(60.0, 50.0, 240.0),
                        rotation_deg: Vec3::ZERO,
                        size: 18.0,
                        material: Material::matte(v(0.62, 0.60, 0.58)),
                    },
                ],
            },
            "noite-estrelada" => {
                let mut s = plain(
                    cam(v(0.0, 14.0, -150.0), v(0.0, 22.0, 0.0), 50.0),
                    sun(15.0, 15.0, v(0.60, 0.72, 1.0), 0.32),
                    Sky { zenith: v(0.004, 0.010, 0.040), horizon: v(0.040, 0.075, 0.160), haze: 0.0007, clouds: clouds(0.25, 66), stars: 1.5 },
                    Some(Water { level: 6.0, wave_height: 0.08, wave_scale: 1.0, absorption: v(0.45, 0.12, 0.08), deep_color: v(0.003, 0.012, 0.03) }),
                    Some(terrain(TerrainKind::Hills, Biome::Grassland, 300.0, 2048, 14, 26.0, 6.0, 56.0, 0.02)),
                );
                s.camera.exposure = 1.0;
                s
            }
            "oceano-aberto" => plain(
                cam(v(0.0, 6.0, -90.0), v(0.0, 5.5, 0.0), 52.0),
                sun(-120.0, 17.0, v(1.0, 0.88, 0.72), 2.3),
                day_sky(0.6),
                Some(Water { level: 0.0, wave_height: 1.1, wave_scale: 1.4, absorption: v(0.50, 0.11, 0.06), deep_color: v(0.005, 0.045, 0.10) }),
                None,
            ),
            "planicie" => plain(
                cam(v(0.0, 9.0, -180.0), v(0.0, 8.0, 0.0), 50.0),
                sun(-65.0, 30.0, v(1.0, 0.92, 0.80), 2.3),
                day_sky(0.5),
                None,
                Some(terrain(TerrainKind::Hills, Biome::Grassland, 400.0, 2048, 27, 26.0, 6.0, 100.0, 0.018)),
            ),
            "jogo-colinas" => game_scene(terrain(TerrainKind::Hills, Biome::Grassland, 256.0, 1025, 14, 36.0, 6.0, 56.0, 0.02)),
            "jogo-montanhas" => game_scene(terrain(TerrainKind::Mountains, Biome::Alpine, 512.0, 1025, 23, 95.0, 4.0, 130.0, 0.0055)),
            "jogo-deserto" => game_scene(terrain(TerrainKind::Dunes, Biome::Desert, 256.0, 1025, 5, 28.0, 6.0, 56.0, 0.022)),
            "jogo-canion" => game_scene(terrain(TerrainKind::Canyon, Biome::RedRock, 384.0, 1025, 8, 60.0, 14.0, 70.0, 0.0055)),
            "jogo-ilha" => {
                let mut s = game_scene(terrain(TerrainKind::Island, Biome::Tropical, 256.0, 1025, 11, 28.0, 14.0, 90.0, 0.022));
                s.water = Some(Water::sea());
                s
            }
            "jogo-arena" => game_scene(terrain(TerrainKind::Hills, Biome::Grassland, 128.0, 513, 2, 3.0, 2.0, 40.0, 0.03)),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_and_roundtrips() {
        for info in TEMPLATES {
            let s = Scene::template(info.id).unwrap_or_else(|| panic!("modelo {} não existe", info.id));
            assert_eq!(Scene::from_ron(&s.to_ron()).unwrap(), s, "{}", info.id);
        }
        assert!(Scene::template("nao-existe").is_none());
    }

    #[test]
    fn game_templates_have_no_water_except_island() {
        for info in TEMPLATES.iter().filter(|i| i.group == TemplateGroup::Game) {
            let s = Scene::template(info.id).unwrap();
            assert!(s.terrain.is_some(), "{}", info.id);
            assert!(s.objects.is_empty());
            if info.id != "jogo-ilha" {
                assert!(s.water.is_none(), "{}", info.id);
            }
        }
    }

    #[test]
    fn old_scene_files_still_load() {
        for f in ["ilha.ron", "por-do-sol.ron"] {
            let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenes/");
            let text = std::fs::read_to_string(format!("{p}{f}")).unwrap();
            let s = Scene::from_ron(&text).unwrap_or_else(|e| panic!("{f}: {e}"));
            let t = s.terrain.unwrap();
            assert_eq!(t.kind, TerrainKind::Island);
            assert_eq!(t.biome, Biome::Tropical);
            assert_eq!(s.sky.stars, 0.0);
        }
    }
}
