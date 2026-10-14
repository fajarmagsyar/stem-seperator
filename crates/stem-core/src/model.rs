use crate::types::{AudioChunk, ContractError, StemId};
use std::collections::HashSet;

pub mod device;
pub mod manifest;
pub mod onnx;

pub use manifest::ModelPackage;

pub fn open_backend(
    package: &ModelPackage,
    runtime: &std::path::Path,
) -> Result<Box<dyn SeparationBackend>, crate::StemError> {
    Ok(Box::new(onnx::OnnxBackend::load(package, runtime, 2)?))
}

#[derive(Debug)]
pub struct ModelCapabilities {
    sample_rate: u32,
    channels: u16,
    chunk_frames: usize,
    stems: Vec<(StemId, String)>,
}

impl ModelCapabilities {
    pub fn new(
        sample_rate: u32,
        channels: u16,
        chunk_frames: usize,
        stems: Vec<(StemId, String)>,
    ) -> Result<Self, ContractError> {
        if sample_rate == 0 || !(1..=2).contains(&channels) || chunk_frames == 0 {
            return Err(ContractError("Invalid model audio dimensions".into()));
        }
        let mut ids = HashSet::new();
        if stems.is_empty()
            || stems
                .iter()
                .any(|(id, label)| label.trim().is_empty() || !ids.insert(id))
        {
            return Err(ContractError(
                "Model must declare distinct stem IDs with nonempty labels".into(),
            ));
        }
        Ok(Self {
            sample_rate,
            channels,
            chunk_frames,
            stems,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn channels(&self) -> u16 {
        self.channels
    }
    pub fn chunk_frames(&self) -> usize {
        self.chunk_frames
    }
    pub fn stems(&self) -> &[(StemId, String)] {
        &self.stems
    }
}

#[derive(Debug)]
pub struct StemChunk(Vec<(StemId, AudioChunk)>);

impl StemChunk {
    pub fn new(
        model: &ModelCapabilities,
        input: &AudioChunk,
        outputs: Vec<(StemId, AudioChunk)>,
    ) -> Result<Self, ContractError> {
        if input.channels().len() != usize::from(model.channels)
            || input.frames() != model.chunk_frames
        {
            return Err(ContractError(
                "Input does not match the model chunk dimensions".into(),
            ));
        }
        if outputs.len() != model.stems.len()
            || outputs
                .iter()
                .zip(&model.stems)
                .any(|((id, audio), (expected, _))| {
                    id != expected
                        || audio.frames() != input.frames()
                        || audio.channels().len() != input.channels().len()
                })
        {
            return Err(ContractError(
                "Backend output has incorrect stem ordering or audio dimensions".into(),
            ));
        }
        Ok(Self(outputs))
    }

    pub fn stems(&self) -> &[(StemId, AudioChunk)] {
        &self.0
    }
}

pub trait SeparationBackend: Send {
    fn capabilities(&self) -> &ModelCapabilities;
    fn infer(&mut self, input: &AudioChunk) -> Result<StemChunk, crate::StemError>;
}
