#!/usr/bin/env python3
"""独立复核 fixtures 地面真值加载器（crates/rpx-engine/tests/common）。

为什么需要这份脚本
------------------
加载器把「产品产物里的真实数字」**写死**进了 `fixture_loader.rs` 的 `SHAPE`
常量（这是刻意设计，见该文件 1-6 行注释：数字一动就必须响亮失败）。但"写死"
同时也意味着「地基读错 → 上面全是幻觉」。本脚本用**第二套独立实现**（Python，
与 Rust 零共享代码）重新从 .expected.json 推导这些数字，逐条比对。

与 `_shape_audit.py` / `_source_audit.py` 的分工（避免重复劳动的误会）
--------------------------------------------------------------------
那两个是 **打印型勘察工具**：只 dump 形状、混淆矩阵，给人看，无判据、无退出码。
本脚本是 **断言型门禁**：逐条 OK/FAIL、退出码 0/1，可直接进 CI 或复现链。
两者互补，不是同一件事的两份拷贝。

它做四件事
----------
  1. 盘上 `engine_*.expected.json` 全集  vs  `loader.rs::all_names()`
     —— 防"产品侧新增夹具、加载器静默漏测"（all_names 是硬编码列表，不 glob）
  2. 从 JSON 重推 `SHAPE` 表（模板尺寸 / 计算态尺寸 / 非空格数 / source 计数）
     —— 并校验 `SHAPE` 恰好覆盖"全部非错误型夹具"这个集合恒等式
  3. 重推分页逐页、samemerge 合并格、ext/merge 锚点抽查、错误型夹具  vs  写死断言
  4. 用 JSON 里**真实出现的键集合**反向校验 loader.rs 的 expect_keys 白名单
     —— 产品侧一旦新增键，加载器会 panic；这里提前把它们列出来

用法：cd <repo-root> && python tests/fixtures/loader_crosscheck.py
退出码：0 = 全部一致；1 = 存在漂移（逐条打印 OK/FAIL）
"""

import codecs
import glob
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
FIXTURES = os.path.join(ROOT, "tests", "fixtures")
TEST_RS = os.path.join(ROOT, "crates", "rpx-engine", "tests", "fixture_loader.rs")
LOADER_RS = os.path.join(ROOT, "crates", "rpx-engine", "tests", "common", "loader.rs")

FAILS = []

# 已知待同步登记表：夹具**已落盘**但 `all_names()` 还没收录时填这里。
# 登记不是"放行"，是**点名** —— 每项必须写清原因与接手人。三条自清理规则：
#   * 登记项若从未出现在盘上 → 过期登记，FAIL
#   * 登记项若已被 all_names() 收录 → 登记腐烂，FAIL（要求删除）
#   * 盘上有、既未收录又未登记 → 静默漏测，FAIL
PENDING_IN_LOADER = {
    # 当前为空：12 个夹具已全部被 loader 收录。
    # 新增夹具时在此登记 {名字: "原因 + 需同步的清单 + 接手人"}，
    # loader 同步后本脚本会因「已同步残留」失败，逼你回来删条目。
}


def check(cond, label, detail=""):
    if cond:
        print(f"  OK   {label}")
    else:
        print(f"  FAIL {label}{('  ' + detail) if detail else ''}")
        FAILS.append(label)


def body(text, fn):
    """截出 `fn <fn>(` 到下一个顶格 `}` 之间的函数体（测试文件是扁平结构）。"""
    m = re.search(rf"\n(?:async )?fn {re.escape(fn)}\b", text)
    if not m:
        raise SystemExit(f"在 {TEST_RS} 里找不到 fn {fn}")
    end = text.find("\n}\n", m.start())
    return text[m.start(): end if end != -1 else len(text)]


# ---------------------------------------------------------------- 读入
with open(TEST_RS, encoding="utf-8") as f:
    TEST = f.read()
with open(LOADER_RS, encoding="utf-8") as f:
    LOADER = f.read()

disk_names = sorted(
    os.path.basename(p)[: -len(".expected.json")]
    for p in glob.glob(os.path.join(FIXTURES, "engine_*.expected.json"))
)
FX = {}
for n in disk_names:
    with open(os.path.join(FIXTURES, f"{n}.expected.json"), encoding="utf-8") as f:
        FX[n] = json.load(f)

print(f"盘上 engine_* 夹具 {len(disk_names)} 个：{', '.join(disk_names)}")

# ---------------------------------------------------------------- 1. 夹具全集
print("\n[1] all_names() 必须覆盖盘上全部 engine_* 夹具")
m = re.search(r"pub fn all_names\(\) -> \[&'static str; (\d+)\] \{(.*?)\n\}", LOADER, re.S)
check(m is not None, "loader.rs::all_names 可解析")
rust_names = []
if m:
    declared = int(m.group(1))
    rust_names = sorted(re.findall(r'"([a-z_]+)"', m.group(2)))
    check(
        declared == len(rust_names),
        f"all_names 声明的长度 {declared} 与实际列出的 {len(rust_names)} 一致",
    )
    only_disk = sorted(set(disk_names) - set(rust_names))
    only_rust = sorted(set(rust_names) - set(disk_names))
    # 幽灵名（声明了但盘上没有）永远算失败：load() 会 panic
    check(not only_rust, "all_names() 无幽灵名（声明了但盘上不存在）", f"{only_rust}")
    # 盘上有、加载器没收：除非已登记为 pending，否则算静默漏测
    unregistered = [n for n in only_disk if n not in PENDING_IN_LOADER]
    check(not unregistered, "盘上夹具全部被 all_names() 收录（无静默漏测）", f"未收录且未登记：{unregistered}")
    # 三条自清理规则
    check(
        not [n for n in PENDING_IN_LOADER if n not in disk_names],
        "PENDING 无过期登记（登记项必须存在于盘上）",
        f"登记了但盘上不存在：{[n for n in PENDING_IN_LOADER if n not in disk_names]}",
    )
    check(
        not [n for n in PENDING_IN_LOADER if n in set(rust_names)],
        "PENDING 无已同步残留（已收录的必须从登记表删除）",
        f"已收录却仍登记：{[n for n in PENDING_IN_LOADER if n in set(rust_names)]}",
    )
    for n in only_disk:
        if n in PENDING_IN_LOADER:
            print(f"  PENDING {n}：{PENDING_IN_LOADER[n]}")

# ---------------------------------------------------------------- 2. SHAPE 表
print("\n[2] SHAPE 表（模板尺寸 / 计算态尺寸 / 非空格数 / source 计数）")
derived = {n: FX[n] for n in disk_names}
calc = {n: d["calculated"] for n, d in derived.items() if "error" not in d["calculated"]}
tmpl_py = {n: (d["template_rows"], d["template_cols"]) for n, d in derived.items()}

# (夹具名, 模板行, 模板列, 计算行, 计算列, 非空格数, 带 source, 不带 source)
# SHAPE 有两种合法写法：裸元组，或具名别名（clippy::type_complexity 的修法）。都接受。
SHAPE_FORMS = [
    r"const SHAPE: \[\(&str, i32, i32, i32, i32, usize, usize, usize\); (\d+)\] = \[(.*?)\n\];",
    r"const SHAPE: \[ShapeRow; (\d+)\] = \[(.*?)\n\];",
]
ms = None
for pat in SHAPE_FORMS:
    ms = re.search(pat, TEST, re.S)
    if ms:
        break
check(ms is not None, "fixture_loader.rs::SHAPE 可解析（结构变了就要改本脚本）")
# 若用了具名别名，必须钉住字段顺序 —— 顺序一变，本脚本的解析会静默错位
alias = re.search(r"type ShapeRow = \(([^)]*)\);", TEST)
if alias:
    fields = [x.strip() for x in alias.group(1).split(",")]
    want = ["&'static str", "i32", "i32", "i32", "i32", "usize", "usize", "usize"]
    check(fields == want, "ShapeRow 字段顺序与脚本假设一致（防静默错位）", f"{fields}")
shape = {}
if ms:
    shape_declared = int(ms.group(1))
    for nm, tr, tc, cr, cc, ce, ws, ws2 in re.findall(
        r'\("(engine_\w+)",\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+)\)',
        ms.group(2),
    ):
        shape[nm] = tuple(int(x) for x in (tr, tc, cr, cc, ce, ws, ws2))
    check(
        shape_declared == len(shape),
        f"SHAPE 声明长度 {shape_declared} 与实际条目 {len(shape)} 一致",
    )
    # 集合恒等式：SHAPE 恰好覆盖「全部非错误型夹具」。
    # 已登记 PENDING 的夹具允许暂缺（[1] 会点名），未登记的缺一律失败 ——
    # 这样脚本在"等 owner 同步"期间保持可绿，而真正的新夹具仍会把它打红。
    pending_calcs = set(PENDING_IN_LOADER) & set(calc)
    missing = (set(shape) ^ set(calc)) - pending_calcs
    check(
        not missing,
        "SHAPE 恰好覆盖全部非错误型夹具（错误型不入表；PENDING 登记的允许暂缺）",
        f"差集={sorted(missing)}",
    )

for n in sorted(set(shape) & set(calc)):
    ce = len(calc[n]["cells"])
    ws = sum(1 for c in calc[n]["cells"] if "source" in c)
    py = (tmpl_py[n][0], tmpl_py[n][1], calc[n]["rows"], calc[n]["cols"], ce, ws, ce - ws)
    check(shape[n] == py, f"{n} SHAPE {py}", f"rust={shape[n]}")

# 内部不变量（与 SHAPE 无关，逐夹具独立复算）
for n, c in calc.items():
    ce = len(c["cells"])
    ws = sum(1 for x in c["cells"] if "source" in x)
    ws2 = ce - ws
    check(ws + ws2 == ce, f"{n}: 有src({ws}) + 无src({ws2}) == 非空格({ce})")
    tr, tc = tmpl_py[n]
    bad = [
        (x["r"], x["c"], x["r"] <= tr and x["c"] <= tc, "source" in x)
        for x in c["cells"]
        if (x["r"] <= tr and x["c"] <= tc) != ("source" in x)
    ]
    check(not bad, f"{n}: source 存在 ⟺ 落在模板 {tr}x{tc} 内", f"违约 {bad[:4]}")

# ---------------------------------------------------------------- 3. 分页/合并/锚点
print("\n[3] 分页逐页 / 合并格 / 扩展与合并锚点抽查 / 错误型夹具")
pager = FX["engine_pager"]["pagination"]
p = body(TEST, "pager_pagination_matches_product_output")
for field in ("page_count", "x_pages", "y_pages", "row_num_per_page"):
    mm = re.search(rf"assert_eq!\(p\.{field}, (\d+)\)", p)
    check(
        mm is not None and int(mm.group(1)) == pager[field],
        f"分页 {field} = {pager[field]}",
        f"rust={mm.group(1) if mm else '未找到'}",
    )
rows_py = [pg["rows"] for pg in pager["pages"]]
mm = re.search(r"vec!\[([0-9,\s]+)\]", p)
check(mm is not None, "分页逐页行数断言可解析")
if mm:
    rows_rs = [int(x) for x in re.findall(r"\d+", mm.group(1))]
    check(rows_rs == rows_py, f"分页逐页行数 {rows_py}", f"rust={rows_rs}")
check(
    [pg["page"] for pg in pager["pages"]] == list(range(1, len(pager["pages"]) + 1)),
    "页号从 1 起递增",
)
check(
    pager["pages"][-1]["rows"] > len(pager["pages"][-1]["col1_values"]),
    f"末页 rows({pager['pages'][-1]['rows']}) > 非空值数({len(pager['pages'][-1]['col1_values'])})：空值被跳过",
)
check(
    pager["pages"][0].get("first_value") == "region"
    and pager["pages"][-1].get("last_value") == "R40",
    "首页首值=region、末页末值=R40",
)

sm = FX["engine_samemerge"]["merged"]
check(
    sm["page_count"] == 1 and len(sm["cells"]) == 4,
    f"samemerge 合并格 1 页 4 格（实际 {sm['page_count']}/{len(sm['cells'])}）",
)
check(all(c["c"] == 1 for c in sm["cells"]), "samemerge 合并只作用第 1 列")
check(all(c["row_merge"] == 2 and c["col_merge"] == 1 for c in sm["cells"]), "samemerge row_merge=2 / col_merge=1")
sm_fn = body(TEST, "samemerge_merged_is_exactly_four_row_spans")
mm = re.search(r"vec!\[(.*?)\]", sm_fn, re.S)
check(mm is not None, "samemerge 期望值 vec 可解析")
if mm:
    # Rust 字面量里写的是 "\\\"A\\\""，其字符串值即 `"A"`（含引号），应与 JSON 原文 token 一致
    lits = re.findall(r'"((?:[^"\\]|\\.)*)"', mm.group(1))
    vals_rs = [codecs.decode(x, "unicode_escape") for x in lits]
    vals_py = [json.dumps(c["value"]) for c in sm["cells"]]
    check(vals_rs == vals_py, f"samemerge 值原文（含引号）{vals_py}", f"rust={vals_rs}")

ext = {c["r"] * 100 + c["c"]: c for c in calc["engine_ext"]["cells"]}
for (r, c, mode, declared) in ((2, 1, 17, "list"), (3, 1, 18, None), (4, 1, 19, None)):
    cell = ext.get(r * 100 + c)
    check(cell is not None, f"engine_ext ({r},{c}) 存在")
    if not cell:
        continue
    s = cell["source"]
    check(s["extend"] == mode, f"engine_ext ({r},{c}) extend={mode}", f"json={s['extend']}")
    if declared:
        check(cell["type"] == declared, f"engine_ext ({r},{c}) type={declared}", f"json={cell['type']}")
    if mode == 17:
        check(s["sub_cells"] == 1, f"engine_ext ({r},{c}) 单格承载整列 list")
    if mode == 18:
        check(s["sub_cells"] == 5, f"engine_ext ({r},{c}) 纵向 sub_cells=5", f"json={s['sub_cells']}")
    if mode == 19:
        pos = [(v["r"], v["c"]) for v in s.get("sub_values", [])]
        check(pos == [(8, 1), (8, 2), (8, 3), (8, 4), (8, 5)], f"engine_ext 横向附属格排同一行连续列 {pos}")

mg = {c["r"] * 100 + c["c"]: c for c in calc["engine_merge"]["cells"]}
a = mg.get(201)
check(
    a is not None and a["source"]["row_merge"] == 2 and a["source"]["extend"] == 18,
    "engine_merge (2,1) 跨行合并锚点 row_merge=2 / extend=18",
    f"json={a['source'] if a else None}",
)

cyc = FX["engine_cycle"]
check("error" in cyc["calculated"], "engine_cycle 是错误型夹具")
if "error" in cyc["calculated"]:
    e = cyc["calculated"]["error"]
    check(e["class"] == "com.raqsoft.commonl.ReportError", "engine_cycle error.class", e["class"])
    check(e.get("chain") == "B2-C2-B2-C2", "engine_cycle 循环链", str(e.get("chain")))
    check((e.get("cause_class") or "").endswith("IIllIIIIIIIIIlll"), "engine_cycle cause_class 后缀", str(e.get("cause_class")))
    check("单" in e.get("message", ""), "engine_cycle message 的 \\uXXXX 已解码为中文", e.get("message"))
    check(cyc.get("pagination") is None and cyc.get("merged") is None, "错误型夹具 pagination/merged 为 null")

# ---------------------------------------------------------------- 4. 白名单
print("\n[4] expect_keys 白名单 vs JSON 真实键集合（防产品侧新增键导致加载器 panic）")
allowed = {
    ctx: set(re.findall(r'"([^"]+)"', keys))
    for ctx, keys in re.findall(r'expect_keys\(\s*"([^"]+)",\s*&\[(.*?)\]', LOADER, re.S)
}
print(f"  加载器白名单层 {len(allowed)} 个：{', '.join(sorted(allowed))}")

CTXS = ["fixture(顶层)", "dataset", "formula", "cell", "source", "sub_value",
        "error", "pagination", "page", "merged", "merged_cell"]
observed = {c: set() for c in CTXS}


def obs(ctx, obj):
    observed[ctx] |= set(obj)


for n, d in FX.items():
    obs("fixture(顶层)", d)
    obs("dataset", d["dataset"])
    for f in d["formulas"]:
        obs("formula", f)
    if "error" in d["calculated"]:
        obs("error", d["calculated"]["error"])
    else:
        for c in d["calculated"]["cells"]:
            obs("cell", c)
            if "source" in c:
                obs("source", c["source"])
                for sv in c["source"].get("sub_values", []):
                    obs("sub_value", sv)
    if d.get("pagination"):
        obs("pagination", d["pagination"])
        for pg in d["pagination"]["pages"]:
            obs("page", pg)
    if d.get("merged"):
        obs("merged", d["merged"])
        for mc in d["merged"]["cells"]:
            obs("merged_cell", mc)

for ctx in CTXS:
    if ctx not in allowed:
        check(not observed[ctx], f"{ctx}：加载器未设白名单但 JSON 有键 {sorted(observed[ctx])}")
        continue
    extra = observed[ctx] - allowed[ctx]
    check(not extra, f"{ctx}：JSON 键被白名单覆盖", f"白名单外的键 = {sorted(extra)} → 加载器会 panic")
    unused = allowed[ctx] - observed[ctx]
    if unused:
        print(f"  note {ctx}：白名单里当前夹具未出现的键 {sorted(unused)}（可选键，合法）")

# ---------------------------------------------------------------- 结论
print()
if FAILS:
    print(f"结论：FAIL —— {len(FAILS)} 项漂移")
    for x in FAILS:
        print(f"  - {x}")
    sys.exit(1)
print("结论：PASS —— 加载器写死的数字与产品产物逐条一致，白名单未漏键")
sys.exit(0)
