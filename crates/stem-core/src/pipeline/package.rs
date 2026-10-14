use crate::{
    StemError,
    job::{CancellationToken, JobEvent},
    model::{ModelPackage, SeparationBackend, device::BackendOptions, onnx::OnnxBackend},
    types::{CachedAudio, SeparationResult, StemId},
};
use std::path::Path;

pub fn separate_passes(
    input: &CachedAudio,
    sources: &[(StemId, String)],
    dir: &Path,
    cancel: &CancellationToken,
    emit: &mut dyn FnMut(JobEvent),
    mut load: impl FnMut(usize) -> Result<Box<dyn SeparationBackend>, StemError>,
) -> Result<SeparationResult, StemError> {
    let mut stems = Vec::new();
    for (index, (id, _)) in sources.iter().enumerate() {
        cancel.check()?;
        emit(JobEvent::Progress {
            phase: "Specialist pass",
            completed: index as u64,
            total: sources.len() as u64,
        });
        let scratch = dir.join(format!("pass-{index}"));
        std::fs::create_dir(&scratch)?;
        let mut backend = load(index)?;
        cancel.check()?;
        let pass = super::separate(input, backend.as_mut(), &scratch, cancel, emit)?;
        drop(backend);
        if pass.stems.len() != 1
            || pass.stems[0].id != *id
            || pass.stems[0].audio.spec != input.spec
        {
            return Err(StemError::Inference(
                "Specialist result contract mismatch".into(),
            ));
        }
        let mut stem = pass.stems.into_iter().next().unwrap();
        let path = dir.join(format!("stem-{}.f32", id.as_str()));
        if path.exists() {
            return Err(StemError::Inference(
                "Specialist output already exists".into(),
            ));
        }
        std::fs::rename(&stem.audio.path, &path)?;
        stem.audio.path = path;
        stems.push(stem);
        std::fs::remove_dir_all(scratch)?;
    }
    cancel.check()?;
    Ok(SeparationResult {
        original: input.clone(),
        stems,
        model_id: "htdemucs-ft".into(),
    })
}
pub fn separate_package(
    input: &CachedAudio,
    package: &ModelPackage,
    runtime: &Path,
    options: &BackendOptions,
    dir: &Path,
    cancel: &CancellationToken,
    emit: &mut dyn FnMut(JobEvent),
) -> Result<SeparationResult, StemError> {
    let mut result = if package.specialists().is_empty() {
        let mut backend = OnnxBackend::load_with_options(package, runtime, options)?;
        emit(JobEvent::BackendSelected(backend.report().clone()));
        super::separate(input, &mut backend, dir, cancel, emit)?
    } else {
        // Select one device for the entire bag; a later failure must not mix providers.
        let mut first = Some(OnnxBackend::load_specialist(package, 0, runtime, options)?);
        let report = first.as_ref().unwrap().report().clone();
        emit(JobEvent::BackendSelected(report.clone()));
        let mut fixed = options.clone();
        fixed.device = match report.active {
            crate::model::device::ActiveDevice::Cpu => crate::model::device::ProcessingDevice::Cpu,
            crate::model::device::ActiveDevice::Cuda => {
                crate::model::device::ProcessingDevice::Nvidia
            }
        };
        let prepared = crate::audio::resample_to_cache(
            input,
            &dir.join("ft-input.f32"),
            44100,
            2,
            None,
            cancel,
        )?;
        let (normalized, mean, scale) = super::normalization::normalize(&prepared, dir, cancel)?;
        let mut output = separate_passes(
            &normalized,
            package.capabilities().stems(),
            dir,
            cancel,
            emit,
            |index| {
                let backend = if index == 0 {
                    first.take().unwrap()
                } else {
                    OnnxBackend::load_specialist(package, index, runtime, &fixed)?
                };
                Ok(Box::new(backend))
            },
        )?;
        super::normalization::restore(&mut output, input, mean, scale, dir, cancel)?;
        output
    };
    result.model_id = package.manifest().id.clone();
    Ok(result)
}
