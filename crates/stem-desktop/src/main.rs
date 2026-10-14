use stem_desktop::app::{DesktopApp, resource_root};
fn main() -> eframe::Result {
    let root = resource_root();
    let cache = std::env::var_os("STEM_CACHE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("stem-separator-cache"));
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([640.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Stem separator",
        options,
        Box::new(move |cc| {
            DesktopApp::configure(&cc.egui_ctx);
            let app = DesktopApp::new(&root, cache)?;
            Ok(Box::new(app))
        }),
    )
}
