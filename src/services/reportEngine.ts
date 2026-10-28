import {
  Dataset,
  Spec,
  Template,
  TemplateCell,
  Candidate,
  Run,
  Requirement,
  ShapeDecision,
  AnalysisDoc,
  ReportShape,
  SemanticVersion,
  IWorkbookData,
  UniverCellData,
} from '../types';
import { PRESET_LIBRARY, SHAPE_LABELS, RAW_SAMPLE_CSV } from '../data/seedData';

export function parseCsv(csvText: string): {
  columns: string[];
  rows: number;
  preview: any[][];
  column_meta: Record<string, { type: string; comment: string; sample: any }>;
  allRows: Record<string, any>[];
} {
  const lines = csvText.trim().split(/\r?\n/).filter(Boolean);
  if (!lines.length) {
    return { columns: [], rows: 0, preview: [], column_meta: {}, allRows: [] };
  }

  const columns = lines[0].split(',').map((c) => c.trim().replace(/^["']|["']$/g, ''));
  const allRows: Record<string, any>[] = [];
  const preview: any[][] = [];

  for (let i = 1; i < lines.length; i++) {
    const vals = lines[i].split(',').map((v) => v.trim().replace(/^["']|["']$/g, ''));
    const obj: Record<string, any> = {};
    columns.forEach((col, idx) => {
      const raw = vals[idx] ?? '';
      const num = Number(raw);
      obj[col] = !isNaN(num) && raw !== '' ? num : raw;
    });
    allRows.push(obj);
    if (preview.length < 5) {
      preview.push(columns.map((c) => obj[c]));
    }
  }

  const column_meta: Record<string, { type: string; comment: string; sample: any }> = {};
  columns.forEach((col) => {
    const samples = allRows.slice(0, 10).map((r) => r[col]);
    const isNum = samples.every((v) => typeof v === 'number' || !isNaN(Number(v)));
    const isDate = samples.some((v) => typeof v === 'string' && /^\d{4}-\d{2}/.test(v));

    let typeStr = 'VARCHAR(64)';
    let comment = '文本维度';
    if (isNum && !isDate) {
      typeStr = 'DECIMAL(12,2)';
      comment = '数值指标 / 金额度量';
    } else if (isDate) {
      typeStr = 'DATE';
      comment = '时间周期维度';
    }

    if (col === 'region') comment = '销售大区维度（华东/华南/华北）';
    if (col === 'store') comment = '门店名称维度';
    if (col === 'cat' || col === 'category') comment = '产品品类维度（办公/数码/家居）';
    if (col === 'amount') comment = '销售金额（元）';
    if (col === 'month') comment = '归属月份（YYYY-MM）';

    column_meta[col] = {
      type: typeStr,
      comment,
      sample: samples[0] ?? '',
    };
  });

  return {
    columns,
    rows: allRows.length,
    preview,
    column_meta,
    allRows,
  };
}

export function createSampleDataset(): Dataset {
  const parsed = parseCsv(RAW_SAMPLE_CSV);
  return {
    kind: 'sample',
    file: 'store-sales.csv',
    rows: parsed.rows,
    columns: parsed.columns,
    column_meta: parsed.column_meta,
    preview: parsed.preview,
    encoding: 'utf-8',
  };
}

export function recommendCandidates(req: Requirement): Candidate[] {
  const cols = req.dataset.columns || [];
  const desc = ((req.title || '') + ' ' + (req.description || '')).toLowerCase();

  return PRESET_LIBRARY.map((preset) => {
    if (preset.source === 'generated') return preset;

    const reqCols = preset.fields || [];
    const missing = reqCols.filter((c) => !cols.includes(c));
    let score = preset.score || 70;

    if (missing.length === 0) score += 10;
    else score -= missing.length * 15;

    if (preset.id.includes('crosstab') && (desc.includes('交叉') || desc.includes('品类'))) {
      score += 15;
    }
    if (preset.id.includes('trend') && (desc.includes('月') || desc.includes('趋势') || desc.includes('走势'))) {
      score += 15;
    }
    if (preset.id.includes('rank') && (desc.includes('排行') || desc.includes('排名') || desc.includes('top'))) {
      score += 15;
    }

    return {
      ...preset,
      score: Math.max(20, Math.min(99, score)),
      missing,
    };
  }).sort((a, b) => (b.score || 0) - (a.score || 0));
}

export function detectShape(spec: Spec, dataset: Dataset, intentText: string = ''): ShapeDecision {
  const rowDims = spec.row_dims || [];
  const colDims = spec.col_dims || [];
  const measures = spec.measures || [];
  const cols = dataset.columns || [];
  const text = (intentText + ' ' + (spec.shape || '')).toLowerCase();

  const reasons: string[] = [];
  const blockers: string[] = [];
  let chosenShape: ReportShape = spec.shape || 'grouped';

  if (spec.shape) {
    chosenShape = spec.shape;
  } else if (text.includes('明细') || text.includes('流水')) {
    chosenShape = 'detail';
  } else if (text.includes('交叉') && (colDims.length > 0 || rowDims.length >= 2)) {
    chosenShape = 'crosstab';
  } else if (text.includes('趋势') || text.includes('走势') || cols.includes('month')) {
    chosenShape = 'trend';
  } else if (text.includes('排行') || text.includes('排名') || text.includes('top')) {
    chosenShape = 'rank';
  } else if (text.includes('对比') || text.includes('环比') || text.includes('同比')) {
    chosenShape = 'compare';
  } else {
    chosenShape = rowDims.length > 0 ? 'grouped' : 'detail';
  }

  // Validate chosen shape against needs
  if (chosenShape === 'crosstab') {
    if (rowDims.length === 0 || colDims.length === 0) {
      blockers.push('交叉表需要至少两个维度：一个放行，一个放列展开');
    } else {
      reasons.push(`行维度 [${rowDims.map((d) => d.label || d.field).join(', ')}]，列维度 [${colDims.map((d) => d.label || d.field).join(', ')}]`);
    }
  } else if (chosenShape === 'trend') {
    const hasDateDim = rowDims.some((d) => d.field === 'month' || d.field.includes('date'));
    if (!hasDateDim) {
      blockers.push('趋势表需要包含日期/月份维度作为时间桶');
    } else {
      reasons.push('按月度时间序列聚合走势');
    }
  } else if (chosenShape === 'rank') {
    reasons.push('单维度聚焦排位，按度量降序排列');
  } else if (chosenShape === 'compare') {
    reasons.push('对比本期与上期度量数值，计算变动率');
  } else if (chosenShape === 'grouped') {
    reasons.push(`按维度 [${rowDims.map((d) => d.label || d.field).join(', ')}] 分组统计汇总`);
  } else if (chosenShape === 'detail') {
    reasons.push('明细平铺展示，无分组聚合');
  }

  if (measures.length === 0 && chosenShape !== 'detail') {
    blockers.push('至少需要选择一个度量指标（如金额）');
  }

  return {
    shape: chosenShape,
    label: SHAPE_LABELS[chosenShape] || '分组汇总',
    reasons,
    blockers,
  };
}

export function compileTemplate(spec: Spec, dataset: Dataset, rev: number = 0): Template {
  const cells: TemplateCell[] = [];
  const rowDims = spec.row_dims || [];
  const colDims = spec.col_dims || [];
  const measures = spec.measures || [];
  const layout = spec.layout || (spec.shape === 'crosstab' ? 'crosstab' : spec.shape === 'detail' ? 'list' : 'grouped');

  let totalRows = 3;
  let totalCols = 1;

  if (layout === 'crosstab') {
    // Row 1: Report Title Banner
    cells.push({
      row: 1,
      col: 1,
      text: '销售多维交叉分析报表',
      bold: true,
      font_size: 16,
      h_align: 'center',
      v_align: 'middle',
      back_color: '#f6e8d7',
      merge: { rows: 1, cols: 5 },
    });

    // Row 2: Headers
    const rowDimLabel = rowDims[0]?.label || rowDims[0]?.field || '区域';
    cells.push({
      row: 2,
      col: 1,
      text: rowDimLabel,
      bold: true,
      font_size: 13,
      h_align: 'center',
      back_color: '#efe9df',
      border: { bottom: true, top: true },
    });

    const colCategories = ['办公用品', '数码配件', '家居清洁'];
    colCategories.forEach((cat, idx) => {
      cells.push({
        row: 2,
        col: 2 + idx,
        text: cat,
        bold: true,
        font_size: 13,
        h_align: 'center',
        back_color: '#efe9df',
        border: { bottom: true, top: true },
      });
    });

    if (spec.totals?.subtotal) {
      cells.push({
        row: 2,
        col: 5,
        text: '小计',
        bold: true,
        font_size: 13,
        h_align: 'center',
        back_color: '#efe9df',
        border: { bottom: true, top: true },
      });
    }

    // Row 3: Detail binding expression
    cells.push({
      row: 3,
      col: 1,
      expr: `d.${rowDims[0]?.field || 'region'}`,
      bold: true,
      font_size: 12,
      h_align: 'left',
      border: { bottom: true },
    });

    colCategories.forEach((_, idx) => {
      cells.push({
        row: 3,
        col: 2 + idx,
        expr: `sum(d.${measures[0]?.field || 'amount'})`,
        font_size: 12,
        h_align: 'right',
        border: { bottom: true },
      });
    });

    if (spec.totals?.subtotal) {
      cells.push({
        row: 3,
        col: 5,
        formula: '=SUM(B3:D3)',
        bold: true,
        font_size: 12,
        h_align: 'right',
        border: { bottom: true },
      });
    }

    // Row 4: Grand total
    if (spec.totals?.grand) {
      cells.push({
        row: 4,
        col: 1,
        text: '总计',
        bold: true,
        font_size: 13,
        h_align: 'center',
        back_color: '#fffcf7',
        border: { bottom: true, top: true },
      });
      colCategories.forEach((_, idx) => {
        cells.push({
          row: 4,
          col: 2 + idx,
          formula: `=SUM(R3C${2 + idx}:R3C${2 + idx})`,
          bold: true,
          font_size: 12,
          h_align: 'right',
          border: { bottom: true, top: true },
        });
      });
      if (spec.totals?.subtotal) {
        cells.push({
          row: 4,
          col: 5,
          formula: '=SUM(E3:E3)',
          bold: true,
          font_size: 12,
          h_align: 'right',
          border: { bottom: true, top: true },
        });
      }
      totalRows = 4;
    } else {
      totalRows = 3;
    }
    totalCols = spec.totals?.subtotal ? 5 : 4;
  } else {
    // Grouped / List layout
    const allHeaders = [
      ...rowDims.map((d) => ({ text: d.label || d.field, isMeasure: false, field: d.field })),
      ...measures.map((m) => ({ text: `${m.label || m.field} (${m.agg})`, isMeasure: true, field: m.field })),
    ];
    totalCols = Math.max(1, allHeaders.length);

    // Title
    cells.push({
      row: 1,
      col: 1,
      text: spec.shape === 'rank' ? '业绩排行榜' : '销售业务数据报表',
      bold: true,
      font_size: 16,
      h_align: 'center',
      v_align: 'middle',
      back_color: '#f6e8d7',
      merge: { rows: 1, cols: totalCols },
    });

    // Headers
    allHeaders.forEach((h, idx) => {
      cells.push({
        row: 2,
        col: 1 + idx,
        text: h.text,
        bold: true,
        font_size: 13,
        h_align: h.isMeasure ? 'right' : 'left',
        back_color: '#efe9df',
        border: { bottom: true, top: true },
      });
    });

    // Bindings
    allHeaders.forEach((h, idx) => {
      cells.push({
        row: 3,
        col: 1 + idx,
        expr: h.isMeasure ? `sum(d.${h.field})` : `d.${h.field}`,
        bold: !h.isMeasure,
        font_size: 12,
        h_align: h.isMeasure ? 'right' : 'left',
        border: { bottom: true },
      });
    });

    if (spec.totals?.grand) {
      cells.push({
        row: 4,
        col: 1,
        text: '总计',
        bold: true,
        font_size: 13,
        h_align: 'center',
        back_color: '#fffcf7',
        border: { bottom: true, top: true },
      });
      allHeaders.slice(1).forEach((h, idx) => {
        cells.push({
          row: 4,
          col: 2 + idx,
          formula: h.isMeasure ? `=SUM(R3C${2 + idx})` : '—',
          bold: true,
          font_size: 12,
          h_align: h.isMeasure ? 'right' : 'center',
          border: { bottom: true, top: true },
        });
      });
      totalRows = 4;
    }
  }

  return {
    rev: rev + 1,
    grid: { rows: totalRows, cols: totalCols },
    cells,
    geometry: { rows: totalRows, cols: totalCols },
    lint: { summary: { total: 0 } },
  };
}

export function executeReport(req: Requirement, rawCsvOverride?: string): Run {
  const parsed = parseCsv(rawCsvOverride || RAW_SAMPLE_CSV);
  const rows = parsed.allRows;
  const spec = req.spec || {
    layout: 'grouped',
    shape: 'grouped',
    row_dims: [{ field: 'region', label: '区域', sort: 'asc' }],
    col_dims: [],
    measures: [{ field: 'amount', agg: 'sum', label: '销售额' }],
    totals: { grand: true, subtotal: false },
  };

  const startTime = Date.now();
  let headers: string[] = [];
  let computedRows: any[][] = [];
  let summaryRow: any[] | undefined = undefined;

  const shape = spec.shape || (spec.layout === 'crosstab' ? 'crosstab' : 'grouped');

  if (shape === 'crosstab') {
    const rowDim = spec.row_dims[0]?.field || 'region';
    const colDim = spec.col_dims[0]?.field || 'cat';
    const msrField = spec.measures[0]?.field || 'amount';

    const rowValSet = new Set<string>();
    const colValSet = new Set<string>();
    rows.forEach((r) => {
      if (r[rowDim]) rowValSet.add(String(r[rowDim]));
      if (r[colDim]) colValSet.add(String(r[colDim]));
    });

    const rowVals = Array.from(rowValSet);
    const colVals = Array.from(colValSet);

    headers = [spec.row_dims[0]?.label || rowDim, ...colVals];
    if (spec.totals?.subtotal) headers.push('小计');

    const matrix: Record<string, Record<string, number>> = {};
    rows.forEach((r) => {
      const rv = String(r[rowDim]);
      const cv = String(r[colDim]);
      const val = Number(r[msrField]) || 0;
      if (!matrix[rv]) matrix[rv] = {};
      matrix[rv][cv] = (matrix[rv][cv] || 0) + val;
    });

    rowVals.forEach((rv) => {
      const rowItem: any[] = [rv];
      let subtotal = 0;
      colVals.forEach((cv) => {
        const amt = matrix[rv]?.[cv] || 0;
        rowItem.push(amt);
        subtotal += amt;
      });
      if (spec.totals?.subtotal) rowItem.push(subtotal);
      computedRows.push(rowItem);
    });

    if (spec.totals?.grand) {
      summaryRow = ['总计'];
      colVals.forEach((cv) => {
        let total = 0;
        rowVals.forEach((rv) => {
          total += matrix[rv]?.[cv] || 0;
        });
        summaryRow!.push(total);
      });
      if (spec.totals?.subtotal) {
        const grandGrand = summaryRow.slice(1).reduce((acc, v) => acc + (Number(v) || 0), 0);
        summaryRow.push(grandGrand);
      }
    }
  } else if (shape === 'detail') {
    headers = req.dataset.columns;
    computedRows = rows.slice(0, 50).map((r) => headers.map((h) => r[h]));
  } else {
    // Grouped or Rank or Trend
    const dimKeys = (spec.row_dims || []).map((d) => d.field);
    const msr = spec.measures[0] || { field: 'amount', agg: 'sum', label: '销售额' };

    headers = [...(spec.row_dims || []).map((d) => d.label || d.field), msr.label || msr.field];

    const groupMap = new Map<string, { dims: any[]; sum: number; count: number; max: number; min: number }>();
    rows.forEach((r) => {
      const key = dimKeys.map((k) => r[k] ?? '—').join(' | ');
      const val = Number(r[msr.field]) || 0;
      if (!groupMap.has(key)) {
        groupMap.set(key, {
          dims: dimKeys.map((k) => r[k] ?? '—'),
          sum: val,
          count: 1,
          max: val,
          min: val,
        });
      } else {
        const item = groupMap.get(key)!;
        item.sum += val;
        item.count += 1;
        item.max = Math.max(item.max, val);
        item.min = Math.min(item.min, val);
      }
    });

    groupMap.forEach((v) => {
      let finalVal = v.sum;
      if (msr.agg === 'avg') finalVal = Math.round(v.sum / v.count);
      if (msr.agg === 'count') finalVal = v.count;
      if (msr.agg === 'max') finalVal = v.max;
      if (msr.agg === 'min') finalVal = v.min;
      computedRows.push([...v.dims, finalVal]);
    });

    // Sorting
    const firstDimSort = spec.row_dims[0]?.sort;
    if (shape === 'rank') {
      computedRows.sort((a, b) => (Number(b[b.length - 1]) || 0) - (Number(a[a.length - 1]) || 0));
    } else if (firstDimSort === 'asc') {
      computedRows.sort((a, b) => String(a[0]).localeCompare(String(b[0])));
    } else if (firstDimSort === 'desc') {
      computedRows.sort((a, b) => String(b[0]).localeCompare(String(a[0])));
    }

    if (spec.totals?.grand) {
      summaryRow = ['总计', ...Array(dimKeys.length - 1).fill('')];
      const grandSum = computedRows.reduce((acc, row) => acc + (Number(row[row.length - 1]) || 0), 0);
      summaryRow.push(grandSum);
    }
  }

  const duration_ms = Math.max(45, Math.min(280, Date.now() - startTime + Math.floor(Math.random() * 80)));
  const runId = `run-${Date.now().toString(36)}`;
  const rev = req.template?.rev || 1;

  // Build CSV string
  const csvContent = [headers.join(','), ...computedRows.map((r) => r.join(',')), ...(summaryRow ? [summaryRow.join(',')] : [])].join('\n');

  // Build HTML report
  const tableHtml = `
    <table class="report-table" style="width:100%;border-collapse:collapse;font-family:sans-serif;font-size:13px">
      <thead>
        <tr style="background:#efe9df">
          ${headers.map((h) => `<th style="border:1px solid #d3c7b5;padding:8px 12px;text-align:left">${h}</th>`).join('')}
        </tr>
      </thead>
      <tbody>
        ${computedRows
          .map(
            (row) => `
          <tr>
            ${row
              .map(
                (v, i) => `
              <td style="border:1px solid #e5dccf;padding:7px 12px;${typeof v === 'number' ? 'text-align:right' : 'text-align:left'}">
                ${typeof v === 'number' ? v.toLocaleString('zh-CN') : v}
              </td>`
              )
              .join('')}
          </tr>`
          )
          .join('')}
        ${
          summaryRow
            ? `
          <tr style="background:#f5f1ea;font-weight:bold;border-top:2px solid #475467">
            ${summaryRow
              .map(
                (v) => `
              <td style="border:1px solid #d3c7b5;padding:8px 12px;${typeof v === 'number' ? 'text-align:right' : 'text-align:left'}">
                ${typeof v === 'number' ? v.toLocaleString('zh-CN') : v}
              </td>`
              )
              .join('')}
          </tr>`
            : ''
        }
      </tbody>
    </table>`;

  const htmlDoc = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8" />
  <title>${req.title} · 报表结果</title>
  <style>
    body { font-family: "Noto Sans SC", sans-serif; background: #fffcf7; color: #191512; padding: 24px; }
    h1 { font-family: "Noto Serif SC", serif; font-size: 20px; margin: 0 0 16px; border-bottom: 2px solid #b45309; padding-bottom: 8px; }
    .meta { font-size: 12px; color: #7b6e60; margin-bottom: 20px; }
  </style>
</head>
<body>
  <h1>${req.title}</h1>
  <div class="meta">模板 rev ${rev} · 运行 ID: ${runId} · 生成时间: ${new Date().toLocaleString('zh-CN')}</div>
  ${tableHtml}
</body>
</html>`;

  // Build Univer snapshot according to docs/spec-m4.md §4
  const cellData: Record<string, Record<string, UniverCellData>> = {};

  // Row 0: Headers
  cellData['0'] = {};
  headers.forEach((h, cIdx) => {
    cellData['0'][String(cIdx)] = {
      v: h,
      t: 1,
      s: {
        bl: 1,
        ff: 'Segoe UI, -apple-system, sans-serif',
        fs: 11,
        cl: { rgb: '#191512' },
        bg: { rgb: '#efe9df' },
        ht: 2,
        vt: 2,
        bd: {
          b: { s: 1, cl: { rgb: '#d3c7b5' } },
          t: { s: 1, cl: { rgb: '#d3c7b5' } },
          l: { s: 1, cl: { rgb: '#d3c7b5' } },
          r: { s: 1, cl: { rgb: '#d3c7b5' } },
        },
      },
    };
  });

  // Data rows
  computedRows.forEach((r, rIdx) => {
    const rowNum = String(rIdx + 1);
    cellData[rowNum] = {};
    r.forEach((val, cIdx) => {
      const isNum = typeof val === 'number';
      cellData[rowNum][String(cIdx)] = {
        v: val,
        t: isNum ? 2 : 1,
        s: {
          ff: 'Segoe UI, -apple-system, sans-serif',
          fs: 11,
          cl: { rgb: '#2c251e' },
          bg: { rgb: rIdx % 2 === 1 ? '#fbf8f4' : '#ffffff' },
          ht: isNum ? 3 : 1,
          vt: 2,
          bd: {
            b: { s: 1, cl: { rgb: '#ede7df' } },
            t: { s: 1, cl: { rgb: '#ede7df' } },
            l: { s: 1, cl: { rgb: '#ede7df' } },
            r: { s: 1, cl: { rgb: '#ede7df' } },
          },
        },
      };
    });
  });

  // Summary row
  if (summaryRow) {
    const sumRowNum = String(computedRows.length + 1);
    cellData[sumRowNum] = {};
    summaryRow.forEach((val, cIdx) => {
      const isNum = typeof val === 'number';
      const colLetter = String.fromCharCode(65 + cIdx);
      cellData[sumRowNum][String(cIdx)] = {
        v: val,
        t: isNum ? 2 : 1,
        f: isNum && cIdx > 0 ? `=SUM(${colLetter}2:${colLetter}${computedRows.length + 1})` : undefined,
        s: {
          bl: 1,
          ff: 'Segoe UI, -apple-system, sans-serif',
          fs: 11,
          cl: { rgb: '#191512' },
          bg: { rgb: '#f5f0e6' },
          ht: isNum ? 3 : 2,
          vt: 2,
          bd: {
            t: { s: 2, cl: { rgb: '#b45309' } },
            b: { s: 2, cl: { rgb: '#b45309' } },
            l: { s: 1, cl: { rgb: '#d3c7b5' } },
            r: { s: 1, cl: { rgb: '#d3c7b5' } },
          },
        },
      };
    });
  }

  const columnData: Record<string, { w?: number }> = {};
  headers.forEach((h, cIdx) => {
    const maxLen = Math.max(h.length * 15, ...computedRows.map((r) => String(r[cIdx] || '').length * 11));
    columnData[String(cIdx)] = { w: Math.max(110, Math.min(260, maxLen + 36)) };
  });

  const rowData: Record<string, { h?: number }> = {
    '0': { h: 36 },
  };
  computedRows.forEach((_, rIdx) => {
    rowData[String(rIdx + 1)] = { h: 28 };
  });
  if (summaryRow) {
    rowData[String(computedRows.length + 1)] = { h: 32 };
  }

  const univerSnapshot: IWorkbookData = {
    id: `rpx-${req.id || 'report'}`,
    name: req.title || '报表输出工作簿',
    appVersion: '1.2.0',
    locale: 'zhCN',
    sheetOrder: ['rpx-sheet-1'],
    sheets: {
      'rpx-sheet-1': {
        id: 'rpx-sheet-1',
        name: 'Sheet1',
        rowCount: Math.max(computedRows.length + (summaryRow ? 10 : 9), 100),
        columnCount: Math.max(headers.length + 4, 26),
        defaultRowHeight: 28,
        defaultColumnWidth: 110,
        showGridlines: 1,
        freeze: {
          xSplit: 0,
          ySplit: 1,
          startRow: 1,
          startColumn: 0,
        },
        cellData,
        rowData,
        columnData,
      },
    },
  };

  const snapshotJson = JSON.stringify(univerSnapshot, null, 2);

  return {
    run_id: runId,
    req_id: req.id,
    req_title: req.title,
    status: 'succeeded',
    template_rev: rev,
    duration_ms,
    shape: { rows: computedRows.length + (summaryRow ? 1 : 0), cols: headers.length },
    finished_at: new Date().toISOString(),
    at: new Date().toISOString(),
    engine_version: 'rpx-engine v0.9.4-rust',
    layout: { page_count: Math.ceil(computedRows.length / 25) },
    parameters: {},
    inputs: [
      {
        kind: req.dataset.kind,
        dataset: req.dataset.file || req.dataset.dsn,
        sha256: '9a84f3e218c89b70b4a1120',
      },
    ],
    artifacts: {
      'report.csv': csvContent,
      'report.xlsx': 'Binary XLSX stream',
      'report.html': htmlDoc,
      'report-print.xlsx': 'Binary Print-Ready XLSX stream',
      'snapshot.json': snapshotJson,
    },
    univerSnapshot,
    computedData: {
      headers,
      rows: computedRows,
      summaryRow,
    },
  };
}

export function runAttributionAnalysis(
  dsn: string,
  metric: string = 'amount',
  timeField: string = 'month',
  grain: string = 'month',
  asOf: string = ''
): AnalysisDoc {
  return {
    status: 'ok',
    summary: '2026 年 9 月整体销售额环比下降 48,500 元 (-7.4%)，主要由办公用品品类及华东区上海一店业绩缩减驱动',
    totals: {
      cur: 605200,
      prev: 653700,
      delta: -48500,
    },
    coverage: 0.942,
    windows: {
      cur_start: '2026-09-01',
      cur_end: '2026-09-30',
      prev_start: '2026-08-01',
      prev_end: '2026-08-31',
    },
    findings: [
      {
        what: '整体口径：2026-09 销售总额 60.52 万元，相比上期 65.37 万元减少 4.85 万元 (-7.4%)',
        kind: 'observed',
        evidence: ['sales 表总体聚合 SUM(amount)', '同比上一完整月份'],
      },
      {
        what: '维度一钻取（按产品品类）：办公用品下降 3.65 万元，占总体降幅的 75.3%',
        kind: 'computed',
        evidence: ['cat = "办公用品" 的 delta / 总体 delta'],
        share: 0.753,
      },
      {
        what: '维度二下钻（办公用品细分到大区）：华东区贡献了办公用品降幅中的 68.2%',
        kind: 'computed',
        evidence: ['region = "华东" AND cat = "办公用品"'],
        share: 0.682,
      },
      {
        what: '正向对冲：数码配件在华南深圳南山店逆势上涨 7,000 元 (+8.5%)，部分抵消了下滑',
        kind: 'observed',
        evidence: ['store = "深圳南山店" AND cat = "数码配件"'],
      },
    ],
    steps: [
      {
        title: '第 1 步：全库基准对比（总体指标）',
        kind: 'baseline',
        row_count: 1,
        ok: true,
        intent: '统计 8 月与 9 月全量销售额及变动差额',
        sql: `SELECT
    SUM(CASE WHEN month = '2026-09' THEN amount ELSE 0 END) AS m0_cur,
    SUM(CASE WHEN month = '2026-08' THEN amount ELSE 0 END) AS m0_prev,
    SUM(CASE WHEN month = '2026-09' THEN amount ELSE 0 END) -
    SUM(CASE WHEN month = '2026-08' THEN amount ELSE 0 END) AS m0_delta
FROM sales
WHERE month IN ('2026-08', '2026-09');`,
      },
      {
        title: '第 2 步：按品类（cat）第一层拆解',
        kind: 'dim_breakdown',
        row_count: 3,
        ok: true,
        intent: '寻找导致整体下降的核心品类',
        sql: `SELECT
    cat,
    SUM(CASE WHEN month = '2026-09' THEN amount ELSE 0 END) AS cur,
    SUM(CASE WHEN month = '2026-08' THEN amount ELSE 0 END) AS prev,
    SUM(CASE WHEN month = '2026-09' THEN amount ELSE 0 END) -
    SUM(CASE WHEN month = '2026-08' THEN amount ELSE 0 END) AS delta
FROM sales
WHERE month IN ('2026-08', '2026-09')
GROUP BY cat
ORDER BY delta ASC;`,
      },
      {
        title: '第 3 步：对最大负贡献品类按大区（region）深度下钻',
        kind: 'drill_down',
        row_count: 3,
        ok: true,
        intent: '聚焦办公用品品类在各区域的表现',
        sql: `SELECT
    region,
    SUM(CASE WHEN month = '2026-09' THEN amount ELSE 0 END) AS cur,
    SUM(CASE WHEN month = '2026-08' THEN amount ELSE 0 END) AS prev,
    SUM(CASE WHEN month = '2026-09' THEN amount ELSE 0 END) -
    SUM(CASE WHEN month = '2026-08' THEN amount ELSE 0 END) AS delta
FROM sales
WHERE month IN ('2026-08', '2026-09')
  AND cat = '办公用品'
GROUP BY region
ORDER BY delta ASC;`,
      },
    ],
    caveats: [
      '算术分解 ≠ 真实业务因果：此分析基于两期数据的代数差额分解，未纳入供应链与节假日促销变量',
      '时间窗口严格按自然月完整周期对齐，规避半月对比的虚假波动',
      '对比数据覆盖度为 94.2%，少数未打标签的补丁订单已自动排除',
    ],
  };
}

export function generateDiff(
  vA: SemanticVersion,
  vB: SemanticVersion
): { a: number; b: number; semantic_changed: boolean; changes: any[] } {
  const hashA = vA.semantic?.hash;
  const hashB = vB.semantic?.hash;
  const semantic_changed = hashA !== hashB;

  const changes: any[] = [];
  if (vA.shape !== vB.shape) {
    changes.push({
      level: 'layout',
      what: '报表形态切换',
      before: vA.shape,
      after: vB.shape,
    });
  }
  if (vA.template_rev !== vB.template_rev) {
    changes.push({
      level: 'visual',
      what: '模板修订版本 (rev)',
      before: `rev ${vA.template_rev}`,
      after: `rev ${vB.template_rev}`,
    });
  }
  if (semantic_changed) {
    changes.push({
      level: 'semantic',
      what: '度量口径指纹变更',
      before: hashA,
      after: hashB,
    });
  } else {
    changes.push({
      level: 'visual',
      what: '口径保持一致，仅调整样式/网格边框',
      before: '初版样式',
      after: '微调高亮与网格',
    });
  }

  return {
    a: vA.version,
    b: vB.version,
    semantic_changed,
    changes,
  };
}
