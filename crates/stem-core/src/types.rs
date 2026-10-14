use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
}
impl AudioSpec {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.sample_rate == 0
            || self.sample_rate > 768000
            || !(1..=2).contains(&self.channels)
            || self.frames == 0
            || self
                .frames
                .checked_mul(u64::from(self.channels) * 4)
                .is_none()
        {
            return Err(ContractError("Invalid cached audio dimensions".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CachedAudio {
    pub path: std::path::PathBuf,
    pub spec: AudioSpec,
}
#[derive(Debug, Clone)]
pub struct StemFile {
    pub id: StemId,
    pub label: String,
    pub audio: CachedAudio,
}
#[derive(Debug, Clone)]
pub struct SeparationResult {
    pub original: CachedAudio,
    pub stems: Vec<StemFile>,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError(pub(crate) String);

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ContractError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StemId(String);

impl StemId {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        let safe = !value.is_empty()
            && value.len() <= 64
            && value.as_bytes()[0].is_ascii_lowercase()
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
        let reserved = matches!(value.as_str(), "con" | "prn" | "aux" | "nul")
            || ((value.starts_with("com") || value.starts_with("lpt"))
                && value.len() == 4
                && matches!(value.as_bytes()[3], b'1'..=b'9'));
        if !safe || reserved {
            return Err(ContractError(format!(
                "Invalid stem ID {value:?}: use a lowercase portable filename identifier"
            )));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioChunk(Vec<Vec<f32>>);

impl AudioChunk {
    pub fn new(channels: Vec<Vec<f32>>) -> Result<Self, ContractError> {
        if !(1..=2).contains(&channels.len()) {
            return Err(ContractError("Audio must have one or two channels".into()));
        }
        let frames = channels[0].len();
        if frames == 0 || channels.iter().any(|channel| channel.len() != frames) {
            return Err(ContractError(
                "Audio channels must have equal nonzero frame counts".into(),
            ));
        }
        if channels.iter().flatten().any(|sample| !sample.is_finite()) {
            return Err(ContractError("Audio contains non-finite samples".into()));
        }
        Ok(Self(channels))
    }

    pub fn channels(&self) -> &[Vec<f32>] {
        &self.0
    }

    pub fn frames(&self) -> usize {
        self.0[0].len()
    }
}
