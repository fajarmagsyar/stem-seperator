"""Development-only CPU reference tensors; does not prove PyTorch export parity."""

import argparse
import time
from pathlib import Path

import numpy as np
import onnxruntime as ort
import soundfile as sf


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--audio", type=Path)
    parser.add_argument("--frames", type=int, default=343980)
    args = parser.parse_args()
    if not 1 <= args.frames <= 343980:
        parser.error("frames must be between 1 and 343980")
    audio = np.zeros((2, 343980), dtype=np.float32)
    if args.audio:
        decoded, rate = sf.read(args.audio, dtype="float32", always_2d=True)
        if rate != 44100 or decoded.shape[1] not in (1, 2):
            parser.error("audio must be mono or stereo at 44100 Hz")
        count = min(args.frames, len(decoded))
        audio[:, :count] = decoded[:count].T
    options = ort.SessionOptions()
    options.intra_op_num_threads = 2
    options.inter_op_num_threads = 1
    session = ort.InferenceSession(str(args.model), sess_options=options,
                                  providers=["CPUExecutionProvider"])
    print(f"runtime={ort.__version__} providers={session.get_providers()}", flush=True)
    started = time.perf_counter()
    stems = session.run(["stems"], {"mix": audio[None]})[0]
    print(f"inference_seconds={time.perf_counter() - started:.6f}", flush=True)
    if stems.shape != (1, 6, 2, 343980) or not np.isfinite(stems).all():
        raise ValueError("invalid model output")
    args.output.mkdir(parents=True, exist_ok=True)
    # Exclusive writes preserve previous validation evidence.
    with (args.output / "input.f32").open("xb") as target:
        target.write(audio.astype("<f4").tobytes())
    with (args.output / "reference.f32").open("xb") as target:
        target.write(stems.astype("<f4").tobytes())
    print(f"shape={stems.shape} finite=True", flush=True)


if __name__ == "__main__":
    main()
