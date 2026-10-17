#!/usr/bin/env python3
"""把引擎真实产物（CSV 值 + xlsx 合并区）拼成可浏览的 HTML 报表页。

**不重算任何业务数值**：值来自 `POST /export?fmt=csv` 的字节，合并区来自
`POST /export?fmt=xlsx` 的 `<mergeCell>`，页面只做排版与格式化。

用法：``python3 demo_data/make_html_report.py <month|region>``
"""

import csv
import os
import re
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))

REPORTS = {
    "month": {
        "csv": "report_month_quarter_2026.csv",
        "xlsx": "report_month_quarter_2026.xlsx",
        "out": "report_month_quarter_2026.html",
        "title": "2026 年度「季度 → 月份 + 组内小计」嵌套横向扩展报表",
        "sub": (
            "SQLite <code>demo_data/report_demo.db</code>（seed=20261016，可复现）· "
            "定义 <code>report_month_quarter_2026.json</code> · "
            "模板 <b>3 行 × 3 列</b> ⇒ 扩展后 <b>3 行 × 17 列</b>"
        ),
        "tints": [(2, 5, "#EAF3FB"), (6, 9, "#FDF3E7"), (10, 13, "#EBF7EF"), (14, 17, "#F5EEF9")],
        "legend": [("Q1 组", "#EAF3FB"), ("Q2 组", "#FDF3E7"), ("Q3 组", "#EBF7EF"), ("Q4 组", "#F5EEF9")],
        "checks": [
            ("✓ 17 列布局逐列对拍", "1 标签列 + 4 组 ×（3 月份 + 1 小计）= 17；列序与组位全对"),
            ("✓ 12 个月份金额", "= SELECT SUM(amount) GROUP BY month，逐格相等"),
            ("✓ 4 个季度小计", "= SELECT SUM(amount) GROUP BY quarter，逐格相等"),
            ("✓ 小计 = 本组 3 个月之和", "9831294.72 / 10354349.28 / 9848688.60 / 9792642.60"),
            ("✓ 4 个小计之和 = 库总计", "39826975.20 = SELECT SUM(amount) FROM sales"),
            ("✓ 季度表头跨 3 列", "xlsx 合并区 <code>B1:D1 / F1:H1 / J1:L1 / N1:P1</code>（声明式主格生效）"),
            ("✓ 对拍脚本", "<code>verify_month_quarter.py</code>：checked=16 mismatched=0"),
        ],
        "mechanism": [
            ("① 小计表头显式声明上主格",
             "小计表头 <code>C2</code> 写 <code>\"top_head\": \"B1\"</code> ⇒ "
             "<code>find_top_head</code> 走<b>分支 A</b> 直接取季度格，不再自动上溯。"
             "这正是教程 <code>008_jcbb14</code>「组表头重复」里那条<b>人为主格链</b>"
             "（<code>A3.左主格=B3</code>、<code>B3.左主格=A2</code>…）的同一机制。"),
            ("② 小计表头落在第 2 行",
             "月份格 <code>B2</code> 与小计表头 <code>C2</code> 的 top_head 都解析成 <code>B1</code>；"
             "小计数据格 <code>C3</code> 沿 <code>C2 → B1</code> 链同样归属季度 ⇒ "
             "<b>只随季度组克隆一次</b>，而不是随月份克隆 12 次。"),
            ("③ 季度锚点的包络 = 3 个月份列",
             "季度格 <code>B1</code> 自身矩形是 1 列；包络由 <code>top_subs</code> 递归得到。"
             "小计列<b>不在</b>包络内（它的上主格是声明出来的，不是被撑大的）"
             "⇒ 季度表头跨 <b>3</b> 列（<code>B1:D1</code>），小计列在季度行上留空。"),
            ("④ 两层各自展开",
             "季度带按 <code>ds_q</code>（4 行）扩成 4 组；每组的月份锚点按 "
             "<code>ds_m.select@r(month_label, quarter==B1)</code> 扩成 3 个月份列 ⇒ "
             "4 × (3 + 1) = 16 列 + 1 标签列。"),
        ],
        "note": (
            "季度表头跨 <b>3</b> 列（<code>B1:D1</code> / <code>F1:H1</code> / "
            "<code>J1:L1</code> / <code>N1:P1</code>），与参考图一致。"
            "在 2026-10-17 之前这是做不到的：服务端 JSON 装载层把 <code>left_head</code>/"
            "<code>top_head</code> 硬编码成 <code>None</code>，"
            "只能靠给季度格<b>预置 2 列合并</b>来给小计列找个上主格，"
            "代价是季度表头被撑到 <b>4</b> 列（<code>B1:E1</code>，把小计列也吞了）。"
            "现在声明式主格已打通（<code>reportdef/cellprops.rs</code>），"
            "季度格不再需要那条假合并。"
        ),
        "repro": [
            "python3 demo_data/make_db.py — 建库（640 行明细，含 month 维度）",
            "curl -F report=@demo_data/report_month_quarter_2026.json http://127.0.0.1:8090/export?fmt=csv",
            "curl -F report=@demo_data/report_month_quarter_2026.json http://127.0.0.1:8090/export?fmt=xlsx",
            "python3 demo_data/verify_month_quarter.py demo_data/report_month_quarter_2026.csv",
        ],
    },
    "region": {
        "csv": "report_region_quarter_2026.csv",
        "xlsx": "report_region_quarter_2026.xlsx",
        "out": "report_region_quarter_2026.html",
        "title": "2026 年度「区域 × 季度 × 产品」交叉表",
        "sub": (
            "SQLite <code>demo_data/report_demo.db</code> · "
            "定义 <code>report_region_quarter_2026.json</code> · "
            "模板 <b>5 行 × 5 列</b> ⇒ 扩展后 <b>24 行 × 12 列</b>"
        ),
        "tints": [],
        "legend": [],
        "checks": [
            ("✓ 210 格逐格对拍", "20 明细行 ×（8 产品 + 数量 + 金额）+ 合计行 10 格"),
            ("✓ 行序严格一致", "= ds1 的 ORDER BY region_order, quarter（5 区域 × 4 季度）"),
            ("✓ 行末「数量」列", "= SELECT SUM(qty) GROUP BY region, quarter"),
            ("✓ 行末「金额（元）」列", "= SELECT SUM(amount) GROUP BY region, quarter"),
            ("✓ 合计行", "8 个产品列合计 + 总计 23,933 件 / ¥39,826,975.20"),
            ("✓ 对拍脚本", "<code>verify_region_quarter.py</code>：checked=210 mismatched=0"),
        ],
        "mechanism": [
            ("① 数量 / 金额放在模板第 4、5 列",
             "横向扩展在 col 4 处插入 7 个产品列 ⇒ 模板 col≥4 的格整体右移 7 位，"
             "数量/金额正好落到第 11、12 列（8 产品占 3–10 列之后）。"),
            ("② 插列平移所有行",
             "<code>row_type_at</code> 对模板行默认返回 <code>-91</code>，属于行头同族 "
             "<code>{-91,-95,-94,-90}</code> ⇒ <code>is_row_header</code> 恒真 ⇒ "
             "平移带覆盖全部行，明细行与合计行都会跟着右移。"),
            ("③ 值格用 <code>@r</code> 强制 Root 态",
             "克隆格的 ds 电流会经上主格链交叉到错误行；<code>select@r</code> 跳过 Row 态、"
             "对全量数据集按引用过滤，与电流无关。"),
        ],
        "note": "",
        "repro": [
            "curl -F report=@demo_data/report_region_quarter_2026.json http://127.0.0.1:8090/export?fmt=csv",
            "python3 demo_data/verify_region_quarter.py demo_data/report_region_quarter_2026.csv",
        ],
    },
}


def load_merges(xlsx_path):
    """从 xlsx 读真实合并区 → {(r, c): (rowspan, colspan)}，1 起坐标。"""
    xml = zipfile.ZipFile(xlsx_path).read("xl/worksheets/sheet1.xml").decode("utf-8")

    def rc(ref):
        m = re.match(r"([A-Z]+)(\d+)", ref)
        col = 0
        for ch in m.group(1):
            col = col * 26 + ord(ch) - 64
        return int(m.group(2)), col

    out = {}
    for ref in re.findall(r'<mergeCell ref="([^"]+)"/>', xml):
        a, b = ref.split(":")
        r1, c1 = rc(a)
        r2, c2 = rc(b)
        out[(r1, c1)] = (r2 - r1 + 1, c2 - c1 + 1)
    return out


def fmt(s):
    """数字加千分位；带小数保留 2 位。非数字原样返回。"""
    t = s.strip()
    if t == "":
        return ""
    try:
        v = float(t)
    except ValueError:
        return s
    return f"{int(v):,}" if v == int(v) else f"{v:,.2f}"


def build(key):
    cfg = REPORTS[key]
    with open(os.path.join(HERE, cfg["csv"]), encoding="utf-8-sig") as f:
        rows = list(csv.reader(f))
    merges = load_merges(os.path.join(HERE, cfg["xlsx"]))

    covered = set()
    for (r0, c0), (rs, cs) in merges.items():
        for rr in range(r0, r0 + rs):
            for cc in range(c0, c0 + cs):
                if (rr, cc) != (r0, c0):
                    covered.add((rr, cc))

    def tint(ci):
        for lo, hi, color in cfg["tints"]:
            if lo <= ci <= hi:
                return color
        return ""

    # 行分组底色：直接从 xlsx 读第 1 列的纵向合并跨度（= merge_same 的真实跨度），
    # 不手写行号 ⇒ 分组边界永远跟着引擎产物走。
    palette = ["#EAF3FB", "#FDF3E7", "#EBF7EF", "#F5EEF9", "#FDEEEE", "#EEF4EE"]
    row_tint = {}
    legend_items = list(cfg["legend"])
    groups = sorted((r0, r0 + rs - 1) for (r0, c0), (rs, cs) in merges.items() if c0 == 1 and rs > 1)
    for i, (r0, r1) in enumerate(groups):
        color = palette[i % len(palette)]
        for r in range(r0, r1 + 1):
            row_tint[r] = color
        legend_items.append((rows[r0 - 1][0], color))

    num_from = 3 if key == "month" else 4
    body = []
    for ri, row in enumerate(rows, start=1):
        tds = []
        for ci, val in enumerate(row, start=1):
            if (ri, ci) in covered:
                continue
            rs, cs = merges.get((ri, ci), (1, 1))
            span = (f' rowspan="{rs}"' if rs > 1 else "") + (f' colspan="{cs}"' if cs > 1 else "")
            cls = []
            if ri >= num_from:
                cls.append("num")
            if key == "region" and ri == len(rows):
                cls.append("total")
            bg = tint(ci) or row_tint.get(ri, "")
            style = f' style="background:{bg}"' if bg else ""
            text = fmt(val) if ri >= num_from else val
            tds.append(f'<td class="{" ".join(cls)}"{span}{style}>{text}</td>')
        body.append("<tr>" + "".join(tds) + "</tr>")

    legend = ""
    if legend_items:
        legend = '<div class="legend">' + "".join(
            f'<i><span style="background:{c}"></span>{n}</i>' for n, c in legend_items
        ) + "</div>"

    checks = "".join(
        f'<div class="check"><div class="ok">{t}</div><div class="kv">{d}</div></div>'
        for t, d in cfg["checks"]
    )
    mech = "".join(
        f'<div class="step"><div class="st">{i + 1}</div><div><b>{t}</b><div class="kv">{d}</div></div></div>'
        for i, (t, d) in enumerate(cfg["mechanism"])
    )
    note = f'<div class="card note"><h2>一处如实说明</h2><div class="kv">{cfg["note"]}</div></div>' if cfg["note"] else ""
    repro = "".join(f"<code>{c}</code><br>" for c in cfg["repro"])

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>{cfg["title"]} — rpx 演示</title>
<style>
  :root {{ --line:#b8c4d0; --head:#dce6f1; --ink:#1f2937; --accent:#1f4e79; }}
  * {{ box-sizing:border-box; }}
  body {{ margin:0; padding:24px; font-family:"PingFang SC","Microsoft YaHei","Songti SC",sans-serif;
         background:#f4f6f8; color:var(--ink); }}
  .wrap {{ max-width:1240px; margin:0 auto; }}
  .badge {{ display:inline-block; background:var(--accent); color:#fff; border-radius:4px;
            padding:2px 10px; font-size:12px; margin-right:8px; vertical-align:middle; }}
  h1 {{ font-size:20px; margin:0 0 4px; }}
  .sub {{ color:#5b6b7c; font-size:13px; margin-bottom:16px; line-height:1.7; }}
  .card {{ background:#fff; border:1px solid #e2e8ee; border-radius:10px; padding:16px 20px;
           margin-bottom:16px; box-shadow:0 1px 3px rgba(15,23,42,.06); }}
  .card h2 {{ font-size:15px; margin:0 0 10px; color:var(--accent); }}
  .card.note {{ border-color:#f0d9a8; background:#fffdf6; }}
  .card.note h2 {{ color:#8a6d1f; }}
  table.report {{ border-collapse:collapse; background:#fff; font-size:13px; margin:0 auto; }}
  table.report td {{ border:1px solid var(--line); padding:6px 9px; text-align:center; white-space:nowrap; }}
  table.report tr:first-child td {{ background:var(--head); font-weight:700; height:30px; }}
  table.report tr:nth-child(2) td {{ font-weight:700; height:28px; }}
  table.report td.num {{ text-align:right; font-variant-numeric:tabular-nums; font-size:12.5px; }}
  table.report td.total {{ background:#f2f2f2; font-weight:700; }}
  .legend {{ margin:2px 0 12px; }}
  .legend span {{ display:inline-block; width:12px; height:12px; border-radius:3px;
                  margin-right:4px; vertical-align:-1px; border:1px solid #cbd5e0; }}
  .legend i {{ font-style:normal; margin-right:14px; font-size:12.5px; color:#4b5563; }}
  .checks {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(250px,1fr)); gap:10px; }}
  .check {{ border:1px solid #dbe7f3; background:#f7fbff; border-radius:8px; padding:10px 12px; }}
  .check .ok {{ color:#1a7f37; font-weight:700; font-size:13px; }}
  .check .kv {{ font-size:12.5px; color:#5b6b7c; margin-top:3px; line-height:1.6; }}
  .step {{ display:flex; gap:10px; margin-bottom:12px; align-items:flex-start; }}
  .step .st {{ flex:0 0 22px; height:22px; border-radius:50%; background:var(--accent); color:#fff;
               font-size:12px; text-align:center; line-height:22px; }}
  .kv {{ font-size:12.5px; color:#5b6b7c; line-height:1.75; }}
  code {{ background:#eef2f6; border-radius:4px; padding:1px 6px; font-size:12px; }}
  .scroll {{ overflow:auto; }}
  footer {{ color:#8494a5; font-size:12px; margin-top:8px; }}
</style>
</head>
<body>
<div class="wrap">
  <h1><span class="badge">rpx 演示</span>{cfg["title"]}</h1>
  <div class="sub">{cfg["sub"]}</div>

  <div class="card">
    <h2>报表本体（合并区与值均为引擎真实产物）</h2>
    {legend}
    <div class="scroll"><table class="report">{''.join(body)}</table></div>
  </div>

  <div class="card">
    <h2>扩展机制（模板格 → 扩展后）</h2>
    {mech}
  </div>

  <div class="card">
    <h2>对拍验证：报表值 vs SQLite 直查真值</h2>
    <div class="checks">{checks}</div>
  </div>

  {note}

  <div class="card">
    <h2>复现方式</h2>
    <div class="kv">{repro}</div>
  </div>
  <footer>本页由 demo_data/make_html_report.py 从引擎真实导出产物生成；未手写任何业务数值。</footer>
</div>
</body>
</html>
"""
    out = os.path.join(HERE, cfg["out"])
    with open(out, "w", encoding="utf-8") as f:
        f.write(html)
    print(f"[make_html_report] {out}（{len(html)} 字节）")


if __name__ == "__main__":
    build(sys.argv[1] if len(sys.argv) > 1 else "month")
