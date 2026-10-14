"""Generate development reference tensors from the original Demucs CPU checkpoint."""

import argparse
import time
from pathlib import Path

import numpy as np
import torch
from demucs.pretrained import get_model


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    audio = np.fromfile(args.fixture, dtype="<f4")
    if audio.size != 687960 or not np.isfinite(audio).all():
        parser.error("fixture must contain 687960 finite float32 samples")
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    bag = get_model("htdemucs_6s")
    if len(bag.models) != 1:
        raise ValueError("expected one checkpoint")
    model = bag.models[0].eval().cpu()
    if model.sources != ["drums", "bass", "other", "vocals", "guitar", "piano"]:
        raise ValueError("unexpected checkpoint sources")
    # Match the exported graph's deterministic positional embedding.
    # No STFT, attention, or inference code is replaced.
    for module in model.modules():
        if hasattr(module, "sin_random_shift"):
            module.sin_random_shift = 0
    started = time.perf_counter()
    with torch.inference_mode():
        stems = model(torch.from_numpy(audio.reshape(1, 2, 343980))).numpy()
    if stems.shape != (1, 6, 2, 343980) or not np.isfinite(stems).all():
        raise ValueError("invalid reference output")
    with args.output.open("xb") as target:
        target.write(stems.astype("<f4").tobytes())
    print(f"torch={torch.__version__} checkpoint=htdemucs_6s sources={model.sources}")
    print(f"inference_seconds={time.perf_counter() - started:.6f} shape={stems.shape}")


if __name__ == "__main__":
    main()
