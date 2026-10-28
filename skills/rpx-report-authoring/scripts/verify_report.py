#!/usr/bin/env python3
"""单命令验证一份 rpx 报表定义：静态 lint + 真跑 + 产物断言。

为什么需要它：`rpx template validate` 的 checked 只有 6 项，deferred 明确包含
data_loading/calculation/pagination/export。也就是说它只保证"结构对"，
不保证"算得出、打得对"。本脚本把"人看一眼就发现"的问题变成可断言的检查。

用法：
    python3 verify_report.py TEMPLATE.json --data-dir DIR --expect-rows 8

退出码：0 = 通过（可有 warning）；1 = 有 error；2 = 用法/环境错误。
"""
import argparse
import csv
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

VAR_RE = re.compile(r"\$\{[^}]+\}")


def a1(row, col):
    letters = ""
    n = int(col or 0)
    while n > 0:
        n, rem = divmod(n - 1, 26)
        letters = chr(65 + rem) + letters
    return f"{letters}{row}"


def issue(code, level, message, hint=""):
    return {"code": code, "level": level, "message": message, "hint": hint}


def load_template(path):
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


def cell_map(tpl):
    return {(c.get("row"), c.get("col")): c for c in tpl.get("cells") or []}


def resolve_dataset_path(tpl, base):
    files = []
    for ds in tpl.get("datasets") or []:
        f = ds.get("file") or {}
        name = f.get("file_name")
        if name:
            files.append((ds.get("name", "?"), os.path.join(base, name), f))
    return files


def rpx_lint(rpx, tpl_path, base):
    """静态检查交给 `rpx lint`——**单一实现**，不在 Python 里养第二套规则。

    旧版二进制没有 lint 子命令时会退化成一条 warning（R000），而不是静默跳过：
    "检查没跑"和"检查通过"必须能区分。
    """
    args = ["lint", tpl_path]
    if base:
        args += ["--data-dir", base]
    _, payload = run_rpx(rpx, args)
    if not payload.get("ok"):
        err = payload.get("error") or {}
        return [issue(
            "R000", "warning",
            f"rpx lint 不可用：{err.get('code')} {str(err.get('message'))[:80]}",
            "二进制版本过旧；在源码仓库里跑 `make install-local` 后再试")]
    return payload["data"]["issues"]


def run_rpx(rpx, args):
    proc = subprocess.run([rpx] + args, capture_output=True, text=True)
    try:
        payload = json.loads(proc.stdout)
    except json.JSONDecodeError:
        payload = {"ok": False, "error": {"code": "NO_JSON",
                                          "message": (proc.stderr or proc.stdout)[:300]}}
    return proc.returncode, payload


def count_csv_rows(path):
    with open(path, encoding="utf-8", newline="") as handle:
        return sum(1 for _ in csv.reader(handle))


def csv_contains(path, needle):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return needle in handle.read()


def xlsx_border_count(path):
    try:
        with zipfile.ZipFile(path) as zf:
            xml = zf.read("xl/styles.xml").decode("utf-8", "replace")
        return xml.count("<border>")
    except (KeyError, zipfile.BadZipFile):
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("template")
    ap.add_argument("--data-dir")
    ap.add_argument("--expect-rows", type=int, help="CSV 总行数（含表头/合计/空白行）")
    ap.add_argument("--max-pages", type=int)
    ap.add_argument("--expect-cell", action="append", default=[], metavar="REF=VALUE",
                    help="断言某个结果格的值，可重复：--expect-cell C4=12000")
    ap.add_argument("--rpx")
    ap.add_argument("--print", action="store_true", help="额外验证 print-xlsx 导出")
    ap.add_argument("--keep", action="store_true", help="保留运行目录以便排查")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    rpx = args.rpx or shutil.which("rpx")
    if not rpx:
        print(json.dumps({"ok": False, "error": "找不到 rpx，用 --rpx 指定"}, ensure_ascii=False))
        return 2

    tpl_path = os.path.abspath(args.template)
    tpl = load_template(tpl_path)
    base = os.path.abspath(args.data_dir) if args.data_dir else os.path.dirname(tpl_path)

    issues = rpx_lint(rpx, tpl_path, base)

    work = tempfile.mkdtemp(prefix="rpx-verify-")
    out_dir = os.path.join(work, "run")
    _, vjson = run_rpx(rpx, ["template", "validate", tpl_path])
    if not vjson.get("ok"):
        err = vjson.get("error") or {}
        issues.append(issue("R001", "error", f"validate 失败：{err.get('code')} {err.get('message')}",
                            "按 error.json_pointer 定位字段；先修 schema 再谈结果"))

    exports = ["--export", "csv", "--export", "xlsx"]
    if args.print:
        exports += ["--export", "print-xlsx"]
    run_args = ["run", tpl_path, "--out-dir", out_dir] + exports
    if base:
        # 必须透传：数据放子目录（如 ./data/）时，run 默认只在模板同级找文件，会误报 DATA_MISSING
        run_args += ["--data-dir", base]
    rc, rjson = run_rpx(rpx, run_args)
    if not rjson.get("ok"):
        err = rjson.get("error") or {}
        issues.append(issue("R002", "error", f"run 失败：{err.get('code')} {err.get('message')}",
                            "看 run.json 的 diagnostics；数据集类错误先跑 dataset inspect"))

    art_dir = os.path.join(out_dir, "artifacts")
    artifacts = sorted(os.listdir(art_dir)) if os.path.isdir(art_dir) else []
    if rjson.get("ok"):
        for need in ("result.json", "workbook.json", "report.csv"):
            if need not in artifacts:
                issues.append(issue("R003", "error", f"缺少产物 {need}", "检查导出阶段 diagnostics"))

    summary = {"artifacts": artifacts, "csv_rows": None, "pages": None, "borders": None}

    csv_path = os.path.join(art_dir, "report.csv")
    if os.path.exists(csv_path):
        rows = count_csv_rows(csv_path)
        summary["csv_rows"] = rows
        if args.expect_rows is not None and rows != args.expect_rows:
            issues.append(issue(
                "R004", "error",
                f"CSV 行数 {rows}，与预期 {args.expect_rows} 不符",
                "行数不符通常意味着扩展块被空行/多扩展列影响；用 result inspect --range 逐行核对"))
        if csv_contains(csv_path, "${"):
            issues.append(issue("R005", "error", "产物里出现字面量 ${，宏没有生效",
                                "宏只能写在 expr，不能写在 text"))

    if args.expect_cell and os.path.exists(csv_path):
        with open(csv_path, encoding="utf-8", newline="") as handle:
            grid = list(csv.reader(handle))
        for spec in args.expect_cell:
            ref, _, want = spec.partition("=")
            m = re.match(r"([A-Za-z]+)([0-9]+)", ref.strip())
            if not m:
                issues.append(issue("R008", "error", f"断言引用不合法：{ref}", "格式形如 C4=12000"))
                continue
            col = 0
            for ch in m.group(1).upper():
                col = col * 26 + (ord(ch) - 64)
            row = int(m.group(2))
            got = grid[row - 1][col - 1] if row - 1 < len(grid) and col - 1 < len(grid[row - 1]) else None
            if got is None or got.strip() != want.strip():
                issues.append(issue(
                    "R008", "error",
                    f"{ref} 期望 {want!r}，实际 {got!r}",
                    "值不符通常是作用域写错（left_head 指错父格）或 select 返回了集合而非单值"))

    wb_path = os.path.join(art_dir, "workbook.json")
    if os.path.exists(wb_path):
        with open(wb_path, encoding="utf-8") as handle:
            wb = json.load(handle)
        p = wb.get("pagination") or {}
        summary["pages"] = {"page_count": p.get("page_count"), "x_pages": p.get("x_pages"),
                            "y_pages": p.get("y_pages"), "columns": p.get("columns")}
        if args.max_pages is not None and (p.get("page_count") or 0) > args.max_pages:
            issues.append(issue(
                "R006", "error",
                f"分页 {p.get('page_count')} 页，超过上限 {args.max_pages}",
                "缩列宽/减行高/收紧打印边距，或接受横向分页并明确告知客户"))

    xlsx_path = os.path.join(art_dir, "report.xlsx")
    if os.path.exists(xlsx_path):
        summary["borders"] = xlsx_border_count(xlsx_path)
        declared = any("border" in c for c in (tpl.get("cells") or []))
        if declared and summary["borders"] == 1:
            issues.append(issue(
                "R007", "warning",
                "模板声明了边框，但 xlsx 里只有默认边框定义（写等于没写）",
                "同上 S004：改用底色分层，或确认版本支持"))

    if not args.keep:
        shutil.rmtree(work, ignore_errors=True)
    else:
        summary["out_dir"] = out_dir

    errors = [i for i in issues if i["level"] == "error"]
    result = {"ok": not errors, "template": tpl_path, "issues": issues, "summary": summary}

    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=1))
    else:
        mark = "PASS" if not errors else "FAIL"
        print(f"[{mark}] {tpl_path}")
        print(f"  产物 {len(summary['artifacts'])} 个 | CSV {summary['csv_rows']} 行 | "
              f"分页 {summary['pages']} | 边框定义 {summary['borders']}")
        for i in issues:
            print(f"  {i['level'].upper():7} {i['code']} {i['message']}")
            if i["hint"]:
                print(f"          → {i['hint']}")
        if not issues:
            print("  无告警")
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
