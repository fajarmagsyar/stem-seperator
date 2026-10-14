pub mod normalization;
pub mod package;
use crate::{
    StemError,
    audio::{CacheReader, resample_to_cache},
    job::{CancellationToken, JobEvent},
    model::SeparationBackend,
    types::{AudioChunk, AudioSpec, CachedAudio, SeparationResult, StemFile},
};
use std::path::Path;

pub fn separate(
    input: &CachedAudio,
    backend: &mut dyn SeparationBackend,
    cache_dir: &Path,
    cancel: &CancellationToken,
    emit: &mut dyn FnMut(JobEvent),
) -> Result<SeparationResult, StemError> {
    cancel.check()?;
    let caps = backend.capabilities();
    let length = caps.chunk_frames();
    let overlap = length / 4;
    let stride = length - overlap;
    let sources = caps.stems().to_vec();
    let model_rate = caps.sample_rate();
    let model_channels = caps.channels();
    let estimated = (u128::from(input.spec.frames)
        * u128::from(input.spec.channels)
        * 4
        * (sources.len() as u128 * 3 + 4)
        * u128::from(model_rate.max(input.spec.sample_rate)))
    .div_ceil(u128::from(input.spec.sample_rate));
    if u128::from(fs2::available_space(cache_dir)?) < estimated {
        return Err(StemError::Io(std::io::Error::other(
            "Not enough free disk space for the separation cache",
        )));
    }
    emit(JobEvent::Progress {
        phase: "Resampling input",
        completed: 0,
        total: 1,
    });
    let prepared = resample_to_cache(
        input,
        &cache_dir.join("model-input.f32"),
        model_rate,
        model_channels,
        None,
        cancel,
    )?;
    let mut reader = CacheReader::open(&prepared)?;
    let mut outputs = sources
        .iter()
        .map(|(id, _)| crate::audio::writer(&cache_dir.join(format!("model-{}.f32", id.as_str()))))
        .collect::<Result<Vec<_>, _>>()?;
    let mut sums = vec![vec![vec![0.0_f32; length]; model_channels as usize]; sources.len()];
    let mut weights = vec![0.0_f32; length];
    let chunks = prepared.spec.frames.div_ceil(stride as u64);
    emit(JobEvent::Progress {
        phase: "Separating",
        completed: 0,
        total: chunks,
    });
    for chunk in 0..chunks {
        cancel.check()?;
        let start = chunk * stride as u64;
        let valid = (prepared.spec.frames - start).min(length as u64) as usize;
        let mut audio = reader.read_at(start, length)?;
        for channel in &mut audio {
            channel.resize(length, 0.0);
        }
        let audio = AudioChunk::new(audio)?;
        let stems = backend.infer(&audio)?;
        cancel.check()?;
        if stems.stems().len() != sources.len() {
            return Err(StemError::Inference("Backend changed source count".into()));
        }
        for (index, (id, output)) in stems.stems().iter().enumerate() {
            if id != &sources[index].0
                || output.frames() != length
                || output.channels().len() != model_channels as usize
            {
                return Err(StemError::Inference(
                    "Backend changed audio contract".into(),
                ));
            }
        }
        for frame in 0..valid {
            let weight = if overlap == 0 {
                1.0
            } else {
                ((frame + 1).min(length - frame).min(overlap + 1)) as f32 / (overlap + 1) as f32
            };
            weights[frame] += weight;
            for (index, (_, output)) in stems.stems().iter().enumerate() {
                for (sum, channel) in sums[index].iter_mut().zip(output.channels()) {
                    sum[frame] += channel[frame] * weight;
                }
            }
        }
        let publish = if chunk + 1 == chunks {
            valid
        } else {
            stride.min(valid)
        };
        for (index, target) in outputs.iter_mut().enumerate() {
            for channel in &mut sums[index] {
                for frame in 0..publish {
                    channel[frame] /= weights[frame];
                }
            }
            crate::audio::append(target, &sums[index], 0, publish)?;
        }
        for source in &mut sums {
            for channel in source {
                channel.copy_within(publish.., 0);
                channel[length - publish..].fill(0.0);
            }
        }
        weights.copy_within(publish.., 0);
        weights[length - publish..].fill(0.0);
        emit(JobEvent::Progress {
            phase: "Separating",
            completed: chunk + 1,
            total: chunks,
        });
    }
    for writer in outputs {
        crate::audio::finish(writer)?;
    }
    let mut stems = Vec::new();
    for (index, (id, label)) in sources.into_iter().enumerate() {
        cancel.check()?;
        emit(JobEvent::Progress {
            phase: "Preparing stems",
            completed: index as u64,
            total: backend.capabilities().stems().len() as u64,
        });
        let model_audio = CachedAudio {
            path: cache_dir.join(format!("model-{}.f32", id.as_str())),
            spec: AudioSpec {
                sample_rate: model_rate,
                channels: model_channels,
                frames: prepared.spec.frames,
            },
        };
        let audio = resample_to_cache(
            &model_audio,
            &cache_dir.join(format!("stem-{}.f32", id.as_str())),
            input.spec.sample_rate,
            input.spec.channels,
            Some(input.spec.frames),
            cancel,
        )?;
        stems.push(StemFile { id, label, audio });
    }
    cancel.check()?;
    Ok(SeparationResult {
        original: input.clone(),
        stems,
        model_id: "htdemucs".into(),
    })
}
