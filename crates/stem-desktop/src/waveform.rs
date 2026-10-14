use std::thread::{self, JoinHandle};
use stem_core::{
    StemError,
    audio::CacheReader,
    job::CancellationToken,
    types::{CachedAudio, SeparationResult, StemId},
};
#[derive(Debug, Clone, Copy, Default)]
pub struct Peak {
    pub min: f32,
    pub max: f32,
}
pub struct WaveformSet {
    pub generation: u64,
    pub original: Vec<Peak>,
    pub stems: Vec<(StemId, Vec<Peak>)>,
    pub frames: u64,
}
pub fn scan(
    audio: &CachedAudio,
    buckets: usize,
    cancel: &CancellationToken,
) -> Result<Vec<Peak>, StemError> {
    let count = buckets.clamp(1, 2048).min(audio.spec.frames as usize);
    let mut peaks = vec![Peak::default(); count];
    let mut reader = CacheReader::open(audio)?;
    for start in (0..audio.spec.frames).step_by(8192) {
        cancel.check()?;
        let data = reader.read_at(start, 8192)?;
        for frame in 0..data[0].len() {
            let bucket = ((u128::from(start + frame as u64) * count as u128)
                / u128::from(audio.spec.frames)) as usize;
            for channel in &data {
                peaks[bucket].min = peaks[bucket].min.min(channel[frame]);
                peaks[bucket].max = peaks[bucket].max.max(channel[frame]);
            }
        }
    }
    Ok(peaks)
}
pub struct WaveformWorker {
    cancel: CancellationToken,
    worker: Option<JoinHandle<Result<WaveformSet, StemError>>>,
}
impl WaveformWorker {
    pub fn start(result: SeparationResult, generation: u64) -> Self {
        let cancel = CancellationToken::default();
        let token = cancel.clone();
        let worker = thread::spawn(move || {
            let original = scan(&result.original, 2048, &token)?;
            let mut stems = Vec::new();
            for stem in &result.stems {
                stems.push((stem.id.clone(), scan(&stem.audio, 2048, &token)?));
            }
            Ok(WaveformSet {
                generation,
                original,
                stems,
                frames: result.original.spec.frames,
            })
        });
        Self {
            cancel,
            worker: Some(worker),
        }
    }
    pub fn poll(&mut self) -> Option<Result<WaveformSet, StemError>> {
        if self.worker.as_ref().is_some_and(|w| w.is_finished()) {
            Some(
                self.worker.take().unwrap().join().unwrap_or_else(|_| {
                    Err(StemError::Unavailable("Waveform worker failed".into()))
                }),
            )
        } else {
            None
        }
    }
    pub fn stop(&mut self) {
        self.cancel.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for WaveformWorker {
    fn drop(&mut self) {
        self.stop();
    }
}
