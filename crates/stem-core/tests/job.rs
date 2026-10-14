use std::{thread, time::Duration};
use stem_core::job::{JobController, JobEvent};

#[test]
fn single_job_lifecycle() {
    let root = tempfile::tempdir().unwrap();
    let mut job = JobController::new(root.path().to_path_buf(), "missing-runtime".into()).unwrap();
    job.start_task(|cancel, _, _| {
        while !cancel.is_cancelled() {
            thread::sleep(Duration::from_millis(2));
        }
        cancel.check()?;
        unreachable!()
    })
    .unwrap();
    assert!(
        job.start_task(|_, _, _| Err(stem_core::StemError::Inference("should not run".into())))
            .is_err()
    );
    job.cancel();
    job.cancel();
    while job.is_running() {
        job.poll();
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        std::fs::read_dir(root.path()).unwrap().count(),
        0,
        "cancelled cache must be removed before retry"
    );
    job.start_task(|_, _, _| Err(stem_core::StemError::Inference("test failure".into())))
        .unwrap();
    let mut failed = false;
    while job.is_running() {
        for event in job.poll() {
            if matches!(event, JobEvent::Failed(_)) {
                failed = true;
            }
        }
        thread::sleep(Duration::from_millis(2));
    }
    assert!(failed);
    assert_eq!(
        std::fs::read_dir(root.path()).unwrap().count(),
        0,
        "failed cache must be removed before retry"
    );
    job.start_task(|cancel, _, _| {
        while !cancel.is_cancelled() {
            thread::sleep(Duration::from_millis(2));
        }
        cancel.check()?;
        unreachable!()
    })
    .unwrap();
    job.shutdown().unwrap();
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn retries_keep_current_result_and_replacement_reclaims_old_cache() {
    use stem_core::{
        audio::write_cache,
        types::{SeparationResult, StemFile, StemId},
    };
    let root = tempfile::tempdir().unwrap();
    let mut job = JobController::new(root.path().into(), "missing-runtime".into()).unwrap();
    let start_result = |job: &mut JobController| {
        job.start_task(|_, dir, _| {
            let audio = write_cache(&dir.join("input.f32"), 44100, vec![vec![0.2; 100]])?;
            Ok(SeparationResult {
                original: audio.clone(),
                stems: vec![StemFile {
                    id: StemId::new("vocals")?,
                    label: "Vocals".into(),
                    audio,
                }],
                model_id: "test".into(),
            })
        })
        .unwrap();
    };
    let finish = |job: &mut JobController| {
        while job.is_running() {
            job.poll();
            thread::sleep(Duration::from_millis(2));
        }
    };
    start_result(&mut job);
    finish(&mut job);
    let previous = job.result().unwrap().original.path.clone();
    job.start_task(|_, _, _| Err(stem_core::StemError::Inference("retry failed".into())))
        .unwrap();
    finish(&mut job);
    assert!(
        previous.exists(),
        "a failed retry must preserve the previous usable result"
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    start_result(&mut job);
    finish(&mut job);
    assert!(
        !previous.exists(),
        "a successful replacement must release the previous cache"
    );
    assert!(job.result().unwrap().original.path.is_file());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    job.shutdown().unwrap();
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
