use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;
use terralux_core::Scene;
use terralux_render::{tonemap_to_srgb8, RenderOptions, Renderer};

#[derive(Parser)]
#[command(name = "terralux", about = "Renderizador de paisagens 3D procedurais")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Renderiza uma cena .ron para PNG.
    Render {
        scene: PathBuf,
        #[arg(long, default_value_t = 1920)]
        width: usize,
        #[arg(long, default_value_t = 1080)]
        height: usize,
        /// Amostras por pixel (antialiasing).
        #[arg(long, default_value_t = 9)]
        spp: usize,
        #[arg(short, long, default_value = "out.png")]
        output: PathBuf,
    },
    /// Imprime a cena de demonstração em RON.
    Example,
    /// Lista os modelos de cena prontos.
    Templates,
    /// Cria uma cena .ron a partir de um modelo (veja `templates`).
    New {
        /// Identificador do modelo.
        template: String,
        /// Arquivo de saída; sem ele, imprime na tela.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Example => println!("{}", Scene::demo().to_ron()),
        Cmd::Templates => {
            for i in terralux_core::TEMPLATES {
                println!("{:<20} {}", i.id, i.desc);
            }
        }
        Cmd::New { template, output } => {
            let sc = Scene::template(&template).with_context(|| format!("modelo desconhecido: {template} (veja `terralux templates`)"))?;
            match output {
                Some(p) => std::fs::write(&p, sc.to_ron()).with_context(|| format!("gravando {}", p.display()))?,
                None => println!("{}", sc.to_ron()),
            }
        }
        Cmd::Render { scene, width, height, spp, output } => {
            let text = std::fs::read_to_string(&scene).with_context(|| format!("lendo {}", scene.display()))?;
            let sc = Scene::from_ron(&text).with_context(|| "cena RON inválida")?;
            let t0 = Instant::now();
            let r = Renderer::new(&sc);
            eprintln!("terreno pronto em {:.1}s", t0.elapsed().as_secs_f32());
            let img = r.render(&RenderOptions { progress: true, ..RenderOptions::new(width, height, spp) });
            let bytes = tonemap_to_srgb8(&img);
            image::save_buffer(&output, &bytes, width as u32, height as u32, image::ColorType::Rgb8)?;
            eprintln!("{}x{} @ {} spp em {:.1}s -> {}", width, height, spp, t0.elapsed().as_secs_f32(), output.display());
        }
    }
    Ok(())
}
