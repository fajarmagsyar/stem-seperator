# Offline stem separator

Rust desktop app and CLI for local CPU audio separation. The included HTDemucs package produces drums, bass, other, vocals, guitar, and piano. Processing, preview, mixing, and export run offline; Python is used only for development reference checks.

## Run the desktop app

The Linux distribution is in `dist/stem-separator-linux-x86_64`. From this project directory:

```sh
bash dist/stem-separator-linux-x86_64/start.sh
```

If the shell has lost its display environment, the Linux launcher discovers a single Wayland socket owned by your user and restores the session bus connection. Explicit display settings are preserved. With no discoverable session, launch from a desktop terminal or set the session's `WAYLAND_DISPLAY` / `DISPLAY`; multiple Wayland sessions require an explicit selection.

Choose a WAV, FLAC, or MP3 file, then select **Separate song**. After processing, audition the stems with mute, solo, and gain controls, or listen to the original. Space toggles playback when a text field is not active. **Export selected WAVs** writes a fresh folder of aligned 32-bit float WAVs at the input sample rate, channel count, and exact decoded frame count. Individual stems retain their original amplitude. **Include audition mix** additionally writes `_audition-mix.wav` using the current mixer settings.

Mono and stereo inputs are supported. Processing uses disk caches and fixed overlapping model chunks; it does not hold the full song in memory. Cancel remains responsive while inference finishes its current chunk. Closing during processing cancels, waits for that operation, and removes owned caches. Failed/cancelled jobs are cleaned before retry. An unavailable audio device leaves separation and export available.

The model currently needs approximately 4.2 GiB peak RAM on the validated Linux CPU setup. Leave several GiB of free disk space for long-song caches and exports. WAV files larger than 4 GiB are rejected. This is a Linux development distribution; Windows builds and hardware listening/device-disconnection checks remain unverified.

## Build and package

Rust 1.88 or newer, Linux ALSA/X11/Wayland development libraries, and provisioned local model/runtime assets are required. Dependencies are pinned in `Cargo.lock` and cached in this workspace.

```sh
export CARGO_HOME="$PWD/.cargo-cache"
cargo test --locked --offline --workspace
cargo run --locked --offline -p stem-desktop
bash tools/package-linux.sh
```

The packager refuses an existing destination. To rebuild into a new directory, pass its path as the first argument. It includes the model manifest, model/license assets, ONNX Runtime shared libraries and notices, launch script, and `SHA256SUMS`. Launch from any working directory. `STEM_RUNTIME` overrides the runtime library; `STEM_CACHE_DIR` overrides desktop cache storage. If an invalid runtime library has been attempted, restart before changing it: the upstream runtime loader selects its path once per process.

## CLI

```sh
dist/stem-separator-linux-x86_64/stem-cli separate \
  --package models/htdemucs-6s \
  --runtime runtime/onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so \
  --input validation-assets/vibe-ace.wav \
  --output exports
```

The CLI prints progress and the completed output folder. Exit codes: 0 success, 1 processing failure, 2 invalid arguments. Existing exports are never overwritten.

Model packages contain `manifest.json`, SHA-256 pinned model/license assets, provenance, and ordered source IDs/labels. Validation rejects unsupported contracts, duplicate IDs, corrupt assets, absolute/traversal paths, and escaping symlinks. Supported graph profile: `htdemucs-onnx-v1` / `graph-internal-v1`, 44.1 kHz stereo, 343,980 frames, four or six declared sources. Different architectures require a new adapter; replacing weights alone does not make an incompatible graph supported.

License correction: Demucs' MIT code license does not grant unrestricted use of its pretrained weights. The [original author states the weights are provided only for scientific purposes](https://github.com/facebookresearch/demucs/issues/327), despite some ONNX conversion cards labeling them MIT. Existing model attribution files record code/exporter notices, not a separate grant of weight rights. Suitable weight permissions must be established before a general-purpose music-production release.

The development commands `infer-chunk`, `compare-reference`, and `validate-model` work on planar little-endian float32 tensors. For package-based numerical validation:

```sh
target/release/stem-cli validate-model \
  --package models/htdemucs-6s \
  --runtime runtime/onnxruntime-linux-x64-1.22.0/lib/libonnxruntime.so \
  --fixture validation-assets/music/input.f32 \
  --reference validation-assets/music/pytorch.f32 \
  --max-abs 0.00005
```

## Validation

Real Rust CPU inference matches the original PyTorch checkpoint on music, silence, and padded short audio at a measured maximum absolute error below `0.00005`. The full 61.459-second licensed music fixture produces six 2,710,336-frame stereo 44.1 kHz float WAVs. Tests cover decoding, resampling delay, overlap boundaries, cancellation, cache ownership, non-overwriting export, mixer/transport logic, device-rate conversion, and desktop control gating. Separate real-asset UI and native window checks exercise processing and mixing/export.

Ground-truth multitrack separation metrics remain pending. Numerical parity does not establish musical quality. The user approved proceeding after listening to the candidate model. The current native desktop screenshots and validation limitations are recorded in [desktop/full-song validation](docs/validation/desktop-full-song.md); model hashes, provenance, attribution, and reference measurements are in [CPU validation](docs/validation/cpu-baseline.md).

The ordinary test suite skips tests that require large local model assets. Run the explicit real desktop control check with:

```sh
cargo test --offline -p stem-desktop --test desktop_controls real_song -- --ignored
cargo build --offline -p stem-desktop --example smoke
bash tools/native-smoke.sh
```

The native smoke requires a desktop display or Xvfb (`STEM_XVFB` can set its path). It captures actual window screenshots and checks a real model reaches Ready.
