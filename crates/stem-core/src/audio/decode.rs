use crate::{
    StemError,
    job::CancellationToken,
    types::{AudioSpec, CachedAudio},
};
use std::{fs::File, io::ErrorKind, path::Path};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
    io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

pub fn decode_to_cache(
    input: &Path,
    cache_dir: &Path,
    cancel: &CancellationToken,
) -> Result<CachedAudio, StemError> {
    cancel.check()?;
    let error = |e: Error| StemError::Decode(format!("Cannot decode {}: {e}", input.display()));
    let source = MediaSourceStream::new(Box::new(File::open(input)?), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = input.extension().and_then(|x| x.to_str()) {
        hint.with_extension(extension);
    }
    let options = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let probe = symphonia::default::get_probe()
        .format(&hint, source, &options, &MetadataOptions::default())
        .map_err(error)?;
    let mut format = probe.format;
    let track = format
        .default_track()
        .ok_or_else(|| StemError::Decode("No audio track".into()))?;
    let id = track.id;
    let params = track.codec_params.clone();
    let rate = params
        .sample_rate
        .ok_or_else(|| StemError::Decode("Missing sample rate".into()))?;
    let channels = params
        .channels
        .ok_or_else(|| StemError::Decode("Missing channel layout".into()))?
        .count();
    if !(1..=2).contains(&channels) {
        return Err(StemError::Decode(
            "Only mono and stereo audio are supported".into(),
        ));
    }
    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions { verify: true })
        .map_err(error)?;
    let target = cache_dir.join("original.f32");
    let mut writer = super::writer(&target)?;
    let result = (|| {
        let mut frames = 0_u64;
        loop {
            cancel.check()?;
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(error(e)),
            };
            if packet.track_id() != id {
                continue;
            }
            let decoded = decoder.decode(&packet).map_err(error)?;
            if decoded.spec().rate != rate || decoded.spec().channels.count() != channels {
                return Err(StemError::Decode(
                    "Audio format changed during decoding".into(),
                ));
            }
            let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
            buffer.copy_interleaved_ref(decoded);
            use std::io::Write;
            for &sample in buffer.samples() {
                if !sample.is_finite() {
                    return Err(StemError::Decode(
                        "Input contains non-finite samples".into(),
                    ));
                }
                writer.write_all(&sample.to_le_bytes())?;
            }
            frames += buffer.samples().len() as u64 / channels as u64;
        }
        if frames == 0
            || params.n_frames.is_some_and(|expected| {
                frames
                    < expected.saturating_sub(
                        u64::from(params.delay.unwrap_or(0))
                            + u64::from(params.padding.unwrap_or(0)),
                    )
            })
        {
            return Err(StemError::Decode("Audio is empty or truncated".into()));
        }
        let spec = AudioSpec {
            sample_rate: rate,
            channels: channels as u16,
            frames,
        };
        spec.validate()?;
        cancel.check()?;
        super::finish(writer)?;
        Ok(CachedAudio {
            path: target.clone(),
            spec,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&target);
    }
    result
}
