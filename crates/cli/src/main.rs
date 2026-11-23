//! CLI do Terralux, pensada para humanos e para agentes (LLMs, scripts, CI).
//!
//! Regras de contrato (veja docs/CLI.md):
//! - `--json` (global) imprime UM objeto JSON no stdout: `{"ok":true,...}` ou `{"ok":false,"error":"..."}`.
//! - Mensagens de progresso vão para o stderr; o stdout fica só com o resultado.
//! - Código de saída: 0 = sucesso, 1 = falha da operação, 2 = uso incorreto dos argumentos.
//! - Cenas: `.ron` ou `.json` (pela extensão); `-` lê a cena do stdin.
mod ops;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "terralux", version, about = "Terralux: terrenos e cenas 3D por linha de comando (para pessoas e agentes)")]
struct Cli {
    /// Saída em JSON (um objeto no stdout), ideal para agentes e scripts.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Lista os modelos de cena prontos (cenários e terrenos para jogo).
    Templates,
    /// Cria uma cena a partir de um modelo (veja `templates`). Sem -o, imprime a cena.
    New {
        template: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Cria uma cena cujo terreno vem de um mapa de altura (PNG/TIFF 16 bits, RAW/R16 quadrado).
    FromHeightmap {
        heightmap: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// Meia largura do terreno em metros (lado = 2×).
        #[arg(long, default_value_t = 250.0)]
        half_size: f32,
        /// Altura máxima do relevo em metros.
        #[arg(long, default_value_t = 60.0)]
        height: f32,
        /// Modelo de cena que fornece céu, sol e câmera.
        #[arg(long, default_value = "vazio")]
        template: String,
    },
    /// Resumo da cena: terreno, objetos, câmera, céu. Com --json, tudo estruturado.
    Info { scene: PathBuf },
    /// Confere a cena (arquivos, valores) sem renderizar. Sai com 1 se houver erros.
    Validate { scene: PathBuf },
    /// Lê um valor da cena por caminho (ex.: `terrain.peak_height`, `objects.0.position`, `objects.Pedra.size`).
    Get { scene: PathBuf, path: String },
    /// Altera valores: `set cena.ron terrain.peak_height=80 sun.elevation_deg=25 -o nova.ron`.
    /// Valores são JSON (`80`, `"Ilha"`, `[1,2,3]`, `null`) ou `x,y,z`. Grava na própria cena, salvo com -o.
    Set {
        scene: PathBuf,
        /// Pares caminho=valor.
        #[arg(required = true)]
        assignments: Vec<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Adiciona um objeto: forma (sphere, cube, cone, skull, icosahedron…) ou arquivo de modelo (glb, gltf, obj, stl, ply, dxf) ou imagem.
    Add {
        scene: PathBuf,
        /// Forma, ou caminho de um modelo/imagem.
        shape: String,
        #[arg(long)]
        name: Option<String>,
        /// Posição x,y,z. Sem ela, fica sobre o terreno no centro.
        #[arg(long)]
        pos: Option<String>,
        /// Rotação x,y,z em graus.
        #[arg(long)]
        rot: Option<String>,
        /// Raio do objeto em metros.
        #[arg(long, default_value_t = 5.0)]
        size: f32,
        /// Cor: #rrggbb ou r,g,b (0 a 1).
        #[arg(long)]
        color: Option<String>,
        /// Reflexo, de 0 a 1.
        #[arg(long)]
        reflect: Option<f32>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Remove um objeto por índice ou nome.
    Remove {
        scene: PathBuf,
        object: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Exporta a cena para engines e DCCs: .glb (Unity, Unreal, Godot, Blender), .obj (+ .mtl) ou .stl.
    Export {
        scene: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// Vértices por lado da malha do terreno (2 a 2049).
        #[arg(long, default_value_t = 513)]
        terrain_res: usize,
        #[arg(long)]
        no_terrain: bool,
        #[arg(long)]
        no_objects: bool,
        /// Inclui a água como plano translúcido.
        #[arg(long)]
        water: bool,
        /// Não inclui câmera e sol (glb).
        #[arg(long)]
        no_camera: bool,
    },
    /// Exporta o terreno como mapa de altura de 16 bits: .png ou .raw (little-endian, para Unity/Unreal).
    ExportTerrain {
        scene: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// Lado da grade; para a Unity use 513, 1025, 2049 ou 4097.
        #[arg(long, default_value_t = 1025)]
        size: usize,
    },
    /// Converte um modelo entre formatos (glb/gltf/obj/stl/ply/dxf → glb/obj/stl), mantendo tamanho e posição.
    Convert {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Mostra triângulos e dimensões de um arquivo de modelo.
    Inspect { model: PathBuf },
    /// Renderiza a cena para PNG.
    Render {
        scene: PathBuf,
        #[arg(long, default_value_t = 1920)]
        width: usize,
        #[arg(long, default_value_t = 1080)]
        height: usize,
        /// Amostras por pixel (antialiasing).
        #[arg(long, default_value_t = 9)]
        spp: usize,
        /// Prévia leve: sem sombras nem reflexos recursivos (bem mais rápida).
        #[arg(long)]
        fast: bool,
        #[arg(short, long, default_value = "out.png")]
        output: PathBuf,
    },
    /// Imprime a cena de demonstração (RON, ou JSON com --json).
    Example,
}

fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli.cmd) {
        Ok(out) => {
            if json {
                let mut v = out.data;
                v["ok"] = json!(true);
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else if !out.text.is_empty() {
                println!("{}", out.text);
            }
        }
        Err(e) => {
            let msg = format!("{e:#}");
            if json {
                println!("{}", json!({"ok": false, "error": msg}));
            } else {
                eprintln!("erro: {msg}");
            }
            std::process::exit(1);
        }
    }
}

/// Resultado de um comando: texto para pessoas e dados para `--json`.
pub struct Out {
    pub text: String,
    pub data: Value,
}

fn run(cmd: Cmd) -> Result<Out> {
    use ops::*;
    match cmd {
        Cmd::Templates => Ok(templates()),
        Cmd::New { template, output } => new_scene(&template, output),
        Cmd::FromHeightmap { heightmap, output, half_size, height, template } => from_heightmap(&heightmap, &output, half_size, height, &template),
        Cmd::Info { scene } => info(&load(&scene)?),
        Cmd::Validate { scene } => validate(&load(&scene)?),
        Cmd::Get { scene, path } => get(&load(&scene)?, &path),
        Cmd::Set { scene, assignments, output } => set(&scene, &assignments, output),
        Cmd::Add { scene, shape, name, pos, rot, size, color, reflect, output } => add(&scene, &shape, name, pos, rot, size, color, reflect, output),
        Cmd::Remove { scene, object, output } => remove(&scene, &object, output),
        Cmd::Export { scene, output, terrain_res, no_terrain, no_objects, water, no_camera } => export(&load(&scene)?, &output, terrain_res, !no_terrain, !no_objects, water, !no_camera),
        Cmd::ExportTerrain { scene, output, size } => export_terrain(&load(&scene)?, &output, size),
        Cmd::Convert { input, output } => convert(&input, &output),
        Cmd::Inspect { model } => inspect(&model),
        Cmd::Render { scene, width, height, spp, fast, output } => render(&load(&scene)?, width, height, spp, fast, &output),
        Cmd::Example => {
            let s = terralux_core::Scene::demo();
            Ok(Out { text: s.to_ron(), data: json!({"scene": serde_json::to_value(&s).context("serializando")?}) })
        }
    }
}

use ops::load;
