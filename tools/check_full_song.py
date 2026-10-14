"""Independent, development-only full-song overlap reconstruction check."""
import argparse
import json
from pathlib import Path
import numpy as np
import onnxruntime as ort
import soundfile as sf

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("package", type=Path)
parser.add_argument("audio", type=Path)
parser.add_argument("export", type=Path)
args = parser.parse_args()
manifest = json.loads((args.package / "manifest.json").read_text())
audio, rate = sf.read(args.audio, dtype="float32", always_2d=True)
assert rate == manifest["sample_rate"] and audio.shape[1] == 2
length = manifest["chunk_frames"]
overlap = manifest["overlap_frames"]
stride = length - overlap
sources = manifest["sources"]
sum_audio = np.zeros((len(sources), 2, len(audio)), dtype=np.float32)
weights = np.zeros(len(audio), dtype=np.float32)
index = np.arange(length)
window = np.minimum(np.minimum(index + 1, length - index), overlap + 1).astype(np.float32) / np.float32(overlap + 1)
options = ort.SessionOptions()
options.intra_op_num_threads = 2
options.inter_op_num_threads = 1
session = ort.InferenceSession(str(args.package / manifest["model"]["path"]), sess_options=options, providers=["CPUExecutionProvider"])
print(f"runtime={ort.__version__} frames={len(audio)}", flush=True)
for start in range(0, len(audio), stride):
    count = min(length, len(audio) - start)
    chunk = np.zeros((1, 2, length), dtype=np.float32)
    chunk[0, :, :count] = audio[start:start + count].T
    stems = session.run([manifest["output"]], {manifest["input"]: chunk})[0][0]
    assert stems.shape == (len(sources), 2, length) and np.isfinite(stems).all()
    sum_audio[:, :, start:start + count] += stems[:, :, :count] * window[:count]
    weights[start:start + count] += window[:count]
    print(f"chunk_start={start} valid={count}", flush=True)
assert (weights > 0).all()
sum_audio /= weights
for index, source in enumerate(sources):
    actual, actual_rate = sf.read(args.export / (source["id"] + ".wav"), dtype="float32", always_2d=True)
    assert actual_rate == rate and actual.shape == audio.shape and np.isfinite(actual).all()
    delta = actual.T.astype(np.float64) - sum_audio[index].astype(np.float64)
    maximum = np.abs(delta).max()
    print(f"{source['id']}: max_abs={maximum:.9g} rmse={np.sqrt(np.mean(delta * delta)):.9g}", flush=True)
    assert maximum <= 0.00005, "full-song reconstruction differs from independent reference"
print("PASS: exact dimensions, finite samples, all boundaries covered, max_abs<=5e-5")
