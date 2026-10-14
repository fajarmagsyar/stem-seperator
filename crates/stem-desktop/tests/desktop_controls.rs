use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use stem_desktop::app::DesktopApp;

#[test]
fn empty_window_controls_and_invalid_model_work_at_minimum_size() {
    let temp = tempfile::tempdir().unwrap();
    let app = DesktopApp::new(temp.path(), temp.path().join("cache")).unwrap();
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(640.0, 600.0))
        .build_state(
            |ctx, app| {
                DesktopApp::configure(ctx);
                app.draw(ctx);
            },
            app,
        );
    assert!(
        harness
            .get_by_label("Separate song")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_by_label("Cancel processing")
            .accesskit_node()
            .is_disabled()
    );
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(5));
        harness.step();
        if harness.state().error.is_some() {
            break;
        }
    }
    assert!(
        harness
            .state()
            .error
            .as_ref()
            .unwrap()
            .contains("Model package")
    );
    harness.get_by_label("Validate").click();
    harness.step();
    harness.set_size(eframe::egui::vec2(1000.0, 1040.0));
    harness.step();
    assert!(
        harness
            .get_by_label("Export WAVs…")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
#[ignore = "requires provisioned local model and runtime; runs real CPU inference"]
fn real_song_mixer_and_export_controls() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().unwrap();
    let mut app = DesktopApp::new(&root, temp.path().join("cache")).unwrap();
    app.input = root
        .join("tests/fixtures/tone.mp3")
        .to_string_lossy()
        .into_owned();
    app.output = temp.path().join("exports").to_string_lossy().into_owned();
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1000.0, 1400.0))
        .build_state(
            |ctx, app| {
                DesktopApp::configure(ctx);
                app.draw(ctx);
            },
            app,
        );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    while !harness.state().can_separate() {
        assert!(
            std::time::Instant::now() < deadline,
            "package validation timeout: {:?}",
            harness.state().error
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        harness.step();
    }
    harness.get_by_label("Separate song").click();
    harness.step();
    assert!(harness.state().state.busy());
    harness.step();
    assert!(
        harness
            .get_by_label("Separate song")
            .accesskit_node()
            .is_disabled()
    );
    while harness.state().state.busy() {
        assert!(std::time::Instant::now() < deadline, "inference timeout");
        std::thread::sleep(std::time::Duration::from_millis(20));
        harness.step();
    }
    assert!(
        harness.state().state.has_result,
        "{:?}",
        harness.state().error
    );
    if std::env::var_os("STEM_TEST_NO_AUDIO").is_some() {
        assert!(!harness.state().state.has_device);
        assert!(harness.state().state.can_export());
    }
    assert_eq!(harness.state().state.mix.len(), 6);
    harness.get_by_label("Mute Vocals").click();
    harness.step();
    assert!(
        harness
            .state()
            .state
            .mix
            .iter()
            .find(|s| s.id.as_str() == "vocals")
            .unwrap()
            .mute
    );
    harness.get_by_label("Solo Piano").click();
    harness.step();
    assert!(
        harness
            .state()
            .state
            .mix
            .iter()
            .find(|s| s.id.as_str() == "piano")
            .unwrap()
            .solo
    );
    harness.get_by_label("Export WAVs…").click();
    harness.step();
    harness.step();
    harness.get_by_label("Include audition mix").click();
    harness.step();
    harness.get_by_label("Export selected WAVs").click();
    harness.step();
    while harness.state().state.busy() {
        assert!(std::time::Instant::now() < deadline, "export timeout");
        std::thread::sleep(std::time::Duration::from_millis(20));
        harness.step();
    }
    let output = std::fs::read_dir(temp.path().join("exports"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(output.join("_audition-mix.wav").is_file());
    assert!(output.join("vocals.wav").is_file());
    harness.state_mut().shutdown();
    assert_eq!(
        std::fs::read_dir(temp.path().join("cache"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn studio_workspace_exposes_import_device_and_persistent_transport() {
    let temp = tempfile::tempdir().unwrap();
    let app = DesktopApp::new(temp.path(), temp.path().join("cache")).unwrap();
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1280., 820.))
        .build_state(
            |ctx, app| {
                DesktopApp::configure(ctx);
                app.draw(ctx);
            },
            app,
        );
    harness.get_by_label("Import audio…");
    harness.get_by_label("Device");
    assert!(harness.get_by_label("Play").accesskit_node().is_disabled());
    assert!(
        harness
            .get_by_label("Export WAVs…")
            .accesskit_node()
            .is_disabled()
    );
    harness.set_size(eframe::egui::vec2(640., 600.));
    harness.step();
    harness.get_by_label("Play");
}
