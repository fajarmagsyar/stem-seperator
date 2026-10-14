"""Compare pinned four-specialist ONNX graphs with original CPU checkpoints."""
import argparse
import json
from pathlib import Path
import time
import numpy as np
import onnxruntime as ort
import torch
from demucs.pretrained import get_model

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--package', type=Path, required=True)
parser.add_argument('--fixtures', type=Path, nargs='+', required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
torch.set_num_threads(2)
torch.set_num_interop_threads(1)
# The development environment caches these original checkpoints locally.
bag = get_model('htdemucs_ft')
metrics = []
for index, source in enumerate(['drums', 'bass', 'other', 'vocals']):
    model = bag.models[index].eval().cpu()
    for module in model.modules():
        if hasattr(module, 'sin_random_shift'):
            module.sin_random_shift = 0
    options = ort.SessionOptions()
    options.intra_op_num_threads = 2
    options.inter_op_num_threads = 1
    session = ort.InferenceSession(str(args.package / f'htdemucs_ft_{source}.onnx'), sess_options=options, providers=['CPUExecutionProvider'])
    for fixture in args.fixtures:
        data = np.fromfile(fixture, dtype='<f4').reshape(1, 2, 343980)
        if not np.isfinite(data).all():
            raise ValueError('nonfinite input')
        started = time.perf_counter()
        with torch.inference_mode():
            reference = model(torch.from_numpy(data.copy())).numpy()
        actual = session.run(['stems'], {'mix': data})[0]
        if actual.shape != (1, 4, 2, 343980) or not np.isfinite(actual).all():
            raise ValueError('invalid native graph output')
        delta = actual.astype(np.float64) - reference.astype(np.float64)
        row = {'source': source, 'fixture': str(fixture), 'max_abs': float(np.max(np.abs(delta))), 'rmse': float(np.sqrt(np.mean(delta * delta))), 'seconds': time.perf_counter() - started}
        metrics.append(row)
        print(json.dumps(row), flush=True)
    del session
(args.output / 'metrics.json').write_text(json.dumps(metrics, indent=2) + '\n')
