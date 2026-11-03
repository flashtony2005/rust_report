import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import {
  listDatasources,
  upsertDatasource,
  deleteDatasource,
  probeDatasource,
  probeRecord,
  listTables,
  listColumns,
} from './dev-server/datasource-store.mjs';
import {
  saveOntologyTrace,
  listOntologyTraces,
  getOntologyTrace,
  deleteOntologyTrace,
  buildOntologyGraph,
  buildLexicon,
  autoParse,
  matchCanon,
  exportOntologyMd,
} from './dev-server/ontology-store.mjs';
import { buildReportFromText, loadGeneratedReports } from './dev-server/report-build.mjs';

function readBody(req: http.IncomingMessage): Promise<string> {
  return new Promise((resolve) => {
    let s = '';
    req.on('data', (c) => {
      s += c;
    });
    req.on('end', () => resolve(s));
  });
}

const initialMockReports: Record<string, any> = {
  customer_ltv_overdue: {
    id: 'customer_ltv_overdue',
    revision: 'rev_02_prod',
    manifest: {
      id: 'customer_ltv_overdue',
      name: '客户价值与逾期率全景表',
      description: '按客户等级维度的 LTV 与逾期率统计（分头聚合防翻车）',
      tags: ['核心报表', '风险控制', '业务知识模型'],
      version: '1.0.0',
    },
    template: {
      cells: {
        "0,0": { v: "客户等级" },
        "0,1": { v: "客户数" },
        "0,2": { v: "平均LTV(元)" },
        "0,3": { v: "逾期率(%)" },
        "1,0": { v: "钻石" },
        "1,1": { v: 12 },
        "1,2": { v: 48200 },
        "1,3": { v: 0.8 },
        "2,0": { v: "白金" },
        "2,1": { v: 45 },
        "2,2": { v: 19500 },
        "2,3": { v: 1.5 },
        "3,0": { v: "黄金" },
        "3,1": { v: 120 },
        "3,2": { v: 6200 },
        "3,3": { v: 3.2 },
      },
    },
    result_state: 'current_revision',
    display_run_id: 'run_20261030_001',
  },
  sales_region_quarter: {
    id: 'sales_region_quarter',
    revision: 'rev_01',
    manifest: {
      id: 'sales_region_quarter',
      name: '大区季度销售业绩对比',
      description: '华东/华北/华南跨期汇总与同比分析',
      tags: ['销售', '汇总'],
      version: '1.0.0',
    },
    template: {
      cells: {
        "0,0": { v: "大区" },
        "0,1": { v: "产品品类" },
        "0,2": { v: "7月销售额" },
        "0,3": { v: "8月销售额" },
        "0,4": { v: "9月销售额" },
        "0,5": { v: "季度总计" },
        "1,0": { v: "华东" },
        "1,1": { v: "办公用品" },
        "1,2": { v: 89500 },
        "1,3": { v: 119700 },
        "1,4": { v: 125700 },
        "1,5": { v: 334900 },
        "2,0": { v: "华东" },
        "2,1": { v: "数码配件" },
        "2,2": { v: 114000 },
        "2,3": { v: 67000 },
        "2,4": { v: 112200 },
        "2,5": { v: 293200 },
      },
    },
    result_state: 'current_revision',
    display_run_id: 'run_20261030_002',
  },
};

const initialMockRuns: any[] = [
  {
    run_id: 'run_20261030_001',
    report_id: 'customer_ltv_overdue',
    report_name: '客户价值与逾期率全景表',
    revision: 'rev_02_prod',
    status: 'succeeded',
    trigger: 'manual',
    started_at: '2026-10-30T10:46:00Z',
    finished_at: '2026-10-30T10:46:12Z',
    duration_ms: 12000,
    layout: { page_count: 2, rows: 45, cols: 7 },
    artifacts: {
      'workbook.json': 'artifacts/workbook.json',
      'result.json': 'artifacts/result.json',
      'report.pdf': 'artifacts/report.pdf',
      'report.xlsx': 'artifacts/report.xlsx',
    },
    error_code: null,
    error_message: null,
    artifact_count: 4,
    artifact_bytes: 94500,
  },
  {
    run_id: 'run_20261030_002',
    report_id: 'sales_region_quarter',
    report_name: '大区季度销售业绩对比',
    revision: 'rev_01',
    status: 'succeeded',
    trigger: 'schedule',
    started_at: '2026-10-30T09:15:00Z',
    finished_at: '2026-10-30T09:15:02Z',
    duration_ms: 2000,
    layout: { page_count: 1, rows: 18, cols: 5 },
    artifacts: {
      'workbook.json': 'artifacts/workbook.json',
      'result.json': 'artifacts/result.json',
      'report.pdf': 'artifacts/report.pdf',
      'report.xlsx': 'artifacts/report.xlsx',
    },
    error_code: null,
    error_message: null,
    artifact_count: 4,
    artifact_bytes: 79200,
  },
];

function escapeXml(str: string): string {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function extractTableRows(template: any): (string | number)[][] {
  const cells = template?.cells || {};
  let maxR = 0;
  let maxC = 0;
  for (const k of Object.keys(cells)) {
    const p = k.split(',').map(Number);
    if (!isNaN(p[0]) && p[0] > maxR) maxR = p[0];
    if (!isNaN(p[1]) && p[1] > maxC) maxC = p[1];
  }
  const rows: (string | number)[][] = [];
  for (let r = 0; r <= maxR; r++) {
    const row: (string | number)[] = [];
    for (let c = 0; c <= maxC; c++) {
      row.push(cells[`${r},${c}`]?.v ?? '');
    }
    rows.push(row);
  }
  return rows.length > 0 ? rows : [
    ['项目', '销售额(元)', '综合毛利率'],
    ['核心业务', 1280000, '36.5%'],
    ['拓展业务', 776200, '38.2%'],
    ['全品类总计', 2056200, '37.1%'],
  ];
}

function encodePdfUtf16(str: string): string {
  let hex = '';
  for (let i = 0; i < str.length; i++) {
    hex += str.charCodeAt(i).toString(16).padStart(4, '0');
  }
  return '<' + hex + '>';
}

function createPdfBuffer(
  title: string,
  rows: (string | number)[][],
  meta?: { runId?: string; generatedAt?: string }
): Buffer {
  const objects: string[] = [];
  function addObj(content: string): number {
    objects.push(content);
    return objects.length;
  }
  addObj('<< /Type /Catalog /Pages 2 0 R >>');
  addObj('<< /Type /Pages /Kids [3 0 R] /Count 1 >>');
  addObj('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] /Resources << /Font << /F1 4 0 R /F2 5 0 R >> >> /Contents 6 0 R >>');
  addObj('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
  addObj('<< /Type /Font /Subtype /Type0 /BaseFont /STSong-Light /Encoding /UniGB-UTF16-H /DescendantFonts [<< /Type /Font /Subtype /CIDFontType0 /BaseFont /STSong-Light /CIDSystemInfo << /Registry (Adobe) /Ordering (GB1) /Supplement 4 >> /FontDescriptor << /Type /FontDescriptor /FontName /STSong-Light /Flags 4 /FontBBox [-25 -254 1000 880] /ItalicAngle 0 /Ascent 880 /Descent -254 /CapHeight 880 /StemV 70 >> >>] >>');

  let stream = '';
  // Top Title Banner
  stream += 'q 0.12 0.18 0.26 rg 40 758 515 52 re f Q\n';
  stream += 'BT /F2 15 Tf 1 1 1 rg 55 788 Td ' + encodePdfUtf16(title) + ' Tj ET\n';
  const metaText = `Report Studio Engine | Run: ${meta?.runId || 'run_latest'} | ${meta?.generatedAt || new Date().toISOString().slice(0, 19).replace('T', ' ')}`;
  stream += 'BT /F1 8.5 Tf 0.85 0.9 0.95 rg 55 770 Td (' + metaText.replace(/[()]/g, '') + ') Tj ET\n';

  // Table header bar
  stream += 'q 0.17 0.27 0.34 rg 40 720 515 24 re f Q\n';

  const colCount = Math.max(1, ...rows.map((r) => r.length));
  const colWidth = Math.floor(505 / colCount);

  let y = 728;
  if (rows.length > 0) {
    const headerRow = rows[0];
    let x = 50;
    for (let c = 0; c < headerRow.length; c++) {
      stream += 'BT /F2 9.5 Tf 1 1 1 rg ' + x + ' ' + y + ' Td ' + encodePdfUtf16(String(headerRow[c] ?? '')) + ' Tj ET\n';
      x += colWidth;
    }
  }

  y = 696;
  for (let r = 1; r < rows.length; r++) {
    const row = rows[r];
    if (r % 2 === 1) {
      stream += 'q 0.97 0.98 0.99 rg 40 ' + (y - 6) + ' 515 22 re f Q\n';
    }
    stream += 'q 0.88 0.90 0.92 RG 0.5 w 40 ' + (y - 6) + ' m 555 ' + (y - 6) + ' l S Q\n';
    let x = 50;
    for (let c = 0; c < row.length; c++) {
      const cellVal = String(row[c] ?? '');
      const isTotal = String(row[0] || '').includes('合计') || String(row[0] || '').includes('总计');
      const color = isTotal ? '0.1 0.3 0.5' : '0.15 0.18 0.2';
      stream += 'BT /F2 9 Tf ' + color + ' rg ' + x + ' ' + y + ' Td ' + encodePdfUtf16(cellVal) + ' Tj ET\n';
      x += colWidth;
    }
    y -= 22;
  }
  stream += 'BT /F1 8 Tf 0.55 0.6 0.65 rg 40 50 Td (Generated by Report Studio | Strict Semantic Caliber Verified) Tj ET\n';

  const streamLen = Buffer.byteLength(stream);
  addObj('<< /Length ' + streamLen + ' >>\nstream\n' + stream + '\nendstream');

  let pdf = '%PDF-1.4\n%\\xE2\\xE3\\xCF\\xD3\n';
  const xref: number[] = [0];
  for (let i = 0; i < objects.length; i++) {
    xref.push(pdf.length);
    pdf += (i + 1) + ' 0 obj\n' + objects[i] + '\nendobj\n';
  }
  const startxref = pdf.length;
  pdf += 'xref\n0 ' + (objects.length + 1) + '\n0000000000 65535 f \n';
  for (let i = 1; i <= objects.length; i++) {
    pdf += String(xref[i]).padStart(10, '0') + ' 00000 n \n';
  }
  pdf += 'trailer\n<< /Size ' + (objects.length + 1) + ' /Root 1 0 R >>\nstartxref\n' + startxref + '\n%%EOF\n';
  return Buffer.from(pdf, 'binary');
}

function createExcelXml(title: string, rows: (string | number)[][]): string {
  const colCount = Math.max(1, ...rows.map((r) => r.length));
  let xml = '<?xml version="1.0" encoding="UTF-8"?>\n';
  xml += '<?mso-application progid="Excel.Sheet"?>\n';
  xml += '<Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet"\n';
  xml += ' xmlns:o="urn:schemas-microsoft-com:office:office"\n';
  xml += ' xmlns:x="urn:schemas-microsoft-com:office:excel"\n';
  xml += ' xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet">\n';
  xml += ' <Styles>\n';
  xml += '  <Style ss:ID="Title">\n';
  xml += '   <Font ss:FontName="Microsoft YaHei" ss:Size="14" ss:Bold="1" ss:Color="#FFFFFF"/>\n';
  xml += '   <Interior ss:Color="#1E293B" ss:Pattern="Solid"/>\n';
  xml += '   <Alignment ss:Horizontal="Center" ss:Vertical="Center"/>\n';
  xml += '  </Style>\n';
  xml += '  <Style ss:ID="Header">\n';
  xml += '   <Font ss:FontName="Microsoft YaHei" ss:Bold="1" ss:Color="#FFFFFF"/>\n';
  xml += '   <Interior ss:Color="#2B4657" ss:Pattern="Solid"/>\n';
  xml += '   <Alignment ss:Horizontal="Center" ss:Vertical="Center"/>\n';
  xml += '  </Style>\n';
  xml += '  <Style ss:ID="Data">\n';
  xml += '   <Font ss:FontName="Microsoft YaHei" ss:Color="#191512"/>\n';
  xml += '   <Alignment ss:Vertical="Center"/>\n';
  xml += '  </Style>\n';
  xml += '  <Style ss:ID="DataNum">\n';
  xml += '   <Font ss:FontName="Microsoft YaHei" ss:Color="#191512"/>\n';
  xml += '   <Alignment ss:Horizontal="Right" ss:Vertical="Center"/>\n';
  xml += '  </Style>\n';
  xml += '  <Style ss:ID="Total">\n';
  xml += '   <Font ss:FontName="Microsoft YaHei" ss:Bold="1" ss:Color="#0F172A"/>\n';
  xml += '   <Interior ss:Color="#E2E8F0" ss:Pattern="Solid"/>\n';
  xml += '   <Alignment ss:Vertical="Center"/>\n';
  xml += '  </Style>\n';
  xml += ' </Styles>\n';
  xml += ' <Worksheet ss:Name="Sheet1">\n';
  xml += '  <Table>\n';
  xml += '   <Row ss:Height="34">\n';
  xml += `    <Cell ss:MergeAcross="${colCount - 1}" ss:StyleID="Title"><Data ss:Type="String">${escapeXml(title)}</Data></Cell>\n`;
  xml += '   </Row>\n';
  for (let r = 0; r < rows.length; r++) {
    const isHeader = r === 0;
    const isTotal = !isHeader && (String(rows[r][0] || '').includes('合计') || String(rows[r][0] || '').includes('总计'));
    xml += '   <Row ss:AutoFitHeight="1">\n';
    for (let c = 0; c < rows[r].length; c++) {
      const val = String(rows[r][c] ?? '');
      const num = Number(val.replace(/[,%]/g, ''));
      const isNum = !isNaN(num) && val.trim() !== '' && !val.includes('-');
      const style = isHeader ? 'Header' : isTotal ? 'Total' : isNum ? 'DataNum' : 'Data';
      const type = isNum && !val.includes('%') ? 'Number' : 'String';
      const cellVal = isNum && !val.includes('%') ? num : escapeXml(val);
      xml += `    <Cell ss:StyleID="${style}"><Data ss:Type="${type}">${cellVal}</Data></Cell>\n`;
    }
    xml += '   </Row>\n';
  }
  xml += '  </Table>\n';
  xml += ' </Worksheet>\n';
  xml += '</Workbook>\n';
  return xml;
}

function createWorkbookSnapshot(title: string, rows: (string | number)[][]): any {
  const cellData: Record<string, Record<string, any>> = {};
  const colCount = Math.max(4, ...rows.map((r) => r.length));

  // Row 0: Merged Report Title
  cellData['0'] = {};
  cellData['0']['0'] = {
    v: title,
    t: 1,
    s: {
      bl: 1,
      fs: 14,
      cl: { rgb: '#FFFFFF' },
      bg: { rgb: '#1E293B' },
      ht: 2,
      vt: 2,
    },
  };
  for (let c = 1; c < colCount; c++) {
    cellData['0'][String(c)] = { s: { bg: { rgb: '#1E293B' } } };
  }

  // Row 1 to N: Data rows
  for (let r = 0; r < rows.length; r++) {
    const rowIdx = String(r + 1);
    cellData[rowIdx] = {};
    const isHeader = r === 0;
    const isTotal = !isHeader && (String(rows[r][0] || '').includes('合计') || String(rows[r][0] || '').includes('总计'));

    for (let c = 0; c < rows[r].length; c++) {
      const val = rows[r][c];
      const isNum = typeof val === 'number' || (!isNaN(Number(String(val).replace(/[,%]/g, ''))) && String(val).trim() !== '' && !String(val).includes('-'));
      cellData[rowIdx][String(c)] = {
        v: val,
        t: isNum ? 2 : 1,
        s: {
          bl: isHeader || isTotal ? 1 : 0,
          cl: { rgb: isHeader ? '#FFFFFF' : isTotal ? '#0F172A' : '#191512' },
          bg: { rgb: isHeader ? '#2B4657' : isTotal ? '#E2E8F0' : r % 2 === 1 ? '#F8FAFC' : '#FFFFFF' },
          ht: isNum && !isHeader ? 3 : isHeader ? 2 : 1,
          vt: 2,
        },
      };
    }
  }

  return {
    id: 'wb_' + Date.now().toString(36),
    name: title,
    appVersion: '1.0.0',
    locale: 'zh-CN',
    sheetOrder: ['sheet_main'],
    sheets: {
      sheet_main: {
        id: 'sheet_main',
        name: title.slice(0, 16),
        rowCount: Math.max(18, rows.length + 5),
        columnCount: Math.max(8, colCount + 2),
        showGridlines: 1,
        mergeData: [
          { startRow: 0, endRow: 0, startColumn: 0, endColumn: colCount - 1 },
        ],
        cellData,
      },
    },
  };
}

function createHtmlReport(
  title: string,
  rows: (string | number)[][],
  meta?: { runId?: string; revision?: string }
): string {
  const headers = rows[0] || [];
  const bodyRows = rows.slice(1);
  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>${escapeXml(title)}</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif; padding: 32px; background: #fdfbf7; color: #191512; }
    h1 { font-size: 22px; margin-bottom: 6px; color: #1e293b; }
    .meta { font-size: 13px; color: #64748b; margin-bottom: 20px; }
    table { width: 100%; border-collapse: collapse; background: #fff; border-radius: 8px; overflow: hidden; box-shadow: 0 1px 4px rgba(0,0,0,0.08); }
    th { background: #1e293b; color: #fff; padding: 12px 16px; text-align: left; font-size: 13px; font-weight: 600; }
    td { padding: 10px 16px; border-bottom: 1px solid #e2e8f0; font-size: 13px; }
    tr:last-child td { font-weight: 600; background: #f8fafc; }
    tr:hover td { background: #f1f5f9; }
  </style>
</head>
<body>
  <h1>${escapeXml(title)}</h1>
  <div class="meta">Report Studio 报表引擎 · 运行: ${meta?.runId || 'run_latest'} · 修订: ${meta?.revision || 'rev_01'}</div>
  <table>
    <thead><tr>${headers.map((h) => `<th>${escapeXml(String(h))}</th>`).join('')}</tr></thead>
    <tbody>${bodyRows.map((r) => `<tr>${r.map((c) => `<td>${escapeXml(String(c))}</td>`).join('')}</tr>`).join('')}</tbody>
  </table>
</body>
</html>`;
}

function devApiMockPlugin(): Plugin {
  const mockReports = { ...initialMockReports };
  const mockRuns = [...initialMockRuns];
  // 磁盘上的生成报表（重启后仍在）：manifest.meta.generated 才接管
  const generated = loadGeneratedReports();
  for (const r of generated.reports) mockReports[r.id] = r;
  for (const r of generated.runs) mockRuns.unshift(r);
  (devApiMockPlugin as any).mockReports = mockReports;
  (devApiMockPlugin as any).mockRuns = mockRuns;

  return {
    name: 'dev-api-fallback',
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        if (!req.url?.startsWith('/api/')) {
          return next();
        }


        // Check if 8090 is alive
        const isUp = await new Promise<boolean>((resolve) => {
          const testReq = http.request(
            {
              hostname: '127.0.0.1',
              port: 8090,
              path: '/health',
              method: 'GET',
              timeout: 300,
            },
            (resp) => {
              resolve(resp.statusCode === 200);
            }
          );
          testReq.on('error', () => resolve(false));
          testReq.on('timeout', () => {
            testReq.destroy();
            resolve(false);
          });
          testReq.end();
        });

        if (isUp) {
          // Pass through to Vite proxy
          return next();
        }

        // Mock fallback so all UI views are previewable in sandbox
        const url = req.url;
        res.setHeader('Content-Type', 'application/json');

        if (url === '/api/health') {
          res.setHeader('Content-Type', 'text/plain');
          res.end('ok');
          return;
        }

        // Requirements creation endpoint (Agent execution pipeline)
        if (url === '/api/requirements' && req.method === 'POST') {
          const body = await readBody(req);
          let parsed: any = {};
          try {
            parsed = JSON.parse(body || '{}');
          } catch {
            parsed = {};
          }
          res.setHeader('Content-Type', 'application/json; charset=utf-8');
          try {
            // 真链路：一句话 → 标准口径 → 连库取数 → 生成报表资产（manifest/template/data/runs）
            const built = await buildReportFromText({
              dsn: parsed.dsn,
              text: parsed.text,
              title: parsed.title,
              limit: parsed.limit,
            });
            devApiMockPlugin.mockReports[built.report.id] = built.report;
            devApiMockPlugin.mockRuns.unshift(built.run);
            res.end(JSON.stringify(built.reply));
          } catch (e: any) {
            res.statusCode = 400;
            res.end(
              JSON.stringify({ error: { code: 'REQUIREMENT', message: String(e?.message || e) } })
            );
          }
          return;
        }

        if (url.startsWith('/api/reports') && req.method === 'POST' && url.includes('/run')) {
          const match = url.match(/\/api\/reports\/([^/?]+)\/run/);
          const reportId = match ? match[1] : 'unknown';
          let bodyStr = '';
          req.on('data', (c) => {
            bodyStr += c;
          });
          req.on('end', () => {
            let parsed: any = {};
            try {
              parsed = JSON.parse(bodyStr || '{}');
            } catch {
              parsed = {};
            }
            const exportsList = Array.isArray(parsed.exports) && parsed.exports.length > 0 ? parsed.exports : ['xlsx', 'pdf'];
            const newRunId = 'run_' + Date.now().toString(36);
            const artifactsMap: Record<string, string> = {
              'workbook.json': 'artifacts/workbook.json',
              'result.json': 'artifacts/result.json',
            };
            if (exportsList.includes('pdf')) artifactsMap['report.pdf'] = 'artifacts/report.pdf';
            if (exportsList.includes('xlsx')) artifactsMap['report.xlsx'] = 'artifacts/report.xlsx';
            if (exportsList.includes('csv')) artifactsMap['report.csv'] = 'artifacts/report.csv';
            if (exportsList.includes('html')) artifactsMap['report.html'] = 'artifacts/report.html';
            if (exportsList.includes('print-xlsx')) artifactsMap['report-print.xlsx'] = 'artifacts/report-print.xlsx';

            const rep = devApiMockPlugin.mockReports[reportId];
            const rows = extractTableRows(rep?.template);

            const runRecord = {
              run_id: newRunId,
              report_id: reportId,
              report_name: rep?.manifest?.name || reportId,
              revision: rep?.revision || 'rev_01',
              status: 'succeeded',
              trigger: 'manual',
              started_at: new Date(Date.now() - 1200).toISOString(),
              finished_at: new Date().toISOString(),
              duration_ms: 1200,
              layout: { page_count: 1, rows: rows.length, cols: rows[0]?.length || 4 },
              artifacts: artifactsMap,
              error_code: null,
              error_message: null,
              artifact_count: Object.keys(artifactsMap).length,
              artifact_bytes: 84000,
            };
            devApiMockPlugin.mockRuns.unshift(runRecord);
            if (devApiMockPlugin.mockReports[reportId]) {
              devApiMockPlugin.mockReports[reportId].display_run_id = newRunId;
              devApiMockPlugin.mockReports[reportId].display_run = runRecord;
              devApiMockPlugin.mockReports[reportId].latest_attempt = runRecord;
            }
            res.end(
              JSON.stringify({
                ok: true,
                run: runRecord,
              })
            );
          });
          return;
        }

        if (url.startsWith('/api/reports') && req.method === 'GET') {
          // Artifact download / inspection
          const artMatch = url.match(/\/api\/reports\/([^/?]+)\/runs\/([^/?]+)\/artifacts\/([^/?]+)$/);
          if (artMatch) {
            const [, rId, runId, artName] = artMatch;
            // 生成报表：产物优先给磁盘上的真文件（examples/library/<id>/runs/<run>/<name>）
            {
              const libRoot = path.resolve(process.cwd(), 'examples/library');
              const disk = path.resolve(
                libRoot,
                decodeURIComponent(rId),
                'runs',
                decodeURIComponent(runId),
                decodeURIComponent(artName)
              );
              if (disk.startsWith(libRoot + path.sep) && fs.existsSync(disk) && fs.statSync(disk).isFile()) {
                const ct = artName.endsWith('.json')
                  ? 'application/json; charset=utf-8'
                  : artName.endsWith('.csv')
                  ? 'text/csv; charset=utf-8'
                  : artName.endsWith('.html')
                  ? 'text/html; charset=utf-8'
                  : 'application/octet-stream';
                res.setHeader('Content-Type', ct);
                res.setHeader('Content-Disposition', `inline; filename="${artName}"`);
                fs.createReadStream(disk).pipe(res);
                return;
              }
            }
            const rep = devApiMockPlugin.mockReports[rId] || devApiMockPlugin.mockReports['customer_ltv_overdue'] || Object.values(devApiMockPlugin.mockReports)[0];
            const rows = extractTableRows(rep?.template);
            const title = rep?.manifest?.name || rId;

            if (artName.endsWith('.pdf')) {
              const pdfBuf = createPdfBuffer(title, rows, { runId, generatedAt: new Date().toISOString() });
              res.setHeader('Content-Type', 'application/pdf');
              res.setHeader('Content-Disposition', `inline; filename="${artName}"`);
              res.setHeader('Content-Length', pdfBuf.length);
              res.end(pdfBuf);
              return;
            }

            if (artName.endsWith('.xlsx')) {
              const excelXml = createExcelXml(title, rows);
              res.setHeader('Content-Type', 'application/vnd.ms-excel; charset=utf-8');
              res.setHeader('Content-Disposition', `attachment; filename="${artName}"`);
              res.end(Buffer.from(excelXml, 'utf-8'));
              return;
            }

            if (artName.endsWith('.csv')) {
              const csvContent = '\uFEFF' + rows.map((r) => r.map((c) => `"${String(c).replace(/"/g, '""')}"`).join(',')).join('\r\n');
              res.setHeader('Content-Type', 'text/csv; charset=utf-8');
              res.setHeader('Content-Disposition', `attachment; filename="${artName}"`);
              res.end(Buffer.from(csvContent, 'utf-8'));
              return;
            }

            if (artName.endsWith('.html')) {
              const html = createHtmlReport(title, rows, { runId, revision: rep?.revision || 'rev_01' });
              res.setHeader('Content-Type', 'text/html; charset=utf-8');
              res.end(html);
              return;
            }

            if (artName.includes('workbook')) {
              res.setHeader('Content-Type', 'application/json; charset=utf-8');
              res.setHeader('Content-Disposition', `inline; filename="${artName}"`);
              res.end(JSON.stringify(createWorkbookSnapshot(title, rows), null, 2));
              return;
            }

            if (artName.endsWith('.json')) {
              res.setHeader('Content-Type', 'application/json; charset=utf-8');
              res.setHeader('Content-Disposition', `inline; filename="${artName}"`);
              res.end(
                JSON.stringify(
                  {
                    ok: true,
                    report_id: rId,
                    run_id: runId,
                    status: 'succeeded',
                    rows,
                    layout: { rows: rows.length, cols: rows[0]?.length || 0 },
                    generated_at: new Date().toISOString(),
                  },
                  null,
                  2
                )
              );
              return;
            }

            res.setHeader('Content-Type', 'application/octet-stream');
            res.setHeader('Content-Disposition', `attachment; filename="${artName}"`);
            res.end(`Artifact ${artName}`);
            return;
          }

          // Single run inspection
          const singleRunMatch = url.match(/\/api\/reports\/([^/?]+)\/runs\/([^/?]+)$/);
          if (singleRunMatch) {
            const [, rId, rRunId] = singleRunMatch;
            const rep = devApiMockPlugin.mockReports[rId];
            const rows = extractTableRows(rep?.template);
            const found = devApiMockPlugin.mockRuns.find((r: any) => r.run_id === rRunId && r.report_id === rId)
              || devApiMockPlugin.mockRuns.find((r: any) => r.run_id === rRunId)
              || {
                run_id: rRunId,
                report_id: rId,
                revision: 'rev_01',
                status: 'succeeded',
                started_at: new Date(Date.now() - 1000).toISOString(),
                finished_at: new Date().toISOString(),
                duration_ms: 1000,
                layout: { page_count: 1, rows: rows.length, cols: rows[0]?.length || 4 },
                artifacts: {
                  'workbook.json': 'artifacts/workbook.json',
                  'result.json': 'artifacts/result.json',
                  'report.pdf': 'artifacts/report.pdf',
                  'report.xlsx': 'artifacts/report.xlsx',
                },
              };
            res.end(JSON.stringify(found));
            return;
          }

          // Runs list for a specific report
          const runsMatch = url.match(/\/api\/reports\/([^/?]+)\/runs(\?.*)?$/);
          if (runsMatch) {
            const reportId = runsMatch[1];
            let matching = devApiMockPlugin.mockRuns.filter((r: any) => r.report_id === reportId);
            if (matching.length === 0) {
              const rep = devApiMockPlugin.mockReports[reportId];
              const runId = rep?.display_run_id || ('run_' + Date.now().toString(36));
              const rows = extractTableRows(rep?.template);
              const autoRun = {
                run_id: runId,
                report_id: reportId,
                report_name: rep?.manifest?.name || reportId,
                revision: rep?.revision || 'rev_01',
                status: 'succeeded',
                trigger: 'agent',
                started_at: new Date(Date.now() - 1500).toISOString(),
                finished_at: new Date().toISOString(),
                duration_ms: 1500,
                layout: { page_count: 1, rows: rows.length, cols: rows[0]?.length || 4 },
                artifacts: {
                  'workbook.json': 'artifacts/workbook.json',
                  'result.json': 'artifacts/result.json',
                  'report.pdf': 'artifacts/report.pdf',
                  'report.xlsx': 'artifacts/report.xlsx',
                },
                error_code: null,
                error_message: null,
                artifact_count: 4,
                artifact_bytes: 94500,
              };
              devApiMockPlugin.mockRuns.unshift(autoRun);
              matching = [autoRun];
            }
            res.end(
              JSON.stringify({
                items: matching,
                total: matching.length,
                returned: matching.length,
                next_offset: null,
                ok: true,
              })
            );
            return;
          }

          if (url.includes('/ledger')) {
            const match = url.match(/\/api\/reports\/([^/?]+)\/ledger/);
            const id = match ? match[1] : 'customer_ltv_overdue';
            const rep = devApiMockPlugin.mockReports[id] || devApiMockPlugin.mockReports['customer_ltv_overdue'] || Object.values(devApiMockPlugin.mockReports)[0];
            const matchingRuns = devApiMockPlugin.mockRuns.filter((r: any) => r.report_id === id);
            const reportRuns = matchingRuns.length > 0 ? matchingRuns : devApiMockPlugin.mockRuns.map((r: any) => ({ ...r, report_id: id }));

            // Aggregate artifacts for all runs
            const artifacts: Array<{ run_id: string; name: string; path: string; bytes: number }> = [];
            for (const r of reportRuns) {
              const runArtifacts = r.artifacts || {
                'workbook.json': 'artifacts/workbook.json',
                'result.json': 'artifacts/result.json',
                'report.pdf': 'artifacts/report.pdf',
                'report.xlsx': 'artifacts/report.xlsx',
              };
              for (const artName of Object.keys(runArtifacts)) {
                artifacts.push({
                  run_id: r.run_id,
                  name: artName,
                  path: `artifacts/${artName}`,
                  bytes: artName.endsWith('.pdf') ? 48200 : artName.endsWith('.xlsx') ? 32100 : artName.endsWith('.json') ? 14200 : 8500,
                });
              }
            }

            res.end(
              JSON.stringify({
                ok: true,
                ledger: {
                  id,
                  name: rep.manifest?.name || id,
                  description: rep.manifest?.description || '报表台账',
                  tags: rep.manifest?.tags || ['业务报表'],
                  current_revision: rep.revision || 'rev_01',
                  template_kind: 'rpx-spec/v1',
                  updated_at: new Date().toISOString(),
                  revisions: [
                    { revision: rep.revision || 'rev_01', note: 'Agent自动装配生成', created_at: new Date().toISOString() },
                  ],
                  runs: reportRuns.map((r: any) => ({
                    run_id: r.run_id,
                    revision: r.revision || 'rev_01',
                    status: r.status || 'succeeded',
                    finished_at: r.finished_at || new Date().toISOString(),
                    duration_ms: r.duration_ms || 1200,
                    error_code: r.error_code || null,
                    error_message: r.error_message || null,
                  })),
                  artifacts,
                },
              })
            );
            return;
          }

          if (url.match(/\/api\/reports\/[^/?]+$/)) {
            const match = url.match(/\/api\/reports\/([^/?]+)$/);
            const id = match ? match[1] : '';
            const rep = devApiMockPlugin.mockReports[id] || devApiMockPlugin.mockReports['customer_ltv_overdue'];
            res.end(JSON.stringify(rep));
            return;
          }

          const items = Object.values(devApiMockPlugin.mockReports);
          res.end(
            JSON.stringify({
              items,
              total: items.length,
              returned: items.length,
              next_offset: null,
            })
          );
          return;
        }

        if (url.startsWith('/api/runs')) {
          res.end(
            JSON.stringify({
              ok: true,
              runs: {
                total: devApiMockPlugin.mockRuns.length,
                failed: 0,
                items: devApiMockPlugin.mockRuns,
              },
            })
          );
          return;
        }

        if (url.startsWith('/api/semantic/db')) {
          res.end(
            JSON.stringify({
              ok: true,
              enabled: true,
              path: 'demo_data/report_demo.db',
              counts: {
                rpt_report: 2,
                rpt_revision: 3,
                rpt_run: 2
              }
            })
          );
          return;
        }

        if (url.startsWith('/api/datasources')) {
          const json = (v: unknown, status = 200) => {
            res.statusCode = status;
            res.setHeader('Content-Type', 'application/json');
            res.end(JSON.stringify(v));
          };
          // 错误体对齐服务端契约：{error:{code,message}}（前端 request() 据此抛 ApiError）
          const fail = (status: number, message: string) =>
            json({ error: { code: 'DATASOURCE', message } }, status);

          // 列表（JSON 注册表，脱敏：password 恒空串）
          if (req.method === 'GET' && /^\/api\/datasources\/?$/.test(url)) {
            const items = listDatasources();
            return json({
              ok: true,
              items,
              total: items.length,
              registry: 'examples/library/.datasources.json',
            });
          }

          // 新建/更新（upsert，口令留空 = 保留）
          if (req.method === 'POST' && /^\/api\/datasources\/?$/.test(url)) {
            const body = await readBody(req);
            try {
              return json({ ok: true, item: upsertDatasource(JSON.parse(body || '{}')) });
            } catch (e: any) {
              return fail(400, String(e?.message || e));
            }
          }

          // 表单「测试连接」：用未保存记录探测（必须在 /:id/probe 之前匹配）
          if (req.method === 'POST' && url === '/api/datasources/probe') {
            const body = JSON.parse((await readBody(req)) || '{}');
            try {
              return json({ ok: true, probe: await probeRecord(body.record || {}, body) });
            } catch (e: any) {
              return fail(400, String(e?.message || e));
            }
          }

          // 已保存数据源探测：{table?|sql?|limit?}，{} = select 1 连通性
          const idProbe = url.match(/^\/api\/datasources\/([^/?]+)\/probe$/);
          if (req.method === 'POST' && idProbe) {
            const body = JSON.parse((await readBody(req)) || '{}');
            try {
              return json({
                ok: true,
                probe: await probeDatasource(decodeURIComponent(idProbe[1]), body),
              });
            } catch (e: any) {
              return fail(400, String(e?.message || e));
            }
          }

          // 表清单（增强项）
          const idTables = url.match(/^\/api\/datasources\/([^/?]+)\/tables(\?.*)?$/);
          if (req.method === 'GET' && idTables) {
            try {
              return json({ ok: true, items: await listTables(decodeURIComponent(idTables[1])) });
            } catch (e: any) {
              return fail(400, String(e?.message || e));
            }
          }

          // 表的列属性（?table=schema.表）：列名/类型/注释/主键
          const idCols = url.match(/^\/api\/datasources\/([^/?]+)\/columns(\?.*)?$/);
          if (req.method === 'GET' && idCols) {
            const q = new URL(url, 'http://x').searchParams;
            try {
              return json({
                ok: true,
                items: await listColumns(decodeURIComponent(idCols[1]), q.get('table') || ''),
              });
            } catch (e: any) {
              return fail(400, String(e?.message || e));
            }
          }

          // 删除
          const delMatch = url.match(/^\/api\/datasources\/([^/?]+)(\?.*)?$/);
          if (req.method === 'DELETE' && delMatch) {
            try {
              const id = decodeURIComponent(delMatch[1]);
              deleteDatasource(id);
              return json({ ok: true, id });
            } catch (e: any) {
              return fail(404, String(e?.message || e));
            }
          }

          return fail(404, `datasources 未匹配的路由: ${req.method} ${url}`);
        }

        if (url.startsWith('/api/shapes')) {
          res.end(
            JSON.stringify({
              ok: true,
              items: [
                { shape: 'detail', label: '明细列表', layout: 'list', needs: ['dimensions', 'metrics'] },
                { shape: 'grouped', label: '分组汇总', layout: 'group', needs: ['dimensions', 'metrics'] },
                { shape: 'crosstab', label: '交叉透视表', layout: 'crosstab', needs: ['row_dims', 'col_dims', 'metrics'] },
                { shape: 'trend', label: '时序趋势表', layout: 'trend', needs: ['time_field', 'metrics'] },
                { shape: 'rank', label: '排行 TopN', layout: 'rank', needs: ['metrics', 'limit'] },
                { shape: 'compare', label: '同比/环比对比', layout: 'compare', needs: ['time_field', 'metrics'] }
              ]
            })
          );
          return;
        }

        // 真模型词表（给四步解析用）：维度/度量/时间/实体
        const lexMatch = url.match(/^\/api\/bizmodels\/([^/?]+)\/lexicon$/);
        if (req.method === 'GET' && lexMatch) {
          try {
            res.end(JSON.stringify(await buildLexicon(decodeURIComponent(lexMatch[1]))));
          } catch (e: any) {
            res.statusCode = 400;
            res.end(JSON.stringify({ error: { code: 'ONTOLOGY', message: String(e?.message || e) } }));
          }
          return;
        }

        // 标准问题库命中查询：GET /api/bizmodels/{dsn}/canon?question=...（页面用；命中返回权威 SQL）
        const canonMatch = url.match(/^\/api\/bizmodels\/([^/?]+)\/canon(\?.*)?$/);
        if (req.method === 'GET' && canonMatch) {
          const q = new URL(url, 'http://x').searchParams.get('question') || '';
          try {
            const item = matchCanon(q);
            res.end(JSON.stringify({ ok: true, item: item || null }));
          } catch (e: any) {
            res.statusCode = 400;
            res.end(JSON.stringify({ error: { code: 'ONTOLOGY', message: String(e?.message || e) } }));
          }
          return;
        }

        // Agent/独立系统自动解析入口：POST /api/bizmodels/{dsn}/auto-parse {question, save?}
        const autoMatch = url.match(/^\/api\/bizmodels\/([^/?]+)\/auto-parse$/);
        if (req.method === 'POST' && autoMatch) {
          const body = JSON.parse((await readBody(req)) || '{}');
          try {
            const r = await autoParse(decodeURIComponent(autoMatch[1]), body.question || '', {
              save: body.save !== false,
            });
            res.end(JSON.stringify({ ok: true, ...r }));
          } catch (e: any) {
            res.statusCode = 400;
            res.end(JSON.stringify({ error: { code: 'ONTOLOGY', message: String(e?.message || e) } }));
          }
          return;
        }

        // 导出本体 MD（为后续工作/上下文准备）：写成文件并回传路径
        const ontoExportMatch = url.match(/^\/api\/bizmodels\/([^/?]+)\/export$/);
        if (req.method === 'GET' && ontoExportMatch) {
          try {
            const r = await exportOntologyMd(decodeURIComponent(ontoExportMatch[1]));
            res.end(JSON.stringify({ ok: true, path: r.path }));
          } catch (e: any) {
            res.statusCode = 400;
            res.end(JSON.stringify({ error: { code: 'ONTOLOGY', message: String(e?.message || e) } }));
          }
          return;
        }

        // 真实本体图谱（现算：真实外键 + 同名列候选）—— 必须在通用 bizmodels mock 之前
        const ontoGraphMatch = url.match(/^\/api\/bizmodels\/([^/?]+)\/ontology$/);
        if (req.method === 'GET' && ontoGraphMatch) {
          try {
            const g = await buildOntologyGraph(decodeURIComponent(ontoGraphMatch[1]));
            res.end(JSON.stringify({ ok: true, ...g }));
          } catch (e: any) {
            res.statusCode = 400;
            res.end(JSON.stringify({ error: { code: 'ONTOLOGY', message: String(e?.message || e) } }));
          }
          return;
        }

        if (url.startsWith('/api/bizmodels')) {
          res.end(
            JSON.stringify({
              ok: true,
              model: {
                schema: 'rpx-bizmodel/v1',
                version: 1,
                source: { dsn: 'demo_sqlite', generated_at: '2026-10-30', generator: 'rpx-model' },
                entities: [
                  { id: 'customer', label: '客户 (Customer)', status: 'confirmed', table: 'customers', grain: { key: ['customer_id'], description: '每行一个客户' } },
                  { id: 'orders', label: '订单 (Order)', status: 'confirmed', table: 'orders', grain: { key: ['order_id'], description: '每行一笔订单' } },
                  { id: 'loan', label: '贷款合同 (Loan)', status: 'confirmed', table: 'loans', grain: { key: ['loan_id'], description: '每行一笔借款' } }
                ],
                relationships: [
                  { id: 'rel_cust_orders', label: '客户与订单', from: { entity: 'customer', key: ['customer_id'] }, to: { entity: 'orders', key: ['customer_id'] }, cardinality: '1:N', status: 'confirmed' },
                  { id: 'rel_cust_loans', label: '客户与贷款', from: { entity: 'customer', key: ['customer_id'] }, to: { entity: 'loan', key: ['customer_id'] }, cardinality: '1:N', status: 'confirmed' }
                ],
                metrics: [
                  { id: 'ltv', label: 'LTV (客户终身价值)', entity: 'orders', column: 'amount', expression: 'SUM(amount)', aliases: ['价值', '消费额', 'LTV'], status: 'confirmed' },
                  { id: 'overdue_rate', label: '逾期率', entity: 'loan', column: 'is_overdue', expression: 'SUM(is_overdue)*1.0/COUNT(loan_id)', aliases: ['逾期风险', '逾期率'], status: 'confirmed' }
                ],
                dimensions: [
                  { id: 'cust_level', label: '客户等级', entity: 'customer', column: 'level', values: ['钻石', '白金', '黄金'], aliases: ['级别', '会员等级'], status: 'confirmed' },
                  { id: 'region', label: '大区', entity: 'customer', column: 'region', values: ['华东', '华北', '华南'], aliases: ['地区', '区域'], status: 'confirmed' }
                ],
                rules: [],
                open_questions: []
              }
            })
          );
          return;
        }

        // 本体解析痕迹：落 SQLite（examples/library/.ontology.db，onto_intent/step/hit）
        if (url.startsWith('/api/ontology')) {
          const json = (v: unknown, status = 200) => {
            res.statusCode = status;
            res.setHeader('Content-Type', 'application/json');
            res.end(JSON.stringify(v));
          };
          const fail = (status: number, message: string) =>
            json({ error: { code: 'ONTOLOGY', message } }, status);

          // 保存（按 id upsert；steps/hits 整体替换）
          if (req.method === 'POST' && /^\/api\/ontology\/?$/.test(url)) {
            const body = JSON.parse((await readBody(req)) || '{}');
            try {
              return json({ ok: true, id: saveOntologyTrace(body).id });
            } catch (e: any) {
              return fail(400, String(e?.message || e));
            }
          }

          // 列表（?dsn=&limit=）
          if (req.method === 'GET' && /^\/api\/ontology\/?(\?.*)?$/.test(url)) {
            const q = new URL(url, 'http://x').searchParams;
            const dsn = q.get('dsn') || '';
            if (!dsn) return fail(400, '缺少 dsn 参数');
            return json(listOntologyTraces(dsn, Number(q.get('limit')) || 50));
          }

          // 读取单条完整痕迹
          const idMatch = url.match(/^\/api\/ontology\/([^/?]+)$/);
          if (req.method === 'GET' && idMatch) {
            try {
              return json(getOntologyTrace(decodeURIComponent(idMatch[1])));
            } catch (e: any) {
              return fail(404, String(e?.message || e));
            }
          }

          // 删除一条历史痕迹（含 steps/hits）
          const idDel = url.match(/^\/api\/ontology\/([^/?]+)$/);
          if (req.method === 'DELETE' && idDel) {
            try {
              return json(deleteOntologyTrace(decodeURIComponent(idDel[1])));
            } catch (e: any) {
              return fail(404, String(e?.message || e));
            }
          }

          return fail(404, `ontology 未匹配的路由: ${req.method} ${url}`);
        }

        next();
      });
    },
  };
}

export default defineConfig({
  root: 'app',
  plugins: [react(), devApiMockPlugin()],
  server: {
    port: 3000,
    host: '0.0.0.0',
    strictPort: true,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8090',
        changeOrigin: false,
        rewrite: (p) => p.replace(/^\/api/, ''),
      },
    },
  },
  preview: {
    port: 3000,
    host: '0.0.0.0',
    strictPort: true,
  },
  build: {
    outDir: '../dist',
    emptyOutDir: true,
  },
});
