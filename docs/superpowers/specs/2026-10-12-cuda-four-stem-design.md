# NVIDIA acceleration and four-stem quality extension

Status: concrete design proposal awaiting user review; implementation has not started. Extends the working offline Rust/Linux app. User requests GPU support for a Windows RTX 5060 Ti 16 GB and prioritizes vocals, drums, bass, and other quality over six-stem output. Existing native execution preference and dark studio/amber UI choice persist.

## Outcome and scope

Add Auto / NVIDIA GPU / CPU processing selection to desktop and CLI, show the selected backend and any fallback reason, and support a local HTDemucs FT four-specialist package. Preserve the existing six-stem option. Keep runtime inference in Rust, offline, without Python or a server. Continue disk-backed decoding, alignment, mixing, export, cancellation, and cache ownership.

HTDemucs FT is the initial four-stem quality candidate, not a claim of universally best separation. Upstream describes the fine-tuned version as possibly better with approximately four times the compute. RoFormer alternatives offer a different architecture and potentially stronger results, but require separately validated spectral preprocessing, checkpoints and licensing. They are outside this first extension; swapping architecture names without implementing those contracts would be incorrect.

## GPU behavior

Processing preference is immutable during a running job. Auto attempts CUDA initialization and model warm-up; if unavailable, it creates a CPU session in the same loaded runtime and reports the concrete reason. Explicit NVIDIA mode reports an actionable error if CUDA cannot initialize or run the selected model; it never silently labels CPU execution as GPU. CPU mode never registers CUDA. CUDA may leave unsupported graph operators on CPU; UI should say CUDA enabled rather than imply every operator runs on the GPU.

A GPU-capable ONNX Runtime DLL also supplies CPU execution, so Windows uses one runtime library per process. The ort binding selects its library once; no switching between CPU/GPU DLLs after initialization. Existing CPU-only Linux distribution remains usable: Auto selects CPU, explicit NVIDIA mode explains the missing provider. Provider availability alone is insufficient: registration/session creation and warm-up must succeed before reporting CUDA enabled.

Baseline is float32 with TF32 disabled for controlled numerical comparison. Start with CUDA device 0, one model session, bounded fixed audio chunks, and a 12 GiB CUDA arena limit on the target 16 GiB card. That limit does not bound cuDNN workspace or total VRAM; actual usage must be measured. No untested float16 conversion, TensorRT engine generation, batching, or speed claim. On later CUDA inference failure, do not silently emit mixed partial results; fail cleanly and offer CPU retry.

Windows CUDA runtime compatibility: retain the current ort API-compatible ONNX Runtime 1.22.0 as the first candidate, GPU x64 distribution, CUDA 12.8-compatible libraries and cuDNN 9. Verify the exact vendor binary/dependency versions and Blackwell support before deployment. Package CUDA/cuDNN redistributable DLLs only when redistribution terms and required notices are recorded; otherwise provide a precise prerequisite check. Do not download or install GPU drivers on the user's computer automatically.

## Four-stem package and memory strategy

HTDemucs FT has four original checkpoints, with one-hot per-source bag weights: f7e0c4bc drums, d12395a8 bass, 92cfc3b6 other, 04573f0d vocals. Use the four specialist ONNX exports from pinned StemSplitio/htdemucs-ft-onnx revision 8616370ed541bc183dbe15fb0d54d5f49918f47e. Published graph contract: mix [1,2,343980] -> stems [1,4,2,343980], float32, 44.1 kHz. The corresponding output row is selected for each specialist. Verify downloaded SHA-256 against pinned LFS metadata and original checkpoint parity; no trust in model-card benchmark claims alone.

Add an explicit versioned manifest profile for this four-asset bag. Each source has exactly one model asset, target source ID and output row; enforce the canonical four-source order and exact one-hot mapping. Validate all paths, hashes, provenance, source IDs, and license/usage notices before loading. Existing schema/profile remains supported without changing its interpretation.

Run the full song through one specialist at a time, release that session, then process the next specialist. The input decodes once. Reuse existing rate conversion, chunk padding, normalized positive overlap weights, exact-frame restoration and export. The adapter consumes a graph with four outputs but exposes its selected source to the existing pipeline. Aggregate only after all four passes succeed. Only one heavy model session is resident at a time; avoid a fused four-network graph until memory measurements justify it. Per-pass scratch files are owned by the job and removed when no reader/writer references them. Final four-stem caches and original remain available for audition/export.

Progress includes specialist name, completed chunks in that pass, pass count, elapsed time and measured remaining estimates. Cancellation checks apply between chunks and specialist loads; closing waits for the current inference before reclaiming owned files. CPU fallback decision happens before output files are published.

Normalization/preprocessing must be verified against the pinned original reference implementation, including any song-level normalization outside the exported graph. A new profile must document that rule explicitly; do not normalize each stem independently or infer it from tensor names. Existing six-stem graph profile remains unchanged.

## Model distribution and license correction

The original Demucs author explicitly states that pretrained weights are not covered by the MIT code license and are provided only for scientific purposes: https://github.com/facebookresearch/demucs/issues/327 . This conflicts with conversion model cards that label derivative weights MIT. Record the primary author's statement, exporter metadata and provenance separately. Do not describe the existing or FT checkpoints as unrestricted MIT weights.

The app can support importing compatible local FT assets with their usage terms; the general-purpose application distribution will not silently add disputed weights. Development reference evaluation is scientific/numerical validation. A production music-workflow release needs a checkpoint with explicitly suitable original rights or permission; an exporter/rehost MIT tag alone is insufficient. Existing local assets are not deleted. Correct misleading weight-license language in docs and distribution notices; separate code, conversion code, runtime and weight terms.

## Windows delivery

Provide a reproducible Windows x64 MSVC build/package workflow with CPU/GPU runtime selection, four-stem package import, prerequisite diagnostics and notices/hashes. Keep the existing Linux launcher recovery behavior. There is no Windows toolchain or NVIDIA GPU in this current environment, so a locally written Windows workflow is not a verified Windows executable. Native Windows build and RTX test results are explicit outstanding verification requirements.

## Validation and acceptance

- Deterministic provider-selection tests: Auto CUDA success; Auto registration/session/warm-up failure chooses CPU with reason; explicit CUDA failure errors; CPU never attempts CUDA; invalid mode/device requests fail.
- Real local CPU runtime: default CPU and Auto produce finite aligned output and unchanged reference tolerance. CUDA-required fails clearly against a CPU-only runtime. Runtime-path freezing remains documented.
- Manifest tests: missing/hash-corrupt specialist, duplicate or wrong source/row mapping, unsupported dimensions, unsafe paths, escaped symlinks and incomplete bag rejected before work.
- Original CPU checkpoint reference: per-source music, silence and padded input parity, source ordering, exact dimensions, full-song joins and frame counts; measure tolerance, do not relax it to conceal a wrong export.
- UI controls: model choices expose four/six rows, processing preference is disabled while busy, fallback is visible, retry/previous results remain usable, mixed/raw export still align and preserve amplitude.
- Native Windows/RTX acceptance: CUDA registration and actual executed graph profile, model warm-up, full-song timing relative to CPU, peak VRAM, non-finite/shape checks, CPU/GPU reference comparison, cancellation/close, audio/file-dialog/export behavior and offline launch. Do not claim acceleration or Windows completion without these results.
- Quality assessment: compare current six-stem and FT four-stem on licensed ground-truth multitracks, report per-stem metrics and listening observations. Until supplied, label FT a quality candidate with upstream evidence, not measured superiority on this project.

## Primary references

- Demucs model options and fine-tuned bag: https://github.com/facebookresearch/demucs ; https://github.com/facebookresearch/demucs/blob/main/demucs/remote/htdemucs_ft.yaml
- Specialist export contract and provenance: https://huggingface.co/StemSplitio/htdemucs-ft-onnx/tree/8616370ed541bc183dbe15fb0d54d5f49918f47e
- Weight usage clarification: https://github.com/facebookresearch/demucs/issues/327
- CUDA runtime dependencies: https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html
- Blackwell compatibility: https://docs.nvidia.com/cuda/archive/12.8.2/blackwell-compatibility-guide/index.html

## UI/UX extension requested with design approval

User approved the GPU/four-stem design and additionally requested a professional interface and better UX. Preserve dark studio/amber direction; Antislop applies during implementation. Design intent: a focused audio workbench for one song, with setup kept compact, stem audition taking most of the screen, and transport/export always reachable. ENERGY 2 / RHYTHM 2 / MOTION 1. Amber highlights the primary action and keyboard focus; dark neutrals separate working surfaces. Waveforms carry audio information, never fabricated motion or decoration.

At normal desktop sizes, use a 260–300 logical-pixel setup panel beside a main aligned stem workspace. The top bar shows the app name, current result/song, and a clearly labeled actual processing backend. The setup panel contains Import audio, a model selector with its source list, processing preference, package validation/loading feedback, and the primary Separate action. Full source/package paths and diagnostic details are expandable. Display original-author weight terms at model import. Selectable models are installed/validated packages; uninstalled FT is an explicit import choice, not a fake working model.

The main workspace has a shared time ruler/playhead and per-stem rows: source name, mute/solo, gain with numeric dB/reset, and a waveform derived from that source's cache. All stem waveforms use one time basis and an honest shared amplitude scale. Build bounded peak envelopes on a background worker after results are ready; never decode/read audio in rendering. Keep transport usable while waveforms prepare, reject stale peak results after song replacement, and stop/join cache readers before deleting results. Show a clear preparation state or timeline without waveform while data is absent. No animated fake meters or filler audio statistics.

A persistent bottom transport contains Play/Pause, Return to start, Original/Stems comparison, native-clock time/seek, audio-device recovery when needed, and Export. Export opens a keyboard-accessible modal with stem selection, output folder, optional audition mix, format/alignment summary, and the concrete Export selected WAVs action. Raw exports remain unaffected by preview gain/mute/solo. Success gives the actual completed folder; cancellation/failure preserves current results and offers recovery. Do not hide processing failures behind an audio-device error.

Processing displays current phase/specialist, progress, elapsed time and estimates only when measured, plus Cancel in the workspace. Controls incompatible with processing are disabled with a reason. Keep old result identity distinct from the next input/model selection, so an unsuccessful replacement never labels old stems as belonging to a new song. Confirm replacement only when its consequence is concrete; new song import alone does not delete an existing result. Space controls transport outside text editing; Ctrl+O imports, Ctrl+E opens export, Home seeks to start outside text editing, Escape closes modal. Tab focus is visible and source-specific accessibility labels are provided for all mute, solo, gain and export controls.

Default window target 1280×820; retain 640×600 minimum compatibility by stacking setup above the workspace with vertical scrolling while transport remains reachable. At intermediate widths reduce waveforms before truncating operational controls. Test six/four/custom source labels, long Unicode filenames, multiline actionable errors, no audio device, busy/cancelling states and 200% UI scale. Text must remain readable; text contrast >=4.5:1 and control/focus boundaries >=3:1. Theme lives in a focused module; layout, export dialog, peak worker, and job logic are separated instead of growing the current minified draw method.

A static layout study accompanies the plan. It is a design artifact using current fixture waveforms, not a completed interface or evidence of FT/GPU output. Final delivery requires actual native-window screenshots plus keyboard/control tests after implementation.
