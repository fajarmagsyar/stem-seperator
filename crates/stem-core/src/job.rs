use crate::{StemError, types::SeparationResult};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), StemError> {
        if self.is_cancelled() {
            Err(StemError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub enum JobEvent {
    BackendSelected(crate::model::device::BackendReport),
    Progress {
        phase: &'static str,
        completed: u64,
        total: u64,
    },
    Ready(SeparationResult),
    Cancelled,
    Failed(String),
    Exported(std::path::PathBuf),
}

use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn reserve_directory(root: &Path, prefix: &str) -> Result<PathBuf, StemError> {
    fs::create_dir_all(root)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| StemError::Io(std::io::Error::other(e)))?
        .as_nanos();
    for attempt in 0..100 {
        let dir = root.join(format!("{prefix}-{stamp}-{}-{attempt}", std::process::id()));
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(StemError::Io(std::io::Error::other(
        "Could not reserve a new output directory",
    )))
}

pub struct JobController {
    root: PathBuf,
    runtime: PathBuf,
    worker: Option<JoinHandle<()>>,
    receiver: Option<Receiver<JobEvent>>,
    cancel: CancellationToken,
    owned: Vec<(PathBuf, File)>,
    result: Option<SeparationResult>,
}
impl JobController {
    pub fn new(root: PathBuf, runtime: PathBuf) -> Result<Self, StemError> {
        fs::create_dir_all(&root)?;
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir()
                || !entry.file_name().to_string_lossy().starts_with("job-")
            {
                continue;
            }
            let marker = entry.path().join(".stem-job-v1");
            if fs::symlink_metadata(&marker).is_ok_and(|m| m.file_type().is_file())
                && let Ok(file) = OpenOptions::new().read(true).write(true).open(&marker)
                && file.try_lock_exclusive().is_ok()
            {
                drop(file);
                let _ = fs::remove_dir_all(entry.path());
            }
        }
        Ok(Self {
            root,
            runtime,
            worker: None,
            receiver: None,
            cancel: CancellationToken::default(),
            owned: Vec::new(),
            result: None,
        })
    }
    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }
    pub fn result(&self) -> Option<&SeparationResult> {
        self.result.as_ref()
    }
    pub fn start(
        &mut self,
        input: PathBuf,
        package: crate::model::ModelPackage,
    ) -> Result<(), StemError> {
        self.start_with_options(
            input,
            package,
            crate::model::device::BackendOptions::default(),
        )
    }
    pub fn start_with_options(
        &mut self,
        input: PathBuf,
        package: crate::model::ModelPackage,
        options: crate::model::device::BackendOptions,
    ) -> Result<(), StemError> {
        let runtime = self.runtime.clone();
        self.start_task(move |cancel, dir, emit| {
            cancel.check()?;
            emit(JobEvent::Progress {
                phase: "Decoding",
                completed: 0,
                total: 1,
            });
            let original = crate::audio::decode_to_cache(&input, dir, cancel)?;
            crate::pipeline::package::separate_package(
                &original, &package, &runtime, &options, dir, cancel, emit,
            )
        })
    }
    pub fn start_paths(&mut self, input: PathBuf, package: PathBuf) -> Result<(), StemError> {
        self.start_paths_with_options(
            input,
            package,
            crate::model::device::BackendOptions::default(),
        )
    }
    pub fn start_paths_with_options(
        &mut self,
        input: PathBuf,
        package: PathBuf,
        options: crate::model::device::BackendOptions,
    ) -> Result<(), StemError> {
        options.validate()?;
        let runtime = self.runtime.clone();
        self.start_task(move |cancel, dir, emit| {
            emit(JobEvent::Progress {
                phase: "Validating package",
                completed: 0,
                total: 1,
            });
            cancel.check()?;
            let package = crate::model::ModelPackage::load(&package)?;
            emit(JobEvent::Progress {
                phase: "Decoding",
                completed: 0,
                total: 1,
            });
            let original = crate::audio::decode_to_cache(&input, dir, cancel)?;
            crate::pipeline::package::separate_package(
                &original, &package, &runtime, &options, dir, cancel, emit,
            )
        })
    }
    pub fn start_task<F>(&mut self, task: F) -> Result<(), StemError>
    where
        F: FnOnce(
                &CancellationToken,
                &Path,
                &mut dyn FnMut(JobEvent),
            ) -> Result<SeparationResult, StemError>
            + Send
            + 'static,
    {
        if self.is_running() {
            return Err(StemError::Unavailable("A job is already running".into()));
        }
        let dir = reserve_directory(&self.root, "job")?;
        let marker = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dir.join(".stem-job-v1"));
        let marker = match marker.and_then(|marker| {
            marker.lock_exclusive()?;
            Ok(marker)
        }) {
            Ok(marker) => marker,
            Err(error) => {
                let _ = fs::remove_dir_all(&dir);
                return Err(error.into());
            }
        };
        self.owned.push((dir.clone(), marker));
        self.cancel = CancellationToken::default();
        let token = self.cancel.clone();
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.worker = Some(thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                task(&token, &dir, &mut |event| {
                    let _ = sender.send(event);
                })
            }));
            let event = if token.is_cancelled() {
                JobEvent::Cancelled
            } else {
                match outcome {
                    Ok(Ok(result)) => JobEvent::Ready(result),
                    Ok(Err(StemError::Cancelled)) => JobEvent::Cancelled,
                    Ok(Err(e)) => JobEvent::Failed(e.to_string()),
                    Err(_) => JobEvent::Failed(
                        "The processing worker failed; the previous result is still available"
                            .into(),
                    ),
                }
            };
            let _ = sender.send(event);
        }));
        Ok(())
    }
    pub fn cancel(&self) {
        if self.is_running() {
            self.cancel.cancel();
        }
    }
    pub fn start_export(&mut self, request: crate::export::ExportRequest) -> Result<(), StemError> {
        let result = self
            .result
            .clone()
            .ok_or_else(|| StemError::Export("Separate a song before exporting".into()))?;
        self.start_task(move |cancel, _, emit| {
            let output = crate::export::export(&result, &request, cancel, emit)?;
            emit(JobEvent::Exported(output));
            Ok(result)
        })
    }
    pub fn poll(&mut self) -> Vec<JobEvent> {
        let mut events = Vec::new();
        if let Some(receiver) = &self.receiver {
            events.extend(receiver.try_iter());
        }
        for event in &mut events {
            if matches!(event, JobEvent::Ready(_)) && self.cancel.is_cancelled() {
                *event = JobEvent::Cancelled;
            }
            if let JobEvent::Ready(result) = event {
                self.result = Some(result.clone());
            }
        }
        if self.worker.as_ref().is_some_and(|w| w.is_finished()) {
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
            if let Some(receiver) = self.receiver.take() {
                for mut event in receiver.try_iter() {
                    if matches!(event, JobEvent::Ready(_)) && self.cancel.is_cancelled() {
                        event = JobEvent::Cancelled;
                    }
                    if let JobEvent::Ready(result) = &event {
                        self.result = Some(result.clone());
                    }
                    events.push(event);
                }
            }
            let mut retained = Vec::new();
            for (dir, marker) in self.owned.drain(..) {
                let needed = self.result.as_ref().is_some_and(|result| {
                    result.original.path.starts_with(&dir)
                        || result
                            .stems
                            .iter()
                            .any(|stem| stem.audio.path.starts_with(&dir))
                });
                if needed {
                    retained.push((dir, marker));
                } else {
                    match fs::remove_dir_all(&dir) {
                        Ok(()) => drop(marker),
                        Err(error) => {
                            events.push(JobEvent::Failed(format!(
                                "Could not clean temporary cache {}: {error}",
                                dir.display()
                            )));
                            retained.push((dir, marker));
                        }
                    }
                }
            }
            self.owned = retained;
        }
        events
    }
    pub fn shutdown(&mut self) -> Result<(), StemError> {
        self.cancel.cancel();
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| StemError::Unavailable("Worker failed while shutting down".into()))?;
        }
        self.receiver = None;
        self.result = None;
        for (dir, marker) in self.owned.drain(..) {
            drop(marker);
            fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}
impl Drop for JobController {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
