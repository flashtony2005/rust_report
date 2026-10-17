#!/usr/bin/env python3
"""Large-template baseline: generate JSON, run rpx, print wall time/size.

Usage:
  python3 scripts/bench_large.py --binary target/debug/rpx --rows 200 --cols 50
"""
import argparse
import json
import os
import subprocess
import tempfile
import time


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--binary", default="target/debug/rpx")
    p.add_argument("--rows", type=int, default=200)
    p.add_argument("--cols", type=int, default=50)
    p.add_argument("--out", default=None)
    args = p.parse_args()

    cells = [
        {"row": r, "col": c, "text": f"{r}:{c}"}
        for r in range(1, args.rows + 1)
        for c in range(1, args.cols + 1)
    ]
    doc = {
        "format": "rpx-report",
        "format_version": 1,
        "schema": "rpx-report/v1",
        "grid": {"rows": args.rows, "cols": args.cols},
        "cells": cells,
    }
    root = tempfile.mkdtemp(prefix="rpx-bench-")
    template = os.path.join(root, "large.json")
    with open(template, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, separators=(",", ":"))
    size = os.path.getsize(template)
    out = args.out or os.path.join(root, "out")
    started = time.perf_counter()
    run = subprocess.run(
        [args.binary, "run", template, "--out-dir", out, "--export", "csv"],
        capture_output=True,
        text=True,
    )
    elapsed = time.perf_counter() - started
    print(
        json.dumps(
            {
                "rows": args.rows,
                "cols": args.cols,
                "cells": len(cells),
                "template_bytes": size,
                "elapsed_seconds": round(elapsed, 4),
                "exit": run.returncode,
                "ok": run.returncode == 0,
            },
            ensure_ascii=False,
        )
    )
    if run.returncode != 0:
        print(run.stdout)
        print(run.stderr)
        return run.returncode
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
