use stem_core::{audio::write_cache, job::CancellationToken};
use stem_desktop::waveform::scan;
#[test]
fn peaks_cover_edges_channels_and_silence() {
    let dir = tempfile::tempdir().unwrap();
    let audio = write_cache(
        &dir.path().join("audio"),
        44100,
        vec![vec![1., 0., 0., 0., 0.], vec![0., 0., 0., 0., -0.7]],
    )
    .unwrap();
    let peaks = scan(&audio, 3, &CancellationToken::default()).unwrap();
    assert_eq!(peaks.len(), 3);
    assert_eq!(peaks[0].max, 1.);
    assert_eq!(peaks[2].min, -0.7);
    assert_eq!(peaks[1].max, 0.);
    let token = CancellationToken::default();
    token.cancel();
    assert!(scan(&audio, 3, &token).is_err());
}
