use rubato::{FftFixedIn, Resampler};
use stem_core::{
    StemError,
    audio::CacheReader,
    export::{MixSetting, gains},
    types::SeparationResult,
};

pub struct DiskMixer {
    readers: Vec<CacheReader>,
    original: CacheReader,
    ids: Vec<stem_core::types::StemId>,
    targets: Vec<f32>,
    current: Vec<f32>,
    original_gain: f32,
    position: u64,
    frames: u64,
    rate: u32,
}
impl DiskMixer {
    pub fn new(result: &SeparationResult) -> Result<Self, StemError> {
        if result.stems.is_empty()
            || result
                .stems
                .iter()
                .any(|s| s.audio.spec != result.original.spec)
        {
            return Err(StemError::Unavailable(
                "Preview audio is not aligned".into(),
            ));
        }
        Ok(Self {
            readers: result
                .stems
                .iter()
                .map(|s| CacheReader::open(&s.audio))
                .collect::<Result<_, _>>()?,
            original: CacheReader::open(&result.original)?,
            ids: result.stems.iter().map(|s| s.id.clone()).collect(),
            targets: vec![1.0; result.stems.len()],
            current: vec![1.0; result.stems.len()],
            original_gain: 0.0,
            position: 0,
            frames: result.original.spec.frames,
            rate: result.original.spec.sample_rate,
        })
    }
    pub fn position(&self) -> u64 {
        self.position
    }
    pub fn seek(&mut self, frame: u64) -> Result<(), StemError> {
        if frame > self.frames {
            return Err(StemError::Unavailable("Seek is outside the track".into()));
        }
        self.position = frame;
        Ok(())
    }
    pub fn set_mix(&mut self, settings: &[MixSetting]) -> Result<(), StemError> {
        self.targets = gains(&self.ids, settings)?;
        Ok(())
    }
    pub fn render(&mut self, frames: usize, original: bool) -> Result<Vec<[f32; 2]>, StemError> {
        let count = (self.frames - self.position).min(frames as u64) as usize;
        if count == 0 {
            return Ok(Vec::new());
        }
        let stems = self
            .readers
            .iter_mut()
            .map(|r| r.read_at(self.position, count))
            .collect::<Result<Vec<_>, _>>()?;
        let source = self.original.read_at(self.position, count)?;
        let mut output = vec![[0.0; 2]; count];
        for frame in 0..count {
            for (current, target) in self.current.iter_mut().zip(&self.targets) {
                *current += (*target - *current).clamp(-1.0 / 64.0, 1.0 / 64.0);
            }
            self.original_gain +=
                (f32::from(original) - self.original_gain).clamp(-1.0 / 64.0, 1.0 / 64.0);
            for channel in 0..2 {
                let original = source[channel.min(source.len() - 1)][frame];
                let mix = stems
                    .iter()
                    .zip(&self.current)
                    .map(|(s, g)| s[channel.min(s.len() - 1)][frame] * g)
                    .sum::<f32>();
                let sample = mix * (1.0 - self.original_gain) + original * self.original_gain;
                if !sample.is_finite() {
                    return Err(StemError::Unavailable(
                        "Preview mix contains non-finite samples".into(),
                    ));
                }
                output[frame][channel] = sample;
            }
        }
        self.position += count as u64;
        Ok(output)
    }
}

pub struct RateMixer {
    pub mixer: DiskMixer,
    converter: Option<FftFixedIn<f32>>,
    pub original: bool,
    pub rate: u32,
    delay: usize,
    published: u64,
    total: u64,
    base: u64,
}
impl RateMixer {
    pub fn new(result: &SeparationResult, rate: u32) -> Result<Self, StemError> {
        let mut value = Self {
            mixer: DiskMixer::new(result)?,
            converter: None,
            original: false,
            rate,
            delay: 0,
            published: 0,
            total: 0,
            base: 0,
        };
        value.seek(0)?;
        Ok(value)
    }
    pub fn seek(&mut self, frame: u64) -> Result<(), StemError> {
        self.mixer.seek(frame)?;
        self.base = frame;
        self.published = 0;
        self.total = (u128::from(self.mixer.frames - frame) * u128::from(self.rate))
            .div_ceil(u128::from(self.mixer.rate)) as u64;
        self.converter = if self.rate != self.mixer.rate {
            Some(
                FftFixedIn::new(self.mixer.rate as usize, self.rate as usize, 1024, 2, 2)
                    .map_err(|e| StemError::Unavailable(e.to_string()))?,
            )
        } else {
            None
        };
        self.delay = self.converter.as_ref().map_or(0, |r| r.output_delay());
        Ok(())
    }
    pub fn next_block(&mut self) -> Result<Vec<([f32; 2], u64)>, StemError> {
        if self.published >= self.total {
            return Ok(Vec::new());
        }
        loop {
            let needed = self
                .converter
                .as_ref()
                .map_or(1024, |r| r.input_frames_next());
            let frames = self.mixer.render(needed, self.original)?;
            let mut planar = vec![vec![0.0; needed]; 2];
            for (index, sample) in frames.iter().enumerate() {
                planar[0][index] = sample[0];
                planar[1][index] = sample[1];
            }
            let audio = if let Some(converter) = &mut self.converter {
                converter
                    .process(&planar, None)
                    .map_err(|e| StemError::Unavailable(e.to_string()))?
            } else {
                planar
            };
            let skip = self.delay.min(audio[0].len());
            self.delay -= skip;
            let count = (audio[0].len() - skip).min((self.total - self.published) as usize);
            if count == 0 {
                continue;
            }
            let result = (0..count)
                .map(|i| {
                    (
                        [audio[0][i + skip], audio[1][i + skip]],
                        (self.base
                            + ((self.published + i as u64 + 1) * u64::from(self.mixer.rate)
                                / u64::from(self.rate)))
                        .min(self.mixer.frames),
                    )
                })
                .collect();
            self.published += count as u64;
            return Ok(result);
        }
    }
}
