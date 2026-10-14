use stem_core::types::{AudioChunk, StemId};

#[test]
fn stem_ids_cannot_escape_export_directories_or_name_windows_devices() {
    for value in [
        "",
        "../vocals",
        "a/b",
        "a\\b",
        "C:stem",
        ".",
        "con",
        "aux",
        "nul",
        "prn",
        "com1",
        "lpt9",
        " vocals",
        "vocals.",
        "Drums",
        "🎹",
    ] {
        assert!(StemId::new(value).is_err(), "accepted unsafe ID: {value:?}");
    }
    assert!(StemId::new("v".repeat(65)).is_err());
    for value in ["vocals", "lead-vocals", "drum_1", "strings"] {
        assert_eq!(StemId::new(value).unwrap().as_str(), value);
    }
}

#[test]
fn chunks_reject_unequal_channels_and_nonfinite_samples() {
    for channels in [
        vec![],
        vec![vec![]],
        vec![vec![1.0], vec![1.0, 2.0]],
        vec![vec![f32::NAN]],
        vec![vec![f32::INFINITY]],
        vec![vec![0.0]; 3],
    ] {
        assert!(AudioChunk::new(channels).is_err());
    }
    let chunk = AudioChunk::new(vec![vec![0.0, 1.5], vec![-1.5, 0.0]]).unwrap();
    assert_eq!(chunk.channels(), &[vec![0.0, 1.5], vec![-1.5, 0.0]]);
    assert!(AudioChunk::new(vec![vec![0.0]]).is_ok());
}
