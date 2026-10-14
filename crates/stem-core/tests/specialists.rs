use stem_core::{
    audio::write_cache,
    job::CancellationToken,
    model::{ModelCapabilities, SeparationBackend, StemChunk},
    types::{AudioChunk, StemId},
};
struct Identity {
    caps: ModelCapabilities,
}
impl SeparationBackend for Identity {
    fn capabilities(&self) -> &ModelCapabilities {
        &self.caps
    }
    fn infer(&mut self, audio: &AudioChunk) -> Result<StemChunk, stem_core::StemError> {
        Ok(StemChunk::new(
            &self.caps,
            audio,
            vec![(
                self.caps.stems()[0].0.clone(),
                AudioChunk::new(audio.channels().to_vec())?,
            )],
        )?)
    }
}
#[test]
fn passes_are_sequential_aligned_and_cancellable() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_cache(
        &dir.path().join("input"),
        44100,
        vec![vec![0.2; 21], vec![-0.3; 21]],
    )
    .unwrap();
    let sources =
        ["drums", "bass", "other", "vocals"].map(|id| (StemId::new(id).unwrap(), id.to_string()));
    let mut calls = 0;
    let result = stem_core::pipeline::package::separate_passes(
        &input,
        &sources,
        dir.path(),
        &CancellationToken::default(),
        &mut |_| {},
        |index| {
            assert_eq!(index, calls);
            calls += 1;
            Ok(Box::new(Identity {
                caps: ModelCapabilities::new(44100, 2, 8, vec![sources[index].clone()]).unwrap(),
            }))
        },
    )
    .unwrap();
    assert_eq!(calls, 4);
    assert_eq!(result.stems.len(), 4);
    for (stem, (id, _)) in result.stems.iter().zip(&sources) {
        assert_eq!(&stem.id, id);
        assert_eq!(stem.audio.spec, input.spec);
    }
    let token = CancellationToken::default();
    token.cancel();
    assert!(
        stem_core::pipeline::package::separate_passes(
            &input,
            &sources,
            dir.path(),
            &token,
            &mut |_| {},
            |_| panic!("cancelled before load")
        )
        .is_err()
    );
}
