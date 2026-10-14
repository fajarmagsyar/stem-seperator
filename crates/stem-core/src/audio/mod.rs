mod decode;
mod resample;
pub use decode::decode_to_cache;
pub use resample::resample_to_cache;

use crate::{
    StemError,
    types::{AudioChunk, AudioSpec, CachedAudio},
};
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};

pub struct CacheReader {
    file: File,
    spec: AudioSpec,
}
impl CacheReader {
    pub fn open(audio: &CachedAudio) -> Result<Self, StemError> {
        audio.spec.validate()?;
        let file = File::open(&audio.path)?;
        if file.metadata()?.len() != audio.spec.frames * u64::from(audio.spec.channels) * 4 {
            return Err(StemError::Decode("Cached audio length mismatch".into()));
        }
        Ok(Self {
            file,
            spec: audio.spec,
        })
    }
    pub fn read_at(&mut self, start: u64, frames: usize) -> Result<Vec<Vec<f32>>, StemError> {
        let count = (self.spec.frames.saturating_sub(start)).min(frames as u64) as usize;
        if count == 0 {
            return Ok(vec![Vec::new(); usize::from(self.spec.channels)]);
        }
        self.file
            .seek(SeekFrom::Start(start * u64::from(self.spec.channels) * 4))?;
        let mut bytes = vec![0_u8; count * usize::from(self.spec.channels) * 4];
        self.file.read_exact(&mut bytes)?;
        let mut result = vec![vec![0.0; count]; usize::from(self.spec.channels)];
        for (frame, values) in bytes
            .chunks_exact(usize::from(self.spec.channels) * 4)
            .enumerate()
        {
            for (channel, value) in values.as_chunks::<4>().0.iter().enumerate() {
                let sample = f32::from_le_bytes(*value);
                if !sample.is_finite() {
                    return Err(StemError::Decode("Non-finite cached audio sample".into()));
                }
                result[channel][frame] = sample;
            }
        }
        Ok(result)
    }
}

pub(crate) fn writer(path: &Path) -> Result<BufWriter<File>, StemError> {
    Ok(BufWriter::new(
        OpenOptions::new().write(true).create_new(true).open(path)?,
    ))
}
pub(crate) fn append(
    writer: &mut BufWriter<File>,
    channels: &[Vec<f32>],
    start: usize,
    count: usize,
) -> Result<(), StemError> {
    for frame in start..start + count {
        for channel in channels {
            let sample = channel[frame];
            if !sample.is_finite() {
                return Err(StemError::Decode(
                    "Audio output contains non-finite samples".into(),
                ));
            }
            writer.write_all(&sample.to_le_bytes())?;
        }
    }
    Ok(())
}
pub(crate) fn finish(mut writer: BufWriter<File>) -> Result<(), StemError> {
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(())
}

pub fn write_cache(
    path: &Path,
    rate: u32,
    channels: Vec<Vec<f32>>,
) -> Result<CachedAudio, StemError> {
    let chunk = AudioChunk::new(channels)?;
    let spec = AudioSpec {
        sample_rate: rate,
        channels: chunk.channels().len() as u16,
        frames: chunk.frames() as u64,
    };
    spec.validate()?;
    let mut target = writer(path)?;
    append(&mut target, chunk.channels(), 0, chunk.frames())?;
    finish(target)?;
    Ok(CachedAudio {
        path: path.to_path_buf(),
        spec,
    })
}
