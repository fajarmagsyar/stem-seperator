use stem_core::{
    model::{ModelCapabilities, StemChunk},
    types::{AudioChunk, StemId},
};

fn id(value: &str) -> StemId {
    StemId::new(value).unwrap()
}
fn stereo(frames: usize) -> AudioChunk {
    AudioChunk::new(vec![vec![0.0; frames]; 2]).unwrap()
}
fn source(value: &str) -> (StemId, String) {
    (id(value), value.into())
}

#[test]
fn model_contract_rejects_ambiguous_sources_and_invalid_audio_dimensions() {
    assert!(ModelCapabilities::new(0, 2, 4, vec![source("vocals")]).is_err());
    assert!(ModelCapabilities::new(44100, 0, 4, vec![source("vocals")]).is_err());
    assert!(ModelCapabilities::new(44100, 3, 4, vec![source("vocals")]).is_err());
    assert!(ModelCapabilities::new(44100, 2, 0, vec![source("vocals")]).is_err());
    assert!(ModelCapabilities::new(44100, 2, 4, vec![]).is_err());
    assert!(ModelCapabilities::new(44100, 2, 4, vec![source("vocals"), source("vocals")]).is_err());
    assert!(ModelCapabilities::new(44100, 2, 4, vec![(id("vocals"), " ".into())]).is_err());
}

#[test]
fn backend_outputs_must_follow_declared_sources_and_exact_chunk_dimensions() {
    let model =
        ModelCapabilities::new(44100, 2, 4, vec![source("strings"), source("synth")]).unwrap();
    let input = stereo(4);
    let valid = StemChunk::new(
        &model,
        &input,
        vec![(id("strings"), stereo(4)), (id("synth"), stereo(4))],
    )
    .unwrap();
    assert_eq!(valid.stems()[0].0.as_str(), "strings");
    for outputs in [
        vec![],
        vec![(id("strings"), stereo(4))],
        vec![(id("synth"), stereo(4)), (id("strings"), stereo(4))],
        vec![(id("strings"), stereo(4)), (id("other"), stereo(4))],
        vec![(id("strings"), stereo(3)), (id("synth"), stereo(4))],
        vec![
            (id("strings"), AudioChunk::new(vec![vec![0.0; 4]]).unwrap()),
            (id("synth"), stereo(4)),
        ],
    ] {
        assert!(StemChunk::new(&model, &input, outputs).is_err());
    }
    assert!(
        StemChunk::new(
            &model,
            &stereo(3),
            vec![(id("strings"), stereo(4)), (id("synth"), stereo(4))]
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires real model, runtime, fixture and independent reference output"]
fn six_stem_cpu_matches_reference() {
    use std::{fs, path::PathBuf};
    use stem_core::model::{SeparationBackend, onnx::OnnxBackend};
    let asset = |name| PathBuf::from(std::env::var_os(name).expect(name));
    let mut backend =
        OnnxBackend::load_development(&asset("STEM_MODEL"), &asset("STEM_RUNTIME"), 2).unwrap();
    let read = |name| {
        let bytes = fs::read(asset(name)).unwrap();
        assert_eq!(bytes.len() % 4, 0);
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect::<Vec<_>>()
    };
    let input = read("STEM_FIXTURE");
    assert_eq!(input.len(), 2 * 343980);
    let input = AudioChunk::new(
        input
            .as_chunks::<343980>()
            .0
            .iter()
            .map(|v| v.to_vec())
            .collect(),
    )
    .unwrap();
    let reference = read("STEM_REFERENCE");
    assert_eq!(reference.len(), 6 * 2 * 343980);
    let tolerance: f64 = std::env::var("STEM_MAX_ABS")
        .expect("STEM_MAX_ABS: measured explicit tolerance required")
        .parse()
        .unwrap();
    assert!(tolerance.is_finite() && tolerance >= 0.0);
    let output = backend.infer(&input).unwrap();
    let expected_ids = ["drums", "bass", "other", "vocals", "guitar", "piano"];
    for (index, (id, audio)) in output.stems().iter().enumerate() {
        assert_eq!(id.as_str(), expected_ids[index]);
        assert_eq!(audio.frames(), 343980);
        assert_eq!(audio.channels().len(), 2);
        let expected = &reference[index * 2 * 343980..(index + 1) * 2 * 343980];
        let mut max_abs = 0.0_f64;
        let mut squared = 0.0_f64;
        for (&actual, &reference) in audio.channels().iter().flatten().zip(expected) {
            assert!(actual.is_finite() && reference.is_finite());
            let error = (f64::from(actual) - f64::from(reference)).abs();
            max_abs = max_abs.max(error);
            squared += error * error;
        }
        println!(
            "{} max_abs={} rmse={}",
            id.as_str(),
            max_abs,
            (squared / expected.len() as f64).sqrt()
        );
        assert!(max_abs <= tolerance, "{} exceeds tolerance", id.as_str());
    }
}
