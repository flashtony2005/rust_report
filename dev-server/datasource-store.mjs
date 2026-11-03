// 数据源注册表：JSON 文件是唯一真相（examples/library/.datasources.json）。
// dev server 启动时读、增删改时原子写回；列表接口不回传口令（回传空串 + has_password 布尔）。
// 探测契约对齐 app/src/services/api.ts：连通性 = select 1；表探测 = 列画像 + 抽样 + 主键。
// 将来接真 Rust 后端时，同一份 JSON 由后端接管，此处直接退位。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
import { PgConn } from './pg-probe.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DS_PATH = path.resolve(__dirname, '../examples/library/.datasources.json');

function readAll() {
  try {
    return JSON.parse(fs.readFileSync(DS_PATH, 'utf8'));
  } catch {
    return {};
  }
}

function writeAll(map) {
  const tmp = DS_PATH + '.tmp';
  fs.writeFileSync(tmp, JSON.stringify(map, null, 2) + '\n', 'utf8');
  fs.renameSync(tmp, DS_PATH);
}

function driverLabel(driver) {
  if (driver === 'sqlite') return 'SQLite 文件';
  if (driver === 'mysql') return 'MySQL';
  return 'PostgreSQL / Kingbase';
}

/** 打码连接串：让人核对"连的是哪个库"。 */
function urlMasked(d) {
  if (d.driver === 'sqlite') return `sqlite://${d.database}`;
  return `${d.driver}://${d.user}:***@${d.host}:${d.port}/${d.database}`;
}

/** 脱敏视图：password 恒为空串（契约：服务端从不回传口令）。 */
function toPublic(id, d) {
  return {
    id,
    label: d.label,
    driver: d.driver,
    driver_label: d.driver_label || driverLabel(d.driver),
    host: d.host,
    port: d.port,
    database: d.database,
    user: d.user,
    password: '',
    has_password: !!d.password,
    options: d.options,
    url_masked: urlMasked(d),
    created_at: d.created_at,
    updated_at: d.updated_at,
  };
}

/** 列表（脱敏）。 */
export function listDatasources() {
  const all = readAll();
  return Object.entries(all).map(([id, d]) => toPublic(id, d));
}

/** 新建/更新（以 id 为主键 upsert；口令留空 = 保留原口令）。 */
export function upsertDatasource(input) {
  const id = String(input.id || '').trim();
  if (!id) throw new Error('标识（id）不能为空');
  if (!input.label || !String(input.label).trim()) throw new Error('显示名（label）不能为空');
  const driver = input.driver || 'postgres';
  const all = readAll();
  const prev = all[id];
  const now = new Date().toISOString();
  all[id] = {
    label: String(input.label).trim(),
    driver,
    host: driver === 'sqlite' ? '' : String(input.host || '127.0.0.1').trim(),
    port: driver === 'sqlite' ? 0 : Number(input.port) || 0,
    database: String(input.database || '').trim(),
    user: driver === 'sqlite' ? '' : String(input.user || '').trim(),
    password: input.password ? String(input.password) : prev?.password || '',
    options: String(input.options || '').trim(),
    created_at: prev?.created_at || now,
    updated_at: now,
  };
  writeAll(all);
  return toPublic(id, all[id]);
}

export function deleteDatasource(id) {
  const all = readAll();
  if (!(id in all)) throw new Error(`数据源不存在: ${id}`);
  delete all[id];
  writeAll(all);
}

// ---- 探测 ----

/** 表名 → 带引号的安全标识（允许 schema.table；引号标识符只禁内嵌双引号，schema 可以数字开头）。 */
function quoteIdent(table) {
  return String(table)
    .split('.')
    .map((p) => {
      const part = p.trim();
      if (!part || /["\0]/.test(part)) throw new Error(`非法标识符: ${part}`);
      return `"${part}"`;
    })
    .join('.');
}

function escLiteral(s) {
  return String(s).replace(/'/g, "''");
}

/** 值的画像类型：靠值判，不靠库里的 DDL。 */
function valueKind(s) {
  const t = String(s).trim();
  if (/^(true|false|t|f|yes|no)$/i.test(t)) return 'bool';
  if (/^-?\d+$/.test(t)) return 'number';
  if (/^-?(\d+\.\d*|\.\d+|\d+)([eE][+-]?\d+)?$/.test(t) && /[.eE]/.test(t)) return 'decimal';
  return 'text';
}

/** 列画像：null 数 / distinct 数 / 样本（≤5 个 distinct 非空值）。 */
function profileColumns(names, rows) {
  return names.map((name, ci) => {
    let nulls = 0;
    const seen = new Set();
    const samples = [];
    const kinds = new Set();
    for (const r of rows) {
      const v = r[ci];
      if (v === null || v === undefined || v === '') {
        if (v === null || v === undefined) nulls++;
        continue;
      }
      const s = String(v);
      if (!seen.has(s)) {
        seen.add(s);
        if (samples.length < 5) samples.push(s);
      }
      kinds.add(valueKind(s));
    }
    let kind = 'null';
    if (kinds.size === 1) kind = [...kinds][0];
    else if (kinds.size > 1) kind = 'mixed';
    return { name, kind, nulls, distinct: seen.size, samples };
  });
}

function buildProbeResult(sql, columnNames, rows, key, t0, limit) {
  const sampled = rows.length;
  const shown = rows.slice(0, limit);
  return {
    sql,
    columns: profileColumns(columnNames, shown),
    rows: shown,
    sampled,
    key,
    elapsed_ms: Date.now() - t0,
  };
}

/** PostgreSQL 探测（最小 wire 客户端）。 */
async function probePg(d, opts, limit, t0) {
  if (!d.host || !Number(d.port)) throw new Error('主机/端口未配置');
  const conn = new PgConn(d);
  try {
    await conn.connect();
    let sql;
    let key = [];
    if (opts.sql && String(opts.sql).trim()) {
      sql = String(opts.sql).trim();
    } else if (opts.table) {
      // 主键：information_schema 两表连接。表名支持 schema.table（如 202605.t101）；
      // 未带 schema 时按 current_schema() 找（简单查询协议没有参数绑定，字面量已转义）
      const raw = String(opts.table);
      const dot = raw.lastIndexOf('.');
      const schemaLit = dot > 0 ? escLiteral(raw.slice(0, dot)) : null;
      const nameLit = escLiteral(dot > 0 ? raw.slice(dot + 1) : raw);
      const schemaClause = schemaLit
        ? `tc.table_schema = '${schemaLit}'`
        : `tc.table_schema = current_schema()`;
      const pk = await conn.query(
        `select k.column_name from information_schema.table_constraints tc` +
          ` join information_schema.key_column_usage k` +
          ` on tc.constraint_name = k.constraint_name and tc.table_schema = k.table_schema` +
          ` where tc.constraint_type = 'PRIMARY KEY' and tc.table_name = '${nameLit}'` +
          ` and ${schemaClause}` +
          ` order by k.ordinal_position`,
      );
      key = pk.rows.map((r) => r[0]);
      sql = `select * from ${quoteIdent(raw)} limit ${limit}`;
    } else {
      sql = 'select 1 as ok';
    }
    const res = await conn.query(sql);
    return buildProbeResult(sql, res.columns, res.rows, key, t0, limit);
  } finally {
    conn.end();
  }
}

/** SQLite 探测（node:sqlite，只读打开）。 */
function probeSqlite(d, opts, limit, t0) {
  if (!d.database) throw new Error('库文件路径未配置');
  let db;
  try {
    db = new DatabaseSync(d.database);
    db.exec('PRAGMA query_only = 1');
  } catch (e) {
    throw new Error(`SQLite 打不开 ${d.database}：${e.message}`);
  }
  try {
    let sql;
    let key = [];
    let columnNames = [];
    let rows = [];
    if (opts.sql && String(opts.sql).trim()) {
      sql = String(opts.sql).trim();
      const stmt = db.prepare(sql);
      columnNames = stmt.columns().map((c) => c.name);
      rows = stmt.all().map((o) => columnNames.map((n) => o[n] ?? null));
    } else if (opts.table) {
      const tbl = String(opts.table).trim();
      if (!tbl || /["'\0]/.test(tbl)) throw new Error(`非法表名: ${tbl}`);
      const pkStmt = db.prepare(`pragma table_info('${tbl}')`);
      key = pkStmt.all().filter((c) => c.pk).map((c) => c.name);
      sql = `select * from "${tbl}" limit ${limit}`;
      const stmt = db.prepare(sql);
      columnNames = stmt.columns().map((c) => c.name);
      rows = stmt.all().map((o) => columnNames.map((n) => o[n] ?? null));
    } else {
      sql = 'select 1 as ok';
      const stmt = db.prepare(sql);
      columnNames = ['ok'];
      rows = [[stmt.get()?.ok ?? 1]];
    }
    return buildProbeResult(sql, columnNames, rows, key, t0, limit);
  } finally {
    db.close();
  }
}

/** 探测（已保存或未保存记录）。`{}` = 只验连通性。 */
export async function probeRecord(d, opts = {}) {
  const t0 = Date.now();
  const limit = Math.max(1, Math.min(Number(opts.limit) || 200, 2000));
  const driver = d.driver || 'postgres';
  if (driver === 'sqlite') return probeSqlite(d, opts, limit, t0);
  if (driver === 'postgres') return probePg(d, opts, limit, t0);
  throw new Error(`dev 探测暂不支持驱动 ${driver}（MySQL 需要接真后端）`);
}

/** 已保存数据源探测。 */
export async function probeDatasource(id, opts = {}) {
  const all = readAll();
  const d = all[id];
  if (!d) throw new Error(`数据源不存在: ${id}`);
  return probeRecord(d, opts);
}

/** 表清单（增强项：拿不到不挡流程）。 */
export async function listTables(id) {  const all = readAll();
  const d = all[id];
  if (!d) throw new Error(`数据源不存在: ${id}`);
  if (d.driver === 'sqlite') {
    if (!d.database) throw new Error('库文件路径未配置');
    const db = new DatabaseSync(d.database);
    try {
      db.exec('PRAGMA query_only = 1');
      const rows = db.prepare(`select name from sqlite_master where type='table' and name not like 'sqlite_%' order by name`).all();
      return rows.map((r) => r.name);
    } finally {
      db.close();
    }
  }
  if (d.driver === 'postgres') {
    const conn = new PgConn(d);
    try {
      await conn.connect();
      // gongzi 库的表在 service / 202605 / service_backup 等业务 schema（public 是空的），
      // 所以跨全部用户 schema 列出，带 schema 前缀（探测的 select * / 主键查询都认这个形式）。
      // service 是主库（用户 2026-11-02 确认），排最前；202605=月度快照、service_backup=备份
      const res = await conn.query(
        `select table_schema || '.' || table_name from information_schema.tables` +
          ` where table_type = 'BASE TABLE'` +
          ` and table_schema not in ('pg_catalog', 'information_schema')` +
          ` order by case when table_schema = 'service' then 0 else 1 end, table_schema, table_name`,
      );
      return res.rows.map((r) => r[0]);
    } finally {
      conn.end();
    }
  }
  throw new Error(`dev 探测暂不支持驱动 ${d.driver}（MySQL 需要接真后端）`);
}

/**
 * 全局"列名→中文名"投票映射：从主库所有列注释里按列名聚合（多数票）。
 * 给没有列注释的表（如 GZ_* 族）当兜底——t101/dwxxb 等表注释覆盖率很高，权威性来自库本身。
 * 带 60s 缓存，避免每次点实体都全库扫一遍。
 */
let CN_MAP_CACHE = { at: 0, map: null };

async function globalCnMap(d) {
  if (CN_MAP_CACHE.map && Date.now() - CN_MAP_CACHE.at < 60_000) return CN_MAP_CACHE.map;
  const conn = new PgConn(d);
  try {
    await conn.connect();
    const res = await conn.query(
      `select a.attname as col, d.description as cn, count(*) as cnt
       from pg_attribute a
       join pg_class c on c.oid = a.attrelid
       join pg_namespace n on n.oid = c.relnamespace
       join pg_description d on d.objoid = c.oid and d.objsubid = a.attnum
       where n.nspname = 'service' and a.attnum > 0 and not a.attisdropped
         and d.description is not null and length(trim(d.description)) > 0
         and lower(trim(d.description)) <> lower(a.attname)  -- 排除"注释=列名本身"的垃圾票
       group by a.attname, d.description
       order by a.attname, cnt desc`,
    );
    const map = {};
    for (const [col, cn] of res.rows) {
      const key = String(col).toLowerCase();
      if (!(key in map)) map[key] = String(cn).trim(); // 同列名按 cnt desc，第一条即多数票
    }
    CN_MAP_CACHE = { at: Date.now(), map };
    return map;
  } catch {
    return {};
  } finally {
    conn.end();
  }
}

/**
 * 表的列属性（列名 / 中文名 / DDL 类型 / 是否主键）。
 * 中文名解析顺序：本表列注释（col_description）→ 全局投票映射 → 空。
 * PG 从 pg_catalog 取；SQLite 走 pragma table_info（无注释）。
 */
export async function listColumns(id, table) {
  const all = readAll();
  const d = all[id];
  if (!d) throw new Error(`数据源不存在: ${id}`);
  const raw = String(table || '').trim();
  if (!raw) throw new Error('缺少 table 参数');
  const dot = raw.lastIndexOf('.');
  const schema = dot > 0 ? raw.slice(0, dot) : '';
  const name = dot > 0 ? raw.slice(dot + 1) : raw;

  if (d.driver === 'postgres') {
    const sch = schema || 'public';
    if (/["'\0;]/.test(sch) || /["'\0;]/.test(name)) throw new Error(`非法表名: ${raw}`);
    const conn = new PgConn(d);
    try {
      await conn.connect();
      const [res, cnMap] = await Promise.all([conn.query(
        `select a.attname,
                format_type(a.atttypid, a.atttypmod),
                col_description(a.attrelid, a.attnum),
                (select 'PK' from pg_index i
                  where i.indrelid = a.attrelid and i.indisprimary
                    and a.attnum = any(i.indkey) limit 1)
         from pg_attribute a
         join pg_class c on c.oid = a.attrelid
         join pg_namespace n on n.oid = c.relnamespace
         where n.nspname = '${escLiteral(sch)}' and c.relname = '${escLiteral(name)}'
           and a.attnum > 0 and not a.attisdropped
         order by a.attnum`,
      ), globalCnMap(d)]);
      return res.rows.map((r) => ({
        name: r[0],
        // 中文名：本表列注释优先，其次全局投票映射（同名列在别的表有注释就复用）
        cn: (r[2] && String(r[2]).trim()) || cnMap[String(r[0]).toLowerCase()] || '',
        type: r[1],
        ispk: !!r[3],
      }));
    } finally {
      conn.end();
    }
  }

  if (d.driver === 'sqlite') {
    if (/["'\0]/.test(name)) throw new Error(`非法表名: ${raw}`);
    if (!d.database) throw new Error('库文件路径未配置');
    const db = new DatabaseSync(d.database);
    try {
      db.exec('PRAGMA query_only = 1');
      const rows = db.prepare(`pragma table_info('${name}')`).all();
      return rows.map((c) => ({ name: c.name, cn: '', type: c.type || '', ispk: !!c.pk }));
    } finally {
      db.close();
    }
  }

  throw new Error(`列属性暂不支持驱动 ${d.driver}（MySQL 需要接真后端）`);
}
