# Offline Rust stem separator

Status: implementation in progress under the saved approved plan. Real Rust CPU chunk inference and reference parity are demonstrated on Linux; full-song processing and desktop delivery remain pending.

## Agreed intent

Build a native Rust desktop application for advanced music production on Windows and Linux. Separation runs offline on CPU-only computers. GPU acceleration is optional. Start with vocals, drums, bass, guitar, piano, and other. Users can change supported models later, and the interface adapts to their stem lists.

Success means importing a song, running real local inference, auditioning synchronized stems, and exporting aligned audio for a DAW. Separation quality is model-dependent; this app does not promise recovery of original studio recordings.

## Initial scope and assumptions

- Target Windows x86_64 and Linux x86_64. macOS is outside the current request.
- Native standalone desktop application; no browser, account, server, or Python installation required by end users.
- One song and one separation job at a time. No batch queue or DAW plugin in the initial version.
- Import WAV, FLAC, and MP3, including mono and stereo input. Reject unsupported channel layouts explicitly.
- Select a local model package and output directory. Show the package's available stems before processing.
- Preview the original and separated stems, with transport, seek, per-stem solo, mute, and gain.
- Export selected stems as 32-bit float WAV at the input sample rate, preserving stereo where present. Offer a separate mixed export using the audition gains.
- Exported individual stems are raw model outputs; mixer changes affect preview and mixed export only. Preserve amplitude relationships and do not normalize each stem independently.
- Model packages can be bundled with a release or installed from a local directory. A complete offline release includes the tested default package. No automatic download is required for first use.

## Technology decisions

Use egui/eframe for the desktop shell: it keeps application UI code in Rust and supports native deployment. Use a separate Rust audio/separation core so UI changes do not affect inference or DSP tests.

Evaluate ONNX Runtime through the Rust `ort` binding first. It offers a default CPU provider and optional acceleration providers. Runtime binaries must ship with the application; end users must not need a system-wide runtime installation.

The first candidate is the six-source HTDemucs model. Its ONNX export, tensor contract, preprocessing, and reference parity must be demonstrated before it becomes the default. An upstream PyTorch checkpoint is not directly loadable by ONNX Runtime. Model conversion tooling may run during development, but is not shipped as a user dependency.

Alternative: a Rust/Burn Demucs implementation. This is a candidate if the ONNX route fails validation, but the existing GPU implementation's existence does not prove CPU suitability. A change of backend requires recording the CPU evidence and updating this design before continuing dependent implementation. A Python sidecar would conflict with the intended runtime and is not the chosen approach.

## Components and boundaries

1. Desktop shell: file selection, job state, model list, progress, transport, and stem controls. No inference runs on the UI thread.
2. Model catalog: validates local manifests and assets, lists supported model capabilities, and creates the appropriate backend adapter.
3. Separation core: decoding, resampling, preprocessing, chunk scheduling, backend inference, reconstruction, and aligned result files.
4. Job worker: owns the running job, publishes phase/progress/error events, and accepts cancellation requests.
5. Playback engine: shares one sample clock across stems and streams cached audio with gain, mute, and solo applied. Audio callbacks do not decode files, allocate large buffers, or perform inference.
6. Export service: streams result files into the selected export format and publishes completed files only after successful writes.

Backend adapters expose model capabilities and chunk inference through a Rust interface. Stem IDs, names, and ordering come from validated capabilities rather than six hardcoded UI rows.

## Supported model packages

A versioned JSON manifest describes package ID/version, backend adapter ID, asset filenames and SHA-256 checksums, source/provenance, license text, model sample rate, channel layout, ordered stem IDs and labels, tensor inputs/outputs, chunk length, context/overlap requirements, preprocessing profile, and tested devices.

Adapters implement preprocessing profiles in application code. A manifest cannot make arbitrary ONNX models compatible: unknown adapters, unsupported profiles, inconsistent tensors, duplicate stem IDs, missing assets, and checksum failures are rejected with a useful message.

Installing a compatible package enables different weights or stem lists without redesigning the UI. A new model architecture may require a new adapter and application update. We do not load executable plugins from model packages.

## Audio flow

Decode the input, record its sample rate and exact frame count, and convert mono to the model's required layout if needed. Resample to the model rate and apply the adapter's reference preprocessing.

Process bounded chunks with the model's required context. Reconstruct using the adapter's tested overlap method, remove padding and latency, undo reference normalization, and resample back to the original rate. Trim or pad all outputs to the original frame count. Verify that every result has the same start time, frame count, and channel layout.

Cache decoded input and completed outputs on disk in an app-owned job directory. Stream into the cache and final exports, rather than retaining every complete song and stem in RAM. Chunk memory is bounded by model requirements, independently of song duration. Estimate cache/output space before processing and detect write failures during the job.

## Jobs and recovery

States: idle, validating, decoding, separating, preparing preview, ready, exporting, cancelling, cancelled, and failed. Processing and export show their own phase progress. Show elapsed time; show remaining time only once measurements support an estimate.

Check cancellation between chunks and file operations. An active native inference call may finish before cancellation takes effect; display cancelling while it does. Delete incomplete app-owned temporary results. Keep a completed result available when an export fails.

On startup, remove abandoned temporary jobs. Close playback handles before deleting caches on Windows. Never replace input files. Export into a fresh job-specific directory and reject existing target files; overwriting is outside initial scope.

Report missing models/runtime libraries, invalid input, unsupported formats, insufficient disk space, memory failures, inference failures, and missing audio devices in user-facing terms. A missing audio device disables audition while preserving separation and export.

## CPU and optional GPU policy

CPU is the baseline on both supported operating systems. Leave processing capacity for the UI/playback rather than consuming every logical core without limit. Measure throughput and peak memory on named reference machines; do not advertise real-time processing or minimum RAM without measurements.

GPU support is a follow-up once CPU separation is proven. Candidate providers are CUDA on Windows/Linux and DirectML on Windows. Ship acceleration dependencies in an optional package so the CPU distribution does not depend on GPU libraries. Display the actual selected device. If acceleration fails, offer retry on CPU; never silently discard the current job.

## Validation and delivery criteria

Before building the full mixer, run a real six-stem CPU job from Rust and compare it against the reference implementation using identical weights and settings. Use licensed multitrack fixtures and public-domain listening material. Record exact model hashes, reference settings, machine specifications, runtime, peak memory, waveform comparisons, and per-stem quality metrics. Do not treat synthetic or passthrough outputs as proof of separation.

Test resampling/alignment on short, odd-length, mono, and non-model-rate inputs. Test chunk joins, normalization reversal, silent inputs, invalid model contracts, cancellation, export failure, and no-overwrite behavior. Assert equal output frame counts and no non-finite samples. Numerical tolerances must be documented from reference parity measurements before release.

Build and run on Windows and Linux. Test native file dialogs, keyboard controls, missing audio hardware, and responsive UI during inference. Perform an offline cold-start test with network access disabled and only distributed files installed. Confirm that the included model and runtime licenses allow the intended redistribution; include attribution and exact asset provenance.

Release requires real six-stem CPU inference, synchronized audition, aligned exports, cancellation, and verified offline operation on both target platforms. GPU acceleration is not required for the first CPU release. Model compatibility and quality remain unverified until these checks run.

## Evidence consulted

- [egui native Rust UI](https://github.com/emilk/egui)
- [Rust ONNX Runtime binding](https://github.com/pykeio/ort)
- [ONNX Runtime execution providers](https://onnxruntime.ai/docs/execution-providers/)
- [Demucs model documentation](https://github.com/facebookresearch/demucs): six-source model is experimental and piano can have substantial artifacts/bleed.
- [Existing Rust Demucs implementation](https://github.com/nikhilunni/demucs-rs): an alternative implementation with GPU support, not evidence of our CPU or packaging validation.

## Initial workspace constraints (historical)

Rust and Cargo are installed. The workspace is empty and its `.git` directory is a read-only environment placeholder, not a usable Git repository. This design can be saved here but cannot currently be committed. No application scaffolding, dependency installation, or inference experiment has been performed.
