# NVIDIA, Four-Stem Quality and Studio UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for the preserved native execution preference. Implement task-by-task, then request one fresh final code review. Steps use checkbox syntax.

**Goal:** Add optional NVIDIA acceleration, validated local HTDemucs FT support, and a professional native audio workspace while preserving reliable CPU separation and aligned exports.

**Architecture:** Explicit processing options flow from desktop/CLI through the job runner to ONNX sessions. A versioned four-specialist package runs one full-song pass at a time with one resident session; the existing one-model package remains supported. The UI separates setup, cached audio timeline, persistent transport and export, using background peak extraction and existing job/playback services.

**Tech stack:** Rust >=1.88, existing pinned ort 2.0.0-rc.10 / ONNX Runtime 1.22.0 first candidate, eframe/egui 0.33.3, Symphonia/Rubato/CPAL, existing local development reference environments. No new end-user Python dependency.

**Spec:** docs/superpowers/specs/2026-10-12-cuda-four-stem-design.md, including the UI/UX extension requested with design approval.

## Global constraints

- Offline native Windows/Linux x86_64; CPU-only Linux must continue working. The current environment cannot certify Windows or RTX execution.
- Auto reports CUDA initialization failure and chooses CPU; explicit NVIDIA errors, explicit CPU never registers CUDA. One runtime library per process.
- Baseline FP32, TF32 disabled, CUDA device 0, 12 GiB arena cap (not a total VRAM guarantee). One heavy session resident.
- Existing six-stem profile unchanged. FT canonical source order is drums/bass/other/vocals with one-hot specialist mapping.
- Preserve exact original frame count/rate/layout, raw amplitude relationships, positive normalized overlap reconstruction, non-overwriting export, and job-owned cache cleanup.
- FT assets are explicit local imports with primary-author usage notices. Do not present original pretrained weights as unrestricted MIT or silently add them to a general-purpose distribution.
- Dark studio with amber controls; Antislop during implementation, ENERGY 2 / RHYTHM 2 / MOTION 1; real waveforms only. Minimum 640×600 and 200% UI scale behavior tested.
- Workspace has no usable Git repository. Record verification in the ledger instead of claiming commits/PRs. Preserve the currently working distribution while preparing its replacement.

## Review focus

- CUDA provider listed but unusable due to missing DLLs/device/OOM/unsupported graph: active-backend reporting follows actual session/warm-up success, not availability alone (Task 1).
- Four-specialist cancellation or disk failure after earlier passes: no incomplete Ready result, clean only owned scratch, keep previous result (Task 2).
- Original external normalization differs from the exported graph: measured original-checkpoint parity and documented profile, never adjust tolerances to hide mismatch (Task 3).
- Async waveform/import result arrives after replacement or cache deletion: generation guard and joined readers; displayed song/model belongs to the current result (Task 4).
- Narrow/zoomed workspace, long Unicode names, modal keyboard focus and missing audio output: all recovery/transport/export actions stay reachable and meaningful (Task 5).

## Task 1: Processing preferences and real CUDA session selection

**Files:** Create crates/stem-core/src/model/device.rs and tests/device.rs; modify model.rs, model/onnx.rs, job.rs and stem-cli/src/main.rs; extend CLI tests.

**Interfaces:** `ProcessingDevice::{Auto,Cpu,Nvidia}`; `BackendOptions { device: ProcessingDevice, device_id: i32, cpu_threads: usize, cuda_memory_limit: usize }`; `BackendReport { requested: ProcessingDevice, active: ActiveDevice, fallback_reason: Option<String> }`; `ActiveDevice::{Cpu,Cuda}`. `OnnxBackend::load_with_options(package, runtime, options) -> Result<Self, StemError>`, `report(&self) -> &BackendReport`. Retain CPU convenience loaders. Add `JobEvent::BackendSelected(BackendReport)`. Job start methods accept options through new variants without breaking existing callers; CLI `--device auto|cpu|nvidia` is optional, defaults Auto for end-user separation and CPU for legacy development checks.

- [ ] Write failing tests for parsing/defaults, CPU never attempting CUDA, Auto initialization/warm-up failure retaining a concrete reason, strict NVIDIA failure, and nonnegative device/nonzero thread/memory parameters. Use an injectable initialization boundary to test deterministic selection, and separate real-runtime checks.
- [ ] Run focused tests and confirm failure before implementation.
- [ ] Implement strict CUDA registration via error_on_failure, session creation and zero-chunk warm-up; fresh CPU session in the same runtime on Auto failure. Disable TF32. Emit report only after success. CLI validates optional flags before model/file IO and prints backend/fallback.
- [ ] Verify with local CPU runtime: CPU and Auto real inference, explicit NVIDIA clear error, malformed runtime, no output on failure, unchanged original-reference parity. Confirm all errors reach worker/desktop rather than unwinding.
- [ ] Record proof and remaining Windows/CUDA hardware checks in the ledger.

## Task 2: Versioned specialist manifest and sequential full-song runner

**Files:** Modify model/manifest.rs and model/onnx.rs; create model/specialists.rs and pipeline/package.rs; modify pipeline.rs, job.rs and CLI; add tests/specialists.rs and extend manifest/job tests.

**Interfaces:** Schema 1 remains the current single-asset profile. Schema 2 `htdemucs-ft-bag-v1` / `graph-internal-ft-v1` declares exactly four `Specialist { source_id, output_row, model: Asset }` values with canonical mapping; use tagged/versioned parsing so absent fields and unknown fields cannot silently change schema-1 meaning. `ModelPackage::specialists()` exposes validated asset paths/mappings. `OnnxBackend::load_specialist(...)` validates native four-output graph but exposes exactly the selected source to SeparationBackend. `separate_package(input, package, runtime, options, cache_dir, cancel, emit) -> Result<SeparationResult, StemError>` dispatches one-model or specialist passes.

- [ ] Write failing manifest tests for incomplete/repeated/cross-mapped specialists, corrupt/missing assets, unsafe paths/symlinks, wrong dimensions/profile and preserved schema-1 loading.
- [ ] Write runner tests with mock specialist factories for canonical aggregation, one resident session, global/pass progress, exact alignment, cancellation between passes, disk/read failure and retention of the previous result.
- [ ] Run the tests RED, then implement strict validation and source-row extraction. Decode once; process each specialist in a distinct owned subdirectory, drop session/readers before removing scratch, and publish Ready only after all passes and contract checks.
- [ ] Verify ordinary core/CLI tests and full-song identity/join tests. Keep cancellation responsive between model operations; never publish mixed partially completed sources.
- [ ] Record the package schema and execution behavior in the ledger.

## Task 3: Real FT provenance, preprocessing and checkpoint comparison

**Files:** Create tools/reference_ft.py and tests/fixtures/manifest-ft.json; extend validation records and model validation CLI as needed. Local large evaluation assets stay outside source under validation-assets/ and models/.

- [ ] Pin export revision 8616370ed541bc183dbe15fb0d54d5f49918f47e and the four LFS SHA-256 values; record original source, export/code notices and primary-author scientific-use clarification separately. Acquire only explicit development/evaluation assets, not a newly bundled general-purpose default.
- [ ] Inspect the actual native graph contract and upstream reference preprocessing. If normalization exists outside the graph, implement it explicitly in this new profile with finite/silence/DC tests before using it; preserve the existing six-stem profile.
- [ ] Generate original CPU references for the four canonical checkpoints with deterministic positional settings. Measure max error/RMSE on music, silence and padded audio, then set a documented regression tolerance. Reject an export/profile mismatch rather than broadening tolerance.
- [ ] Run package CLI through all four specialists on a short real audio fixture and a licensed full-song fixture; independently verify source order, frame count, finite values and overlap reconstruction.
- [ ] Record CPU timing/memory and remaining ground-truth quality/GPU comparison requirements. Describe FT as a quality candidate until project measurements justify a stronger claim.

## Task 4: Background waveform envelopes and result identity

**Files:** Create stem-desktop/src/waveform.rs and tests/waveform.rs; modify app.rs and add a focused session-view module if needed.

**Interfaces:** `Peak { min: f32, max: f32 }`; `WaveformSet { generation: u64, original: Vec<Peak>, stems: Vec<(StemId, Vec<Peak>)>, frames: u64 }`; bounded cache scanning with at most 2048 time buckets per waveform. A cancellable worker returns generation-tagged results. The app stores result song identity separately from pending input/model choices.

- [ ] Write failing tests for mono/stereo, silence, first/last impulses, exact bucket coverage, finite extrema and equal time alignment. Test stale generation rejection and replacement/shutdown waiting for readers before cache reclamation.
- [ ] Implement bounded CacheReader scanning off the UI thread, cancel/join on replacement, and explicit loading/failure states. Use a shared raw amplitude scale; no invented waveform or random meter animation.
- [ ] Verify waveform mechanics and preserve usable transport/export while envelopes prepare. The current result keeps its original filename/model after a failed replacement.
- [ ] Record bounded memory/work duration and lifecycle evidence.

## Task 5: Studio workspace and export UX

**Files:** Create stem-desktop/src/theme.rs, workspace.rs, export_dialog.rs; refactor app.rs into job coordination plus focused draw methods; modify state.rs, main.rs, desktop_controls.rs and native smoke example.

**Interfaces:** Setup selection and result-session display remain separate. Theme centralizes colors/type/spacing. Native wide layout uses 260–300 px setup plus aligned stem workspace; below the breakpoint setup stacks with scroll. Persistent transport stays reachable. Export dialog builds the existing ExportRequest from source selections/settings and closes with Escape; processing preference and validated local model selector integrate Tasks 1–3.

- [ ] Write failing harness tests for Import/Separate/Cancel/Retry/Export, current-result identity, source-specific mute/solo/gain controls, real fallback status, busy-state gating, model/import validation and export modal selections. Keep existing real-model workflow test meaningful across the layout change.
- [ ] Implement header/setup/workspace/transport/export flow; expandable paths and diagnostics; phase/pass progress; clearly attributed failures and recovery; original/stems comparison; numeric gain reset; keyboard Space/Ctrl+O/Ctrl+E/Home/Escape and visible Tab focus. Show recovery buttons only when relevant.
- [ ] Verify 640×600, 1280×820 and 200% UI scale with long Unicode names and multiline errors, four/six/custom labels, no output device, cancellation and failed export. Measure text/control/focus contrast against the spec thresholds.
- [ ] Run ordinary and real-model harness tests, then native window smoke; capture and inspect actual idle/processing/ready/export/error screenshots. Static design studies are not completion evidence.
- [ ] Record the UI/accessibility delivery checks and any platform-only limitations.

## Task 6: Packaging, Windows handoff and final review

**Files:** Modify tools/package-linux.sh, tools/package-windows.ps1, README.md, validation records and notices; add a Windows build/validation script or CI workflow consistent with available repository infrastructure.

- [ ] Add reproducible Windows MSVC CPU/GPU runtime packaging with prerequisite diagnostics, optional explicit local model packages, CUDA/cuDNN dependency version checks, notices and SHA256SUMS. Keep one runtime DLL per package/process and preserve Linux launcher session recovery.
- [ ] Build/check the Linux workspace offline; run relevant real asset checks, strict Clippy and formatting, launcher tests, shell syntax checks and release/package hash verification. Produce a new reviewable Linux package without overwriting the user's current working one.
- [ ] Provide Windows commands that verify CUDA registration, executed graph profile, CPU/GPU error metrics, peak VRAM, same-song timing, cancellation, playback, file dialogs and export on the RTX machine. Explicitly leave native Windows/RTX tests pending if this environment cannot execute them; do not claim a usable Windows executable from unrun packaging code.
- [ ] Dispatch one fresh read-only reviewer under requesting-code-review; fix Critical/Important findings with focused proof. No unrelated cleanup.
- [ ] Deliver runnable verified artifacts, a concise change/test report, and the remaining Windows/RTX and ground-truth quality checks.

## Self-review and execution handoff

Spec coverage: provider behavior, model/profile validation, normalization, bounded specialist memory, cache safety, new UI states, keyboard/resize/accessibility, packaging and hardware limitations each have an owning task. Interfaces are declared above and compatibility methods are retained. The five review-focus cases have explicit tests in their tasks. No fake model-quality/speed claim or completed Windows/GPU certification is permitted. Native execution preference is preserved; this concrete implementation plan awaits user review before code changes, as required by writing-plans.
