use std::path::Path;

use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider},
    session::{Session, builder::GraphOptimizationLevel},
    tensor::TensorElementType,
    value::{Tensor, ValueType},
};

use super::device::{
    ActiveDevice, BackendOptions, BackendReport, ProcessingDevice, select_backend,
};
use super::{ModelCapabilities, SeparationBackend, StemChunk};
use crate::{
    StemError,
    types::{AudioChunk, StemId},
};

pub struct OnnxBackend {
    session: Session,
    capabilities: ModelCapabilities,
    report: BackendReport,
    output_row: Option<usize>,
    graph_sources: usize,
}

fn inference(error: ort::Error) -> StemError {
    StemError::Inference(error.to_string())
}

fn tensor_matches(value: &ValueType, dimensions: &[i64]) -> bool {
    matches!(value, ValueType::Tensor { ty: TensorElementType::Float32, shape, .. } if &shape[..] == dimensions)
}

impl OnnxBackend {
    pub fn load(
        package: &super::ModelPackage,
        runtime: &Path,
        threads: usize,
    ) -> Result<Self, StemError> {
        let declared = package.capabilities();
        let caps = ModelCapabilities::new(
            declared.sample_rate(),
            declared.channels(),
            declared.chunk_frames(),
            declared.stems().to_vec(),
        )?;
        Self::load_contract(
            package.model_path(),
            runtime,
            &BackendOptions {
                device: ProcessingDevice::Cpu,
                cpu_threads: threads,
                ..Default::default()
            },
            caps,
        )
    }
    /// Loads the fixed development contract; package manifests follow CPU validation.
    /// The runtime is process-global, so callers must use one runtime library per process.
    pub fn load_development(
        model: &Path,
        runtime: &Path,
        threads: usize,
    ) -> Result<Self, StemError> {
        let stems = ["drums", "bass", "other", "vocals", "guitar", "piano"]
            .into_iter()
            .map(|name| Ok((StemId::new(name)?, name.into())))
            .collect::<Result<Vec<_>, crate::types::ContractError>>()?;
        Self::load_contract(
            model,
            runtime,
            &BackendOptions {
                device: ProcessingDevice::Cpu,
                cpu_threads: threads,
                ..Default::default()
            },
            ModelCapabilities::new(44100, 2, 343980, stems)?,
        )
    }

    pub fn load_with_options(
        package: &super::ModelPackage,
        runtime: &Path,
        options: &BackendOptions,
    ) -> Result<Self, StemError> {
        let declared = package.capabilities();
        let caps = ModelCapabilities::new(
            declared.sample_rate(),
            declared.channels(),
            declared.chunk_frames(),
            declared.stems().to_vec(),
        )?;
        Self::load_contract(package.model_path(), runtime, options, caps)
    }
    pub fn report(&self) -> &BackendReport {
        &self.report
    }

    pub fn load_specialist(
        package: &super::ModelPackage,
        index: usize,
        runtime: &Path,
        options: &BackendOptions,
    ) -> Result<Self, StemError> {
        let specialist = package
            .specialists()
            .get(index)
            .ok_or_else(|| StemError::InvalidPackage("Missing specialist".into()))?;
        let caps =
            ModelCapabilities::new(44100, 2, 343980, package.capabilities().stems().to_vec())?;
        let mut backend = Self::load_contract(&specialist.model_path, runtime, options, caps)?;
        backend.capabilities = ModelCapabilities::new(
            44100,
            2,
            343980,
            vec![package.capabilities().stems()[index].clone()],
        )?;
        backend.output_row = Some(specialist.output_row);
        Ok(backend)
    }
    fn load_contract(
        model: &Path,
        runtime: &Path,
        options: &BackendOptions,
        capabilities: ModelCapabilities,
    ) -> Result<Self, StemError> {
        options.validate()?;
        let runtime = runtime.canonicalize()?;
        let model = model.canonicalize()?;
        let runtime = runtime.to_str().ok_or_else(|| {
            StemError::Unavailable("Runtime library path must be valid UTF-8".into())
        })?;
        // ort's dynamic loader panics on invalid libraries instead of returning an error.
        std::panic::catch_unwind(|| ort::init_from(runtime).commit())
            .map_err(|_| {
                StemError::Unavailable(format!("Could not load ONNX Runtime library: {runtime}"))
            })?
            .map_err(inference)?;
        let (session, report) = select_backend(options, |active| {
            let provider = match active {
                ActiveDevice::Cpu => CPUExecutionProvider::default().build(),
                ActiveDevice::Cuda => CUDAExecutionProvider::default()
                    .with_device_id(options.device_id)
                    .with_memory_limit(options.cuda_memory_limit)
                    .with_tf32(false)
                    .build()
                    .error_on_failure(),
            };
            let mut session = Session::builder()
                .map_err(inference)?
                .with_execution_providers([provider])
                .map_err(inference)?
                .with_intra_threads(options.cpu_threads)
                .map_err(inference)?
                .with_inter_threads(1)
                .map_err(inference)?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(inference)?
                .commit_from_file(&model)
                .map_err(inference)?;
            if active == ActiveDevice::Cuda {
                let tensor = Tensor::from_array((
                    [1usize, 2, 343980],
                    vec![0f32; 2 * 343980].into_boxed_slice(),
                ))
                .map_err(inference)?;
                let outputs = session
                    .run(ort::inputs!["mix" => tensor])
                    .map_err(inference)?;
                let value = outputs
                    .get("stems")
                    .ok_or_else(|| StemError::Inference("Missing stems output".into()))?;
                let (_, samples) = value.try_extract_tensor::<f32>().map_err(inference)?;
                if samples.iter().any(|v| !v.is_finite()) {
                    return Err(StemError::Inference("Non-finite CUDA warm-up".into()));
                }
            }
            Ok(session)
        })?;
        if session.inputs.len() != 1
            || session.outputs.len() != 1
            || session.inputs[0].name != "mix"
            || session.outputs[0].name != "stems"
            || !tensor_matches(&session.inputs[0].input_type, &[1, 2, 343980])
            || !tensor_matches(
                &session.outputs[0].output_type,
                &[1, capabilities.stems().len() as i64, 2, 343980],
            )
        {
            return Err(StemError::Inference(
                "Model does not match the declared tensor contract".into(),
            ));
        }
        let graph_sources = capabilities.stems().len();
        Ok(Self {
            session,
            capabilities,
            report,
            output_row: None,
            graph_sources,
        })
    }
}

impl SeparationBackend for OnnxBackend {
    fn capabilities(&self) -> &ModelCapabilities {
        &self.capabilities
    }

    fn infer(&mut self, input: &AudioChunk) -> Result<StemChunk, StemError> {
        if input.frames() != 343980 || input.channels().len() != 2 {
            return Err(StemError::Inference(
                "Expected exactly 343980 stereo frames at the model boundary".into(),
            ));
        }
        let samples: Vec<f32> = input.channels().iter().flatten().copied().collect();
        let tensor = Tensor::from_array(([1usize, 2, 343980], samples.into_boxed_slice()))
            .map_err(inference)?;
        let output = self
            .session
            .run(ort::inputs!["mix" => tensor])
            .map_err(inference)?;
        let (shape, samples) = output["stems"]
            .try_extract_tensor::<f32>()
            .map_err(inference)?;
        if shape[..] != [1, self.graph_sources as i64, 2, 343980]
            || samples.len() != self.graph_sources * 2 * 343980
        {
            return Err(StemError::Inference(
                "Unexpected inference output dimensions".into(),
            ));
        }
        let samples = if let Some(row) = self.output_row {
            &samples[row * 2 * 343980..(row + 1) * 2 * 343980]
        } else {
            samples
        };
        let stems = samples
            .as_chunks::<{ 2 * 343980 }>()
            .0
            .iter()
            .zip(self.capabilities.stems())
            .map(|(samples, (id, _))| {
                Ok((
                    id.clone(),
                    AudioChunk::new(
                        samples
                            .as_chunks::<343980>()
                            .0
                            .iter()
                            .map(|channel| channel.to_vec())
                            .collect(),
                    )?,
                ))
            })
            .collect::<Result<Vec<_>, crate::types::ContractError>>()?;
        Ok(StemChunk::new(&self.capabilities, input, stems)?)
    }
}
