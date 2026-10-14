use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stem-cli"))
        .args(args)
        .output()
        .unwrap()
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "stem-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(
            dir.join("actual.f32"),
            [0.0_f32, 0.5]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        fs::write(
            dir.join("reference.f32"),
            [0.0_f32, 0.25]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        Self(dir)
    }

    fn run(&self, tail: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_stem-cli"))
            .arg("compare-reference")
            .arg(self.0.join("actual.f32"))
            .arg(self.0.join("reference.f32"))
            .args(tail)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn parity_command_has_machine_checkable_pass_and_fail_exit_codes() {
    let fixture = Fixture::new();
    let passing = fixture.run(&["--samples", "2", "--max-abs", "0.25"]);
    assert!(passing.status.success());
    let stdout = String::from_utf8(passing.stdout).unwrap();
    assert!(stdout.contains("samples=2"));
    assert!(stdout.contains("max_abs=0.25"));
    let failing = fixture.run(&["--samples", "2", "--max-abs", "0.24"]);
    assert_eq!(failing.status.code(), Some(1));
    assert!(
        String::from_utf8(failing.stderr)
            .unwrap()
            .contains("exceeds")
    );
    assert_eq!(
        fixture
            .run(&["--samples", "3", "--max-abs", "0.25"])
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn invalid_cli_requests_cannot_report_validation_success() {
    assert_eq!(cli(&[]).status.code(), Some(2));
    assert_eq!(cli(&["separate", "song.wav"]).status.code(), Some(2));
    let fixture = Fixture::new();
    for args in [
        vec![],
        vec!["--samples", "2"],
        vec!["--samples", "2", "--max-abs", "NaN"],
        vec!["--samples", "0", "--max-abs", "1"],
        vec!["--samples", "two", "--max-abs", "1"],
        vec!["--samples", "2", "--samples", "2", "--max-abs", "1"],
        vec!["--samples", "2", "--max-abs", "1", "--typo"],
    ] {
        assert_eq!(
            fixture.run(&args).status.code(),
            Some(2),
            "accepted {args:?}"
        );
    }
    let help = cli(&["--help"]);
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("Offline CPU stem separation")
    );
}

#[test]
fn full_song_and_model_validation_commands_report_missing_assets() {
    let fixture = Fixture::new();
    let output = Command::new(env!("CARGO_BIN_EXE_stem-cli"))
        .arg("separate")
        .args([
            "--package",
            "missing-package",
            "--runtime",
            "missing-runtime",
            "--input",
        ])
        .arg(fixture.0.join("song.wav"))
        .arg("--output")
        .arg(fixture.0.join("exports"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!fixture.0.join("exports").exists());
    assert_eq!(
        cli(&[
            "validate-model",
            "--package",
            "missing",
            "--runtime",
            "missing",
            "--fixture",
            "missing",
            "--reference",
            "missing",
            "--max-abs",
            "0.00005"
        ])
        .status
        .code(),
        Some(1)
    );
}

#[test]
fn inference_command_rejects_invalid_requests_before_creating_outputs() {
    let fixture = Fixture::new();
    let run = |input: &str, tail: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_stem-cli"))
            .args([
                "infer-chunk",
                "--model",
                "missing.onnx",
                "--runtime",
                "missing.so",
                "--input",
            ])
            .arg(fixture.0.join(input))
            .arg("--output")
            .arg(fixture.0.join("output.f32"))
            .args(tail)
            .output()
            .unwrap()
    };
    // A malformed tensor must fail before attempting to load a runtime.
    let malformed = run("actual.f32", &[]);
    assert_eq!(malformed.status.code(), Some(1));
    assert!(
        String::from_utf8(malformed.stderr)
            .unwrap()
            .contains("687960")
    );
    assert!(!fixture.0.join("output.f32").exists());
    for tail in [
        vec!["--threads", "0"],
        vec!["--threads", "two"],
        vec!["--typo", "1"],
    ] {
        assert_eq!(run("actual.f32", &tail).status.code(), Some(2));
    }
}

#[test]
fn unavailable_runtime_returns_an_error_and_preserves_existing_outputs() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("input.f32"), vec![0_u8; 687960 * 4]).unwrap();
    fs::write(fixture.0.join("fake.onnx"), b"invalid model").unwrap();
    fs::write(fixture.0.join("fake.so"), b"invalid library").unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_stem-cli"))
            .arg("infer-chunk")
            .arg("--model")
            .arg(fixture.0.join("fake.onnx"))
            .arg("--runtime")
            .arg(fixture.0.join("fake.so"))
            .arg("--input")
            .arg(fixture.0.join("input.f32"))
            .arg("--output")
            .arg(fixture.0.join("output.f32"))
            .output()
            .unwrap()
    };
    assert_eq!(run().status.code(), Some(1));
    assert!(!fixture.0.join("output.f32").exists());
    fs::write(fixture.0.join("output.f32"), b"keep me").unwrap();
    let existing = run();
    assert_eq!(existing.status.code(), Some(1));
    assert!(
        String::from_utf8(existing.stderr)
            .unwrap()
            .contains("already exists")
    );
    assert_eq!(fs::read(fixture.0.join("output.f32")).unwrap(), b"keep me");
}
