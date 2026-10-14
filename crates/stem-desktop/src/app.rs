use crate::state::{Phase, UiState};
use eframe::egui;
use std::{
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use stem_core::{
    job::{JobController, JobEvent},
    model::ModelPackage,
    types::SeparationResult,
};
use stem_playback::PlaybackEngine;

#[path = "export_dialog.rs"]
mod export_dialog;
#[path = "workspace.rs"]
mod workspace;
pub struct DesktopApp {
    pub device: stem_core::model::device::ProcessingDevice,
    pub backend_label: String,
    pub result_song: String,
    pending_song: String,
    export_open: bool,
    generation: u64,
    waveforms: Option<crate::waveform::WaveformSet>,
    waveform_worker: Option<crate::waveform::WaveformWorker>,
    waveform_error: Option<String>,
    pub state: UiState,
    pub input: String,
    pub output: String,
    pub status: String,
    pub error: Option<String>,
    job: JobController,
    result: Option<SeparationResult>,
    playback: Option<PlaybackEngine>,
    package: Option<ModelPackage>,
    package_worker: Option<JoinHandle<(String, Result<ModelPackage, String>)>>,
    dialog_worker: Option<JoinHandle<Option<(u8, PathBuf)>>>,
    progress: f32,
    started: Option<Instant>,
    progress_phase: String,
    phase_started: Instant,
    remaining: Option<Duration>,
    is_exporting: bool,
    original: bool,
    selected: Vec<bool>,
    export_mix: bool,
    closing: bool,
}

pub fn resource_root() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        for root in exe.ancestors().skip(1).take(5) {
            if root.join("models").is_dir() && root.join("runtime").is_dir() {
                return root.to_path_buf();
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
pub fn runtime_path(root: &Path) -> PathBuf {
    if let Some(path) = std::env::var_os("STEM_RUNTIME") {
        return path.into();
    }
    if cfg!(target_os = "windows") {
        root.join("runtime/onnxruntime.dll")
    } else {
        let flat = root.join("runtime/libonnxruntime.so");
        if flat.exists() {
            flat
        } else {
            root.join("runtime/onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so")
        }
    }
}

impl DesktopApp {
    pub fn new(root: &Path, cache: PathBuf) -> Result<Self, stem_core::StemError> {
        let mut value = Self {
            device: Default::default(),
            backend_label: "Device selected when processing starts".into(),
            result_song: String::new(),
            pending_song: String::new(),
            export_open: false,
            generation: 0,
            waveforms: None,
            waveform_worker: None,
            waveform_error: None,
            state: UiState::default(),
            input: String::new(),
            output: root.join("exports").to_string_lossy().into_owned(),
            status: "Choose a song to separate locally.".into(),
            error: None,
            job: JobController::new(cache, runtime_path(root))?,
            result: None,
            playback: None,
            package: None,
            package_worker: None,
            dialog_worker: None,
            progress: 0.0,
            started: None,
            progress_phase: String::new(),
            phase_started: Instant::now(),
            remaining: None,
            is_exporting: false,
            original: false,
            selected: Vec::new(),
            export_mix: false,
            closing: false,
        };
        value.state.selected_package = root
            .join("models/htdemucs-6s")
            .to_string_lossy()
            .into_owned();
        value.validate_package();
        Ok(value)
    }
    pub fn configure(ctx: &egui::Context) {
        crate::theme::configure(ctx);
    }
    pub fn validate_package(&mut self) {
        if self.package_worker.is_some() {
            return;
        }
        let selection = self.state.selected_package.clone();
        self.package = None;
        self.package_worker = Some(thread::spawn(move || {
            let result = ModelPackage::load(Path::new(&selection)).map_err(|e| e.to_string());
            (selection, result)
        }));
    }
    pub fn waveforms_ready(&self) -> bool { self.waveforms.is_some() }
    pub fn open_export(&mut self) { if self.state.can_export() { self.export_open = true; } }
    pub fn can_separate(&self) -> bool {
        self.state.can_start()
            && self.package.is_some()
            && !self.input.trim().is_empty()
            && self.package_worker.is_none()
    }
    fn dialog(&mut self, kind: u8) {
        if self.dialog_worker.is_some() {
            return;
        }
        self.dialog_worker = Some(thread::spawn(move || {
            let dialog = rfd::FileDialog::new();
            let selected = if kind == 0 {
                dialog
                    .add_filter("Audio", &["wav", "flac", "mp3"])
                    .pick_file()
            } else {
                dialog.pick_folder()
            };
            selected.map(|path| (kind, path))
        }));
    }
    pub fn start(&mut self) {
        if !self.state.can_start() || self.package.is_none() {
            return;
        }
        if let Some(mut playback) = self.playback.take() {
            playback.close();
        }
        self.state.has_device = false;
        if let Some(mut worker) = self.waveform_worker.take() {
            worker.stop();
        }
        self.pending_song = self.input.clone();
        match self.job.start_paths_with_options(
            PathBuf::from(&self.input),
            PathBuf::from(&self.state.selected_package),
            stem_core::model::device::BackendOptions {
                device: self.device,
                ..Default::default()
            },
        ) {
            Ok(()) => {
                self.state.phase = Phase::Validating;
                self.started = Some(Instant::now());
                self.progress = 0.0;
                self.progress_phase.clear();
                self.remaining = None;
                self.error = None;
                self.status = "Validating package…".into();
                self.is_exporting = false;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn open_preview(&mut self) {
        if let Some(mut previous) = self.playback.take() {
            previous.close();
        }
        if let Some(result) = &self.result {
            match PlaybackEngine::open(result) {
                Ok(mut playback) => {
                    for setting in &self.state.mix {
                        if let Err(e) =
                            playback.set_mix(&setting.id, setting.gain, setting.mute, setting.solo)
                        {
                            self.error = Some(e.to_string());
                            return;
                        }
                    }
                    if let Err(e) = playback.set_original(self.original) {
                        self.error = Some(e.to_string());
                        return;
                    }
                    self.playback = Some(playback);
                    self.state.has_device = true;
                    self.error = None;
                }
                Err(e) => {
                    self.state.has_device = false;
                    self.error = Some(e.to_string());
                }
            }
        }
    }
    fn poll(&mut self) {
        if let Some(worker) = &mut self.waveform_worker
            && let Some(outcome) = worker.poll()
        {
            match outcome {
                Ok(set) if set.generation == self.generation => self.waveforms = Some(set),
                Ok(_) => {}
                Err(e) => self.waveform_error = Some(e.to_string()),
            }
            self.waveform_worker = None;
        }
        if self.dialog_worker.as_ref().is_some_and(|w| w.is_finished())
            && let Some(worker) = self.dialog_worker.take()
            && let Ok(Some((kind, path))) = worker.join()
        {
            let value = path.to_string_lossy().into_owned();
            match kind {
                0 => self.input = value,
                1 => {
                    self.state.selected_package = value;
                    self.validate_package();
                }
                _ => self.output = value,
            }
        }
        if self
            .package_worker
            .as_ref()
            .is_some_and(|w| w.is_finished())
            && let Some(worker) = self.package_worker.take()
        {
            match worker.join() {
                Ok((selection, result)) => {
                    if selection != self.state.selected_package {
                        self.validate_package();
                    } else {
                        match result {
                            Ok(package) => {
                                self.package = Some(package);
                                self.error = None;
                            }
                            Err(e) => self.error = Some(format!("Model package: {e}")),
                        }
                    }
                }
                Err(_) => {
                    self.error = Some("Model validation failed. Select the package again.".into())
                }
            }
        }
        for event in self.job.poll() {
            match event {
                JobEvent::BackendSelected(report) => {
                    self.backend_label = match report.active {
                        stem_core::model::device::ActiveDevice::Cpu => "CPU".into(),
                        stem_core::model::device::ActiveDevice::Cuda => "CUDA enabled".into(),
                    };
                    if let Some(reason) = report.fallback_reason {
                        self.backend_label = format!("CPU · GPU unavailable: {reason}");
                    }
                }
                JobEvent::Progress {
                    phase,
                    completed,
                    total,
                } => {
                    if self.progress_phase != phase {
                        self.progress_phase = phase.into();
                        self.phase_started = Instant::now();
                        self.remaining = None;
                    } else if completed >= 2 && completed < total {
                        self.remaining = Some(
                            self.phase_started
                                .elapsed()
                                .mul_f64((total - completed) as f64 / completed as f64),
                        );
                    } else {
                        self.remaining = None;
                    }
                    self.progress = if total == 0 {
                        0.0
                    } else {
                        completed as f32 / total as f32
                    };
                    self.status = format!("{phase} · {completed}/{total}");
                    if self.state.phase != Phase::Cancelling {
                        self.state.phase = if self.is_exporting {
                            Phase::Exporting
                        } else {
                            match phase {
                                "Validating package" | "Loading model" => Phase::Validating,
                                "Decoding" | "Resampling input" => Phase::Decoding,
                                "Separating" => Phase::Separating,
                                _ => Phase::Preparing,
                            }
                        };
                    }
                }
                JobEvent::Ready(result) => {
                    if !self.is_exporting {
                        self.state.set_sources(
                            &result
                                .stems
                                .iter()
                                .map(|s| (s.id.clone(), s.label.clone()))
                                .collect::<Vec<_>>(),
                        );
                        self.selected = vec![true; result.stems.len()];
                        self.original = false;
                        self.result_song = self.pending_song.clone();
                        self.generation = self.generation.wrapping_add(1);
                        self.waveforms = None;
                        self.waveform_error = None;
                        self.waveform_worker = Some(crate::waveform::WaveformWorker::start(
                            result.clone(),
                            self.generation,
                        ));
                        self.result = Some(result);
                        self.state.has_result = true;
                        self.open_preview();
                        self.status = "Stems are ready. Audition or export aligned WAVs.".into();
                    }
                    self.state.phase = Phase::Ready;
                    self.is_exporting = false;
                    self.progress = 1.0;
                }
                JobEvent::Failed(error) => {
                    self.error = Some(error);
                    self.state.phase = if self.state.has_result {
                        Phase::Ready
                    } else {
                        Phase::Failed
                    };
                    self.status = "Processing stopped. Correct the error and retry.".into();
                    self.is_exporting = false;
                }
                JobEvent::Cancelled => {
                    self.state.phase = if self.state.has_result {
                        Phase::Ready
                    } else {
                        Phase::Cancelled
                    };
                    self.status = "Cancelled. You can choose another song or retry.".into();
                    self.is_exporting = false;
                }
                JobEvent::Exported(path) => {
                    self.status = format!("Exported aligned WAVs to {}", path.display())
                }
            }
        }
        if self.playback.as_ref().is_some_and(|p| p.failed()) {
            self.state.has_device = false;
            self.error = Some(
                "Audio device disconnected or preview read failed. Retry audio to reconnect."
                    .into(),
            );
        }
    }
    pub fn draw(&mut self, ctx: &egui::Context) {
        self.poll();
        self.draw_workspace(ctx);
        self.draw_export_dialog(ctx);
        if self.state.busy()
            || self.waveform_worker.is_some()
            || self.package_worker.is_some()
            || self.dialog_worker.is_some()
            || self.playback.as_ref().is_some_and(|p| p.is_playing())
        {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
    fn toggle_play(&mut self) {
        if let Some(p) = &self.playback {
            if p.is_playing() {
                p.pause();
            } else if let Err(e) = p.play() {
                self.error = Some(e.to_string());
            }
        }
    }
    pub fn shutdown(&mut self) {
        if let Some(mut worker) = self.waveform_worker.take() {
            worker.stop();
        }
        if let Some(mut playback) = self.playback.take() {
            playback.close();
        }
        let _ = self.job.shutdown();
    }
}
pub fn time_label(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && self.job.is_running() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
            self.job.cancel();
            self.state.cancel();
        }
        self.draw(ctx);
        if self.closing && !self.job.is_running() {
            self.shutdown();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.shutdown();
    }
}
