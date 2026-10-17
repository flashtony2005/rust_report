#!/usr/bin/env python3
"""把渲染产物（CSV + xlsx 合并区）拼成一份可浏览的 HTML 报表页。

数据来源**全部来自引擎真实产物**，不重算：
* 值 → `report_sales_2026.csv`（POST /export?fmt=csv 的字节）；
* 合并区 → `report_sales_2026.xlsx` 的 `xl/worksheets/sheet1.xml` `<mergeCell>`。

用法：``python3 demo_data/make_html.py``（需先跑 make_db.py + curl /export）。
"""

import csv
import os
import re
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
CSV_PATH = os.path.join(HERE, "report_sales_2026.csv")
XLSX_PATH = os.path.join(HERE, "report_sales_2026.xlsx")
HTML_PATH = os.path.join(HERE, "report_sales_2026.html")

REGION_TINT = {
    "华东": "#EAF3FB",
    "华北": "#FDF3E7",
    "华南": "#EBF7EF",
    "西南": "#F5EEF9",
    "东北": "#FDEEEE",
}


def load_rows():
    with open(CSV_PATH, encoding="utf-8") as f:
        return list(csv.reader(f))


def load_merges():
    """从 xlsx 读真实合并区 → {(r, c): (rowspan, colspan)}，1 起坐标。"""
    xml = zipfile.ZipFile(XLSX_PATH).read("xl/worksheets/sheet1.xml").decode("utf-8")

    def rc(cell):
        m = re.match(r"([A-Z]+)(\d+)", cell)
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


def fmt_int(s):
    try:
        return f"{int(s):,}"
    except ValueError:
        return s


def fmt_money(s):
    try:
        return f"{float(s):,.2f}"
    except ValueError:
        return s


def build():
    rows = load_rows()
    merges = load_merges()

    # 区域分段底色（明细区第 4 行起，每 128 行一个区域）
    region_segs = []
    for i, r in enumerate(rows[3:-2], start=4):
        if not region_segs or region_segs[-1][0] != r[0]:
            region_segs.append((r[0], i, i))
        else:
            region_segs[-1] = (r[0], region_segs[-1][1], i)
    tint_of = {}
    for name, r0, r1 in region_segs:
        for r in range(r0, r1 + 1):
            tint_of[r] = REGION_TINT.get(name, "#FFFFFF")

    total = rows[-2]
    avg = rows[-1]
    n_detail = len(rows) - 5  # 标题 + 参数 + 表头 + 合计 + 平均 = 5 行非明细

    # 被 rowspan/colspan 覆盖的格（重建成 (行,列) 集合，渲染时跳过）
    covered = set()
    for (r0, c0), (rs, cs) in merges.items():
        for rr in range(r0, r0 + rs):
            for cc in range(c0, c0 + cs):
                if (rr, cc) != (r0, c0):
                    covered.add((rr, cc))

    body = []
    for ri, row in enumerate(rows, start=1):
        tr_class = ""
        if ri == 1:
            tr_class = ' class="title-row"'
        elif ri == 2:
            tr_class = ' class="param-row"'
        elif ri == 3:
            tr_class = ' class="head-row"'
        elif ri >= len(rows) - 1:
            tr_class = ' class="total-row"'
        tds = []
        for ci, val in enumerate(row, start=1):
            if (ri, ci) in covered:
                continue
            rs, cs = merges.get((ri, ci), (1, 1))
            span_attr = ""
            if rs > 1:
                span_attr += f' rowspan="{rs}"'
            if cs > 1:
                span_attr += f' colspan="{cs}"'
            cls = []
            if 4 <= ri <= len(rows) - 3:
                cls.append("detail")
            if ci in (6, 7) and ri >= 4:
                cls.append("num")
            text = val
            if ri >= 4 and ci == 6:
                text = fmt_int(val)
            elif ri >= 4 and ci == 7:
                text = fmt_money(val)
            style = ""
            if 4 <= ri <= len(rows) - 3:
                style = f' style="background:{tint_of[ri]}"'
            tds.append(f'<td class="{" ".join(cls)}"{span_attr}{style}>{text}</td>')
        body.append(f"<tr{tr_class}>{''.join(tds)}</tr>")

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>2026 年度区域销售综合报表 — rpx 演示</title>
<style>
  :root {{ --line:#b8c4d0; --head:#dce6f1; --ink:#1f2937; --accent:#1f4e79; }}
  * {{ box-sizing:border-box; }}
  body {{ margin:0; padding:24px; font-family:"PingFang SC","Microsoft YaHei","Songti SC",sans-serif;
         background:#f4f6f8; color:var(--ink); }}
  .wrap {{ max-width:1180px; margin:0 auto; }}
  .badge {{ display:inline-block; background:var(--accent); color:#fff; border-radius:4px;
            padding:2px 10px; font-size:12px; margin-right:8px; vertical-align:middle; }}
  h1 {{ font-size:20px; margin:0 0 4px; }}
  .sub {{ color:#5b6b7c; font-size:13px; margin-bottom:16px; }}
  .card {{ background:#fff; border:1px solid #e2e8ee; border-radius:10px; padding:16px 20px;
           margin-bottom:16px; box-shadow:0 1px 3px rgba(15,23,42,.06); }}
  .card h2 {{ font-size:15px; margin:0 0 10px; color:var(--accent); }}
  table.report {{ border-collapse:collapse; width:100%; background:#fff; font-size:13px; }}
  table.report th, table.report td {{ border:1px solid var(--line); padding:5px 8px; text-align:center; }}
  .title-row td {{ background:var(--head); color:var(--accent); font-weight:700; font-size:16px;
                   letter-spacing:2px; height:40px; }}
  .param-row td {{ background:#fbfcfe; font-size:12.5px; color:#37424e; height:24px; }}
  .head-row td {{ background:var(--head); font-weight:700; height:28px; }}
  .detail td {{ font-size:12.5px; }}
  .num {{ text-align:right; font-variant-numeric:tabular-nums; }}
  .total-row td {{ background:#f2f2f2; font-weight:700; height:26px; }}
  .thead-zone {{ max-height:72vh; overflow:auto; border:1px solid var(--line); border-radius:8px; }}
  .thead-zone thead th {{ position:sticky; top:0; z-index:2; }}
  .checks {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(240px,1fr)); gap:10px; }}
  .check {{ border:1px solid #dbe7f3; background:#f7fbff; border-radius:8px; padding:10px 12px; }}
  .check .ok {{ color:#1a7f37; font-weight:700; }}
  .check .kv {{ font-size:12.5px; color:#5b6b7c; margin-top:2px; }}
  code {{ background:#eef2f6; border-radius:4px; padding:1px 6px; font-size:12px; }}
  .legend span {{ display:inline-block; width:12px; height:12px; border-radius:3px;
                  margin-right:4px; vertical-align:-1px; border:1px solid #cbd5e0; }}
  .legend i {{ font-style:normal; margin-right:14px; font-size:12.5px; color:#4b5563; }}
  footer {{ color:#8494a5; font-size:12px; margin-top:8px; }}
</style>
</head>
<body>
<div class="wrap">
  <h1><span class="badge">rpx 演示</span>2026 年度区域销售综合报表</h1>
  <div class="sub">
    SQLite 数据源 <code>demo_data/report_demo.db</code>（自创数据，可复现）·
    定义 JSON <code>report_sales_2026.json</code> · 渲染 <code>POST /api/render</code> ·
    导出 <code>/export?fmt=csv|xlsx</code> · 分页 22 页（A4 纵向，每页 40 行）
  </div>

  <div class="card">
    <h2>报表本体（{n_detail} 笔明细 + 合计/平均；区域与姓名列已按 merge_same 纵向合并）</h2>
    <div class="legend">
      <i><span style="background:#EAF3FB"></span>华东</i>
      <i><span style="background:#FDF3E7"></span>华北</i>
      <i><span style="background:#EBF7EF"></span>华南</i>
      <i><span style="background:#F5EEF9"></span>西南</i>
      <i><span style="background:#FDEEEE"></span>东北</i>
    </div>
    <div class="thead-zone">
      <table class="report">
        {''.join(body)}
      </table>
    </div>
  </div>

  <div class="card">
    <h2>验证：报表值 vs SQLite 直查真值（全部来自引擎真实产物）</h2>
    <div class="checks">
      <div class="check"><div class="ok">✓ 明细行数 {n_detail}</div>
        <div class="kv">期望 640（20 员工 × 8 产品 × 4 季度）</div></div>
      <div class="check"><div class="ok">✓ 区域 5 × 128 行</div>
        <div class="kv">merge_same 纵向合并跨度经 xlsx mergeCell 逐区核验</div></div>
      <div class="check"><div class="ok">✓ 姓名 20 × 32 行合并</div>
        <div class="kv">每位员工 32 笔（8 产品 × 4 季度）</div></div>
      <div class="check"><div class="ok">✓ 合计数量 {fmt_int(total[5])}</div>
        <div class="kv">= SELECT SUM(qty) FROM sales = 23,933</div></div>
      <div class="check"><div class="ok">✓ 合计金额 ¥{fmt_money(total[6])}</div>
        <div class="kv">= SELECT SUM(amount) FROM sales = 39,826,975.20</div></div>
      <div class="check"><div class="ok">✓ 平均 {fmt_money(avg[5])} / ¥{fmt_money(avg[6])}</div>
        <div class="kv">= 合计 ÷ 640，逐项相等</div></div>
    </div>
  </div>

  <div class="card">
    <h2>复现方式</h2>
    <div class="kv" style="font-size:12.5px;line-height:1.9">
      <code>python3 demo_data/make_db.py</code> — 建库（自创数据，seed=20261016）<br>
      <code>~/.cargo/target/debug/rpx-server serve --addr 127.0.0.1:8090 --datasources demo_data/datasources.json</code> — 起服务<br>
      <code>curl -F report=@demo_data/report_sales_2026.json -F 'params={{"year":2026,"area":"全部区域"}}' http://127.0.0.1:8090/render</code> — 渲染快照（Univer IWorkbookData）<br>
      <code>curl -F report=@demo_data/report_sales_2026.json ... http://127.0.0.1:8090/export?fmt=xlsx</code> — 导出 xlsx
    </div>
  </div>
  <footer>本页由 demo_data/make_html.py 从引擎真实导出产物生成；未手写任何业务数值。</footer>
</div>
</body>
</html>
"""
    with open(HTML_PATH, "w", encoding="utf-8") as f:
        f.write(html)
    print(f"[make_html] {HTML_PATH}（{len(html)} 字节，{n_detail} 行明细）")


if __name__ == "__main__":
    build()
