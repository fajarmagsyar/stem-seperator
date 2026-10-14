use stem_core::{
    audio::write_cache,
    export::{ExportRequest, MixSetting, export},
    job::CancellationToken,
    types::{SeparationResult, StemFile, StemId},
};

#[test]
fn export_float_alignment_and_mix_amplitude() {
    let cache = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let audio = write_cache(
        &cache.path().join("input.f32"),
        48000,
        vec![vec![0.5; 101]; 2],
    )
    .unwrap();
    let id = StemId::new("vocals").unwrap();
    let result = SeparationResult {
        original: audio.clone(),
        stems: vec![StemFile {
            id: id.clone(),
            label: "../unsafe display label".into(),
            audio,
        }],
        model_id: "model".into(),
    };
    let request = ExportRequest {
        output_root: output.path().join("音楽"),
        stem_ids: vec![id.clone()],
        mix: Some(vec![MixSetting {
            id: id.clone(),
            gain: 0.25,
            mute: false,
            solo: false,
        }]),
    };
    let path = export(
        &result,
        &request,
        &CancellationToken::default(),
        &mut |_| {},
    )
    .unwrap();
    for (name, amplitude) in [("vocals.wav", 0.5_f32), ("_audition-mix.wav", 0.125)] {
        let mut reader = hound::WavReader::open(path.join(name)).unwrap();
        assert_eq!(
            (
                reader.spec().sample_rate,
                reader.spec().channels,
                reader.duration()
            ),
            (48000, 2, 101)
        );
        assert_eq!(reader.spec().sample_format, hound::SampleFormat::Float);
        assert_eq!(reader.spec().bits_per_sample, 32);
        assert!(reader.samples::<f32>().all(|x| x.unwrap() == amplitude));
    }
    let existing = std::fs::read(path.join("vocals.wav")).unwrap();
    let second = export(
        &result,
        &request,
        &CancellationToken::default(),
        &mut |_| {},
    )
    .unwrap();
    assert_ne!(second, path);
    assert_eq!(std::fs::read(path.join("vocals.wav")).unwrap(), existing);
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(export(&result, &request, &cancel, &mut |_| {}).is_err());
    assert!(result.original.path.exists());
}

#[test]
fn export_failure_preserves_results_and_preexisting_files() {
    let cache = tempfile::tempdir().unwrap();
    let audio = write_cache(&cache.path().join("input.f32"), 44100, vec![vec![0.2; 100]]).unwrap();
    let id = StemId::new("vocals").unwrap();
    let result = SeparationResult {
        original: audio.clone(),
        stems: vec![StemFile {
            id: id.clone(),
            label: "Voice".into(),
            audio,
        }],
        model_id: "model".into(),
    };
    let blocked = cache.path().join("file-as-directory");
    std::fs::write(&blocked, b"keep").unwrap();
    let request = ExportRequest {
        output_root: blocked.clone(),
        stem_ids: vec![id.clone()],
        mix: None,
    };
    assert!(
        export(
            &result,
            &request,
            &CancellationToken::default(),
            &mut |_| {}
        )
        .is_err()
    );
    assert_eq!(std::fs::read(blocked).unwrap(), b"keep");
    let output = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::default();
    let request = ExportRequest {
        output_root: output.path().to_path_buf(),
        stem_ids: vec![id],
        mix: None,
    };
    assert!(export(&result, &request, &cancel, &mut |_| cancel.cancel()).is_err());
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
    assert!(result.stems[0].audio.path.exists());
}

#[test]
fn stem_named_mix_can_be_exported_with_audition_mix() {
    let cache = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let audio = write_cache(&cache.path().join("input.f32"), 44100, vec![vec![0.2; 100]]).unwrap();
    let id = StemId::new("mix").unwrap();
    let result = SeparationResult {
        original: audio.clone(),
        stems: vec![StemFile {
            id: id.clone(),
            label: "Mix stem".into(),
            audio,
        }],
        model_id: "test".into(),
    };
    let request = ExportRequest {
        output_root: output.path().into(),
        stem_ids: vec![id.clone()],
        mix: Some(vec![MixSetting {
            id,
            gain: 1.0,
            mute: false,
            solo: false,
        }]),
    };
    let path = export(
        &result,
        &request,
        &CancellationToken::default(),
        &mut |_| {},
    )
    .unwrap();
    assert!(path.join("mix.wav").is_file());
    assert!(path.join("_audition-mix.wav").is_file());
}
