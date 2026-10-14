# Offline Rust Stem Separator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking. Execution method must be selected by the user before implementation.

**Goal:** Deliver offline six-stem separation, synchronized audition, and aligned WAV export in a native Windows/Linux Rust application.

**Architecture:** Keep the egui desktop shell separate from a reusable Rust separation core. Validate real CPU inference before implementing playback/UI. Model manifests select known backend adapters, and results are cached on disk.

**Tech Stack:** Rust, egui/eframe, ONNX Runtime through ort (subject to Task 1), Symphonia, Rubato, CPAL, serde, SHA-256, and a float WAV reader/writer. Pin actual compatible versions and commit Cargo.lock during implementation; no unverified version numbers are prescribed here.

**Spec:** `docs/superpowers/specs/2026-10-12-stem-separator-design.md`

## Global Constraints

- Target Windows x86_64 and Linux x86_64. macOS is outside the current request.
- Native standalone desktop application; no browser, account, server, or Python installation required by end users.
- One song and one separation job at a time. No batch queue or DAW plugin in the initial version.
- Import WAV, FLAC, and MP3, including mono and stereo input. Reject unsupported channel layouts explicitly.
- Export selected stems as 32-bit float WAV at the input sample rate, preserving stereo where present.
- CPU is the baseline on both supported operating systems.
- No automatic download is required for first use.
- Model compatibility and quality remain unverified until these checks run.
- Never replace input files. Export into a fresh job-specific directory and reject existing target files.
- Git operations are unavailable in this workspace. Save and verify each task; commit only if a usable writable repository becomes available.
- Optional GPU packages follow the CPU release; GPU support is not a CPU release acceptance condition.

## Review Focus

- Non-ASCII paths and long song names: import, caching, and export work on both platforms (Tasks 3 and 6).
- Empty/truncated audio and mid-stream decoding errors: stop without publishing partial successful results (Task 3).
- Unsafe manifest paths and stem labels: reject escaped asset paths and keep exports inside the chosen directory (Tasks 2 and 6).
- Repeated Start, Cancel, and close actions: one worker remains authoritative and playback handles close before cleanup (Tasks 4 and 7).
- Device removal and disk exhaustion during use: preserve completed separation results and show actionable errors (Tasks 5 and 6).

## Files and ownership

- `Cargo.toml`, `Cargo.lock`: workspace and pinned dependencies.
- `crates/stem-core/src/{lib,error,types}.rs`: shared public contracts.
- `crates/stem-core/src/model/{mod,manifest,onnx}.rs`: capabilities, package validation, and inference.
- `crates/stem-core/src/audio/{mod,decode,resample,cache}.rs`: streaming audio conversion and disk cache.
- `crates/stem-core/src/{pipeline,job,export}.rs`: separation reconstruction, worker state, and export.
- `crates/stem-playback/src/{lib,transport,mixer,device}.rs`: streaming audition.
- `crates/stem-desktop/src/{main,app,model_panel,stem_mixer}.rs`: native shell.
- `crates/stem-cli/src/main.rs`: headless inference/validation runner using the same core.
- `crates/stem-core/tests/{model_contract,manifest,audio,pipeline,job,export}.rs` and `crates/stem-playback/tests/transport.rs`: integration proofs.
- `tools/{export_demucs,compare_reference}.py`: development-only conversion and parity tools.
- `tests/fixtures/{README.md,manifest.json}`: fixture provenance and hashes; large audio/model assets are kept outside source control unless their size/license permit inclusion.
- `docs/validation/{cpu-baseline,platform-release}.md`: measured evidence.
- `packaging/{windows,linux}/`: reproducible package instructions and scripts.

Do not create all files upfront. Create each when its task needs it.

## Shared contracts

Use `Result<T, StemError>` throughout public core functions. `StemError` has separate IO, decode, invalid package, unsupported format/profile, inference, cancelled, and export variants with human-readable context.

`AudioSpec { sample_rate: u32, channels: u16, frames: u64 }` records exact decoded length. `AudioChunk { channels: Vec<Vec<f32>> }` is planar and validated for equal lengths. `StemId(String)` is a validated, unique package identifier; filenames are generated from its validated safe form, never display labels.

`ModelCapabilities` contains model ID/version, audio contract, ordered `(StemId, String)` labels, chunk/context settings, and preprocessing profile. `ModelPackage` owns its canonical local path, parsed manifest, and capabilities. `StemChunk` contains ordered `(StemId, AudioChunk)` outputs.

`trait SeparationBackend: Send { fn capabilities(&self) -> &ModelCapabilities; fn infer(&mut self, input: &AudioChunk) -> Result<StemChunk, StemError>; }`

`CachedAudio { path: PathBuf, spec: AudioSpec }`, `StemFile { id: StemId, label: String, audio: CachedAudio }`, and `SeparationResult { original: CachedAudio, stems: Vec<StemFile>, model_id: String }` carry disk-backed results.

`CancellationToken` wraps a shared atomic flag with `cancel()` and `is_cancelled()`. `JobEvent` carries phase, completed/total work, result, cancellation, or failure. Public constructors validate contracts; prevent mismatched channel lengths and stem ordering from reaching DSP code.

### Task 1: Prove a distributable six-stem CPU backend

**Files:** Workspace manifests, core shared contracts, `model/onnx.rs`, CLI, conversion/comparison tools, `tests/model_contract.rs`, fixture provenance, `docs/validation/cpu-baseline.md`.

**Interfaces:** Produces `SeparationBackend`, shared audio/model types, and `OnnxBackend::load(package: &ModelPackage) -> Result<Self, StemError>`. Begin with a validated hardcoded development package contract; Task 2 replaces package creation with manifest validation.

- [x] Write `six_stem_cpu_matches_reference`: load external real model/fixture paths, force CPU execution, assert six ordered source outputs, correct shape, finite values, and reference parity. Missing assets must fail this explicit validation command rather than quietly skip.
- [x] Run `cargo test -p stem-core --test model_contract -- --ignored --nocapture`; expect failure until actual assets and backend exist. Keep asset-dependent tests explicitly marked ignored for ordinary unit runs, and run them separately as a mandatory gate.
- [ ] Trace six-source HTDemucs preprocessing, padding, normalization, tensor names, and reconstruction from upstream implementation; document an exact contract. Prepare a licensed, hashed local checkpoint and development-only export tool if no verified export exists.
- [ ] Implement `OnnxBackend::load` and `infer` using bundled-runtime loading and the default CPU provider. Pin dependencies only after a real compile/load succeeds.
- [ ] Implement CLI `stem-cli validate-model --package PATH --fixture PATH --reference PATH`, invoking the same adapter; obtain reference output using identical checkpoint/settings. Choose and record parity tolerances from measured export-versus-reference error, with per-stem metrics and listening results rather than a loose arbitrary threshold.
- [ ] Run the real validation test and CLI on named CPU hardware. Record hashes, durations, peak RAM, reference metrics, license/provenance, and raw logs in `cpu-baseline.md`. Compare short/odd-length and silent fixtures as well as a musical excerpt.
- [ ] If conversion/runtime parity cannot be demonstrated, stop dependent tasks and evaluate Rust/Burn CPU support. Update the design with concrete evidence before choosing that route. Do not substitute fake separation or declare the backend complete.

### Task 2: Validate offline model packages

**Files:** `model/{mod,manifest}.rs`, `tests/manifest.rs`, package format documentation in `README.md`.

**Interfaces:** Produces `ModelPackage::load(path: &Path) -> Result<Self, StemError>` and `open_backend(package: &ModelPackage) -> Result<Box<dyn SeparationBackend>, StemError>`.

- [ ] Write `manifest_contract_validation` with cases for schema version, asset hash mismatch, absent assets, unknown backend/profile, tensor mismatch, duplicate stem IDs, and escaped asset paths. Assert rejection of `../outside.onnx` and symlink assets outside package root; assert valid non-six-source capabilities preserve manifest order.
- [ ] Run `cargo test -p stem-core --test manifest`; expect failing assertions or missing implementation.
- [ ] Implement the JSON schema from the spec, canonical containment checks, streaming checksums, provenance/license fields, and adapter-specific shape/profile validation. Load only local assets; no HTTP client is necessary.
- [ ] Replace Task 1's development package construction with `ModelPackage::load` and rerun manifest tests plus real model validation. Expect the tested package to load and incompatible packages to fail before inference.

### Task 3: Stream audio and reconstruct aligned stems

**Files:** `audio/{mod,decode,resample,cache}.rs`, `pipeline.rs`, `tests/{audio,pipeline}.rs`.

**Interfaces:** Produces `decode_to_cache(input: &Path, cache_dir: &Path, cancel: &CancellationToken) -> Result<CachedAudio, StemError>` and `separate(input: &CachedAudio, backend: &mut dyn SeparationBackend, cache_dir: &Path, cancel: &CancellationToken, emit: &mut dyn FnMut(JobEvent)) -> Result<SeparationResult, StemError>`.

- [ ] Write `decoded_audio_contract` for WAV/FLAC/MP3, mono/stereo, 48 kHz and model-rate input, silent and odd-length signals, Unicode names, empty/truncated files, mid-stream failure, and rejected multichannel layouts. Assert exact decoded metadata and actionable errors.
- [ ] Write `reconstruction_alignment` asserting `stem.audio.spec.frames == input.spec.frames` and equal sample rates/layout across every stem. Use deterministic test adapters for chunk mechanics, and the real adapter for the separation proof. Include impulses at chunk joins and cancellation before/after inference.
- [ ] Run `cargo test -p stem-core --test audio --test pipeline`; expect failure before pipeline implementation.
- [ ] Implement streaming Symphonia decoding, disk caches, delay-aware Rubato conversion, reference normalization reversal, model-specific contextual chunks and overlap reconstruction. Trim final frames to the original exact count and reject non-finite model outputs. Use memory bounded by chunk size, not song duration.
- [ ] Add space estimation and runtime IO failure handling. Run focused tests and real fixture parity through the complete pipeline; record memory scaling for increasingly long inputs and verify no boundary artifacts introduced by reconstruction.

### Task 4: Own one cancellable background job

**Files:** `job.rs`, `tests/job.rs`.

**Interfaces:** Produces `JobController::start(input: PathBuf, package: ModelPackage) -> Result<(), StemError>`, `cancel(&self)`, `poll(&mut self) -> Vec<JobEvent>`, and `shutdown(&mut self) -> Result<(), StemError>`. Controller owns worker lifetime and its temporary directory; it retains completed results until replaced or shut down.

- [ ] Write `single_job_lifecycle`: repeated start is rejected, Cancel is idempotent, cancelling persists until current inference finishes, no success arrives after cancellation, and shutdown joins the worker before cache deletion. Test failures in decode/inference and interrupted prior-run directories.
- [ ] Run `cargo test -p stem-core --test job`; expect failure before controller implementation.
- [ ] Implement state transitions from the spec, channel-based events, CPU thread budgeting, monotonic per-phase progress, elapsed time, and measured remaining-time estimates. Validate cleanup ownership before deleting abandoned jobs.
- [ ] Run focused lifecycle tests with delayed test adapters. Verify completed cache remains available after export failure and all worker handles are joined on shutdown.

### Task 5: Audition synchronized disk-backed stems

**Files:** Playback crate and `tests/transport.rs`.

**Interfaces:** Produces `PlaybackEngine::open(result: &SeparationResult) -> Result<Self, PlaybackError>`, `play()`, `pause()`, `seek(frame: u64)`, `set_mix(id: &StemId, gain: f32, mute: bool, solo: bool)`, `set_original(enabled: bool)`, `position_frames() -> u64`, and `close()`. PlaybackError includes no-device and disconnected-device cases.

- [ ] Write `shared_transport_clock` using a deterministic output sink: stems stay aligned through play/pause/seek, solo/mute rules are consistent, switching to original preserves position, and input rate differing from device rate preserves duration/pitch. Test device disconnect and end-of-track behavior.
- [ ] Run `cargo test -p stem-playback --test transport`; expect failure before engine implementation.
- [ ] Implement CPAL output with a disk reader/resampler producer and bounded ring buffer. Maintain one transport clock, prefill after seeks, output silence on underrun, and keep IO/allocation/inference out of the audio callback. Gain changes use short ramps to prevent clicks.
- [ ] Run deterministic tests and audition on available hardware during a simultaneous separation job. Record manual disconnect/reconnect behavior; missing audio devices must not prevent core separation/export.

### Task 6: Export raw stems and audition mixes safely

**Files:** `export.rs`, `tests/export.rs`.

**Interfaces:** Produces `ExportRequest { output_root: PathBuf, stem_ids: Vec<StemId>, mix: Option<Vec<MixSetting>> }`, `MixSetting { id: StemId, gain: f32, mute: bool, solo: bool }`, and `export(result: &SeparationResult, request: &ExportRequest, cancel: &CancellationToken, emit: &mut dyn FnMut(JobEvent)) -> Result<PathBuf, StemError>`.

- [ ] Write `export_float_alignment`: reread exported files and assert 32-bit float format, original sample rate/frame count/layout, and unchanged raw stem amplitude. Assert mix samples follow mixer settings without independent normalization.
- [ ] Write `export_failure_preserves_results`: filename containment, Unicode paths, existing target rejection, insufficient-space/write errors, and cancellation leave cached results usable and no falsely completed export directory. Preexisting files remain byte-identical.
- [ ] Run `cargo test -p stem-core --test export`; expect failure before export implementation.
- [ ] Implement streaming float WAV writes into an exclusively reserved new job directory; publish completion only after all files close successfully. Partial-directory cleanup must only remove paths created by the current export. Respect mute/solo/gain only for mixed export.
- [ ] Run focused tests and import a real exported stem set into a DAW to verify shared start time and duration.

### Task 7: Connect the native desktop flow

**Files:** Desktop crate, `tests/desktop_state.rs`, `docs/validation/platform-release.md`.

**Interfaces:** `DesktopApp` owns `JobController`, optional `PlaybackEngine`, model catalog, current result, and view state. Uses prior public contracts without duplicating DSP or inference.

- [ ] Before visual implementation, establish visual direction and applicable UI skill preferences. Product layout centers on file/model selection, processing status, and the dynamic stem mixer; visual direction is not yet approved in the technical spec.
- [ ] Write state tests for Start/Cancel/close races, model changes applying to the next job, non-six-source mixer rows, export enabled only for completed results, and audition disabled when no device exists. Run `cargo test -p stem-desktop`; expect failure before state wiring.
- [ ] Implement native file/folder selection, local package installation/selection, transport and seek, dynamic stem rows, output choices, progress, empty/error/cancelling states, and explicit result replacement. Closing the app closes playback before joining worker/cleaning cache.
- [ ] Run `cargo run -p stem-desktop --release`. Verify every control, keyboard focus and activation, resizing, errors, no-device behavior, and responsiveness while real inference runs. Record manual evidence on Windows and Linux; headless state tests do not prove GUI behavior.

### Task 8: Package and prove offline operation on both platforms

**Files:** Packaging scripts, license notices, README, platform validation report.

**Interfaces:** Produces complete Windows/Linux CPU distributions containing the executable, required runtime libraries, tested default six-stem package, model manifest, and attribution. Optional acceleration remains a separately validated follow-up.

- [ ] Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` after relevant tasks pass. Run mandatory real-asset validation separately; ignored tests are not release proof.
- [ ] Create platform-local release builds with `cargo build --workspace --release`. Document Linux system/display/audio dependencies and a tested distro baseline; do not claim all Linux distributions are supported by one build.
- [ ] Verify redistribution rights for exact model/runtime assets, include license texts and provenance, and generate package asset hashes. Do not distribute unresolved-license weights.
- [ ] Test each packaged app on a clean target machine with networking disabled, no Python, no system ONNX Runtime, and no GPU requirement. Import, separate, audition, cancel/retry, switch local packages, and export into a DAW.
- [ ] Record runtime, peak memory, actual CPU/device, package hashes, model quality limitations, keyboard/click-through checks, and known limitations in `platform-release.md`. If Windows or Linux cannot be exercised here, label that release validation pending and continue available work; never assert both platforms passed from a Linux-only build.

## Implementation order and stop conditions

Execute Tasks 1–8 sequentially; Tasks 5 and 6 both consume Task 3 results but share playback/export semantics. Task 1 is a feasibility gate: full UI work must wait for evidence of real CPU inference and a lawful, usable default model package. A blocked conversion is reported with logs and alternatives, rather than hidden behind a working-looking UI.

At each task boundary, run its focused proof and record results. Avoid unrelated cleanup or repeatedly running broad tests before new code warrants them. The current workspace cannot commit; do not attempt to alter its read-only `.git` placeholder.

## Library evidence

- [Symphonia decoding formats](https://github.com/pdeljanov/Symphonia)
- [Rubato resampling](https://github.com/HEnquist/rubato)
- [CPAL native audio output](https://github.com/RustAudio/cpal)
- Inference, native UI, and model references are linked in the approved spec.

## Plan self-review

The tasks cover the approved CPU-release scope, model replacement limits, disk-backed processing, recovery, playback, safe exports, and platform evidence. Shared contracts keep IDs and frame counts consistent between pipeline, playback, and export. All five review-focus conditions have explicit test ownership. GPU acceleration is scheduled after the first CPU release as specified; CPU validation is not replaced by scaffold tests.
