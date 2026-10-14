# Audio decoder fixtures

`tone.flac` and `tone.mp3` are generated 440 Hz sine tones: 0.25 seconds, 48 kHz, stereo. They contain no sampled recording and are dedicated to the public domain under CC0. They test decoding, resampling, and application wiring, not musical separation quality.

Generated with FFmpeg's `sine=frequency=440:sample_rate=48000:duration=0.25`, `-ac 2`, and the FLAC / libmp3lame encoders. `manifest.json` is the example package manifest; the large model asset is provisioned separately under `models/`.
