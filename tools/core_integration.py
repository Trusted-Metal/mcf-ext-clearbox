"""Dev-only check that core accepts what `pack` produces: write a packed tree through core, read it back.

    python tools/core_integration.py <files.txt> <parity-dir> <core python/src>

For each real file, core writes (in the file's own File_Version) the config once with the original
ClearBox tree and once with the tree `pack(unpack(tree))` produced by the Node codec
(<parity-dir>/<n>/packed.json, see tools/unpack_all.mjs), re-reads both and compares canonical digests.
"""
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

list_file, parity, core_src = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
sys.path.insert(0, core_src)

from machine_config import MachineConfigReader, MachineConfigWriter  # noqa: E402
from machine_config.extension_contract import canonical_digest, tree_from_json  # noqa: E402

paths = {Path(l.strip()).name: l.strip() for l in list_file.read_text(encoding="utf-8-sig").splitlines() if l.strip()}


def strip_empty(tree):
    """Drop empty-string attributes: present-but-empty and absent mean the same to every reader."""
    tree.attrs = {k: a for k, a in tree.attrs.items() if not (a.dtype == "utf8" and a.value == "")}
    for c in tree.children.values():
        strip_empty(c)
    return tree


def roundtrip(cfg, tree, tmp: Path):
    cfg.extensions["ClearBox"] = tree
    out = tmp / "rt.h5"
    MachineConfigWriter(cfg).write(out)
    return canonical_digest(strip_empty(MachineConfigReader(out).parse().extensions["ClearBox"]))


bad = ok = 0
with tempfile.TemporaryDirectory() as t:
    tmp = Path(t)
    for d in sorted(parity.iterdir()):
        packed = d / "packed.json"
        if not packed.exists():
            continue
        name = (d / "name.txt").read_text(encoding="utf-8")
        src = paths[name]
        cfg = MachineConfigReader(src).parse()
        original = cfg.extensions["ClearBox"]
        new_tree = tree_from_json(json.loads(packed.read_text(encoding="utf-8")))
        try:
            rt_original = roundtrip(MachineConfigReader(src).parse(), original, tmp)
            rt_packed = roundtrip(MachineConfigReader(src).parse(), new_tree, tmp)
        except Exception as e:  # noqa: BLE001
            bad += 1
            print(f"FAIL {name}: core rejected the packed tree: {e}")
            continue
        if rt_original != rt_packed:
            bad += 1
            print(f"DIFF {name} (v{cfg.meta.file_version}): core round trip differs between the original and the packed tree")
        else:
            ok += 1
print(f"{ok} ok, {bad} problems")
