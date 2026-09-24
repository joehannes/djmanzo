#!/usr/bin/env python3
"""Write the stand-in separation models dj-stems tests against.

Each has the interface of an HT-Demucs ONNX export (StemSplit's
demucs-onnx: input `mix` [1, 2, N], output `stems` [1, S, 2, N], stems in
the order drums, bass, other, vocals[, guitar, piano]) and none of its
weights: every stem is the mix times a fixed gain, so what djmanzo makes of
the output can be checked sample by sample. A few hundred bytes each.

Needs the `onnx` Python package (Apache-2.0), which is not shipped and not
needed to build or test djmanzo; the files it writes are committed.

    python3 scripts/stems-standin-models.py crates/dj-stems/tests/fixtures
"""
import sys
from pathlib import Path

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

SEGMENT = 1024


def model(path, gains, *, input_name="mix", output_name="stems", length=SEGMENT, rank_ok=True):
    n = length if length is not None else "n"
    mix = helper.make_tensor_value_info(input_name, TensorProto.FLOAT, [1, 2, n])
    nodes = []
    inits = []
    if rank_ok:
        s = len(gains)
        out = helper.make_tensor_value_info(output_name, TensorProto.FLOAT, [1, s, 2, n])
        inits.append(numpy_helper.from_array(np.array([1], dtype=np.int64), "axis"))
        inits.append(
            numpy_helper.from_array(
                np.array(gains, dtype=np.float32).reshape(1, s, 1, 1), "gains"
            )
        )
        nodes.append(helper.make_node("Unsqueeze", [input_name, "axis"], ["wide"]))
        nodes.append(helper.make_node("Mul", ["wide", "gains"], [output_name]))
    else:
        out = helper.make_tensor_value_info(output_name, TensorProto.FLOAT, [1, 2, n])
        nodes.append(helper.make_node("Identity", [input_name], [output_name]))
    graph = helper.make_graph(nodes, "standin", [mix], [out], inits)
    m = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17)])
    m.ir_version = 8
    onnx.checker.check_model(m)
    onnx.save(m, path)


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
    out.mkdir(parents=True, exist_ok=True)
    model(out / "standin-4.onnx", [0.1, 0.2, 0.3, 0.4])
    model(out / "standin-6.onnx", [0.1, 0.2, 0.3, 0.25, 0.1, 0.05])
    model(
        out / "standin-dynamic.onnx",
        [0.1, 0.2, 0.3, 0.4],
        input_name="input",
        output_name="output",
        length=None,
    )
    model(out / "standin-wrong.onnx", [1.0], rank_ok=False)


if __name__ == "__main__":
    main()
