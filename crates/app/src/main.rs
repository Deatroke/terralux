#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]
mod aero;
mod keymap;
mod app;
mod icons;
mod inspector;
mod library;
mod library_ui;
mod shell;
mod terrain_editor;
mod theme;
mod theme_ui;
mod toast;
mod view;
mod viewport;
mod worker;

fn main() -> eframe::Result {
    // Padrão: X11/XWayland (testado). TERRALUX_WAYLAND=1 liga o Wayland nativo.
    if std::env::var_os("TERRALUX_WAYLAND").is_none() {
        std::env::remove_var("WAYLAND_DISPLAY");
    }
    let args = app::Args::parse();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(app_icon())
            .with_inner_size(args.size.map(|s| [s.0, s.1]).unwrap_or([1500.0, 900.0]))
            .with_min_inner_size([1000.0, 640.0])
            .with_title("Terralux Studio"),
        ..Default::default()
    };
    eframe::run_native("Terralux Studio", opts, Box::new(move |cc| Ok(Box::new(app::App::new(cc, args)))))
}

/// Ícone da janela (barra de tarefas e Alt+Tab).
fn app_icon() -> egui::IconData {
    match image::load_from_memory(include_bytes!("../assets/icon/terralux-256.png")) {
        Ok(img) => {
            let img = img.to_rgba8();
            egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }
        }
        Err(_) => egui::IconData::default(),
    }
}
