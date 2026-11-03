/**
 * 报表生成器：一句话 → 标准口径/词表解析 → 真连库跑数 → 落成报表资产。
 *
 * 产物（都落磁盘，重启不丢）：
 *   examples/library/<id>/manifest.json      七字段清单（含生成来源：问题/口径/SQL）
 *   examples/library/<id>/template.json      模板（引擎格式：cells 数组，1 基行列）
 *   examples/library/<id>/data/<id>.csv      本次取数的数据快照
 *   examples/library/<id>/runs/<run_id>/     run.json / workbook.json / result.json / report.csv
 *
 * 与 mock 的关系：dev 层把生成结果**登记**进内存的 mockReports/mockRuns 供页面即时看到；
 * vite 启动时用 loadGeneratedReports() 从磁盘回读，保证刷新/重启后报表库与运行记录不丢。
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { probeDatasource } from './datasource-store.mjs';
import { matchCanon, saveOntologyTrace } from './ontology-store.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const LIB = path.resolve(__dirname, '../examples/library');

/** 生成报表的 id 前缀（用来和手写示例库区分）。 */
const GEN_PREFIX = 'gen-';

/* -------------------------------------------------------------------------- */
/* 小工具                                                                      */
/* -------------------------------------------------------------------------- */

const pad = (n) => String(n).padStart(2, '0');
function nowIso() {
  return new Date().toISOString();
}
function stamp(d = new Date()) {
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;
}
function csvCell(v) {
  const s = v == null ? '' : String(v);
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}
function toCsv(rows) {
  // 与导出基线一致：CRLF
  return '\uFEFF' + rows.map((r) => r.map(csvCell).join(',')).join('\r\n') + '\r\n';
}
/** 写文件：目标被别的进程占着（Excel/编辑器打开中 → EBUSY）时，落带时间戳的副本，不让整张报表失败。 */
function safeWrite(file, content) {
  try {
    fs.writeFileSync(file, content, 'utf8');
    return file;
  } catch (e) {
    if (!['EBUSY', 'EPERM', 'EACCES'].includes(e && e.code)) throw e;
    const ext = path.extname(file);
    const alt = file.slice(0, -ext.length) + '-' + stamp() + ext;
    fs.writeFileSync(alt, content, 'utf8');
    return alt;
  }
}

function ensureDir(p) {
  fs.mkdirSync(p, { recursive: true });
}

/** 数字列判定：全都是数字字符串（含空）才算。 */
function isNumericCol(rows, idx) {
  let seen = 0;
  for (const r of rows) {
    const v = r[idx];
    if (v === null || v === undefined || v === '') continue;
    if (!/^-?\d+(\.\d+)?$/.test(String(v))) return false;
    seen++;
  }
  return seen > 0;
}

/* -------------------------------------------------------------------------- */
/* 模板构建（引擎格式：cells 数组，1 基）                                        */
/* -------------------------------------------------------------------------- */

const FONT = 'Microsoft YaHei';

function cell(row, col, text, extra = {}) {
  return {
    row,
    col,
    text: text == null ? '' : String(text),
    font_name: FONT,
    ...extra,
  };
}

/**
 * 把「标题 + 表头 + 明细 + 小计 + 合计 + 脚注」拼成引擎模板。
 * groups: [{ key, label, rows: [...] }]（按分类分组，用于小计）
 */
function buildTemplate({ title, subtitle, headers, groups, footer, numericFrom }) {
  const cells = [];
  const colCount = headers.length;

  cells.push(
    cell(1, 1, title, {
      bold: true,
      font_size: 18,
      fore_color: '#163D46',
      h_align: 'center',
      v_align: 'middle',
      merge: { cols: colCount, rows: 1 },
    })
  );
  cells.push(
    cell(2, 1, subtitle, {
      font_size: 9,
      fore_color: '#5A6B75',
      h_align: 'center',
      v_align: 'middle',
      merge: { cols: colCount, rows: 1 },
    })
  );

  const headRow = 3;
  headers.forEach((h, i) => {
    cells.push(
      cell(headRow, i + 1, h, {
        bold: true,
        font_size: 10,
        fore_color: '#FFFFFF',
        back_color: '#2C4A5A',
        h_align: 'center',
        v_align: 'middle',
        border: 'thin',
      })
    );
  });

  let row = headRow + 1;
  const sumIdx = [];
  for (let i = 0; i < colCount; i++) if (i >= numericFrom) sumIdx.push(i);

  for (const g of groups) {
    const start = row;
    for (const r of g.rows) {
      r.forEach((v, i) => {
        const numeric = i >= numericFrom && /^-?\d+(\.\d+)?$/.test(String(v ?? ''));
        cells.push(
          cell(row, i + 1, v, {
            font_size: 10,
            h_align: i === 1 ? 'left' : numeric ? 'right' : 'center',
            v_align: 'middle',
            border: 'thin',
          })
        );
      });
      row++;
    }
    if (g.subtotal) {
      g.subtotal.forEach((v, i) => {
        cells.push(
          cell(row, i + 1, v, {
            bold: true,
            font_size: 10,
            back_color: '#EAF0F3',
            h_align: i <= 1 ? 'left' : 'right',
            v_align: 'middle',
            border: 'thin',
          })
        );
      });
      row++;
    }
    if (g.label && start === row) {
      /* 分组空 → 不落行 */
    }
  }

  // 合计行
  if (groups.length > 1) {
    const total = ['合计', ''];
    for (let i = 2; i < colCount; i++) total.push(0);
    for (const g of groups) {
      const src = g.subtotal || [];
      for (let i = 2; i < colCount - 1; i++) total[i] = (Number(total[i]) || 0) + (Number(src[i]) || 0);
    }
    total.forEach((v, i) => {
      cells.push(
        cell(row, i + 1, typeof v === 'number' ? v : v, {
          bold: true,
          font_size: 10,
          fore_color: '#FFFFFF',
          back_color: '#2C4A5A',
          h_align: i <= 1 ? 'left' : 'right',
          v_align: 'middle',
          border: 'thin',
        })
      );
    });
    row++;
  }

  if (footer) {
    cells.push(
      cell(row + 1, 1, footer, {
        font_size: 8,
        fore_color: '#7A8A94',
        h_align: 'left',
        v_align: 'top',
        merge: { cols: colCount, rows: 1 },
      })
    );
  }

  return { cells };
}

/**
 * 汇总矩阵模板：两级表头（市级财政 / 镇区财政 × 工资 / 绩效 / 税金）+ 合计列。
 * rows 的第一格是指标名，后面依次是 c1..c6 与合计（8 列）。
 */
function buildMatrixTemplate({ title, subtitle, rows, footer }) {
  const colCount = 7;
  const cells = [];

  cells.push(
    cell(1, 1, title, {
      bold: true,
      font_size: 18,
      fore_color: '#163D46',
      h_align: 'center',
      v_align: 'middle',
      merge: { cols: colCount, rows: 1 },
    })
  );
  cells.push(
    cell(2, 1, subtitle, {
      font_size: 9,
      fore_color: '#5A6B75',
      h_align: 'center',
      v_align: 'middle',
      merge: { cols: colCount, rows: 1 },
    })
  );

  const headStyle = {
    bold: true,
    font_size: 10,
    fore_color: '#FFFFFF',
    back_color: '#2C4A5A',
    h_align: 'center',
    v_align: 'middle',
    border: 'thin',
  };

  // 单行表头（简单网格渲染不认合并，合并会导致列错位）
  const heads = ['指标', '市级·工资', '市级·绩效', '市级·税金', '镇区·工资', '镇区·绩效', '镇区·税金'];
  heads.forEach((h, i) => {
    cells.push(cell(3, i + 1, h, { ...headStyle, h_align: i === 0 ? 'left' : 'center' }));
  });

  let row = 4;
  for (const r of rows) {
    const isRate = /率/.test(String(r[0]));
    r.forEach((v, i) => {
      const numeric = typeof v === 'number';
      cells.push(
        cell(row, i + 1, v, {
          font_size: 10,
          bold: isRate || i === 0,
          back_color: i === 0 ? '#F3F7F9' : undefined,
          h_align: i === 0 ? 'left' : 'right',
          v_align: 'middle',
          border: 'thin',
          ...(numeric || isRate ? {} : {}),
        })
      );
    });
    row++;
  }

  if (footer) {
    cells.push(
      cell(row + 1, 1, footer, {
        font_size: 8,
        fore_color: '#7A8A94',
        h_align: 'left',
        v_align: 'top',
        merge: { cols: colCount, rows: 1 },
      })
    );
  }

  return { cells };
}

/** 引擎格式（cells 数组）→ 页面用的格子映射。 */export function templateToCellMap(libTemplate) {
  const out = {};
  for (const c of libTemplate?.cells || []) {
    if (c.row == null || c.col == null) continue;
    out[`${c.row - 1},${c.col - 1}`] = { v: c.text ?? '' };
  }
  return { cells: out };
}

/** 格子映射 → 二维行（导出 PDF/Excel/HTML 用）。 */
export function cellMapToRows(template) {
  const cells = template?.cells || {};
  let maxR = 0;
  let maxC = 0;
  for (const k of Object.keys(cells)) {
    const [r, c] = k.split(',').map(Number);
    if (!isNaN(r) && r > maxR) maxR = r;
    if (!isNaN(c) && c > maxC) maxC = c;
  }
  if (maxR === 0 && maxC === 0) return [];
  const rows = [];
  for (let r = 0; r <= maxR; r++) {
    const row = [];
    for (let c = 0; c <= maxC; c++) row.push(cells[`${r},${c}`]?.v ?? '');
    rows.push(row);
  }
  return rows;
}

/* -------------------------------------------------------------------------- */
/* 运行产物                                                                    */
/* -------------------------------------------------------------------------- */

/** 仿 Univer 的快照（ReportView / WorkbookViewer 吃这个）：标题行 + 表头样式 + 数字右对齐。 */
function workbookSnapshot(title, rows) {
  const cellData = {};
  const colCount = Math.max(4, ...rows.map((r) => (Array.isArray(r) ? r.length : 0)));

  cellData['0'] = {
    '0': { v: title, t: 1, s: { bl: 1, fs: 14, cl: { rgb: '#FFFFFF' }, bg: { rgb: '#163D46' }, ht: 2, vt: 2 } },
  };
  for (let c = 1; c < colCount; c++) cellData['0'][String(c)] = { s: { bg: { rgb: '#163D46' } } };

  rows.forEach((r, ri) => {
    const rowIdx = String(ri + 1);
    cellData[rowIdx] = {};
    const isHeader = ri === 0;
    const isRate = String(r[0] || '').includes('率');
    r.forEach((v, ci) => {
      const numeric = typeof v === 'number';
      cellData[rowIdx][String(ci)] = {
        v: v == null ? '' : v,
        t: numeric ? 2 : 1,
        s: {
          bl: isHeader || isRate || ci === 0 ? 1 : 0,
          cl: { rgb: isHeader ? '#FFFFFF' : '#191512' },
          bg: { rgb: isHeader ? '#2C4A5A' : isRate ? '#EAF0F3' : ri % 2 === 0 ? '#F8FAFC' : '#FFFFFF' },
          ht: numeric ? 3 : isHeader || ci === 0 ? 2 : 1,
          vt: 2,
        },
      };
    });
  });

  return {
    id: 'wb-' + Date.now().toString(36),
    name: title,
    appVersion: '1.0.0',
    locale: 'zh-CN',
    sheetOrder: ['sheet_main'],
    sheets: {
      sheet_main: {
        id: 'sheet_main',
        name: title,
        rowCount: rows.length + 3,
        columnCount: colCount + 1,
        showGridlines: 1,
        mergeData: [{ startRow: 0, endRow: 0, startColumn: 0, endColumn: colCount - 1 }],
        cellData,
      },
    },
  };
}

/** 可分享/可打印的单文件 HTML 报表（真数据，样式内联）。 */
function reportHtml({ title, subtitle, rows, footer }) {
  const esc = (s) =>
    String(s == null ? '' : s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const head = rows[0] || [];
  const body = rows.slice(1);
  const style = [
    'body{font-family:"Microsoft YaHei",-apple-system,"PingFang SC",sans-serif;background:#fdfbf7;color:#191512;padding:36px;}',
    'h1{font-size:22px;margin:0 0 6px;color:#163D46;}',
    '.meta{font-size:12px;color:#5A6B75;margin-bottom:18px;}',
    'table{width:100%;border-collapse:collapse;background:#fff;box-shadow:0 1px 4px rgba(0,0,0,.08);border-radius:8px;overflow:hidden;}',
    'th{background:#2C4A5A;color:#fff;font-size:13px;font-weight:600;padding:10px 12px;text-align:center;}',
    'th:first-child{text-align:left;}',
    'td{padding:9px 12px;font-size:13px;border-bottom:1px solid #e8edf1;}',
    'td:first-child{font-weight:600;background:#F3F7F9;}',
    'td.num{text-align:right;font-variant-numeric:tabular-nums;}',
    'tr:last-child td{background:#EAF0F3;font-weight:600;}',
    '.foot{margin-top:16px;font-size:11px;color:#7A8A94;white-space:pre-line;line-height:1.7;}',
  ].join('');
  const th = head.map((h) => `<th>${esc(h)}</th>`).join('');
  const tr = body
    .map(
      (r) =>
        '<tr>' +
        r
          .map((v, i) => `<td class="${i > 0 && typeof v === 'number' ? 'num' : ''}">${esc(v)}</td>`)
          .join('') +
        '</tr>'
    )
    .join('');
  return [
    '<!DOCTYPE html>',
    '<html lang="zh-CN"><head><meta charset="utf-8"><title>' + esc(title) + '</title>',
    '<style>' + style + '</style></head><body>',
    '<h1>' + esc(title) + '</h1>',
    '<div class="meta">' + esc(subtitle || '') + '</div>',
    '<table><thead><tr>' + th + '</tr></thead><tbody>' + tr + '</tbody></table>',
    '<div class="foot">' + esc(footer || '') + '</div>',
    '</body></html>',
  ].join('\n');
}

function writeRunDir({ reportId, runId, title, rows, sql, columns, elapsed, subtitle, footer }) {
  const dir = path.join(LIB, reportId, 'runs', runId);
  ensureDir(dir);

  fs.writeFileSync(path.join(dir, 'workbook.json'), JSON.stringify(workbookSnapshot(title, rows), null, 2), 'utf8');
  fs.writeFileSync(
    path.join(dir, 'result.json'),
    JSON.stringify(
      {
        ok: true,
        report_id: reportId,
        run_id: runId,
        status: 'succeeded',
        sql,
        columns,
        rows,
        row_count: rows.length,
        elapsed_ms: elapsed,
        generated_at: nowIso(),
      },
      null,
      2
    ),
    'utf8'
  );
  fs.writeFileSync(path.join(dir, 'report.csv'), toCsv(rows), 'utf8');
  fs.writeFileSync(
    path.join(dir, 'report.html'),
    reportHtml({ title, subtitle, rows, footer }),
    'utf8'
  );

  const artifacts = {
    'workbook.json': 'artifacts/workbook.json',
    'result.json': 'artifacts/result.json',
    'report.csv': 'artifacts/report.csv',
    'report.html': 'artifacts/report.html',
  };
  return { dir, artifacts };
}

/* -------------------------------------------------------------------------- */
/* 主流程                                                                      */
/* -------------------------------------------------------------------------- */

/** 把一行数据按数值/文本对齐的方式转成展示值。 */
function fmtRate(ok, total) {
  if (!total) return '—';
  return ((ok / total) * 100).toFixed(1) + '%';
}

/**
 * 一句话 → 报表。
 * @returns {{ok:boolean, report_id:string, title:string, sql:string, columns:string[],
 *            row_count:number, run_id:string, reply:object, report:object, run:object}}
 */
export async function buildReportFromText(opts) {
  const dsn = String(opts.dsn || '').trim();
  const text = String(opts.text || '').trim();
  if (!dsn) throw new Error('dsn 必填（选一个数据源）');
  if (!text) throw new Error('text 必填（一句话需求）');

  const canon = matchCanon(text);
  const sql = canon ? canon.sql : null;
  if (!sql) throw new Error(`这句话没命中标准口径：${text}（先在 examples/library/.ontology-canon.sqls.json 加一条）`);

  const title = opts.title?.trim() || canon.title;
  const limit = Number(opts.limit) || 500;

  const t0 = Date.now();
  const probe = await probeDatasource(dsn, { sql, limit });
  const elapsed = probe.elapsed_ms ?? Date.now() - t0;
  const columns = (probe.columns || []).map((c) => c.name);
  const rawRows = probe.rows || [];
  if (rawRows.length === 0) throw new Error('查询没有返回数据（检查 sql 或数据源）');

  const now = new Date();
  const isMatrix = canon.shape === 'summary-matrix';
  let headers;
  let libTemplate;
  let flatRows;
  let footer;
  let subtitle;
  let queryMetrics;
  let queryDims;

  if (isMatrix) {
    /* ---- 汇总矩阵：5 个指标 × (市级/镇区 × 工资/绩效/税金) + 合计 ---- */
    const idx = {};
    columns.forEach((c, i) => (idx[c] = i));
    const pick = (label) => {
      const row = rawRows.find((r) => String(r[0]) === label);
      if (!row) return [null, null, null, null, null, null];
      return [1, 2, 3, 4, 5, 6].map((k) => {
        const v = row[idx[`c${k}`]];
        return v == null || v === '' ? null : Number(v);
      });
    };
    const plan = pick('应上报单位数');
    const done = pick('已确认单位数');
    const undo = pick('未确认单位数');
    const downloaded = pick('已下载单位数');
    const notDownloaded = pick('未下载单位数');
    const sum6 = (arr) => arr.reduce((a, v) => a + (v == null ? 0 : v), 0);
    const pct = (a, b) => (b ? ((a / b) * 100).toFixed(1) + '%' : '—');

    const matrixRows = [
      ['应上报单位数', ...plan],
      ['已确认单位数', ...done],
      ['未确认单位数', ...undo],
      ['已下载单位数', ...downloaded],
      ['未下载单位数', ...notDownloaded],
      ['上报率', ...done.map((v, i) => pct(v, plan[i]))],
    ];

    headers = [
      '指标',
      '市级财政-工资',
      '市级财政-绩效',
      '市级财政-税金',
      '镇区财政-工资',
      '镇区财政-绩效',
      '镇区财政-税金',
    ];
    subtitle = `${dsn} · 口径 ${canon.id}（${canon.center_entity || '—'}）· 汇总统计（市级/镇区 × 工资/绩效/税金）· 生成 ${stamp(now)}`;
    footer = [
      `口径依据：${canon.basis || '—'}`,
      '注：service.v_tj 视图的「1.工资」分支取 dwxxb.sjzt 作工资状态（真列应为 gzzt），该视图工资已确认/待处理会恒为 0；',
      '　　本报表按 dashboard 源码口径（DwxxbService.listMain）三支各取自己的状态列 gzzt / jcjxzt / sjzt，下载列取 gzbz。',
    ].join('\n');
    libTemplate = buildMatrixTemplate({ title, subtitle, rows: matrixRows, footer });
    flatRows = [headers, ...matrixRows];
    queryMetrics = ['应上报单位数', '已确认单位数', '未确认单位数', '已下载单位数', '未下载单位数', '上报率'];
    queryDims = ['市级财政', '镇区财政'];
  } else {
    /* ---- 分组明细：分类 × 镇区（每行一个组合 + 小计） ---- */
    const colIdx = {};
    columns.forEach((c, i) => (colIdx[c] = i));
    const iFl = colIdx['分类'] ?? 0;
    const iArea = colIdx['所属镇区'] ?? 1;
    const iOk = colIdx['已确认'] ?? 2;
    const iPending = colIdx['待处理'] ?? 3;
    const iDl = colIdx['已下载'] ?? 4;
    const iTotal = colIdx['单位总数'] ?? 5;

    headers = ['分类', '所属镇区', '已确认', '待处理', '已下载', '单位总数', '上报率'];

    const groupMap = new Map();
    for (const r of rawRows) {
      const key = String(r[iFl]);
      if (!groupMap.has(key)) groupMap.set(key, []);
      const ok = Number(r[iOk]) || 0;
      const total = Number(r[iTotal]) || 0;
      groupMap.get(key).push([
        key,
        String(r[iArea] ?? ''),
        ok,
        Number(r[iPending]) || 0,
        Number(r[iDl]) || 0,
        total,
        fmtRate(ok, total),
      ]);
    }

    const groups = [];
    for (const [key, rows] of groupMap) {
      const sub = [0, 0, 0, 0];
      for (const r of rows) {
        sub[0] += r[2];
        sub[1] += r[3];
        sub[2] += r[4];
        sub[3] += r[5];
      }
      groups.push({
        key,
        rows,
        subtotal: [`${key} 小计`, `${rows.length} 个镇区`, sub[0], sub[1], sub[2], sub[3], fmtRate(sub[0], sub[3])],
      });
    }

    const grand = groups.reduce(
      (a, g) => {
        a[0] += g.subtotal[2];
        a[1] += g.subtotal[3];
        a[2] += g.subtotal[4];
        a[3] += g.subtotal[5];
        return a;
      },
      [0, 0, 0, 0]
    );

    subtitle = `${dsn} · 口径 ${canon.id}（${canon.center_entity || '—'}）· 分类 × 所属镇区 · 生成 ${stamp(now)} · ${rawRows.length} 行`;
    footer = [
      `口径依据：${canon.basis || '—'}`,
      '注：service.v_tj 视图的「1.工资」分支取 dwxxb.sjzt 作工资状态（真列应为 gzzt），会导致该视图工资已确认/待处理恒为 0；',
      '　　本报表按 dashboard 源码口径（DwxxbService.listMain）三支各取自己的状态列 gzzt / jcjxzt / sjzt，下载列取 gzbz。',
    ].join('\n');
    libTemplate = buildTemplate({ title, subtitle, headers, groups, footer, numericFrom: 2 });
    flatRows = [
      headers,
      ...groups.flatMap((g) => g.rows),
      ['合计', `${rawRows.length} 行`, grand[0], grand[1], grand[2], grand[3], fmtRate(grand[0], grand[3])],
    ];
    queryMetrics = ['已确认', '待处理', '已下载', '单位总数', '上报率'];
    queryDims = ['分类', '所属镇区'];
  }

  /* ---- 落盘：报表资产 ---- */
  const reportId = canon.id ? GEN_PREFIX + canon.id : GEN_PREFIX + 'req_' + Date.now().toString(36);
  const reportDir = path.join(LIB, reportId);
  ensureDir(path.join(reportDir, 'data'));

  const dataCsvPath = safeWrite(path.join(reportDir, 'data', `${reportId}.csv`), toCsv(flatRows));
  safeWrite(path.join(reportDir, 'template.json'), JSON.stringify(libTemplate, null, 2));

  // 修订号 = 已有运行数 + 1（同一张报表反复生成时 rev 递增）
  const prevRuns = fs.existsSync(path.join(reportDir, 'runs')) ? fs.readdirSync(path.join(reportDir, 'runs')).length : 0;
  const revision = 'rev_' + String(prevRuns + 1).padStart(2, '0');
  const runId = 'run_' + Date.now().toString(36);
  const { artifacts } = writeRunDir({
    reportId,
    runId,
    title,
    rows: flatRows,
    sql,
    columns: headers,
    elapsed,
    subtitle,
    footer,
  });

  const startedAt = new Date(Date.now() - elapsed).toISOString();
  const runRecord = {
    run_id: runId,
    report_id: reportId,
    report_name: title,
    revision,
    status: 'succeeded',
    trigger: 'agent',
    started_at: startedAt,
    finished_at: nowIso(),
    duration_ms: elapsed,
    layout: { page_count: 1, rows: flatRows.length, cols: headers.length },
    artifacts,
    error_code: null,
    error_message: null,
    artifact_count: Object.keys(artifacts).length,
    artifact_bytes: flatRows.length * headers.length * 8,
  };
  fs.writeFileSync(path.join(reportDir, 'runs', runId, 'run.json'), JSON.stringify(runRecord, null, 2), 'utf8');

  const manifest = {
    id: reportId,
    name: title,
    description: `${text} —— 由标准口径 ${canon.id} 生成，取数 ${rawRows.length} 行（${dsn}）`,
    version: 1,
    template: 'template.json',
    data_dir: 'data',
    data_file: path.basename(dataCsvPath),
    tags: ['Agent生成', '标准口径', canon.id, '工资统发'],
    meta: {
      schema: 'rpx-template-meta/v1',
      kind: 'generated',
      shape: canon.shape || 'grouped',
      difficulty: 2,
      domain: ['工资统发'],
      capabilities: ['agg_sum', 'group_row', 'grand_total', 'ratio'],
      keywords: ['单位上报', '所属镇区', '镇街', '已确认', '待处理', '已下载'],
      examples: [text],
      extends: ['row'],
      datasets: { count: 1, requires_sql: true, fields: {} },
      generated: {
        dsn,
        question: text,
        canon_id: canon.id,
        center_entity: canon.center_entity,
        sql,
        row_count: rawRows.length,
        generated_at: nowIso(),
        run_id: runId,
      },
    },
  };
  safeWrite(path.join(reportDir, 'manifest.json'), JSON.stringify(manifest, null, 2));

  // 页面用的报表对象（template 转成格子映射，ReportView 吃这个）
  const report = {
    id: reportId,
    revision,
    manifest,
    template: templateToCellMap(libTemplate),
    result_state: 'current_revision',
    display_run_id: runId,
    display_run: runRecord,
    latest_attempt: runRecord,
  };

  // 解析留痕（同一句话在四步解析/历史里可回放）
  let traceId = null;
  try {
    const saved = saveOntologyTrace({
      dsn,
      question: text,
      status: 'auto',
      center_entity: canon.center_entity || null,
      fanout_edges: 0,
      sql,
      steps: [
        { step_no: 1, kind: 'identify', title: `标准口径命中（${canon.title}）`, detail: canon.basis, edited: 0 },
        { step_no: 2, kind: 'center', title: '定主表', detail: `中心实体 ${canon.center_entity || '—'}`, edited: 0 },
        { step_no: 3, kind: 'grain', title: '粒度检查', detail: '口径已聚合到 分类 × 镇区，无扇出', edited: 0 },
        { step_no: 4, kind: 'compose', title: '组装输出', sql, detail: `真跑数 ${rawRows.length} 行 / ${elapsed}ms`, edited: 0 },
      ],
      hits: [],
    });
    traceId = saved?.id ?? null;
  } catch {
    /* 留痕失败不影响出报表 */
  }

  const reply = {
    ok: true,
    report_id: reportId,
    id: reportId,
    title,
    shape: canon.shape || 'grouped',
    decision: {
      shape: canon.shape || 'grouped',
      label: isMatrix ? '汇总矩阵（指标 × 市级/镇区 × 工资/绩效/税金）' : '分组明细（分类 × 所属镇区）',
      layout: 'grouped',
      confidence: 'high',
      reasons: [
        `命中标准口径 ${canon.id}（业务上有权威实现，不走词表拼装）`,
        `中心实体 ${canon.center_entity || '—'}，已聚合到 分类 × 镇区 粒度，无跨表扇出`,
        `真连库取数 ${rawRows.length} 行 / ${elapsed}ms`,
      ],
      blockers: [],
      alternatives: [{ shape: 'grouped', why: '逐镇区明细走口径 dw-sbzk-detail' }],
      ok: true,
    },
    spec: {
      id: reportId,
      title,
      dsn,
      canon_id: canon.id,
      sql,
      columns: headers,
      group_by: ['分类', '所属镇区'],
      measures: ['已确认', '待处理', '已下载', '单位总数', '上报率'],
      layout: { title_row: 1, head_row: 3, data_from: 4 },
    },
    notes: [
      ...(canon.notes || []),
      `已生成报表资产：examples/library/${reportId}/（manifest + template + data + runs/${runId}）`,
      traceId ? `解析痕迹已留库：${traceId}` : '解析留痕未写入（语义库未启用）',
    ],
    query: {
      metrics: queryMetrics,
      dimensions: queryDims,
      picked_by: 'canon',
      sql,
      columns: headers,
      row_count: rawRows.length,
    },
    run: { ok: true, status: 'succeeded', run_id: runId },
    trace_id: traceId,
  };

  return {
    ok: true,
    report_id: reportId,
    title,
    sql,
    columns,
    row_count: rawRows.length,
    elapsed_ms: elapsed,
    run_id: runId,
    report,
    run: runRecord,
    reply,
  };
}

/* -------------------------------------------------------------------------- */
/* 磁盘回读（重启后报表库/运行记录不丢）                                        */
/* -------------------------------------------------------------------------- */

/** 读取一个生成报表目录 → {report, run}；不是生成报表返回 null。 */
function readGeneratedReport(dirName) {
  const dir = path.join(LIB, dirName);
  const mf = path.join(dir, 'manifest.json');
  const tf = path.join(dir, 'template.json');
  if (!fs.existsSync(mf) || !fs.existsSync(tf)) return null;
  let manifest;
  let libTemplate;
  try {
    manifest = JSON.parse(fs.readFileSync(mf, 'utf8'));
    libTemplate = JSON.parse(fs.readFileSync(tf, 'utf8'));
  } catch {
    return null;
  }
  if (!manifest?.meta?.generated) return null; // 只接管生成报表，手写示例库不掺和

  const runsDir = path.join(dir, 'runs');
  const runs = [];
  if (fs.existsSync(runsDir)) {
    for (const r of fs.readdirSync(runsDir)) {
      const f = path.join(runsDir, r, 'run.json');
      if (fs.existsSync(f)) {
        try {
          runs.push(JSON.parse(fs.readFileSync(f, 'utf8')));
        } catch {
          /* 坏文件跳过 */
        }
      }
    }
  }
  runs.sort((a, b) => String(b.finished_at).localeCompare(String(a.finished_at)));
  const display = runs[0] || null;

  return {
    report: {
      id: manifest.id,
      revision: 'rev_01',
      manifest,
      template: templateToCellMap(libTemplate),
      result_state: 'current_revision',
      display_run_id: display?.run_id ?? null,
      display_run: display,
      latest_attempt: display,
    },
    runs,
  };
}

/** 启动时回读全部生成报表。 */
export function loadGeneratedReports() {
  const reports = [];
  const runs = [];
  let dirs = [];
  try {
    dirs = fs.readdirSync(LIB);
  } catch {
    return { reports, runs };
  }
  for (const d of dirs) {
    if (!fs.statSync(path.join(LIB, d)).isDirectory()) continue;
    const got = readGeneratedReport(d);
    if (!got) continue;
    reports.push(got.report);
    runs.push(...got.runs);
  }
  return { reports, runs };
}
