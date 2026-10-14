use crate::StemError;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessingDevice {
    #[default]
    Auto,
    Cpu,
    Nvidia,
}
impl std::str::FromStr for ProcessingDevice {
    type Err = StemError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "cpu" => Ok(Self::Cpu),
            "nvidia" => Ok(Self::Nvidia),
            _ => Err(StemError::Unavailable(
                "Device must be auto, cpu or nvidia".into(),
            )),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveDevice {
    Cpu,
    Cuda,
}
#[derive(Debug, Clone)]
pub struct BackendOptions {
    pub device: ProcessingDevice,
    pub device_id: i32,
    pub cpu_threads: usize,
    pub cuda_memory_limit: usize,
}
impl Default for BackendOptions {
    fn default() -> Self {
        Self {
            device: ProcessingDevice::Auto,
            device_id: 0,
            cpu_threads: 2,
            cuda_memory_limit: 12 * 1024 * 1024 * 1024,
        }
    }
}
impl BackendOptions {
    pub fn validate(&self) -> Result<(), StemError> {
        if self.device_id < 0 || self.cpu_threads == 0 || self.cuda_memory_limit == 0 {
            return Err(StemError::Unavailable(
                "Device ID must be nonnegative; thread count and CUDA memory limit must be nonzero"
                    .into(),
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct BackendReport {
    pub requested: ProcessingDevice,
    pub active: ActiveDevice,
    pub fallback_reason: Option<String>,
}
pub fn select_backend<T>(
    options: &BackendOptions,
    mut initialize: impl FnMut(ActiveDevice) -> Result<T, StemError>,
) -> Result<(T, BackendReport), StemError> {
    options.validate()?;
    let mut report = BackendReport {
        requested: options.device,
        active: ActiveDevice::Cpu,
        fallback_reason: None,
    };
    if options.device != ProcessingDevice::Cpu {
        match initialize(ActiveDevice::Cuda) {
            Ok(value) => {
                report.active = ActiveDevice::Cuda;
                return Ok((value, report));
            }
            Err(error) if options.device == ProcessingDevice::Auto => {
                report.fallback_reason = Some(error.to_string())
            }
            Err(error) => {
                return Err(StemError::Unavailable(format!(
                    "NVIDIA initialization failed: {error}. Check the GPU runtime, CUDA 12.8 and cuDNN 9, or choose CPU."
                )));
            }
        }
    }
    Ok((initialize(ActiveDevice::Cpu)?, report))
}
