use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use stem_desktop::{
    app::{DesktopApp, resource_root},
    state::Phase,
};

struct Smoke {
    app: DesktopApp,
    started: Instant,
    stage: u8,
    output: PathBuf,
}
impl eframe::App for Smoke {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.update(ctx, frame);
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                let name = match self.stage {
                    1 => "studio-idle-640.png",
                    3 => "studio-ready-1280.png",
                    _ => "studio-ready-640.png",
                };
                image::save_buffer(
                    self.output.join(name),
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap();
                match self.stage {
                    1 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1280.0, 820.0,
                        )));
                        self.app.start();
                        self.stage = 2;
                    }
                    3 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            640.0, 600.0,
                        )));
                        self.stage = 4;
                    }
                    5 => {
                        println!(
                            "Native smoke passed: real model Ready, {} mixer rows, screenshots saved",
                            self.app.state.mix.len()
                        );
                        self.app.shutdown();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        self.stage = 6;
                    }
                    _ => {}
                }
            }
        }
        match self.stage {
            0 if self.app.can_separate() && self.started.elapsed() > Duration::from_secs(1) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.stage = 1;
            }
            2 if self.app.state.phase == Phase::Ready && self.app.waveforms_ready() => {
                assert_eq!(self.app.state.mix.len(), 6);
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.stage = 3;
            }
            4 => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.stage = 5;
            }
            _ => {}
        }
        assert!(
            self.app.state.phase != Phase::Failed,
            "{}",
            self.app.error.as_deref().unwrap_or("Processing failed")
        );
        assert!(
            self.started.elapsed() < Duration::from_secs(180),
            "Native smoke timed out"
        );
        ctx.request_repaint_after(Duration::from_millis(33));
    }
}
fn main() -> eframe::Result {
    let root = resource_root();
    let output = root.join("docs/validation/screenshots");
    std::fs::create_dir_all(&output).expect("create screenshot directory");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Stem separator validation",
        options,
        Box::new(move |cc| {
            DesktopApp::configure(&cc.egui_ctx);
            let mut app = DesktopApp::new(&root, root.join("validation-assets/gui-cache"))?;
            app.input = root
                .join("validation-assets/vibe-ace.wav")
                .to_string_lossy()
                .into_owned();
            Ok(Box::new(Smoke {
                app,
                started: Instant::now(),
                stage: 0,
                output,
            }))
        }),
    )
}
