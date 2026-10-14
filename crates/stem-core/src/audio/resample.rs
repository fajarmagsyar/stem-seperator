use crate::{
    StemError,
    job::CancellationToken,
    types::{AudioSpec, CachedAudio},
};
use rubato::{FftFixedIn, Resampler};
use std::path::Path;

pub fn resample_to_cache(
    input: &CachedAudio,
    path: &Path,
    rate: u32,
    channels: u16,
    exact_frames: Option<u64>,
    cancel: &CancellationToken,
) -> Result<CachedAudio, StemError> {
    cancel.check()?;
    input.spec.validate()?;
    let target_frames = exact_frames.unwrap_or(
        (u128::from(input.spec.frames) * u128::from(rate))
            .div_ceil(u128::from(input.spec.sample_rate)) as u64,
    );
    let spec = AudioSpec {
        sample_rate: rate,
        channels,
        frames: target_frames,
    };
    spec.validate()?;
    let mut reader = super::CacheReader::open(input)?;
    let mut writer = super::writer(path)?;
    let result = (|| {
        let mut cursor = 0_u64;
        let mut written = 0_u64;
        let mut converter = if rate != input.spec.sample_rate {
            Some(
                FftFixedIn::<f32>::new(
                    input.spec.sample_rate as usize,
                    rate as usize,
                    1024,
                    2,
                    channels as usize,
                )
                .map_err(|e| StemError::Decode(e.to_string()))?,
            )
        } else {
            None
        };
        let mut delay = converter.as_ref().map_or(0, |r| r.output_delay());
        while written < target_frames {
            cancel.check()?;
            let needed = converter.as_ref().map_or(1024, |r| r.input_frames_next());
            let raw = reader.read_at(cursor, needed)?;
            let available = raw[0].len();
            cursor += available as u64;
            let mut mapped = vec![vec![0.0; needed]; channels as usize];
            for frame in 0..available {
                for (channel, output) in mapped.iter_mut().enumerate() {
                    output[frame] = match (input.spec.channels, channels) {
                        (1, 2) => raw[0][frame],
                        (2, 1) => (raw[0][frame] + raw[1][frame]) * 0.5,
                        _ => raw[channel][frame],
                    };
                }
            }
            let output = if let Some(converter) = converter.as_mut() {
                converter
                    .process(&mapped, None)
                    .map_err(|e| StemError::Decode(e.to_string()))?
            } else {
                mapped
            };
            let skip = delay.min(output[0].len());
            delay -= skip;
            let count = (output[0].len() - skip).min((target_frames - written) as usize);
            super::append(&mut writer, &output, skip, count)?;
            written += count as u64;
        }
        cancel.check()?;
        super::finish(writer)?;
        Ok(CachedAudio {
            path: path.to_path_buf(),
            spec,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}
