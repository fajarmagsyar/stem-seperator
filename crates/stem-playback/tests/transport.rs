use stem_core::{
    audio::write_cache,
    export::MixSetting,
    types::{SeparationResult, StemFile, StemId},
};
use stem_playback::transport::DiskMixer;

#[test]
fn shared_transport_clock() {
    let dir = tempfile::tempdir().unwrap();
    let original = write_cache(
        &dir.path().join("original.f32"),
        44100,
        vec![vec![0.5; 1000]; 2],
    )
    .unwrap();
    let a = write_cache(&dir.path().join("a.f32"), 44100, vec![vec![0.1; 1000]; 2]).unwrap();
    let b = write_cache(&dir.path().join("b.f32"), 44100, vec![vec![0.2; 1000]; 2]).unwrap();
    let id_a = StemId::new("drums").unwrap();
    let id_b = StemId::new("bass").unwrap();
    let result = SeparationResult {
        original,
        stems: vec![
            StemFile {
                id: id_a.clone(),
                label: "Drums".into(),
                audio: a,
            },
            StemFile {
                id: id_b.clone(),
                label: "Bass".into(),
                audio: b,
            },
        ],
        model_id: "test".into(),
    };
    let mut mixer = DiskMixer::new(&result).unwrap();
    let settings = vec![
        MixSetting {
            id: id_a.clone(),
            gain: 1.0,
            mute: false,
            solo: true,
        },
        MixSetting {
            id: id_b.clone(),
            gain: 1.0,
            mute: false,
            solo: false,
        },
    ];
    mixer.set_mix(&settings).unwrap();
    mixer.seek(100).unwrap();
    let audio = mixer.render(512, false).unwrap();
    assert!((audio.last().unwrap()[0] - 0.1).abs() < 1e-6);
    assert_eq!(mixer.position(), 612);
    let original = mixer.render(100, true).unwrap();
    assert_eq!(original[99], [0.5, 0.5]);
    assert_eq!(mixer.position(), 712);
    mixer.seek(999).unwrap();
    assert_eq!(mixer.render(100, false).unwrap().len(), 1);
    assert!(mixer.render(100, false).unwrap().is_empty());
    assert!(mixer.seek(1001).is_err());
    let mut converted = stem_playback::transport::RateMixer::new(&result, 48000).unwrap();
    let mut frames = Vec::new();
    loop {
        let block = converted.next_block().unwrap();
        if block.is_empty() {
            break;
        }
        frames.extend(block);
    }
    assert_eq!(frames.len(), 1089);
    assert_eq!(frames.last().unwrap().1, 1000);
    converted.seek(500).unwrap();
    let first = converted.next_block().unwrap();
    assert!(first[0].1 >= 500);
}
