#!/usr/bin/env python3
"""M2-11 分页与 M2-22 属性计算故障注入 —— 证明分页/属性判据**有牙齿**。

判据：按错误类各注入一处**最小改动**，在临时仓库副本中跑 `cargo test -p rpx-engine`，期望**至少一条**用例变红
（或按期望保持绿）。编译失败算「注入不合格」，**不进战果**。

用法::

    python3 scripts/fault_inject_pager.py --check-anchors   # 秒级：只验锚点恰好 1 处
    python3 scripts/fault_inject_pager.py                   # 跑全轮

⚠ 与 `fault-injection-verify` 的纪律对齐：
  * 所有变异只作用于临时仓库副本，真实工作树字节不改；
  * 基线必须先绿，且每条注入还原后都重跑绿；
  * 锚点必须**恰好匹配 1 处**（失配 = 注入不合格，不静默跳过）；
  * 编译失败（`could not compile` / `error[E..]`）**不算抓住**；
  * 每条注入都要能说出「红在哪条用例」，故 `want` 是必须出现的失败用例名片段；
  * 注入前后各取一次被测文件的**字节指纹**，不一致 ⇒ 报「结论无效」；
  * 必须含**期望绿**的对照组（等价改写 + 已知覆盖缺口），否则「全红」证明不了方向。
"""

from __future__ import annotations

import hashlib
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

PAGER = "crates/rpx-engine/src/pager.rs"
RUN = "crates/rpx-engine/src/calc/run.rs"
PAGINATION = "crates/rpx-engine/src/emit/pagination.rs"
COMPARE = "crates/rpx-engine/tests/engine_grid_compare.rs"
COMPARE_SECTIONS = "crates/rpx-engine/tests/common/compare/sections.rs"
STAGE_B = "crates/rpx-engine/src/stage_b.rs"
STAGE_B_TESTS = "crates/rpx-engine/src/stage_b_tests.rs"
PROPERTIES = "crates/rpx-engine/src/calc/properties.rs"
SAME = "crates/rpx-engine/src/merge/same.rs"
NULL = "crates/rpx-engine/src/merge/null.rs"
MATRIX = "crates/rpx-engine/src/merge/matrix.rs"

# 目标文件（用于指纹校验）
TARGETS = [PAGER, RUN, PAGINATION, COMPARE, COMPARE_SECTIONS, STAGE_B, STAGE_B_TESTS, PROPERTIES, SAME, NULL, MATRIX]

# (名字, 错误类, 文件, old, new, 期望, 必须红在这条用例名里的片段)
INJECTIONS = [
    # ── A. 算术：三条必须照抄的式子 ────────────────────────────────────
    (
        "tolerance_dropped",
        "算术/千分位容差",
        PAGER,
        "const TOLERANCE: f64 = 1000.0;",
        "const TOLERANCE: f64 = 1.0;",
        "red",
        "break_test_carries_the_thousandth_tolerance",
    ),
    (
        "paper_two_roundings_collapsed",
        "算术/两次舍入",
        PAGER,
        "    let mm = pt * 25.4_f32 / 72.0_f32;\n    f64::from(mm) * 72.0 / 25.4",
        "    let mm = pt * 25.4_f32 / 72.0_f32;\n    f64::from(mm)",
        "red",
        "paper_size_round_trip",
    ),
    (
        "unit_mm_treated_as_inch",
        "算术/单位分支",
        PAGER,
        "        1 => d * 72.0 / 25.4,",
        "        1 => d * 72.0,",
        "red",
        "unit_transfer_matches_java",
    ),
    (
        "unit_inch_treated_as_mm",
        "算术/单位分支",
        PAGER,
        "        2 => d * 72.0,",
        "        2 => d * 72.0 / 25.4,",
        "red",
        "unit_transfer_matches_java",
    ),
    (
        "unit_default_scaled",
        "算术/单位分支",
        PAGER,
        "        2 => d * 72.0,\n        1 => d * 72.0 / 25.4,\n        _ => d,",
        "        2 => d * 72.0,\n        1 => d * 72.0 / 25.4,\n        _ => d * 72.0,",
        "red",
        "unit_transfer_matches_java",
    ),
    # ── B. 断页语义 ──────────────────────────────────────────────────
    (
        "never_break_rows",
        "语义/断页永真",
        PAGER,
        "    cost > ((limit - used) * TOLERANCE).ceil() / TOLERANCE",
        "    let _ = (cost, used, limit);\n    false",
        "red",
        "pager_shaped_grid_splits_into_14_14_14_4",
    ),
    (
        "always_break_rows",
        "语义/断页永假",
        PAGER,
        "    cost > ((limit - used) * TOLERANCE).ceil() / TOLERANCE",
        "    let _ = (cost, used, limit);\n    true",
        "red",
        "pager_shaped_grid_splits_into_14_14_14_4",
    ),
    (
        "group_copy_does_not_consume_height",
        "语义/分组头吃高",
        PAGER,
        "            used += height_of(g);",
        "            used += 0.0;",
        "red",
        "the_group_header_copy_consumes_height",
    ),
    # ── C. 页的组成（区带 / 分组头副本）──────────────────────────────
    (
        "prefix_bands_not_copied",
        "结构/页首区带复制",
        PAGER,
        "        for &r in &prefix {\n"
        "            page.push(PageRow {\n"
        "                row: r,\n"
        "                kind: PageRowKind::Header,\n"
        "            });\n"
        "        }\n",
        "",
        "red",
        "page_prefix_bands_are_copied_onto_every_page",
    ),
    (
        "prefix_order_reversed",
        "结构/区带顺序",
        PAGER,
        "const PAGE_PREFIX_TYPES: [i8; 3] = [-93, -96, -95];",
        "const PAGE_PREFIX_TYPES: [i8; 3] = [-95, -96, -93];",
        "red",
        "page_prefix_bands_are_copied_onto_every_page",
    ),
    (
        "leading_run_collects_all_not_first",
        "结构/区带只取连续段",
        PAGER,
        "            end = row;\n        } else if start.is_some() {\n            break;\n        }",
        "            end = row;\n        }",
        "red",
        "band_lookup_takes_contiguous_runs_only",
    ),
    # ── D. 横断 / 首列 ───────────────────────────────────────────────
    (
        "column_split_disabled",
        "语义/横断",
        PAGER,
        "        if c > start && !fits(cost, used, page_w) {",
        "        if c > start && false {",
        "red",
        "nine_25mm_columns_split_into_8_plus_1",
    ),
    (
        # ⚠ 这条在修对拍器之前**是绿的**（单列页上注入为空操作 + 对拍器逐页比较恒真）。
        # 修好后必须红，且**两道门禁都红**：单元层（新加的 `a_second_column_page_*`）
        # 与夹具层（`engine_sum` 第 2 页）。
        "col1_values_from_grid_column_1",
        "语义/首列跟随本页",
        PAGER,
        "    for pr in page_rows {\n        if let Some(cell) = grid.get(pr.row, base_col) {",
        "    for pr in page_rows {\n        if let Some(cell) = grid.get(pr.row, 1) {",
        "red",
        "a_second_column_page_reads_its_own_first_column",
    ),
    # ── E. 导出（键序 / 记法 / 两个渲染器）──────────────────────────
    (
        "pagination_none_written_as_empty_object",
        "导出/空值口径",
        PAGINATION,
        '        None => "null".to_string(),',
        '        None => "{}".to_string(),',
        "red",
        "render_document_concatenates_all_three_sections",
    ),
    (
        "first_value_uses_value_renderer",
        "导出/两个渲染器",
        PAGINATION,
        '        f.push(("first_value", json::quote(&val::java_tostring(first))));',
        '        f.push(("first_value", json::quote(&val::render_value(first))));',
        "red",
        "render_pagination_pins_key_order_float_notation_and_optional_keys",
    ),
    (
        "paper_keys_swapped",
        "导出/键序",
        PAGINATION,
        '        ("paper_w", num::java_float_to_string(p.paper_w)),\n'
        '        ("paper_h", num::java_float_to_string(p.paper_h)),',
        '        ("paper_w", num::java_float_to_string(p.paper_h)),\n'
        '        ("paper_h", num::java_float_to_string(p.paper_w)),',
        "red",
        "render_pagination_pins_key_order_float_notation_and_optional_keys",
    ),
    (
        "paper_w_written_as_int",
        "导出/float 记法",
        PAGINATION,
        '        ("paper_w", num::java_float_to_string(p.paper_w)),',
        '        ("paper_w", json::int(i64::from(p.paper_w as i32))),',
        "red",
        "render_pagination_pins_key_order_float_notation_and_optional_keys",
    ),
    (
        "columns_reported_as_x_pages",
        "导出/接线",
        PAGINATION,
        '        ("columns", json::int(i64::from(p.columns))),',
        '        ("columns", json::int(i64::from(p.x_pages))),',
        "red",
        "engine_output_matches_every_fixture_cell_by_cell",
    ),
    # ── F. 接线 / 属性表达式 ─────────────────────────────────────────
    (
        "pagination_not_wired_into_grid",
        "接线/漏拷字段",
        RUN,
        "    out.pagination = Some(pg);",
        "    out.pagination = None;",
        "red",
        "engine_output_matches_every_fixture_cell_by_cell",
    ),
    (
        "row_height_expression_ignored",
        "属性/expMap 键 1 解析",
        STAGE_B,
        "matches!(key, 1 | 2 | 7 | 13 | 15)",
        "matches!(key, 2 | 7 | 13 | 15)",
        "red",
        "row_height_property_expression_is_parsed_from_row_exp_map",
    ),
    (
        # 这一条专打**对拍器里那个刚被修好的静默恒真洞**：合并格的 `row_merge` 与
        # `col_merge` 互换。修洞之前 `push_if_diff` 拿标签当 JSON 键去 `get` ⇒
        # 两侧都取不到 ⇒ 恒真；修好之后必须红。
        "merged_row_col_swap",
        "对拍器/逐格合并比较是否活着",
        RUN,
        "                h.value.clone(),\n                h.row_merge,\n                h.col_merge,",
        "                h.value.clone(),\n                h.col_merge,\n                h.row_merge,",
        "red",
        "engine_output_matches_every_fixture_cell_by_cell",
    ),
    (
        "computed_row_height_not_read_from_runtime",
        "属性/覆盖值读取",
        PROPERTIES,
        "match property_value(ctx, owner, key)? {\n        Value::Float(value) => Some(*value),\n        _ => None,\n    }",
        "match property_value(ctx, owner, key)? {\n        Value::Float(_) => None,\n        _ => None,\n    }",
        "red",
        "row_height_property_is_read_as_a_float_overlay",
    ),
    # ── G. 合并合成矩阵：四趟同值合并 + 四趟空格合并 ─────────────────
    (
        "same_p1_forward_skipped",
        "矩阵/mergeSame P1 正向纵趟",
        SAME,
        "            if s.value_null\n"
        "                || s.row != i3\n"
        "                || s.col != j3\n"
        "                || (s.merge_same != SAME_V && s.merge_same != SAME_VH)\n"
        "                || s.merge_same_mode == MODE_REVERSE\n"
        "            {",
        "            if true {",
        "red",
        "same_p1_merges_equal_values_downward",
    ),
    (
        "same_p2_reverse_skipped",
        "矩阵/mergeSame P2 反向纵趟",
        SAME,
        "            if s.value_null\n"
        "                || s.row != i3\n"
        "                || s.col != j3\n"
        "                || (s.merge_same != SAME_V && s.merge_same != SAME_VH)\n"
        "                || s.merge_same_mode != MODE_REVERSE\n"
        "            {",
        "            if true {",
        "red",
        "same_p2_reverse_mode_merges_upward_from_the_bottom_anchor",
    ),
    (
        "same_p3_horizontal_skipped",
        "矩阵/mergeSame P3 正向横趟",
        SAME,
        "            if max2 >= col_size {",
        "            if true {",
        "red",
        "same_p3_merges_equal_values_to_the_right_when_p1_left_max2_below_boundary",
    ),
    (
        "same_p4_reverse_skipped",
        "矩阵/mergeSame P4 反向横趟",
        SAME,
        "            if s.value_null\n"
        "                || s.row != i\n"
        "                || s.col != j\n"
        "                || (s.merge_same != SAME_H && s.merge_same != SAME_VH)\n"
        "                || s.merge_same_mode != MODE_REVERSE\n"
        "            {",
        "            if true {",
        "red",
        "same_p4_reverse_mode_merges_leftward_and_moves_the_anchor",
    ),
    (
        "null_p1_up_flag_wrong",
        "矩阵/mergeNull P1 UP",
        NULL,
        "                    || os.merge_null != NULL_UP\n",
        "                    || os.merge_null != NULL_DOWN\n",
        "red",
        "null_p1_absorbs_an_up_marked_blank_downward",
    ),
    (
        "null_p2_down_flag_wrong",
        "矩阵/mergeNull P2 DOWN",
        NULL,
        "                    || os.merge_null != NULL_DOWN\n",
        "                    || os.merge_null != NULL_UP\n",
        "red",
        "null_p2_absorbs_a_down_marked_blank_upward_and_reanchors",
    ),
    (
        "null_p3_left_flag_wrong",
        "矩阵/mergeNull P3 LEFT",
        NULL,
        "                    || os.merge_null != NULL_LEFT\n",
        "                    || os.merge_null != NULL_RIGHT\n",
        "red",
        "null_p3_absorbs_a_left_marked_blank_to_the_right",
    ),
    (
        "null_p4_right_flag_wrong",
        "矩阵/mergeNull P4 RIGHT",
        NULL,
        "                    || os.merge_null != NULL_RIGHT\n",
        "                    || os.merge_null != NULL_LEFT\n",
        "red",
        "null_p4_absorbs_a_right_marked_blank_to_the_left_and_reanchors",
    ),
    # ── H. 期望绿：对照组 + 已知覆盖缺口 ─────────────────────────────
    (
        "fits_rewritten_equivalently",
        "对照/等价改写（期望绿）",
        PAGER,
        "    !breaks(cost, used, limit)\n}",
        "    cost <= ((limit - used) * TOLERANCE).ceil() / TOLERANCE\n}",
        "green",
        None,
    ),
    (
        "footer_height_ignored",
        "覆盖缺口/页脚未建模（期望绿）",
        PAGER,
        "        let mut used = prefix_h + footer_h;",
        "        let mut used = prefix_h;",
        "green",
        None,
    ),
]

FAIL_RE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.M)
COMPILE_RE = re.compile(r"^error\[E\d+\]", re.M)


def is_compile_break(out: str) -> bool:
    """编译不过 ≠ 用例抓住了 —— 必须区分，否则「注入语法错误」会被读成通过。"""
    return "could not compile" in out or COMPILE_RE.search(out) is not None


def fingerprint(paths: list[str], base: Path = ROOT) -> str:
    h = hashlib.sha256()
    for p in paths:
        h.update((base / p).read_bytes())
    return h.hexdigest()


def run_tests(cwd: Path) -> tuple[int, str]:
    # 隔离构建缓存：临时副本的 `cargo` 绝不允许写进真实工作树的
    # `~/.cargo/target`。否则临时目录的 CARGO_MANIFEST_DIR 会被烤进测试二进制，
    # 真实仓库里 `cargo test` 会因夹具路径校验失败而报 "No such file or
    # directory"（假红）。cwd 即 .../workspace，缓存放在临时目录里随它一起删除。
    env = {**os.environ, "CARGO_TARGET_DIR": str(cwd.parent / "target")}
    proc = subprocess.run(
        ["cargo", "test", "-p", "rpx-engine", "--offline"],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
    )
    return proc.returncode, proc.stdout + proc.stderr


def check_anchors() -> int:
    bad = 0
    for name, cls, rel, old, new, _expect, _want in INJECTIONS:
        text = (ROOT / rel).read_text(encoding="utf-8")
        n = text.count(old)
        flag = "OK " if n == 1 and old != new else "BAD"
        if flag == "BAD":
            bad += 1
        print(f"[{flag}] {name:<42} {rel}  命中 {n} 处  old!=new={old != new}")
        if n != 1:
            print(f"      ⚠ 锚点原文（repr）：{old!r}")
    print(f"\n锚点检查：{len(INJECTIONS) - bad}/{len(INJECTIONS)} 合格")
    return 1 if bad else 0


def main() -> int:
    if "--check-anchors" in sys.argv:
        return check_anchors()

    real_before = fingerprint(TARGETS, ROOT)
    results: list[tuple[str, str, str, str, str]] = []
    with tempfile.TemporaryDirectory(prefix="rpx-fault-inject-") as td:
        workspace = Path(td) / "workspace"

        def ignore_generated(directory: str, names: list[str]) -> set[str]:
            here = Path(directory)
            ignore = {name for name in names if name in {".git", ".workbuddy-ai"}}
            # Build artifacts are ignored by the repo, but the test oracle
            # expects the copied source directory beneath cargo's target path.
            if here == ROOT / "crates" / "rpx-engine" and "target" in names:
                ignore.add("target")
            return ignore

        shutil.copytree(ROOT, workspace, ignore=ignore_generated)
        originals = {p: (workspace / p).read_bytes() for p in TARGETS}
        baseline_fingerprint = fingerprint(TARGETS, workspace)
        baseline_code, baseline_out = run_tests(workspace)
        if baseline_code != 0 or FAIL_RE.search(baseline_out):
            print("基线不绿：本轮故障注入无效，先修复/确认测试基线。")
            return 2

        for name, cls, rel, old, new, expect, want in INJECTIONS:
            text = originals[rel].decode("utf-8")
            if text.count(old) != 1 or old == new:
                results.append((name, cls, expect, "锚点失效", ""))
                continue

            target = workspace / rel
            target.write_text(text.replace(old, new, 1), encoding="utf-8")
            code, out = run_tests(workspace)
            failed = sorted(set(FAIL_RE.findall(out)))
            detail = ",".join(failed[:3]) + ("…" if len(failed) > 3 else "")

            if is_compile_break(out):
                verdict = "注入不合格(编译失败)"
            elif code != 0 and not failed:
                verdict = "未跑成(无用例失败)"
            elif expect == "red":
                if not failed:
                    verdict = "没抓到(全绿)"
                elif want and not any(want in f for f in failed):
                    verdict = f"红错地方(缺 {want})"
                else:
                    verdict = "抓到"
            else:
                verdict = "保持绿" if not failed and code == 0 else "意外变红"

            results.append((name, cls, expect, verdict, detail))

            # 每条注入后还原临时副本，并重跑绿基线；真实工作树从未被写入。
            for path, content in originals.items():
                (workspace / path).write_bytes(content)
            restored_code, restored_out = run_tests(workspace)
            restored_failed = FAIL_RE.findall(restored_out)
            if restored_code != 0 or restored_failed:
                results[-1] = (name, cls, expect, "还原后基线未绿", detail)
                break
            if fingerprint(TARGETS, workspace) != baseline_fingerprint:
                results[-1] = (name, cls, expect, "临时副本指纹未还原", detail)
                break

    real_after = fingerprint(TARGETS, ROOT)
    if real_before != real_after:
        print("⚠⚠ 真实工作树指纹变化 ⇒ 本轮结论无效")
        return 2

    print(f"{'注入':<42} {'错误类':<28} {'期望':<6} {'结果':<22} 红在哪条")
    print("-" * 130)
    hit = miss = bad = 0
    for name, cls, expect, verdict, detail in results:
        print(f"{name:<42} {cls:<28} {expect:<6} {verdict:<22} {detail}")
        if verdict in ("抓到", "保持绿"):
            hit += 1
        elif verdict in ("锚点失效",) or verdict.startswith(("注入不合格", "还原后", "临时副本", "未跑成")):
            bad += 1
        else:
            miss += 1

    print("-" * 130)
    print(f"合格 {hit} / 漏网 {miss} / 注入不合格 {bad} / 共 {len(results)}")
    print(f"真实工作树指纹一致：{real_before == real_after}")
    return 0 if miss == 0 and bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
