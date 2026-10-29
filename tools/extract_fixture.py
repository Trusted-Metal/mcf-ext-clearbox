"""Dump the `ClearBox` extension tree of a machine-config file as contract JSON (tree.json).

Reads with the Machine Config Library (Python) and writes what core hands to extension codecs.
Real grids are 257x257x2 (about 130k numbers each), so by default each grid is replaced by a small
deterministic stand-in (3x3x2, with NaN) to keep fixtures reviewable; pass --real-grids to keep them.

    python tools/extract_fixture.py <file.h5> <out-dir> [--real-grids]
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

from machine_config import MachineConfigReader
from machine_config.extension_contract import Attr, Dataset, tree_to_json

SMALL_SHAPE = [3, 3, 2]


def stand_in(seed: int) -> list:
    n = SMALL_SHAPE[0] * SMALL_SHAPE[1] * SMALL_SHAPE[2]
    return [math.nan if (k + seed) % 7 == 0 else round(k * 0.5 - 3 + seed * 0.125, 3) for k in range(n)]


def shrink(tree) -> None:
    for i, (name, child) in enumerate(sorted(tree.children.items())):
        for ds_name, ds in list(child.datasets.items()):
            seed = i * 2 + (1 if ds_name.startswith("Inverse") else 0)
            attrs = dict(ds.attrs)
            if "shape" in attrs:
                attrs["shape"] = Attr("utf8", "x".join(map(str, SMALL_SHAPE)))
            child.datasets[ds_name] = Dataset("f64", list(SMALL_SHAPE), stand_in(seed), attrs)


def main() -> None:
    src, out = Path(sys.argv[1]), Path(sys.argv[2])
    real = "--real-grids" in sys.argv
    cfg = MachineConfigReader(src).parse()
    tree = cfg.extensions.get("ClearBox")
    if tree is None:
        sys.exit(f"{src.name} has no ClearBox extension")
    if not real:
        shrink(tree)
    out.mkdir(parents=True, exist_ok=True)
    (out / "tree.json").write_text(json.dumps(tree_to_json(tree), indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {out / 'tree.json'} from {src.name} (file_version {cfg.meta.file_version})")


if __name__ == "__main__":
    main()
