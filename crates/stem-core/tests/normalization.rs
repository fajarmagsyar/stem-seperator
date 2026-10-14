use stem_core::{
    audio::{CacheReader, write_cache},
    job::CancellationToken,
    pipeline::normalization::normalize,
};
#[test]
fn song_normalization_handles_dc_and_stereo_mean() {
    let dir = tempfile::tempdir().unwrap();
    let token = CancellationToken::default();
    let audio = write_cache(
        &dir.path().join("input"),
        44100,
        vec![vec![1., 2., 3.], vec![3., 4., 5.]],
    )
    .unwrap();
    let (normalized, mean, scale) = normalize(&audio, dir.path(), &token).unwrap();
    assert_eq!(mean, 3.);
    assert_eq!(scale, 1.);
    assert_eq!(
        CacheReader::open(&normalized)
            .unwrap()
            .read_at(0, 3)
            .unwrap()[0],
        vec![-2., -1., 0.]
    );
    let dcdir = tempfile::tempdir().unwrap();
    let dc = write_cache(&dcdir.path().join("input"), 44100, vec![vec![0.5; 3]]).unwrap();
    let (normalized, _, scale) = normalize(&dc, dcdir.path(), &token).unwrap();
    assert_eq!(scale, 1.);
    assert_eq!(
        CacheReader::open(&normalized)
            .unwrap()
            .read_at(0, 3)
            .unwrap()[0],
        vec![0.; 3]
    );
}
