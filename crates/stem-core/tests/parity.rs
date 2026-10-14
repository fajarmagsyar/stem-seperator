use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use stem_core::parity::compare_f32;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Files {
    root: PathBuf,
    actual: PathBuf,
    reference: PathBuf,
}

impl Files {
    fn new(actual: &[f32], reference: &[f32]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "stem-parity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let files = Self {
            actual: root.join("actual.f32"),
            reference: root.join("reference.f32"),
            root,
        };
        fs::write(
            &files.actual,
            actual
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        fs::write(
            &files.reference,
            reference
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        files
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn comparison_measures_real_errors_and_enforces_explicit_tolerance() {
    let files = Files::new(&[0.0, 0.5, -0.5, 0.0], &[0.0, 0.25, -0.25, 0.0]);
    let metrics = compare_f32(&files.actual, &files.reference, 4, 0.25).unwrap();
    assert_eq!(metrics.samples, 4);
    assert_eq!(metrics.max_abs, 0.25);
    assert!((metrics.rmse - 0.1767766952966369).abs() < 1e-12);
    assert!(compare_f32(&files.actual, &files.reference, 4, 0.24).is_err());
}

#[test]
fn comparison_rejects_incomplete_wrong_shape_and_nonfinite_outputs() {
    let files = Files::new(&[0.0, 1.0], &[0.0, 1.0]);
    assert!(compare_f32(&files.actual, &files.reference, 3, 1.0).is_err());
    for bad in [
        vec![],
        vec![0.0],
        vec![0.0, 1.0, 2.0],
        vec![f32::NAN, 1.0],
        vec![0.0, f32::INFINITY],
    ] {
        fs::write(
            &files.actual,
            bad.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(compare_f32(&files.actual, &files.reference, 2, 1.0).is_err());
    }
    fs::write(&files.actual, [0_u8; 7]).unwrap();
    assert!(compare_f32(&files.actual, &files.reference, 2, 1.0).is_err());
    fs::write(&files.actual, [0_u8; 8]).unwrap();
    fs::write(&files.reference, f32::NAN.to_le_bytes().repeat(2)).unwrap();
    assert!(compare_f32(&files.actual, &files.reference, 2, 1.0).is_err());
}

#[test]
fn comparison_rejects_invalid_parameters_and_missing_files() {
    let files = Files::new(&[0.0], &[0.0]);
    for tolerance in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(compare_f32(&files.actual, &files.reference, 1, tolerance).is_err());
    }
    assert!(compare_f32(&files.actual, &files.reference, 0, 1.0).is_err());
    fs::remove_file(&files.actual).unwrap();
    assert!(compare_f32(&files.actual, &files.reference, 1, 1.0).is_err());
}
