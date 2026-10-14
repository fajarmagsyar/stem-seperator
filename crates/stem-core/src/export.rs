use crate::{
    StemError,
    audio::CacheReader,
    job::{CancellationToken, JobEvent, reserve_directory},
    types::{SeparationResult, StemId},
};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MixSetting {
    pub id: StemId,
    pub gain: f32,
    pub mute: bool,
    pub solo: bool,
}
#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub output_root: PathBuf,
    pub stem_ids: Vec<StemId>,
    pub mix: Option<Vec<MixSetting>>,
}

pub fn gains(ids: &[StemId], settings: &[MixSetting]) -> Result<Vec<f32>, StemError> {
    if settings.len() != ids.len()
        || settings
            .iter()
            .any(|s| !s.gain.is_finite() || !(0.0..=4.0).contains(&s.gain))
    {
        return Err(StemError::Export("Invalid mixer settings".into()));
    }
    let solo = settings.iter().any(|s| s.solo);
    let mut result = Vec::new();
    for id in ids {
        let matches = settings.iter().filter(|s| &s.id == id).collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(StemError::Export(
                "Mixer IDs must match the result exactly".into(),
            ));
        }
        let s = matches[0];
        result.push(if s.mute || (solo && !s.solo) {
            0.0
        } else {
            s.gain
        });
    }
    Ok(result)
}

pub fn export(
    result: &SeparationResult,
    request: &ExportRequest,
    cancel: &CancellationToken,
    emit: &mut dyn FnMut(JobEvent),
) -> Result<PathBuf, StemError> {
    cancel.check()?;
    result.original.spec.validate()?;
    if request.stem_ids.is_empty() && request.mix.is_none() {
        return Err(StemError::Export(
            "Select at least one stem or a mix".into(),
        ));
    }
    let mut selected = std::collections::HashSet::new();
    for id in &request.stem_ids {
        if !selected.insert(id) || !result.stems.iter().any(|s| &s.id == id) {
            return Err(StemError::Export("Unknown or repeated export stem".into()));
        }
    }
    for stem in &result.stems {
        if stem.audio.spec != result.original.spec {
            return Err(StemError::Export(
                "Stems are not aligned with the original".into(),
            ));
        }
    }
    let ids = result
        .stems
        .iter()
        .map(|s| s.id.clone())
        .collect::<Vec<_>>();
    let mix_gains = request
        .mix
        .as_ref()
        .map(|mix| gains(&ids, mix))
        .transpose()?;
    let spec = result.original.spec;
    let bytes = spec
        .frames
        .checked_mul(u64::from(spec.channels) * 4)
        .ok_or_else(|| StemError::Export("Audio is too large".into()))?;
    if bytes > u64::from(u32::MAX) - 128 {
        return Err(StemError::Export(
            "This audio exceeds the 4 GiB WAV limit".into(),
        ));
    }
    std::fs::create_dir_all(&request.output_root)?;
    if fs2::available_space(&request.output_root)?
        < bytes.saturating_mul(request.stem_ids.len() as u64 + u64::from(mix_gains.is_some()))
    {
        return Err(StemError::Export("Not enough disk space for export".into()));
    }
    let output = reserve_directory(&request.output_root, "stems")?;
    let operation = (|| {
        let wav_spec = hound::WavSpec {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let total = request.stem_ids.len() as u64 + u64::from(mix_gains.is_some());
        let mut completed = 0;
        for stem in &result.stems {
            if !selected.contains(&stem.id) {
                continue;
            }
            cancel.check()?;
            emit(JobEvent::Progress {
                phase: "Exporting",
                completed,
                total,
            });
            cancel.check()?;
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output.join(format!("{}.wav", stem.id.as_str())))?;
            let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), wav_spec)
                .map_err(|e| StemError::Export(e.to_string()))?;
            let mut reader = CacheReader::open(&stem.audio)?;
            let mut start = 0;
            while start < spec.frames {
                cancel.check()?;
                let channels = reader.read_at(start, 4096)?;
                for frame in 0..channels[0].len() {
                    for channel in &channels {
                        writer
                            .write_sample(channel[frame])
                            .map_err(|e| StemError::Export(e.to_string()))?;
                    }
                }
                start += channels[0].len() as u64;
            }
            writer
                .finalize()
                .map_err(|e| StemError::Export(e.to_string()))?;
            completed += 1;
        }
        if let Some(gains) = mix_gains {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output.join("_audition-mix.wav"))?;
            let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), wav_spec)
                .map_err(|e| StemError::Export(e.to_string()))?;
            let mut readers = result
                .stems
                .iter()
                .map(|s| CacheReader::open(&s.audio))
                .collect::<Result<Vec<_>, _>>()?;
            let mut start = 0;
            while start < spec.frames {
                cancel.check()?;
                let audio = readers
                    .iter_mut()
                    .map(|r| r.read_at(start, 4096))
                    .collect::<Result<Vec<_>, _>>()?;
                let count = (spec.frames - start).min(4096) as usize;
                for frame in 0..count {
                    for channel in 0..spec.channels as usize {
                        let sample = audio
                            .iter()
                            .zip(&gains)
                            .map(|(s, g)| s[channel][frame] * g)
                            .sum::<f32>();
                        if !sample.is_finite() {
                            return Err(StemError::Export(
                                "Mixed export contains non-finite samples".into(),
                            ));
                        }
                        writer
                            .write_sample(sample)
                            .map_err(|e| StemError::Export(e.to_string()))?;
                    }
                }
                start += count as u64;
                emit(JobEvent::Progress {
                    phase: "Exporting mix",
                    completed: start,
                    total: spec.frames,
                });
            }
            writer
                .finalize()
                .map_err(|e| StemError::Export(e.to_string()))?;
        }
        cancel.check()?;
        std::fs::write(
            output.join("complete.txt"),
            format!(
                "model={}\nframes={}\nsample_rate={}\n",
                result.model_id, spec.frames, spec.sample_rate
            ),
        )?;
        Ok(output.clone())
    })();
    if operation.is_err() {
        let _ = std::fs::remove_dir_all(&output);
    }
    operation
}
