#!/usr/bin/env node
/**
 * rpx-onto CLI：给终端 / 脚本 / 其它程序直接调用「四步解析」，与 MCP 同一套后端。
 *
 *   node dev-server/rpx-onto-cli.mjs resolve "按所属镇街统计单位上报情况" [--dsn gongzi-pg] [--no-save]
 *   node dev-server/rpx-onto-cli.mjs canon-list
 *   node dev-server/rpx-onto-cli.mjs run-sql "select count(*) from service.dwxxb" [--limit 20]
 *   node dev-server/rpx-onto-cli.mjs export-md [--dsn gongzi-pg]
 *
 * 输出一律 JSON（--pretty 缩进），方便管道给 jq / 其它 Agent。
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PgConn } from './pg-probe.mjs';
import { autoParse, exportOntologyMd } from './ontology-store.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DS_PATH = path.resolve(__dirname, '../examples/library/.datasources.json');
const CANON_PATH = path.resolve(__dirname, '../examples/library/.ontology-canon.sqls.json');
const DEFAULT_DSN = 'gongzi-pg';

function parseArgs(argv) {
  const pos = [];
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith('--')) {
      const key = a.slice(2);
      if (key === 'no-save' || key === 'pretty') opts[key] = true;
      else {
        opts[key] = argv[i + 1];
        i++;
      }
    } else pos.push(a);
  }
  return { cmd: pos[0], arg: pos[1], opts };
}

function readDatasource(dsn) {
  const d = JSON.parse(fs.readFileSync(DS_PATH, 'utf8'))[dsn];
  if (!d) throw new Error(`数据源不存在: ${dsn}`);
  return d;
}

const out = (v, pretty) => process.stdout.write(JSON.stringify(v, null, pretty ? 2 : 0) + '\n');

(async () => {
  const { cmd, arg, opts } = parseArgs(process.argv.slice(2));
  const dsn = opts.dsn || DEFAULT_DSN;
  try {
    if (cmd === 'resolve') {
      if (!arg) throw new Error('用法: resolve "问题原文"');
      out(await autoParse(dsn, arg, { save: !opts['no-save'] }), opts.pretty);
      return;
    }
    if (cmd === 'canon-list') {
      const items = JSON.parse(fs.readFileSync(CANON_PATH, 'utf8')).items || [];
      out(
        items.map(({ id, title, match, center_entity }) => ({ id, title, match, center_entity })),
        opts.pretty,
      );
      return;
    }
    if (cmd === 'run-sql') {
      if (!arg) throw new Error('用法: run-sql "select ..."');
      if (!/^(select|with)\b/i.test(arg.trim())) throw new Error('只允许 SELECT/WITH 只读查询');
      const limit = Math.max(1, Math.min(Number(opts.limit) || 100, 500));
      const conn = new PgConn(readDatasource(dsn));
      try {
        await conn.connect();
        const res = await conn.query(arg);
        out({ sql: arg, columns: res.columns, row_count: res.rows.length, returned: Math.min(res.rows.length, limit), rows: res.rows.slice(0, limit) }, opts.pretty);
      } finally {
        conn.end();
      }
      return;
    }
    if (cmd === 'export-md') {
      const r = await exportOntologyMd(dsn);
      out({ path: r.path }, opts.pretty);
      return;
    }
    out({
      usage: [
        'resolve "问题原文" [--dsn gongzi-pg] [--no-save] [--pretty]',
        'canon-list [--pretty]',
        'run-sql "select ..." [--limit 100] [--pretty]',
        'export-md [--dsn gongzi-pg]',
      ],
    }, true);
    process.exit(cmd ? 1 : 0);
  } catch (e) {
    out({ error: e?.message || String(e) }, true);
    process.exit(1);
  }
})();
