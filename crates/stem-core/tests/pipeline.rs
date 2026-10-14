use stem_core::{
    audio::write_cache,
    job::CancellationToken,
    model::{ModelCapabilities, SeparationBackend, StemChunk},
    pipeline::separate,
    types::{AudioChunk, StemId},
};

struct Identity(ModelCapabilities);
impl SeparationBackend for Identity {
    fn capabilities(&self) -> &ModelCapabilities {
        &self.0
    }
    fn infer(&mut self, input: &AudioChunk) -> Result<StemChunk, stem_core::StemError> {
        Ok(StemChunk::new(
            &self.0,
            input,
            self.0
                .stems()
                .iter()
                .map(|(id, _)| (id.clone(), input.clone()))
                .collect(),
        )?)
    }
}

#[test]
fn reconstruction_alignment_and_join_impulses() {
    for (rate, channels) in [(44100, 2), (48000, 1)] {
        let dir = tempfile::tempdir().unwrap();
        let mut signal = vec![0.25; 1001];
        for i in [0, 23, 24, 31, 32, 47, 48, 1000] {
            signal[i] = 0.75;
        }
        let input = write_cache(
            &dir.path().join("input.f32"),
            rate,
            vec![signal.clone(); channels],
        )
        .unwrap();
        let caps = ModelCapabilities::new(
            44100,
            2,
            32,
            vec![(StemId::new("vocals").unwrap(), "Voice".into())],
        )
        .unwrap();
        let result = separate(
            &input,
            &mut Identity(caps),
            dir.path(),
            &CancellationToken::default(),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(result.stems[0].audio.spec, input.spec);
        let mut reader = stem_core::audio::CacheReader::open(&result.stems[0].audio).unwrap();
        let out = reader.read_at(0, 1001).unwrap();
        if rate == 44100 {
            for (&a, &b) in out[0].iter().zip(&signal) {
                assert!((a - b).abs() < 1e-6);
            }
        }
        assert!(out.iter().flatten().all(|x| x.is_finite()));
    }
}

#[test]
fn separation_cancellation_never_returns_success() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_cache(
        &dir.path().join("input.f32"),
        44100,
        vec![vec![0.0; 100]; 2],
    )
    .unwrap();
    let token = CancellationToken::default();
    let caps = ModelCapabilities::new(
        44100,
        2,
        32,
        vec![(StemId::new("vocals").unwrap(), "Voice".into())],
    )
    .unwrap();
    let result = separate(&input, &mut Identity(caps), dir.path(), &token, &mut |_| {
        token.cancel()
    });
    assert!(matches!(result, Err(stem_core::StemError::Cancelled)));
}
