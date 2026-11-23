//! Operações da CLI. Cada uma devolve texto (humanos) e dados (`--json`).
use crate::Out;
use anyhow::{anyhow, bail, Context, Result};
use glam::Vec3;
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;
use terralux_core::heightmap::Heightmap;
use terralux_core::mesh::Primitive;
use terralux_core::{Heightfield, Material, Object, Polyhedron, Scene, Shape, TerrainDef};
use terralux_render::export::{export_mesh, export_scene, ExportOptions};
use terralux_render::{tonemap_to_srgb8, RenderOptions, Renderer};

fn is_json(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

/// Lê uma cena `.ron` ou `.json`; `-` lê o stdin (tenta JSON, depois RON).
pub fn load(path: &Path) -> Result<Scene> {
    let text = if path.as_os_str() == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).context("lendo o stdin")?;
        s
    } else {
        std::fs::read_to_string(path).with_context(|| format!("lendo {}", path.display()))?
    };
    let json_like = is_json(path) || text.trim_start().starts_with('{') && text.contains("\"camera\"");
    if json_like {
        serde_json::from_str(&text).with_context(|| "cena JSON inválida")
    } else {
        Scene::from_ron(&text).with_context(|| "cena RON inválida")
    }
}

fn save(scene: &Scene, path: &Path) -> Result<()> {
    let text = if is_json(path) { serde_json::to_string_pretty(scene)? } else { scene.to_ron() };
    std::fs::write(path, text).with_context(|| format!("gravando {}", path.display()))
}

fn to_value(s: &Scene) -> Result<Value> {
    serde_json::to_value(s).context("serializando a cena")
}

fn from_value(v: Value) -> Result<Scene> {
    serde_json::from_value(v).map_err(|e| anyhow!("valor inválido para a cena: {e}"))
}

fn v3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

fn parse_v3(s: &str, what: &str) -> Result<Vec3> {
    let p: Vec<f32> = s.split(',').map(|x| x.trim().parse::<f32>()).collect::<Result<_, _>>().map_err(|_| anyhow!("{what}: use x,y,z (ex.: 0,10,0)"))?;
    if p.len() != 3 {
        bail!("{what}: use x,y,z (ex.: 0,10,0)");
    }
    Ok(Vec3::new(p[0], p[1], p[2]))
}

fn parse_color(s: &str) -> Result<Vec3> {
    if let Some(h) = s.strip_prefix('#') {
        if h.len() == 6 {
            if let Ok(n) = u32::from_str_radix(h, 16) {
                return Ok(Vec3::new(((n >> 16) & 255) as f32, ((n >> 8) & 255) as f32, (n & 255) as f32) / 255.0);
            }
        }
        bail!("cor inválida: {s} (use #rrggbb ou r,g,b)");
    }
    parse_v3(s, "cor")
}

// ---------- modelos ----------

pub fn templates() -> Out {
    let items: Vec<Value> = terralux_core::TEMPLATES.iter().map(|i| json!({"id": i.id, "name": i.name, "description": i.desc, "group": format!("{:?}", i.group)})).collect();
    let text = terralux_core::TEMPLATES.iter().map(|i| format!("{:<20} {}", i.id, i.desc)).collect::<Vec<_>>().join("\n");
    Out { text, data: json!({"templates": items}) }
}

pub fn new_scene(template: &str, output: Option<PathBuf>) -> Result<Out> {
    let sc = Scene::template(template).with_context(|| format!("modelo desconhecido: {template} (veja `terralux templates`)"))?;
    match output {
        Some(p) => {
            save(&sc, &p)?;
            Ok(Out { text: format!("cena criada: {}", p.display()), data: json!({"scene": p, "template": template}) })
        }
        None => Ok(Out { text: sc.to_ron(), data: json!({"template": template, "scene": to_value(&sc)?}) }),
    }
}

pub fn from_heightmap(hm: &Path, output: &Path, half_size: f32, height: f32, template: &str) -> Result<Out> {
    let mut sc = Scene::template(template).with_context(|| format!("modelo desconhecido: {template}"))?;
    let spec = std::fs::canonicalize(hm).with_context(|| format!("{}: não encontrado", hm.display()))?;
    let spec = spec.to_string_lossy().to_string();
    terralux_core::heightmap::load_gray(&spec).map_err(|e| anyhow!(e))?;
    let mut t = TerrainDef::from_image(&spec);
    t.half_size = half_size;
    t.peak_height = height;
    sc.terrain = Some(t);
    save(&sc, output)?;
    Ok(Out { text: format!("cena criada: {}", output.display()), data: json!({"scene": output, "heightmap": spec, "half_size": half_size, "height": height}) })
}

// ---------- inspeção ----------

fn shape_json(s: &Shape) -> Value {
    match s {
        Shape::Sphere => json!({"kind": "sphere"}),
        Shape::Plane => json!({"kind": "plane"}),
        Shape::Poly(p) => json!({"kind": "polyhedron", "value": format!("{p:?}")}),
        Shape::Prim(p) => json!({"kind": "primitive", "value": format!("{p:?}")}),
        Shape::Model(m) => json!({"kind": "model", "path": m}),
        Shape::Picture(m) => json!({"kind": "picture", "path": m}),
    }
}

pub fn info(sc: &Scene) -> Result<Out> {
    let mut data = json!({
        "camera": {"position": v3(sc.camera.position), "target": v3(sc.camera.target), "fov_deg": sc.camera.fov_deg},
        "sun": {"azimuth_deg": sc.sun.azimuth_deg, "elevation_deg": sc.sun.elevation_deg, "intensity": sc.sun.intensity},
        "sky": {"haze": sc.sky.haze, "stars": sc.sky.stars, "clouds": sc.sky.clouds.is_some()},
        "water": sc.water.as_ref().map(|w| json!({"level": w.level, "wave_height": w.wave_height})),
        "objects": sc.objects.iter().enumerate().map(|(i, o)| json!({"index": i, "name": o.name, "shape": shape_json(&o.shape), "position": v3(o.position), "rotation_deg": v3(o.rotation_deg), "size": o.size})).collect::<Vec<_>>(),
    });
    let mut text = format!("câmera em {:?} olhando {:?}\nsol: azimute {:.0}°, elevação {:.0}°\n", sc.camera.position, sc.camera.target, sc.sun.azimuth_deg, sc.sun.elevation_deg);
    match &sc.terrain {
        Some(d) => {
            let hf = Heightfield::from_def(d);
            data["terrain"] = json!({
                "kind": d.kind.id(), "biome": d.biome.id(), "half_size": d.half_size, "side_m": 2.0 * d.half_size, "resolution": d.resolution,
                "seed": d.seed, "peak_height": d.peak_height, "min_height": hf.min, "max_height": hf.max,
                "source": format!("{:?}", d.source),
            });
            text += &format!("terreno {} / {}: lado {:.0} m, grade {}², altura de {:.1} a {:.1} m\n", d.kind.name(), d.biome.name(), 2.0 * d.half_size, d.resolution, hf.min, hf.max);
        }
        None => text += "sem terreno\n",
    }
    if let Some(w) = &sc.water {
        text += &format!("água no nível {:.1} m\n", w.level);
    }
    text += &format!("{} objeto(s)", sc.objects.len());
    for (i, o) in sc.objects.iter().enumerate() {
        text += &format!("\n  [{i}] {} ({}) em {:?}, raio {}", o.name, o.shape.label(), o.position, o.size);
    }
    Ok(Out { text, data })
}

pub fn validate(sc: &Scene) -> Result<Out> {
    let (mut errors, mut warnings): (Vec<String>, Vec<String>) = (vec![], vec![]);
    if let Some(d) = &sc.terrain {
        if d.resolution < 8 || d.resolution > 8193 {
            errors.push(format!("terrain.resolution fora de 8 a 8193: {}", d.resolution));
        }
        if d.half_size <= 0.0 {
            errors.push("terrain.half_size deve ser positivo".into());
        }
        if let terralux_core::TerrainSource::Image(p) = &d.source {
            if let Err(e) = terralux_core::heightmap::load_gray(p) {
                errors.push(format!("mapa de altura: {e}"));
            }
        }
        if d.resolution > 2049 {
            warnings.push(format!("terrain.resolution {} é pesada para pré-visualizar", d.resolution));
        }
    }
    for (i, o) in sc.objects.iter().enumerate() {
        if o.size <= 0.0 {
            errors.push(format!("objects.{i} ({}): size deve ser positivo", o.name));
        }
        match &o.shape {
            Shape::Model(m) => {
                if let Err(e) = terralux_core::assets::load_mesh(m) {
                    errors.push(format!("objects.{i} ({}): {e}", o.name));
                }
            }
            Shape::Picture(m) => {
                if let Err(e) = terralux_core::assets::cached_image(m) {
                    errors.push(format!("objects.{i} ({}): {e}", o.name));
                }
            }
            _ => {}
        }
    }
    if sc.terrain.is_none() && sc.objects.is_empty() && sc.water.is_none() {
        warnings.push("a cena está vazia (só céu)".into());
    }
    if (sc.camera.position - sc.camera.target).length() < 1e-3 {
        errors.push("camera.position e camera.target coincidem".into());
    }
    let ok = errors.is_empty();
    let text = if ok { format!("cena válida ({} aviso(s))", warnings.len()) } else { format!("{} erro(s):\n  {}", errors.len(), errors.join("\n  ")) };
    let data = json!({"valid": ok, "errors": errors, "warnings": warnings});
    if !ok {
        bail!("{}", text);
    }
    Ok(Out { text, data })
}

// ---------- get / set por caminho ----------

fn step<'a>(cur: &'a mut Value, seg: &str) -> Result<&'a mut Value> {
    match cur {
        Value::Object(m) => {
            if !m.contains_key(seg) {
                let keys: Vec<_> = m.keys().cloned().collect();
                bail!("campo '{seg}' não existe (campos: {})", keys.join(", "));
            }
            Ok(m.get_mut(seg).unwrap())
        }
        Value::Array(a) => {
            let idx = match seg.parse::<usize>() {
                Ok(i) => i,
                Err(_) => a.iter().position(|v| v.get("name").and_then(|n| n.as_str()) == Some(seg)).ok_or_else(|| anyhow!("nenhum item com nome '{seg}'"))?,
            };
            let n = a.len();
            a.get_mut(idx).ok_or_else(|| anyhow!("índice {idx} fora do intervalo (há {n})"))
        }
        _ => bail!("'{seg}': o valor anterior não tem campos"),
    }
}

fn walk<'a>(root: &'a mut Value, path: &str) -> Result<&'a mut Value> {
    let mut cur = root;
    for seg in path.split('.').filter(|s| !s.is_empty()) {
        cur = step(cur, seg).with_context(|| format!("caminho '{path}'"))?;
    }
    Ok(cur)
}

fn parse_val(s: &str) -> Value {
    if let Ok(v) = serde_json::from_str::<Value>(s) {
        return v;
    }
    let parts: Vec<_> = s.split(',').map(|p| p.trim().parse::<f64>()).collect();
    if parts.len() > 1 && parts.iter().all(|p| p.is_ok()) {
        return Value::Array(parts.into_iter().map(|p| json!(p.unwrap())).collect());
    }
    Value::String(s.to_string())
}

pub fn get(sc: &Scene, path: &str) -> Result<Out> {
    let mut v = to_value(sc)?;
    let found = walk(&mut v, path)?.clone();
    Ok(Out { text: serde_json::to_string_pretty(&found)?, data: json!({"path": path, "value": found}) })
}

fn dest(scene: &Path, output: Option<PathBuf>) -> Result<PathBuf> {
    match output {
        Some(p) => Ok(p),
        None if scene.as_os_str() == "-" => bail!("com a cena vindo do stdin, informe -o"),
        None => Ok(scene.to_path_buf()),
    }
}

pub fn set(scene: &Path, assignments: &[String], output: Option<PathBuf>) -> Result<Out> {
    let sc = load(scene)?;
    let mut v = to_value(&sc)?;
    let mut changed = vec![];
    for a in assignments {
        let (path, val) = a.split_once('=').ok_or_else(|| anyhow!("'{a}': use caminho=valor"))?;
        let slot = walk(&mut v, path.trim())?;
        let new = parse_val(val.trim());
        changed.push(json!({"path": path.trim(), "old": slot.clone(), "new": new}));
        *slot = new;
    }
    // Converte de volta: qualquer valor de tipo errado é rejeitado aqui, antes de gravar.
    let sc2 = from_value(v).context("a alteração deixaria a cena inválida; nada foi gravado")?;
    let out = dest(scene, output)?;
    save(&sc2, &out)?;
    Ok(Out { text: format!("{} alteração(ões) gravada(s) em {}", changed.len(), out.display()), data: json!({"scene": out, "changed": changed}) })
}

// ---------- objetos ----------

fn parse_shape(s: &str) -> Result<Shape> {
    let low = s.to_ascii_lowercase();
    match low.as_str() {
        "sphere" | "esfera" => return Ok(Shape::Sphere),
        "plane" | "plano" => return Ok(Shape::Plane),
        _ => {}
    }
    if let Some(p) = Primitive::ALL.iter().find(|p| format!("{p:?}").to_ascii_lowercase() == low) {
        return Ok(Shape::Prim(*p));
    }
    if let Some(p) = Polyhedron::ALL.iter().find(|p| format!("{p:?}").to_ascii_lowercase() == low) {
        return Ok(Shape::Poly(*p));
    }
    let path = Path::new(s);
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let abs = || std::fs::canonicalize(path).map(|p| p.to_string_lossy().to_string()).with_context(|| format!("{s}: arquivo não encontrado"));
    match ext.as_str() {
        "glb" | "gltf" | "obj" | "stl" | "ply" | "dxf" => Ok(Shape::Model(abs()?)),
        "png" | "jpg" | "jpeg" | "webp" | "gif" => Ok(Shape::Picture(abs()?)),
        _ => {
            let names: Vec<String> = ["sphere", "plane"].iter().map(|s| s.to_string()).chain(Primitive::ALL.iter().map(|p| format!("{p:?}").to_ascii_lowercase())).chain(Polyhedron::ALL.iter().map(|p| format!("{p:?}").to_ascii_lowercase())).collect();
            bail!("forma desconhecida: {s}. Formas: {}; ou um arquivo glb/gltf/obj/stl/ply/dxf/png/jpg", names.join(", "))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn add(scene: &Path, shape: &str, name: Option<String>, pos: Option<String>, rot: Option<String>, size: f32, color: Option<String>, reflect: Option<f32>, output: Option<PathBuf>) -> Result<Out> {
    let mut sc = load(scene)?;
    let shape = parse_shape(shape)?;
    if let Shape::Model(m) = &shape {
        terralux_core::assets::load_mesh(m).map_err(|e| anyhow!(e))?;
    }
    let position = match pos {
        Some(p) => parse_v3(&p, "--pos")?,
        None => {
            let y = sc.terrain.as_ref().map_or(0.0, |d| Heightfield::from_def(d).height(0.0, 0.0));
            Vec3::new(0.0, y + size, 0.0)
        }
    };
    let mut material = Material::matte(Vec3::splat(0.8));
    if let Some(c) = color {
        material.color = parse_color(&c)?;
    }
    if let Some(r) = reflect {
        material.reflectivity = r.clamp(0.0, 1.0);
    }
    let base = name.unwrap_or_else(|| shape.label().to_string());
    let n = sc.objects.iter().filter(|o| o.name.starts_with(&base)).count();
    let name = if n == 0 { base } else { format!("{base} {}", n + 1) };
    let rotation_deg = match rot {
        Some(r) => parse_v3(&r, "--rot")?,
        None => Vec3::ZERO,
    };
    sc.objects.push(Object { name: name.clone(), shape, position, rotation_deg, size, material });
    let out = dest(scene, output)?;
    save(&sc, &out)?;
    let index = sc.objects.len() - 1;
    Ok(Out { text: format!("objeto '{name}' (índice {index}) adicionado em {}", out.display()), data: json!({"scene": out, "index": index, "name": name, "position": v3(position)}) })
}

pub fn remove(scene: &Path, object: &str, output: Option<PathBuf>) -> Result<Out> {
    let mut sc = load(scene)?;
    let idx = match object.parse::<usize>() {
        Ok(i) => i,
        Err(_) => sc.objects.iter().position(|o| o.name == object).ok_or_else(|| anyhow!("nenhum objeto chamado '{object}'"))?,
    };
    if idx >= sc.objects.len() {
        bail!("índice {idx} fora do intervalo (há {})", sc.objects.len());
    }
    let o = sc.objects.remove(idx);
    let out = dest(scene, output)?;
    save(&sc, &out)?;
    Ok(Out { text: format!("objeto '{}' removido", o.name), data: json!({"scene": out, "removed": o.name, "remaining": sc.objects.len()}) })
}

// ---------- exportação ----------

pub fn export(sc: &Scene, output: &Path, terrain_res: usize, terrain: bool, objects: bool, water: bool, camera: bool) -> Result<Out> {
    let o = ExportOptions { terrain_res, include_terrain: terrain, include_objects: objects, include_water: water, include_camera: camera };
    let r = export_scene(sc, output, &o).map_err(|e| anyhow!(e))?;
    Ok(Out {
        text: format!("{}: {} triângulos, {} vértices, {} KB{}", output.display(), r.triangles, r.vertices, r.bytes / 1024, if r.skipped.is_empty() { String::new() } else { format!("\navisos: {}", r.skipped.join("; ")) }),
        data: json!({"output": output, "triangles": r.triangles, "vertices": r.vertices, "bytes": r.bytes, "skipped": r.skipped, "up_axis": "Y", "units": "meters", "handedness": "right"}),
    })
}

pub fn export_terrain(sc: &Scene, output: &Path, size: usize) -> Result<Out> {
    let d = sc.terrain.as_ref().ok_or_else(|| anyhow!("a cena não tem terreno"))?;
    let hf = Heightfield::from_def(d);
    let hm = Heightmap::from_heightfield(&hf, size);
    let ext = output.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" => hm.write_png16(output),
        "raw" | "r16" => hm.write_raw16(output),
        e => Err(format!("formato não suportado: .{e} (use .png ou .raw)")),
    }
    .map_err(|e| anyhow!(e))?;
    Ok(Out {
        text: format!("{}: {size}×{size}, lado {:.0} m, altura {:.1} m (de {:.1} a {:.1})", output.display(), hm.extent, hm.height_range(), hm.min, hm.max),
        data: json!({
            "output": output, "size": size, "format": if ext == "png" { "png16" } else { "raw16-little-endian" },
            "extent_m": hm.extent, "min_height_m": hm.min, "max_height_m": hm.max, "terrain_height_m": hm.height_range(),
            "note": "Na Unity: Terrain Width/Length = extent_m, Height = terrain_height_m, Byte Order = Windows. A engine é canhota, então o relevo sai espelhado em x."
        }),
    })
}

pub fn convert(input: &Path, output: &Path) -> Result<Out> {
    let m = terralux_core::assets::load_mesh_raw(&input.to_string_lossy()).map_err(|e| anyhow!(e))?;
    let r = export_mesh(&m, output).map_err(|e| anyhow!(e))?;
    Ok(Out { text: format!("{} → {}: {} triângulos", input.display(), output.display(), r.triangles), data: json!({"input": input, "output": output, "triangles": r.triangles, "bytes": r.bytes}) })
}

pub fn inspect(model: &Path) -> Result<Out> {
    let m = terralux_core::assets::load_mesh_raw(&model.to_string_lossy()).map_err(|e| anyhow!(e))?;
    let (lo, hi) = m.bounds();
    let size = hi - lo;
    Ok(Out {
        text: format!("{}: {} triângulos, {} material(is), dimensões {:.3} × {:.3} × {:.3}", model.display(), m.tris.len(), m.mats.len(), size.x, size.y, size.z),
        data: json!({"triangles": m.tris.len(), "materials": m.mats.len(), "min": v3(lo), "max": v3(hi), "size": v3(size)}),
    })
}

// ---------- render ----------

pub fn render(sc: &Scene, width: usize, height: usize, spp: usize, fast: bool, output: &Path) -> Result<Out> {
    let t0 = Instant::now();
    let r = Renderer::new(sc).fast(fast);
    eprintln!("terreno pronto em {:.1}s", t0.elapsed().as_secs_f32());
    let img = r.render(&RenderOptions { progress: true, ..RenderOptions::new(width, height, spp) });
    let bytes = tonemap_to_srgb8(&img);
    image::save_buffer(output, &bytes, width as u32, height as u32, image::ColorType::Rgb8)?;
    let secs = t0.elapsed().as_secs_f32();
    Ok(Out { text: format!("{width}x{height} @ {spp} spp em {secs:.1}s -> {}", output.display()), data: json!({"output": output, "width": width, "height": height, "spp": spp, "fast": fast, "seconds": secs}) })
}
