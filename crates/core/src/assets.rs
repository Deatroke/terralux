//! Biblioteca de assets do usuário (CD do Bryce, lido em tempo de execução; nada é copiado).
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Prefixo de caminhos relativos à biblioteca (assim as cenas não guardam caminhos absolutos).
pub const LIB_PREFIX: &str = "lib:";

fn root_cell() -> &'static RwLock<PathBuf> {
    static R: OnceLock<RwLock<PathBuf>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(load_config_root().unwrap_or_else(default_root)))
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

fn default_root() -> PathBuf {
    if let Some(p) = std::env::var_os("TERRALUX_LIB") {
        return PathBuf::from(p);
    }
    home().join("Documents/Bryce3D/cdrom")
}

/// Pasta de configuração do usuário (XDG no Linux, APPDATA no Windows).
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    base.join("terralux")
}

/// Pasta de cache (miniaturas); nunca dentro do repositório.
pub fn cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache"));
    base.join("terralux")
}

fn load_config_root() -> Option<PathBuf> {
    if std::env::var_os("TERRALUX_LIB").is_some() {
        return None;
    }
    let t = std::fs::read_to_string(config_dir().join("library.txt")).ok()?;
    let t = t.trim();
    (!t.is_empty()).then(|| PathBuf::from(t))
}

pub fn library_root() -> PathBuf {
    root_cell().read().unwrap().clone()
}

/// Muda a pasta da biblioteca e a grava na configuração do usuário.
pub fn set_library_root(p: PathBuf) {
    let _ = std::fs::create_dir_all(config_dir());
    let _ = std::fs::write(config_dir().join("library.txt"), p.to_string_lossy().as_bytes());
    *root_cell().write().unwrap() = p;
}

/// Resolve `lib:pasta/arquivo` (ou um caminho absoluto) para o caminho real.
pub fn resolve(spec: &str) -> PathBuf {
    match spec.strip_prefix(LIB_PREFIX) {
        Some(rel) => library_root().join(rel),
        None => PathBuf::from(spec),
    }
}

/// Acha um arquivo ignorando maiúsculas (o CD mistura `LAMP.MTL` e `lamp.mtl`).
pub fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.exists() {
        return Some(exact);
    }
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name)))
}

/// Carrega um modelo (OBJ ou DXF), já normalizado para meia-extensão 1.
pub fn load_mesh(spec: &str) -> Result<crate::mesh::Mesh, String> {
    use crate::mesh::Mesh;
    let path = resolve(spec);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut m = match ext.as_str() {
        "obj" => Mesh::from_obj(&text, &|n| find_ci(dir, n).and_then(|p| std::fs::read(p).ok()).map(|b| String::from_utf8_lossy(&b).into_owned())),
        "dxf" => Mesh::from_dxf(&text),
        e => return Err(format!("formato de modelo não suportado: .{e}")),
    };
    if m.tris.is_empty() {
        return Err(format!("{}: nenhum triângulo", path.display()));
    }
    m.normalize();
    Ok(m)
}

/// Imagem decodificada, com cache por caminho.
pub fn cached_image(spec: &str) -> Result<Arc<Img>, String> {
    static C: OnceLock<Mutex<HashMap<String, Arc<Img>>>> = OnceLock::new();
    let c = C.get_or_init(Default::default);
    if let Some(i) = c.lock().unwrap().get(spec) {
        return Ok(i.clone());
    }
    let img = Arc::new(load_image(&resolve(spec))?);
    c.lock().unwrap().insert(spec.to_string(), img.clone());
    Ok(img)
}

/// Imagem decodificada (RGBA8, linhas de cima para baixo).
#[derive(Clone, Debug)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
    /// Verdadeiro se o arquivo tinha canal alfa de verdade.
    pub has_alpha: bool,
}

pub fn load_image(path: &Path) -> Result<Img, String> {
    let err = |e: String| format!("{}: {e}", path.display());
    match image::open(path) {
        Ok(dy) => {
            let has_alpha = dy.color().has_alpha();
            let im = dy.to_rgba8();
            Ok(Img { w: im.width() as usize, h: im.height() as usize, rgba: im.into_raw(), has_alpha })
        }
        Err(_) => load_palette_tiff(path).map_err(err),
    }
}

/// TIFFs com paleta (mapas de terreno, gels): nem o `image` nem o `tiff` os decodificam.
/// O CD só usa TIFF 8 bits, sem compressão, em tiras; este leitor cobre exatamente isso.
fn load_palette_tiff(path: &Path) -> Result<Img, String> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    let le = match b.get(0..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return Err("não é TIFF".into()),
    };
    let u16at = |o: usize| -> Result<u32, String> {
        let s: [u8; 2] = b.get(o..o + 2).ok_or("TIFF truncado")?.try_into().unwrap();
        Ok(if le { u16::from_le_bytes(s) } else { u16::from_be_bytes(s) } as u32)
    };
    let u32at = |o: usize| -> Result<u32, String> {
        let s: [u8; 4] = b.get(o..o + 4).ok_or("TIFF truncado")?.try_into().unwrap();
        Ok(if le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) })
    };
    let ifd = u32at(4)? as usize;
    let n = u16at(ifd)? as usize;
    let (mut w, mut h, mut comp, mut photo, mut rps) = (0u32, 0u32, 1u32, 0u32, u32::MAX);
    // (tipo, quantidade, valor/offset)
    let mut strips = (0u32, 0u32, 0u32);
    let mut counts = (0u32, 0u32, 0u32);
    let mut cmap = (0u32, 0u32);
    for i in 0..n {
        let e = ifd + 2 + i * 12;
        let (tag, ty, cnt) = (u16at(e)?, u16at(e + 2)?, u32at(e + 4)?);
        let val = if ty == 3 && cnt == 1 { u16at(e + 8)? } else { u32at(e + 8)? };
        match tag {
            256 => w = val,
            257 => h = val,
            259 => comp = val,
            262 => photo = val,
            273 => strips = (ty, cnt, val),
            278 => rps = val,
            279 => counts = (ty, cnt, val),
            320 => cmap = (cnt, val),
            _ => {}
        }
    }
    if comp != 1 || photo != 3 {
        return Err(format!("TIFF não suportado (compressão {comp}, fotométrico {photo})"));
    }
    let entry = |(ty, cnt, val): (u32, u32, u32), i: u32| -> Result<usize, String> {
        if cnt == 1 {
            return Ok(val as usize);
        }
        Ok(if ty == 3 { u16at(val as usize + 2 * i as usize)? } else { u32at(val as usize + 4 * i as usize)? } as usize)
    };
    let rps = rps.min(h).max(1);
    let (wu, hu) = (w as usize, h as usize);
    let mut idx = Vec::with_capacity(wu * hu);
    for s in 0..strips.1 {
        let (off, len) = (entry(strips, s)?, entry(counts, s)?);
        let rows = (hu - (s as usize * rps as usize).min(hu)).min(rps as usize);
        let take = len.min(rows * wu);
        idx.extend_from_slice(b.get(off..off + take).ok_or("tira truncada")?);
    }
    if idx.len() != wu * hu {
        return Err("tamanho de dados inesperado".into());
    }
    let nc = (cmap.0 / 3) as usize;
    let mut rgba = Vec::with_capacity(idx.len() * 4);
    for &i in &idx {
        let i = i as usize;
        if i >= nc {
            return Err("índice fora da paleta".into());
        }
        let c = |k: usize| u16at(cmap.1 as usize + 2 * (k * nc + i)).map(|v| (v >> 8) as u8);
        rgba.extend_from_slice(&[c(0)?, c(1)?, c(2)?, 255]);
    }
    Ok(Img { w: wu, h: hu, rgba, has_alpha: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodifica todos os TIFs do CD. Rode com `cargo test -- --ignored` com o CD disponível.
    #[test]
    #[ignore]
    fn decode_all_cd_tifs() {
        let root = PathBuf::from(std::env::var("TERRALUX_LIB").unwrap_or_else(|_| format!("{}/Documents/Bryce3D/cdrom", std::env::var("HOME").unwrap())));
        let mut n = 0;
        let mut bad = vec![];
        for dir in ["Terrain  Maps", "Picture Textures", "Picture Objects", "Light Gels"] {
            for e in std::fs::read_dir(root.join(dir)).unwrap().flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("tif")) {
                    n += 1;
                    if let Err(er) = load_image(&p) {
                        bad.push(er);
                    }
                }
            }
        }
        eprintln!("{n} TIFs, {} falhas", bad.len());
        for b in bad.iter().take(5) {
            eprintln!("{b}");
        }
        assert!(bad.is_empty());
    }

    #[test]
    #[ignore]
    fn load_all_cd_models() {
        let root = library_root();
        let mut bad = 0;
        let mut n = 0;
        for dir in ["ONYX", "OBJ Models", "Viewpoint DXF's", "DXF Models"] {
            let Ok(rd) = std::fs::read_dir(root.join(dir)) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                if ext == "obj" || ext == "dxf" {
                    n += 1;
                    match load_mesh(p.to_str().unwrap()) {
                        Ok(m) => println!("{} -> {} tris", p.display(), m.tris.len()),
                        Err(e) => {
                            bad += 1;
                            println!("FALHA {e}");
                        }
                    }
                }
            }
        }
        println!("{n} modelos, {bad} falhas");
        assert_eq!(bad, 0);
    }
}
