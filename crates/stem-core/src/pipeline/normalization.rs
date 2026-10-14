use crate::{
    StemError,
    audio::{self, CacheReader},
    job::CancellationToken,
    types::{CachedAudio, SeparationResult},
};
use std::path::Path;
pub fn normalize(
    input: &CachedAudio,
    dir: &Path,
    cancel: &CancellationToken,
) -> Result<(CachedAudio, f32, f32), StemError> {
    let mut reader = CacheReader::open(input)?;
    let mut count = 0u64;
    let mut mean = 0f64;
    let mut m2 = 0f64;
    for start in (0..input.spec.frames).step_by(8192) {
        cancel.check()?;
        let data = reader.read_at(start, 8192)?;
        for frame in 0..data[0].len() {
            let x = data.iter().map(|c| f64::from(c[frame])).sum::<f64>() / data.len() as f64;
            count += 1;
            let delta = x - mean;
            mean += delta / count as f64;
            m2 += delta * (x - mean);
        }
    }
    let scale = if count > 1 {
        (m2 / (count - 1) as f64).sqrt()
    } else {
        0.
    };
    let scale = if scale > 1e-8 { scale as f32 } else { 1. };
    let mean = mean as f32;
    let path = dir.join("normalized.f32");
    let mut writer = audio::writer(&path)?;
    for start in (0..input.spec.frames).step_by(8192) {
        cancel.check()?;
        let mut data = reader.read_at(start, 8192)?;
        for c in &mut data {
            for x in c {
                *x = (*x - mean) / scale;
            }
        }
        audio::append(&mut writer, &data, 0, data[0].len())?;
    }
    audio::finish(writer)?;
    Ok((
        CachedAudio {
            path,
            spec: input.spec,
        },
        mean,
        scale,
    ))
}
pub fn restore(
    result: &mut SeparationResult,
    original: &CachedAudio,
    mean: f32,
    scale: f32,
    dir: &Path,
    cancel: &CancellationToken,
) -> Result<(), StemError> {
    for stem in &mut result.stems {
        let path = dir.join(format!("restored-{}.f32", stem.id.as_str()));
        let mut reader = CacheReader::open(&stem.audio)?;
        let mut writer = audio::writer(&path)?;
        for start in (0..stem.audio.spec.frames).step_by(8192) {
            cancel.check()?;
            let mut data = reader.read_at(start, 8192)?;
            for c in &mut data {
                for x in c {
                    *x = *x * scale + mean;
                }
            }
            audio::append(&mut writer, &data, 0, data[0].len())?;
        }
        audio::finish(writer)?;
        drop(reader);
        let restored = CachedAudio {
            path,
            spec: stem.audio.spec,
        };
        stem.audio = audio::resample_to_cache(
            &restored,
            &dir.join(format!("native-{}.f32", stem.id.as_str())),
            original.spec.sample_rate,
            original.spec.channels,
            Some(original.spec.frames),
            cancel,
        )?;
    }
    result.original = original.clone();
    Ok(())
}
