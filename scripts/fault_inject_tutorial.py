#!/usr/bin/env python3
"""教程五层的行为故障注入；在隔离副本中逐条绿→断言红→恢复绿。

运行 python3 scripts/fault_inject_tutorial.py；日志写 output/tutorial/faults/。
只改产品实现，编译失败、零测试或未命中指定失败用例均不算有效注入。
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
CASES = [
    ("heads", "crates/rpx-runtime/src/reportdef/cellprops.rs",
     'cell.left_head = Some(super::heads::parse_declared_head(as_str(h, &p)?, &p)?);',
     'let _ = super::heads::parse_declared_head(as_str(h, &p)?, &p)?; cell.left_head = None;',
     "tutorial_formulas", "explicit_right_hand_head_drives_ordinal_and_copies"),
    ("coordinates", "crates/rpx-engine/src/expr/coord.rs",
     '.filter(|&c| under(ctx, c, lnode, false) && under(ctx, c, tnode, true))',
     '.filter(|&c| under(ctx, c, lnode, false) || under(ctx, c, tnode, true))',
     "tutorial_formulas", "identical_cellsets_follow_education_department_and_report_scopes"),
    ("macros", "crates/rpx-engine/src/macros.rs",
     'out.push_str(&value);', 'out.push_str(&format!("({value})+1"));',
     "reportdef_macros", "own_macro_definitions_survive_saved_rpx_and_engine_entry"),
    ("paging", "crates/rpx-engine/src/pager/configured.rs",
     'count + block.len() > ps.row_num_per_page as usize',
     'count + block.len() < ps.row_num_per_page as usize',
     "tutorial_print_layout", "columns_flow_down_then_across_and_share_the_same_export_layout"),
    ("graphics", "crates/rpx-engine/src/graphics/chart.rs",
     'series.push(Series { name, values });',
     'let mut values = values; values.reverse(); series.push(Series { name, values });',
     "tutorial_graphics", "charts_follow_groups_and_bind_hidden_cells_after_rpx_roundtrip"),
]


def digest(base: Path) -> dict[str, str]:
    return {rel: hashlib.sha256((base / rel).read_bytes()).hexdigest()
            for _, rel, *_ in CASES}


def main() -> None:
    for name, rel, old, _, _, _ in CASES:
        count = (ROOT / rel).read_text().count(old)
        if count != 1:
            raise RuntimeError(f"{name}: 锚点命中 {count} 处，应为 1")
    if "--check-anchors" in sys.argv:
        print("5/5 anchors OK")
        return
    logs = ROOT / "output/tutorial/faults"
    logs.mkdir(parents=True, exist_ok=True)
    originals = digest(ROOT)
    result = []
    with tempfile.TemporaryDirectory(prefix="rpx-tutorial-faults-") as folder:
        workspace = Path(folder) / "workspace"
        workspace.mkdir()
        for name in ("crates", "tests", "examples"):
            shutil.copytree(ROOT / name, workspace / name,
                            ignore=shutil.ignore_patterns("target", ".DS_Store"))
        for name in ("Cargo.toml", "Cargo.lock", "rustfmt.toml", ".rustfmt.toml"):
            if (ROOT / name).exists():
                shutil.copy2(ROOT / name, workspace / name)
        env = {**os.environ, "CARGO_TARGET_DIR": str(Path(folder) / "target")}

        def run(name: str, phase: str, test: str, case: str) -> tuple[int, str]:
            command = ["cargo", "test", "--offline", "-p", "rpx-runtime", "--test", test, case, "--", "--exact"]
            proc = subprocess.run(command, cwd=workspace, env=env,
                                  capture_output=True, text=True, timeout=600)
            output = proc.stdout + proc.stderr
            (logs / f"{name}-{phase}.log").write_text(output)
            if "running 1 test" not in output or re.search(r"error\[E\d+\]|could not compile", output):
                raise RuntimeError(f"{name}/{phase}: 未运行指定测试或编译失败，见日志")
            return proc.returncode, output

        for name, rel, old, new, test, case in CASES:
            path = workspace / rel
            saved = path.read_bytes()
            code, _ = run(name, "baseline", test, case)
            if code:
                raise RuntimeError(f"{name}: 基线不绿")
            print(f"{name}: baseline GREEN", flush=True)
            try:
                path.write_bytes(saved.replace(old.encode(), new.encode(), 1))
                code, output = run(name, "mutated", test, case)
                if code == 0 or f"test {case} ... FAILED" not in output or "panicked at" not in output:
                    raise RuntimeError(f"{name}: 变异未被运行时断言捕获")
            finally:
                path.write_bytes(saved)
            code, _ = run(name, "restored", test, case)
            if code or digest(workspace) != originals:
                raise RuntimeError(f"{name}: 恢复后不绿或文件指纹改变")
            result.append({"layer": name, "test": case, "baseline": "green", "mutated": "assertion_red", "restored": "green"})
            print(f"{name}: assertion RED → restored GREEN", flush=True)
    if digest(ROOT) != originals:
        raise RuntimeError("原工作区文件指纹改变")
    (logs / "results.json").write_text(json.dumps({"cases": result, "original_sha256": originals}, ensure_ascii=False, indent=2) + "\n")
    print("5/5 behavioral mutations caught; original workspace unchanged")


if __name__ == "__main__":
    main()
