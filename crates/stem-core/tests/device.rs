use stem_core::model::device::{ActiveDevice, BackendOptions, ProcessingDevice, select_backend};
#[test]
fn selection_and_validation() {
    assert_eq!(
        "auto".parse::<ProcessingDevice>().unwrap(),
        ProcessingDevice::Auto
    );
    assert!("gpu".parse::<ProcessingDevice>().is_err());
    let mut options = BackendOptions::default();
    assert_eq!(options.device, ProcessingDevice::Auto);
    options.device = ProcessingDevice::Cpu;
    let (value, report) = select_backend(&options, |active| {
        assert_eq!(active, ActiveDevice::Cpu);
        Ok(7)
    })
    .unwrap();
    assert_eq!(value, 7);
    assert_eq!(report.active, ActiveDevice::Cpu);
    options.device = ProcessingDevice::Auto;
    let (_, report) = select_backend(&options, |active| {
        if active == ActiveDevice::Cuda {
            Err(stem_core::StemError::Unavailable("missing DLL".into()))
        } else {
            Ok(7)
        }
    })
    .unwrap();
    assert!(report.fallback_reason.unwrap().contains("missing DLL"));
    options.device = ProcessingDevice::Nvidia;
    assert!(
        select_backend(&options, |_| Err::<(), _>(
            stem_core::StemError::Unavailable("warm-up failed".into())
        ))
        .is_err()
    );
    options.device_id = -1;
    assert!(options.validate().is_err());
    options.device_id = 0;
    options.cpu_threads = 0;
    assert!(options.validate().is_err());
    options.cpu_threads = 2;
    options.cuda_memory_limit = 0;
    assert!(options.validate().is_err());
}
