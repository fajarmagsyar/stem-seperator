use std::fs;
use stem_core::{
    audio::{CacheReader, decode_to_cache, resample_to_cache},
    job::CancellationToken,
};

fn wav(path: &std::path::Path, rate: u32, channels: u16, frames: usize) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        for _ in 0..channels {
            writer
                .write_sample(if i == 400 { 0.5_f32 } else { 0.0 })
                .unwrap();
        }
    }
    writer.finalize().unwrap();
}

#[test]
fn decoded_audio_contract() {
    for (rate, channels) in [(44100, 2), (48000, 1)] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("lagu-日本.wav");
        wav(&input, rate, channels, 110251);
        let cache = decode_to_cache(&input, dir.path(), &CancellationToken::default()).unwrap();
        assert_eq!(
            (
                cache.spec.sample_rate,
                cache.spec.channels,
                cache.spec.frames
            ),
            (rate, channels, 110251)
        );
        let mut reader = CacheReader::open(&cache).unwrap();
        let chunk = reader.read_at(399, 3).unwrap();
        assert_eq!(chunk[0], [0.0, 0.5, 0.0]);
        let converted = resample_to_cache(
            &cache,
            &dir.path().join("converted.f32"),
            44100,
            2,
            None,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(
            converted.spec.frames,
            (110251_u64 * 44100).div_ceil(u64::from(rate))
        );
        let back = resample_to_cache(
            &converted,
            &dir.path().join("back.f32"),
            rate,
            channels,
            Some(110251),
            &CancellationToken::default(),
        )
        .unwrap();
        let mut reader = CacheReader::open(&back).unwrap();
        let signal = reader.read_at(0, 110251).unwrap();
        let peak = signal[0]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0;
        assert!(peak.abs_diff(400) <= 1, "resampling added delay: {peak}");
    }
}

#[test]
fn decode_rejects_empty_truncated_multichannel_and_cancelled_inputs() {
    let dir = tempfile::tempdir().unwrap();
    for kind in ["empty", "truncated", "multichannel"] {
        let path = dir.path().join(format!("{kind}.wav"));
        wav(
            &path,
            44100,
            if kind == "multichannel" { 3 } else { 2 },
            if kind == "empty" { 0 } else { 1000 },
        );
        if kind == "truncated" {
            let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.set_len(100).unwrap();
        }
        let cache = tempfile::tempdir().unwrap();
        assert!(
            decode_to_cache(&path, cache.path(), &CancellationToken::default()).is_err(),
            "accepted {kind}"
        );
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }
    let token = CancellationToken::default();
    token.cancel();
    let path = dir.path().join("valid.wav");
    wav(&path, 44100, 2, 100);
    assert!(matches!(
        decode_to_cache(&path, dir.path(), &token),
        Err(stem_core::StemError::Cancelled)
    ));
}

#[test]
fn flac_and_gapless_mp3_decode_at_their_native_rate() {
    for extension in ["flac", "mp3"] {
        let dir = tempfile::tempdir().unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/tone.{extension}"));
        let audio = decode_to_cache(&path, dir.path(), &CancellationToken::default()).unwrap();
        assert_eq!(
            (
                audio.spec.sample_rate,
                audio.spec.channels,
                audio.spec.frames
            ),
            (48000, 2, 12000)
        );
    }
}
