# CPU inference validation

Status: real six-stem Rust CPU inference and original PyTorch checkpoint parity passed on Linux on 2026-10-12. Task 1 remains incomplete: listening assessment, licensed multitrack quality measurements, complete package provenance/attribution, and package-based validation CLI remain pending. Windows, GPU, full-song reconstruction, and release validation remain pending.

## Environment evidence

- Linux x86_64 workspace with Rust and Cargo 1.99.0.
- Network access now works; Rust dependencies are pinned in Cargo.lock and cached in `.cargo-cache/`. Use `CARGO_HOME="$PWD/.cargo-cache"` to build offline.
- Intel Core Ultra 5 125H, 18 logical CPUs, approximately 15 GiB RAM.
- `ort` and `ort-sys` pinned to `2.0.0-rc.10`; local ONNX Runtime CPU 1.22.0 from [Microsoft's release](https://github.com/microsoft/onnxruntime/releases/tag/v1.22.0). MIT license and third-party notices are retained in `runtime/onnxruntime-linux-x64-1.22.0/`.
- CPU provider only, two intra-op threads, one inter-op thread, graph optimization Level3.
- Original-reference environment: Python 3.12.15, torch/torchaudio 2.4.1+cpu, Demucs 4.0.1, NumPy 1.26.4, under `validation-assets/reference-venv/`.
- Diagnostic Python ONNX reference uses Runtime 1.31.0 under `validation-assets/venv/`. That comparison alone does not prove export parity; acceptance uses original PyTorch outputs.
- No system-wide libraries or Python packages were installed. Native inference requires no Python.

Runtime loading is process-global in ort. A failed attempt to load an existing invalid library freezes that selected path; retrying with a different library requires restarting the process. Current CLI invocations are separate processes. Runtime selection/recovery must be addressed before the desktop lifecycle is implemented.

## Observed model contract

The [StemSplitio six-stem ONNX candidate](https://huggingface.co/StemSplitio/htdemucs-6s-onnx/tree/49df9b6989cf2150840ea65b0bef77a2e471b678) is pinned to revision `49df9b6989cf2150840ea65b0bef77a2e471b678`, file `htdemucs_6s.onnx`. Observed input `mix`: float32 `[1, 2, 343980]`, planar stereo at 44.1 kHz. Serialized output dimensions are symbolic; ONNX Runtime resolves `stems` to `[1, 6, 2, 343980]`. The adapter verifies resolved metadata and actual outputs, finite samples, and order drums, bass, other, vocals, guitar, piano. Graph producer metadata: PyTorch 2.4.1.

At the chunk boundary, PCM enters the graph directly without additional outer normalization. [Original HTDemucs](https://github.com/facebookresearch/demucs/blob/v4.0.1/demucs/htdemucs.py) handles STFT, reflect padding, waveform/spectral normalization, and reversal internally; equivalent operations are in the graph. Original-checkpoint references use CPU, eval mode, and `sin_random_shift=0` to match exported deterministic positional embeddings. STFT and attention methods are not patched in the reference.

The odd fixture contains 110,251 music frames zero-padded on the right to 343,980. Tests compare the entire padded output. They do not establish original-length trimming, resampling, mono conversion, or overlap reconstruction (Task 3). Upstream `infer.py` uses quarter-chunk overlap with a zero first window coefficient, which loses the first sample under its weight division; do not adopt that wrapper without boundary tests and an explicit reconstruction decision.

The card declares MIT. Original Demucs v4.0.1 MIT license is retained in `models/htdemucs-6s/LICENSE.demucs`. A complete exported-package manifest and attribution notices remain required before redistribution. No distribution has been published.

Export tooling inspected: [StemSplit/demucs-onnx](https://github.com/StemSplit/demucs-onnx/tree/85db5c80aba33f0f2bdf88034a4be6539feec85b), revision `85db5c80aba33f0f2bdf88034a4be6539feec85b`. Original checkpoint: [5c90dfd2-34c22ccb.th](https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/5c90dfd2-34c22ccb.th), acquired by Demucs 4.0.1. We did not independently reproduce the ONNX export.

## Provenance and SHA-256

| Asset | SHA-256 |
|---|---|
| ONNX model | `48f8e84945579f8ab340e083339e9221e03785dbe733a52c388200b6d3ca779a` |
| libonnxruntime.so.1.22.0 | `3da6146e14e7b8aaec625dde11d6114c7457c87a5f93d744897da8781e35c673` |
| Original checkpoint | `34c22ccb381c6f9fdbf324f04e1e2fe21aaaf293f5ded163a162697ff9a02ddd` |
| Music OGG | `6c23aed3dd5aa57f2b1652ecab68d15d9b82ad257f54e639eb2880ca09bc118a` |
| Music input.f32 | `8a584a8d9f7df6451249dc70bdce7cd490364d665df0dc6b2b8526d7efc5e81e` |
| Music pytorch.f32 | `2e246a1f53e502f52014c014b59761fd594988039e881a13494c4554fe4f107c` |
| Silence input.f32 | `d8afde9e4932cabe1b79aa742534300e65daa1f7431697fe1b3721ab99b853a4` |
| Silence pytorch.f32 | `70d77f125d09d739725f8e50597c6935f607e954c3c99d26bfb7af438055df32` |
| Odd input.f32 | `f762a248173802bdb68f8f957a6fe5e3a4fcf014f9bb75ee10120388ede48b25` |
| Odd pytorch.f32 | `8f1b4dd50d6f941d2f833f45eb7cb7cc0e66436177a290723be9fc33dbb04228` |

Music: Kevin MacLeod, **Vibe Ace**, from [librosa's example archive](https://librosa.org/data/audio/Kevin_MacLeod_-_Vibe_Ace.ogg). [Archive attribution](https://librosa.org/data/audio/Kevin_MacLeod_-_Vibe_Ace.txt) identifies the [Free Music Archive source](https://freemusicarchive.org/music/Kevin_MacLeod/Jazz_Sampler/Vibe_Ace) and [CC-BY 3.0](https://creativecommons.org/licenses/by/3.0/) license. Modification: FFmpeg decode/resample to stereo 44.1 kHz float PCM; first 343,980 frames used, or first 110,251 frames followed by zero padding. This mixed recording has no ground-truth stems, so it cannot establish per-stem SDR.

## Measured CPU parity and resources

Rust ONNX Runtime 1.22.0 versus original PyTorch on the 7.8-second music fixture:

| Stem | Max absolute error | RMSE |
|---|---:|---:|
| drums | 5.08130e-6 | 2.26545e-7 |
| bass | 1.43424e-5 | 9.54169e-7 |
| other | 2.02954e-5 | 1.99473e-6 |
| vocals | 1.02591e-7 | 5.17127e-9 |
| guitar | 1.47505e-7 | 5.94270e-9 |
| piano | 4.07230e-5 | 1.74101e-6 |

Silence: exactly zero error for all sources. Odd padded music: worst source error `1.86451e-5` (bass). All outputs contain six ordered stereo sources, exact frame counts, and finite samples. Raw acceptance logs: `logs/rust-pytorch-{music,silence,odd}.log`.

Development tolerance: `5e-5`, chosen after measuring the worst error `4.07231e-5` and rounding upward. This is a regression threshold for these CPU/model/fixtures, not a universal quality or release threshold. Initial zero-tolerance comparison failed as expected. Python-ONNX comparison with tolerance 1 was a diagnostic measurement, not an acceptance gate. Original-checkpoint acceptance passes separately at `5e-5`.

Release CLI: `3.444810` seconds inference plus writing, `6.648515` seconds wall time including model initialization, Linux peak RSS `4,403,616 KiB` (about 4.20 GiB) measured through child-process rusage. One development-machine observation, not minimum RAM or guaranteed throughput. Across 4,127,760 samples: max error `4.0723011e-5`, RMSE `1.1526702e-6`. See `logs/release-cpu-music.log`.

## Resume procedure

1. README contains native `infer-chunk` and mandatory real-asset test commands. Ordinary tests explicitly ignore the asset-dependent test; invoking it without assets fails rather than skips.
2. `tools/reference_onnx.py` produces inputs and diagnostic graph outputs. `tools/reference_pytorch.py` generates original-checkpoint reference outputs. Use `TORCH_HOME="$PWD/validation-assets/torch"` to keep reference acquisition local. Reference tools reject existing output tensors; they are development-only.
3. Original and six unnormalized float WAV previews are in `validation-assets/music/listening/`. Listening assessment has not been performed.
4. Obtain licensed multitrack fixtures with known stems and record quality metrics/listening. Complete development package attribution and the package-based CLI, then proceed to dependent tasks when the approved feasibility gate is satisfied.
5. Full-song reconstruction, Windows verification, and clean-machine offline packaging remain separate mandatory checks.

## Evidence completed in this workspace

The ordinary contracts/comparator/CLI suite passes 11 integration tests, with one real-asset test ignored. That asset test was run separately and passed on music, silence, and odd padded music against original PyTorch outputs. Offline release build, formatting, and strict Clippy pass. These prove real chunk inference and current regression behavior; they do not prove musical quality, full-song alignment, or platform release readiness. Execution details are in the plan ledger.

## Subsequent full-song / desktop milestone

Weight-license correction discovered during GPU/model research: the original author's [clarification](https://github.com/facebookresearch/demucs/issues/327) says pretrained weights are not covered by the MIT code license and are provided only for scientific purposes. Exporter MIT tags and the included code notices are not evidence of unrestricted original weight rights. Prior attribution records describe exporter declarations; this correction takes precedence for distribution and use of the checkpoints.

The earlier pending-status sections describe the chunk-validation checkpoint. The user subsequently approved proceeding after listening. Manifest validation, attribution, full-song streaming reconstruction, desktop mixing/export, and the Linux development package are implemented. The independent whole-song graph reference matches all six Rust reconstructed outputs exactly. See [current full-song/desktop evidence and remaining release checks](desktop-full-song.md). Ground-truth multitrack metrics and Windows verification remain pending.
