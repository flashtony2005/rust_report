#!/usr/bin/env node
/**
 * rpx-onto MCP server（stdio，零依赖手写 JSON-RPC 2.0）。
 * 让 AI 客户端直接调用「四步解析」：标准口径优先 → 词表解析 → 生成 SQL → 自动留痕。
 *
 * 工具：
 *   onto_resolve    {question, dsn?, save?}  解析一句话 → 命中/中心表/SQL/依据/留痕 id
 *   onto_canon_list {}                       列出标准问题库（权威口径清单）
 *   onto_run_sql    {sql, dsn?, limit?}      只读执行校验（仅允许 SELECT/WITH）
 *   onto_export_md  {dsn?}                   导出本体 MD（上下文沉淀）
 *
 * 挂载（~/.workbuddy/mcp.json）：
 *   { "mcpServers": { "rpx-onto": { "command": "node",
 *       "args": ["F:/project/report/rpx/dev-server/rpx-onto-mcp.mjs"] } } }
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PgConn } from './pg-probe.mjs';
import { autoParse, buildLexicon, exportOntologyMd } from './ontology-store.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DS_PATH = path.resolve(__dirname, '../examples/library/.datasources.json');
const CANON_PATH = path.resolve(__dirname, '../examples/library/.ontology-canon.sqls.json');
const DEFAULT_DSN = 'gongzi-pg';

function readDatasource(dsn) {
  const all = JSON.parse(fs.readFileSync(DS_PATH, 'utf8'));
  const d = all[dsn];
  if (!d) throw new Error(`数据源不存在: ${dsn}`);
  return d;
}

const TOOLS = [
  {
    name: 'onto_resolve',
    description:
      '把一句中文业务问题解析成 SQL（四步解析）：先命中标准问题库（权威口径），否则用词表（列注释）拼单表聚合。返回 center/SQL/依据/留痕 id。',
    inputSchema: {
      type: 'object',
      properties: {
        question: { type: 'string', description: '业务问题原文，例：按所属镇街统计单位上报情况' },
        dsn: { type: 'string', description: `数据源 id，默认 ${DEFAULT_DSN}` },
        save: { type: 'boolean', description: '是否把痕迹写入 SQLite（默认 true）' },
      },
      required: ['question'],
    },
  },
  {
    name: 'onto_canon_list',
    description: '列出标准问题库：业务上有权威口径的经典问题（id/标题/匹配正则/中心实体）。',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'onto_run_sql',
    description: '只读执行一条 SQL 做结果校验（仅允许 SELECT/WITH，自动限行）。',
    inputSchema: {
      type: 'object',
      properties: {
        sql: { type: 'string' },
        dsn: { type: 'string' },
        limit: { type: 'number', description: '返回行数上限，默认 100，最大 500' },
      },
      required: ['sql'],
    },
  },
  {
    name: 'onto_export_md',
    description: '导出本体 MD（实体+关系+视图+业务组说明+校验规则+dashboard 口径），返回文件路径，供后续工作做上下文。',
    inputSchema: { type: 'object', properties: { dsn: { type: 'string' } } },
  },
];

const ok = (id, result) => ({ jsonrpc: '2.0', id, result });
const fail = (id, message) => ({
  jsonrpc: '2.0',
  id,
  result: { content: [{ type: 'text', text: `错误：${message}` }], isError: true },
});
const text = (id, value) => ok(id, { content: [{ type: 'text', text: typeof value === 'string' ? value : JSON.stringify(value, null, 2) }] });

async function callTool(id, name, args = {}) {
  const dsn = args.dsn || DEFAULT_DSN;
  try {
    if (name === 'onto_resolve') {
      const r = await autoParse(dsn, args.question || '', { save: args.save !== false });
      return text(id, r);
    }
    if (name === 'onto_canon_list') {
      const items = JSON.parse(fs.readFileSync(CANON_PATH, 'utf8')).items || [];
      return text(id, items.map(({ id: cid, title, match, center_entity }) => ({ id: cid, title, match, center_entity })));
    }
    if (name === 'onto_run_sql') {
      const sql = String(args.sql || '').trim();
      if (!/^(select|with)\b/i.test(sql)) throw new Error('只允许 SELECT/WITH 只读查询');
      const limit = Math.max(1, Math.min(Number(args.limit) || 100, 500));
      const d = readDatasource(dsn);
      const conn = new PgConn(d);
      try {
        await conn.connect();
        const res = await conn.query(sql);
        const rows = res.rows.slice(0, limit);
        return text(id, {
          sql,
          columns: res.columns,
          row_count: res.rows.length,
          returned: rows.length,
          rows,
        });
      } finally {
        conn.end();
      }
    }
    if (name === 'onto_export_md') {
      const r = await exportOntologyMd(dsn);
      return text(id, { path: r.path });
    }
    return fail(id, `未知工具: ${name}`);
  } catch (e) {
    return fail(id, e?.message || String(e));
  }
}

async function handle(msg) {
  const { id, method, params } = msg;
  const isNotification = id === undefined || id === null;
  if (method === 'initialize') {
    return ok(id, {
      protocolVersion: '2024-11-05',
      capabilities: { tools: {} },
      serverInfo: { name: 'rpx-onto', version: '1.0.0' },
    });
  }
  if (method === 'notifications/initialized' || method === 'notifications/cancelled') return;
  if (method === 'ping') return ok(id, {});
  if (method === 'tools/list') return ok(id, { tools: TOOLS });
  if (method === 'tools/call') {
    const out = await callTool(id, params?.name, params?.arguments || {});
    return out;
  }
  if (!isNotification) {
    return { jsonrpc: '2.0', id, error: { code: -32601, message: `method not found: ${method}` } };
  }
}

let buf = '';
const pending = new Set();
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
  buf += chunk;
  let idx;
  while ((idx = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, idx).trim();
    buf = buf.slice(idx + 1);
    if (!line) continue;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      continue;
    }
    const p = Promise.resolve(handle(msg))
      .then((out) => {
        if (out) process.stdout.write(JSON.stringify(out) + '\n');
      })
      .catch((e) => {
        if (msg && msg.id != null) {
          process.stdout.write(
            JSON.stringify({ jsonrpc: '2.0', id: msg.id, error: { code: -32603, message: e?.message || String(e) } }) + '\n',
          );
        }
      })
      .finally(() => pending.delete(p));
    pending.add(p);
  }
});
// stdin 关闭时等未完成的工具调用（如 SQL 查询）写完响应再退出——否则会被 exit 抢跑
process.stdin.on('end', async () => {
  await Promise.allSettled([...pending]);
  process.exit(0);
});
