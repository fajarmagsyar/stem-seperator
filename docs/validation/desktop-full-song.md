# Full-song and desktop validation — Linux, 2026-10-12

The user approved continuing after listening to the candidate model and selected a dark studio workspace with amber controls. Antislop applies during implementation. Visual choices: dark neutral working surface for long mixing sessions; amber identifies the primary processing action and active/focused controls; labeled mixer rows match declared model sources. ENERGY 1 / RHYTHM 1 / MOTION 1. No decorative animation or marketing sections.

## Full-song CPU evidence

Fixture: Kevin MacLeod, **Vibe Ace**, CC BY 3.0, attribution/provenance in [CPU baseline](cpu-baseline.md). Stereo 44.1 kHz, 2,710,336 decoded frames, 61.458866 seconds. The native CLI separates all 11 overlapping fixed model chunks, including the last zero-padded chunk, and exports six float32 WAVs with the exact original rate, channel count, and frame count.

Output: `validation-assets/full-song/stems-1791772193923978541-3-0/`. [Native processing log](logs/full-song-native.log).

An independent Python ONNX Runtime 1.22.0 reference reconstructs the whole song into full-length arrays using positive triangular overlap weights. The Rust implementation uses sliding accumulators and streamed disk caches. Every resulting sample agrees exactly for all six sources (maximum absolute error and RMSE both 0). This covers the first sample, all joins, and the final partial chunk. [Reference log](logs/full-song-reference.log), [reference checker](../../tools/check_full_song.py). This proves reconstruction against the same graph; original-checkpoint parity is separately measured in the CPU baseline. It does not measure separation quality against true multitrack stems.

## Desktop evidence

Native eframe/egui window runs through Xvfb with software OpenGL. The real model processes a stereo 48 kHz FLAC fixture, reaches Ready with six mixer rows, and closes cleanly. Actual screenshots were inspected at 1000×1040 and 640×600; the smaller window uses vertical scrolling and path fields clip within their assigned width.

- [Ready, 1000 pixels wide](screenshots/ready-1000.png)
- [Ready, minimum size](screenshots/ready-640.png)
- [Idle, minimum size](screenshots/idle-640.png)
- [Native run log](logs/native-gui.log)

A separate real-model egui accessibility/control harness processes MP3, checks busy-state gating, clicks Mute Vocals and Solo Piano, includes the audition mix, exports WAVs, and verifies the mix/raw files and cache removal. Anonymous mixer checkboxes now expose source-specific accessibility labels. An additional run forces an unavailable ALSA device and checks export remains available. [No-device UI run](logs/desktop-no-audio.log).

Ordinary tests exercise WAV/FLAC/MP3 decoding, malformed/truncated input, mono/stereo layouts, resampler delay, exact frame restoration, first/last/join impulses, job cancellation/retry, partial-cache cleanup, failure recovery, float WAV output amplitude, filename collisions, non-overwriting output, mixer solo/mute logic, seek/EOF, and 44.1→48 kHz preview duration. [Workspace test log](logs/workspace-tests.log), [strict Clippy](logs/clippy.log).

The fresh reviewer found two Important issues: abandoned/obsolete job caches were retained until exit, and raw source ID `mix` collided with mixed export. Both were reproduced with failing regression tests and fixed. Finished jobs retain only caches referenced by the current result; mixed export uses `_audition-mix.wav`, which cannot equal a valid portable source filename.

## Offline Linux distribution

`dist/stem-separator-linux-x86_64/start.sh` launches the release desktop executable beside the verified package and ONNX Runtime 1.22.0 shared libraries. Model/runtime licenses and notices are included. All files are listed in `SHA256SUMS`; verification passes. The packaged CLI passes original-checkpoint comparison at max-absolute-error tolerance 5e-5 and separates the MP3 fixture when launched from `/tmp`, using only packaged model/runtime assets. No Python is required by either application binary. [Package build](logs/package-linux.log), [model validation](logs/package-model-validation.log), [packaged CLI smoke](logs/package-cli-smoke.log).

## Remaining platform/release checks

This is a usable Linux development build, not a cross-platform release certification. Windows compilation, DLL packaging execution, and native Windows behavior remain unverified; `tools/package-windows.ps1` supplies the platform packaging workflow. Native file chooser interaction on an actual desktop portal, audible hardware audition, device removal/reconnection, screen-reader traversal, and a clean-machine/network-disabled distribution test still need manual platform validation. Ground-truth multitrack metrics remain pending. The synthetic tones test mechanics only. No claim of Windows readiness, musical-quality metrics, or physical listening validation is made.

## Launcher session recovery

A user run exposed missing display environment variables despite an active local Wayland session. The original launcher reproduced Winit's startup error. Restoring the existing session variables kept the app running; the patched packaged launcher now discovers a single owned Wayland socket and session bus when variables are absent. Explicit connection settings remain unchanged. Four regression tests pass, including clear errors for no socket or ambiguous sessions. The patched launcher was tested on the actual Wayland session without manual variables; it remained running until the five-second smoke timeout with no startup error.
