"""Prepare inputs for the parity test: the old typed ClearBox model vs the new extension tree, per file.

    python tools/prepare_parity.py <files.txt> <out-dir> --old-src <old python/src> --new-src <new python/src>

For every file that parses in BOTH core versions it writes <out>/<n>/old.json (typed clearbox per train,
including correction grids) and <out>/<n>/new.json (the ClearBox extension tree, contract JSON).
Run with the Node suite: MCF_PARITY_DIR=<out> npm test -- parity
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

CHILD = r"""
import json, sys
from pathlib import Path
from machine_config import MachineConfigReader
mode, src, out = sys.argv[1], sys.argv[2], sys.argv[3]
r = MachineConfigReader(src)
if mode == "old":
    d = json.loads(r.to_json(indent=0, include_binary=True))
    trains = [t.get("optional_components", {}).get("clearbox") for t in d["optical_trains"]]
    Path(out).write_text(json.dumps({"trains": trains}), encoding="utf-8")
else:
    from machine_config.extension_contract import tree_to_json
    cfg = r.parse()
    tree = cfg.extensions.get("ClearBox")
    Path(out).write_text(json.dumps(None if tree is None else tree_to_json(tree)), encoding="utf-8")
"""


def run(mode: str, pysrc: str, f: str, out: Path) -> str | None:
    env = dict(os.environ, PYTHONPATH=pysrc, PYTHONUTF8="1")
    r = subprocess.run([sys.executable, "-c", CHILD, mode, f, str(out)], capture_output=True, text=True, env=env, encoding="utf-8")
    return None if r.returncode == 0 else (r.stderr.strip().splitlines() or ["?"])[-1]


def main() -> None:
    args = sys.argv[1:]
    old_src = args[args.index("--old-src") + 1]
    new_src = args[args.index("--new-src") + 1]
    files = [l.strip() for l in Path(args[0]).read_text(encoding="utf-8-sig").splitlines() if l.strip()]
    out = Path(args[1])
    kept = skipped = 0
    for i, f in enumerate(files):
        d = out / f"{i:03d}"
        d.mkdir(parents=True, exist_ok=True)
        e_old = run("old", old_src, f, d / "old.json")
        e_new = run("new", new_src, f, d / "new.json")
        if e_old or e_new:
            skipped += 1
            for p in (d / "old.json", d / "new.json"):
                p.unlink(missing_ok=True)
            d.rmdir()
            continue
        (d / "name.txt").write_text(Path(f).name, encoding="utf-8")
        kept += 1
    print(f"kept {kept} files, skipped {skipped} (failed to parse in old or new core)")


if __name__ == "__main__":
    main()
