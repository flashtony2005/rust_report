// 本体解析：痕迹落 SQLite（examples/library/.ontology.db），真图谱从库里现算。
// 为什么 SQLite：用户 2026-11-02 拍板"本体解析结果存 sqlite"（数据源注册表仍是 JSON，两者分工不同）。
// 表结构对齐老 onto_* 设计的三张表（intent / step / hit），按可迁 PG 规矩写：
//   业务主键不自增 / 布尔 INTEGER 0/1 / JSON 存 TEXT / 时间 TEXT ISO。
// 真图谱：POSTGRES information_schema —— 真实外键（已确认）+ 同名列候选关系（代码列是某表主键、又出现在别的表）。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';
import { PgConn } from './pg-probe.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DB_PATH = path.resolve(__dirname, '../examples/library/.ontology.db');

function openDb() {
  const db = new DatabaseSync(DB_PATH);
  db.exec(`
    create table if not exists onto_intent (
      id text primary key,
      dsn text not null,
      question text not null,
      status text not null default 'confirmed',
      center_entity text,
      fanout_edges integer not null default 0,
      sql text,
      elapsed_ms integer,
      created_at text not null
    );
    create table if not exists onto_step (
      id text not null,
      step_no integer not null,
      kind text,
      title text,
      payload_json text,
      sql text,
      row_count integer,
      ok integer,
      detail text,
      primary key (id, step_no)
    );
    create table if not exists onto_hit (
      id text not null,
      ord integer not null,
      token text,
      kind text not null,
      target text,
      entity_id text,
      evidence text,
      primary key (id, ord)
    );
    create index if not exists onto_intent_dsn on onto_intent(dsn);
  `);
  // 结构升级：人工修订留痕（0=自动生成 / 1=人工修改；命中可停用、别名可改）
  ensureColumn(db, 'onto_step', 'edited', 'edited integer not null default 0');
  ensureColumn(db, 'onto_hit', 'selected', 'selected integer not null default 1');
  ensureColumn(db, 'onto_hit', 'alias', 'alias text');
  return db;
}

/** 老库文件补列（create table if not exists 不会改已有表结构）。 */
function ensureColumn(db, table, col, ddl) {
  const cols = db.prepare(`pragma table_info('${table}')`).all();
  if (!cols.some((c) => c.name === col)) db.exec(`alter table ${table} add column ${ddl}`);
}

/** 保存一次四步解析痕迹（按 id upsert；steps/hits 整体替换）。 */
export function saveOntologyTrace(body) {
  if (!body || !body.dsn || !body.question) throw new Error('dsn 与 question 必填');
  const id = body.id || `onto-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  const now = new Date().toISOString();
  const db = openDb();
  try {
    db.exec('BEGIN');
    db.prepare(
      `insert into onto_intent (id, dsn, question, status, center_entity, fanout_edges, sql, elapsed_ms, created_at)
       values (?, ?, ?, ?, ?, ?, ?, ?, ?)
       on conflict(id) do update set dsn = excluded.dsn, question = excluded.question,
         status = excluded.status, center_entity = excluded.center_entity,
         fanout_edges = excluded.fanout_edges, sql = excluded.sql, elapsed_ms = excluded.elapsed_ms`,
    ).run(
      id,
      String(body.dsn),
      String(body.question),
      body.status || 'confirmed',
      body.center_entity || null,
      Number(body.fanout_edges) || 0,
      body.sql || null,
      body.elapsed_ms ?? null,
      now,
    );
    db.prepare('delete from onto_step where id = ?').run(id);
    db.prepare('delete from onto_hit where id = ?').run(id);
    for (const s of body.steps || []) {
      db.prepare(
        `insert into onto_step (id, step_no, kind, title, payload_json, sql, row_count, ok, detail, edited)
         values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      ).run(
        id,
        Number(s.step_no) || 0,
        s.kind || null,
        s.title || null,
        s.payload_json || null,
        s.sql || null,
        s.row_count ?? null,
        s.ok ? 1 : 0,
        s.detail || null,
        s.edited ? 1 : 0,
      );
    }
    let ord = 0;
    for (const h of body.hits || []) {
      db.prepare(
        `insert into onto_hit (id, ord, token, kind, target, entity_id, evidence, selected, alias)
         values (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      ).run(
        id,
        h.ord ?? ord,
        h.token || null,
        h.kind,
        h.target || null,
        h.entity_id || null,
        h.evidence || null,
        h.selected === false ? 0 : 1,
        h.alias || null,
      );
      ord++;
    }
    db.exec('COMMIT');
    return { id };
  } catch (e) {
    try {
      db.exec('ROLLBACK');
    } catch {
      /* 忽略 */
    }
    throw e;
  } finally {
    db.close();
  }
}

/** 按数据源列解析历史（新在前）。 */
export function listOntologyTraces(dsn, limit = 50) {
  const db = openDb();
  try {
    const lim = Math.max(1, Math.min(Number(limit) || 50, 200));
    const items = db
      .prepare(
        `select id, question, status, center_entity, fanout_edges, created_at
         from onto_intent where dsn = ? order by created_at desc, rowid desc limit ?`,
      )
      .all(dsn, lim);
    return { ok: true, items, returned: items.length, store: 'sqlite' };
  } finally {
    db.close();
  }
}

/** 读回完整痕迹（steps + hits）。 */
export function getOntologyTrace(id) {
  const db = openDb();
  try {
    const row = db.prepare('select * from onto_intent where id = ?').get(id);
    if (!row) throw new Error(`解析痕迹不存在: ${id}`);
    const steps = db
      .prepare(
        `select step_no, kind, title, payload_json, sql, row_count, ok, detail, edited
         from onto_step where id = ? order by step_no`,
      )
      .all(id);
    const hits = db
      .prepare(
        `select ord, token, kind, target, entity_id, evidence, selected, alias
         from onto_hit where id = ? order by ord`,
      )
      .all(id);
    return { ok: true, ...row, steps, hits };
  } finally {
    db.close();
  }
}

/** 删除一条解析痕迹（含 steps 与 hits）。 */
export function deleteOntologyTrace(id) {
  const db = openDb();
  try {
    const row = db.prepare('select id from onto_intent where id = ?').get(id);
    if (!row) throw new Error(`解析痕迹不存在: ${id}`);
    db.exec('BEGIN');
    db.prepare('delete from onto_step where id = ?').run(id);
    db.prepare('delete from onto_hit where id = ?').run(id);
    db.prepare('delete from onto_intent where id = ?').run(id);
    db.exec('COMMIT');
    return { id };
  } catch (e) {
    try {
      db.exec('ROLLBACK');
    } catch {
      /* 忽略 */
    }
    throw e;
  } finally {
    db.close();
  }
}

// ---- 真实模型图谱（现算，不落库） ----

/** 子列像外键：xxx_id / xxx_code / 中文"代码/编号"结尾；裸 id 太泛不认。 */
function isCodeLike(col) {
  return /(_id$|_code$|代码$|编号$)/i.test(col) && !/^id$/i.test(col);
}

/** 用户拍板（2026-11-02）：本体只看主库 service.* —— workflow 引擎表 / service_backup / 快照 schema 全部排除。 */
const ONLY_SCHEMA = 'service';

/**
 * 从数据库现算本体图谱：
 * - 真实外键 → 边（cardinality '1:N'，evidence 外键约束）
 * - 同名列候选 → 边（cardinality '候选'，evidence 写明依据；仅当该列恰是另一张表的主键）
 * - 实体 = 出现在任一条边两端的表（名字优先取表注释）
 */
export async function buildOntologyGraph(dsn) {
  const regPath = path.resolve(__dirname, '../examples/library/.datasources.json');
  let d;
  try {
    const all = JSON.parse(fs.readFileSync(regPath, 'utf8'));
    d = all[dsn];
  } catch {
    d = null;
  }
  if (!d) throw new Error(`数据源不存在: ${dsn}`);
  if (d.driver !== 'postgres') {
    return { has_model: false, ontology: null, reason: `真图谱目前只支持 postgres（${d.driver} 无关系可推）` };
  }
  if (!d.host || !Number(d.port)) throw new Error('主机/端口未配置');

  const conn = new PgConn(d);
  try {
    await conn.connect();

    // 真实外键（只留主库 service.*，去重到表对）
    const fk = await conn.query(
      `select tc.table_schema || '.' || tc.table_name as fk_table,
              ccu.table_schema || '.' || ccu.table_name as ref_table
       from information_schema.table_constraints tc
       join information_schema.key_column_usage kcu
         on tc.constraint_name = kcu.constraint_name and tc.table_schema = kcu.table_schema
       join information_schema.constraint_column_usage ccu
         on ccu.constraint_name = tc.constraint_name and ccu.table_schema = tc.table_schema
       where tc.constraint_type = 'FOREIGN KEY'
         and tc.table_schema = '${ONLY_SCHEMA}'
       group by 1, 2`,
    );

    // 主键映射（仅主库）：表 → 主键列集合
    const pk = await conn.query(
      `select tc.table_schema || '.' || tc.table_name as tbl, kcu.column_name as col
       from information_schema.table_constraints tc
       join information_schema.key_column_usage kcu
         on tc.constraint_name = kcu.constraint_name and tc.table_schema = kcu.table_schema
       where tc.constraint_type = 'PRIMARY KEY'
         and tc.table_schema = '${ONLY_SCHEMA}'`,
    );

    // 主库全部表的列
    const cols = await conn.query(
      `select table_schema || '.' || table_name as tbl, column_name as col
       from information_schema.columns
       where table_schema = '${ONLY_SCHEMA}'`,
    );

    // 表注释（实体显示名；含视图/物化视图）
    let comments = new Map();
    try {
      const cm = await conn.query(
        `select n.nspname || '.' || c.relname as tbl, d.description as comment
         from pg_class c
         join pg_namespace n on n.oid = c.relnamespace
         left join pg_description d on d.objoid = c.oid and d.objsubid = 0
         where n.nspname = '${ONLY_SCHEMA}' and c.relkind in ('r', 'v', 'm')`,
      );
      comments = new Map(cm.rows.map((r) => [r[0], r[1]]));
    } catch {
      /* 注释拿不到就用表名 */
    }

    // ---- 组边 ----
    const edgeMap = new Map(); // from|to -> {from,to,cardinality,evidence:Set}
    const PRIO = { '1:N': 3, '源码': 2, '候选': 1 };
    const addEdge = (from, to, cardinality, evidence) => {
      if (!from || !to || from === to) return;
      const k = `${from}|${to}`;
      if (!edgeMap.has(k)) edgeMap.set(k, { from, to, cardinality, evidence: new Set() });
      const e = edgeMap.get(k);
      if ((PRIO[cardinality] || 0) > (PRIO[e.cardinality] || 0)) e.cardinality = cardinality;
      e.evidence.add(evidence);
    };

    for (const [fkTable, refTable] of fk.rows) {
      addEdge(refTable, fkTable, '1:N', '外键约束');
    }

    // 同名列候选：列 C 代码类 + 恰是同 schema 某表 R 的主键 + 出现在同 schema 的表 T
    const pkByTable = new Map(); // tbl -> Set(colLower)
    const pkOwners = new Map(); // colLower -> [{tbl, schema}]
    for (const [tbl, col] of pk.rows) {
      const cl = String(col).toLowerCase();
      if (!pkByTable.has(tbl)) pkByTable.set(tbl, new Set());
      pkByTable.get(tbl).add(cl);
      if (!pkOwners.has(cl)) pkOwners.set(cl, []);
      pkOwners.get(cl).push({ tbl, schema: tbl.split('.')[0] });
    }
    const fkPairSet = new Set(fk.rows.map(([a, b]) => `${b}|${a}`));
    const colCount = new Map(); // colLower -> 出现次数（防爆炸）
    const colRows = cols.rows.map(([tbl, col]) => ({ tbl, col, schema: tbl.split('.')[0] }));
    for (const { col } of colRows) {
      const cl = String(col).toLowerCase();
      colCount.set(cl, (colCount.get(cl) || 0) + 1);
    }
    for (const { tbl, col, schema } of colRows) {
      const cl = String(col).toLowerCase();
      if (!isCodeLike(cl)) continue;
      const owners = (pkOwners.get(cl) || []).filter((o) => o.schema === schema && o.tbl !== tbl);
      if (!owners.length || (colCount.get(cl) || 0) > 30) continue; // 泛列不推
      for (const owner of owners) {
        const k = `${owner.tbl}|${tbl}`;
        if (fkPairSet.has(k)) continue; // 已有真外键，不重复推
        addEdge(owner.tbl, tbl, '候选', `同名列 ${col}：${owner.tbl} 的主键，出现在 ${tbl}`);
      }
    }

    // 源码确认边（gzxt-main Java 源码里的 SQL join，静态证据；优先级：真外键 > 源码 > 候选）
    try {
      const srcPath = path.resolve(__dirname, '../examples/library/.ontology-edges.json');
      const src = JSON.parse(fs.readFileSync(srcPath, 'utf8'));
      for (const e of src.edges || []) {
        if (!e.from || !e.to) continue;
        if (!e.from.startsWith(`${ONLY_SCHEMA}.`) || !e.to.startsWith(`${ONLY_SCHEMA}.`)) continue;
        addEdge(e.from, e.to, e.cardinality || '源码', e.evidence || '源码 join');
      }
    } catch {
      /* 没有源码证据文件就跳过 */
    }

    // 排序：真外键 > 源码 > 候选；总量封顶
    const edges = [...edgeMap.values()]
      .map((e) => ({ from: e.from, to: e.to, cardinality: e.cardinality, evidence: [...e.evidence].join('；') }))
      .sort((a, b) => {
        if (a.cardinality !== b.cardinality) return (PRIO[b.cardinality] || 0) - (PRIO[a.cardinality] || 0);
        return `${a.from}${a.to}` < `${b.from}${b.to}` ? -1 : 1;
      })
      .slice(0, 120);

    // 实体 = 主库全部基表 + 全部视图/物化视图（用户拍板：service.* 所有的都进目录）
    const allTables = await conn.query(
      `select table_name from information_schema.tables
       where table_schema = '${ONLY_SCHEMA}' and table_type = 'BASE TABLE'
       order by table_name`,
    );
    const allViews = await conn.query(
      `select c.relname from pg_class c join pg_namespace n on n.oid = c.relnamespace
       where n.nspname = '${ONLY_SCHEMA}' and c.relkind in ('v', 'm')
       order by c.relname`,
    );
    // 视图依赖（pg_rewrite/pg_depend）：视图由哪些基表/视图派生 —— 只进 MD 的视图清单，不做图谱边（2387 对会爆）
    const depRows = await conn.query(
      `select distinct v.nspname || '.' || vc.relname as view_name,
              t.nspname || '.' || tc.relname as ref_table
       from pg_depend d
       join pg_rewrite r on r.oid = d.objid
       join pg_class vc on vc.oid = r.ev_class
       join pg_namespace v on v.oid = vc.relnamespace
       join pg_class tc on tc.oid = d.refobjid
       join pg_namespace t on t.oid = tc.relnamespace
       where vc.relkind in ('v', 'm') and v.nspname = '${ONLY_SCHEMA}'
         and t.nspname = '${ONLY_SCHEMA}' and tc.relkind in ('r', 'v')
       order by 1, 2`,
    );
    const viewDeps = {};
    for (const [viewName, refTable] of depRows.rows) {
      if (!viewDeps[viewName]) viewDeps[viewName] = [];
      viewDeps[viewName].push(refTable);
    }
    const tableIds = allTables.rows.map((r) => `${ONLY_SCHEMA}.${r[0]}`);
    const viewIds = allViews.rows.map((r) => `${ONLY_SCHEMA}.${r[0]}`);
    const entities = [
      ...tableIds.map((id) => ({ id, name: comments.get(id) || id, table: id, status: 'confirmed', kind: 'table' })),
      ...viewIds.map((id) => ({ id, name: comments.get(id) || id, table: id, status: 'confirmed', kind: 'view' })),
    ];

    return {
      has_model: entities.length > 0,
      ontology: { entities, edges, metrics: [], dims: [] },
      viewDeps,
    };
  } finally {
    conn.end();
  }
}

/**
 * 真模型词表：从库里现算"维度 / 度量 / 时间 / 实体"四类词汇，给四步解析用。
 * - 词 = 列的中文注释（本表注释优先，没有注释的列不进词表——避免噪音）
 * - 度量 = 数值类型列（numeric/int/float 族），聚合默认 SUM
 * - 维度 = 其余类型列；时间列（注释/列名含 年月/时间/日期）单列一类
 * 仅 postgres；sqlite 走示例夹具。
 */
// 词表缓存（60s）：Agent/独立系统高频调用自动解析时不用每次全库扫
let LEX_CACHE = { at: 0, dsn: null, lex: null };

export async function buildLexicon(dsn, useCache = true) {
  if (useCache && LEX_CACHE.dsn === dsn && LEX_CACHE.lex && Date.now() - LEX_CACHE.at < 60_000) {
    return LEX_CACHE.lex;
  }
  const regPath = path.resolve(__dirname, '../examples/library/.datasources.json');
  let d;
  try {
    d = JSON.parse(fs.readFileSync(regPath, 'utf8'))[dsn];
  } catch {
    d = null;
  }
  if (!d) throw new Error(`数据源不存在: ${dsn}`);
  if (d.driver !== 'postgres') {
    return { ok: true, mode: 'none', reason: `真模型词表目前只支持 postgres（${d.driver} 走示例夹具）` };
  }
  const conn = new PgConn(d);
  try {
    await conn.connect();
    // 有中文注释的列（排除"注释=列名"垃圾）
    const cols = await conn.query(
      `select c.relname, c.relkind, a.attname, format_type(a.atttypid, a.atttypmod), trim(d.description)
       from pg_attribute a
       join pg_class c on c.oid = a.attrelid
       join pg_namespace n on n.oid = c.relnamespace
       join pg_description d on d.objoid = c.oid and d.objsubid = a.attnum
       where n.nspname = '${ONLY_SCHEMA}' and c.relkind in ('r', 'v')
         and a.attnum > 0 and not a.attisdropped
         and length(trim(d.description)) > 0
         and lower(trim(d.description)) <> lower(a.attname)
       order by c.relname, a.attnum`,
    );
    // 表/视图注释
    const tc = await conn.query(
      `select c.relname, c.relkind, trim(d.description)
       from pg_class c
       join pg_namespace n on n.oid = c.relnamespace
       join pg_description d on d.objoid = c.oid and d.objsubid = 0
       where n.nspname = '${ONLY_SCHEMA}' and c.relkind in ('r', 'v', 'm')
         and length(trim(d.description)) > 0`,
    );
    const NUMERIC = /^(smallint|integer|bigint|numeric|real|double precision|money)/i;
    const TIMEish = /(年月|月份|时间|日期|月份)/;
    const JUNK = /(^del_|(_bak|_backup|_copy\d?|_tmp)$|_\d{6})/i;
    const dims = [];
    const metrics = [];
    const times = [];
    const seen = new Set();
    for (const [relname, relkind, col, dataType, cn] of cols.rows) {
      const landing = `${ONLY_SCHEMA}.${relname}.${col}`;
      const table = `${ONLY_SCHEMA}.${relname}`;
      const token = String(cn).trim();
      if (token.length < 2 || token.length > 14) continue;
      const key = `${token}|${landing}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const kindMark = relkind === 'v' ? '（视图）' : '';
      const junk = JUNK.test(relname) ? 1 : 0;
      if (TIMEish.test(token) || /^(ym|date|time|create_time|update_time)$/i.test(col)) {
        times.push({ token, landing, table, evidence: `注释「${token}」${kindMark}`, junk });
      } else if (NUMERIC.test(dataType)) {
        metrics.push({ token, landing, table, agg: 'sum', evidence: `注释「${token}」，数值列 ${dataType}`, junk });
      } else {
        dims.push({ token, landing, table, evidence: `注释「${token}」${kindMark}`, junk });
      }
    }
    const entities = tc.rows.map(([relname, relkind, cn]) => ({
      id: `${ONLY_SCHEMA}.${relname}`,
      name: cn || `${ONLY_SCHEMA}.${relname}`,
      kind: relkind === 'r' ? 'table' : 'view',
    }));
    const lexResult = {
      ok: true,
      mode: 'real',
      dims,
      metrics,
      times,
      entities,
      counts: { dims: dims.length, metrics: metrics.length, times: times.length, entities: entities.length },
    };
    LEX_CACHE = { at: Date.now(), dsn, lex: lexResult };
    return lexResult;
  } finally {
    conn.end();
  }
}

// ---- 自动解析（独立系统 / AI Agent 的 API 入口） ----

// ---- 标准问题库（权威口径优先于词表临时拼装） ----

const CANON_PATH = path.resolve(__dirname, '../examples/library/.ontology-canon.sqls.json');

/** 按问题原文匹配标准问题库（按顺序取第一个命中），返回 {id,title,sql,basis,notes,center_entity,match} 或 null。 */
export function matchCanon(question) {
  const q = String(question || '').trim();
  if (!q) return null;
  let items = [];
  try {
    items = JSON.parse(fs.readFileSync(CANON_PATH, 'utf8')).items || [];
  } catch {
    return null;
  }
  for (const it of items) {
    try {
      if (it.match && new RegExp(it.match).test(q)) return it;
    } catch {
      /* 坏正则跳过 */
    }
  }
  return null;
}

const AUTO_JUNK = /(^del_|(_bak|_backup|_copy|_tmp)\d*$|_\d{6})/i;
const AUTO_HUB = 'service.dwxxb'; // 维度枢纽表（源码分析结论）：同分时优先

/**
 * 自动解析一句话 → 命中 + 中心表 + SQL + 自动留痕。对外入口（重试一次，远端库偶发抖动不直接失败）。
 * 规则与前端 resolveFromLexicon 同一套（exact/去尾字模糊、备份表降权、dwxxb 枢纽加分、按词去重、
 * 度量只取中心表自己的列）——改这里记得同步 app/src/services/ontology.ts。
 * opts.save=false 只算不留痕（默认留，status='auto'）。
 */
export async function autoParse(dsn, question, opts = {}) {
  let lastErr;
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      return await autoParseOnce(dsn, question, opts);
    } catch (e) {
      lastErr = e;
    }
  }
  throw lastErr;
}

async function autoParseOnce(dsn, question, opts = {}) {
  if (!question || !String(question).trim()) throw new Error('question 必填');
  // 标准口径优先：有权威 SQL 的经典问题直接命中，不用词表临时拼装
  const canon = matchCanon(question);
  if (canon) {
    const result = {
      dsn,
      question: String(question).trim(),
      mode: 'canon',
      canon: { id: canon.id, title: canon.title },
      center: canon.center_entity || null,
      sql: canon.sql,
      dims: [],
      metrics: [],
      times: [],
      entities: canon.center_entity ? [canon.center_entity] : [],
      notes: [canon.basis, ...(canon.notes || [])],
    };
    if (opts.save !== false) {
      result.trace_id = saveOntologyTrace({
        dsn,
        question: result.question,
        status: 'auto',
        center_entity: canon.center_entity || null,
        fanout_edges: 0,
        sql: canon.sql,
        steps: [
          { step_no: 1, kind: 'identify', title: `标准口径命中（${canon.title}）`, detail: canon.basis, edited: 0 },
          { step_no: 2, kind: 'center', title: '定主表', detail: `中心实体 ${canon.center_entity || '—'}`, edited: 0 },
          { step_no: 4, kind: 'compose', title: '组装输出', sql: canon.sql, detail: '标准口径 SQL（权威）', edited: 0 },
        ],
        hits: [],
      }).id;
    }
    return result;
  }
  const lex = await buildLexicon(dsn);
  if (lex.mode !== 'real') throw new Error('该数据源没有真模型词表，无法自动解析');
  const q = String(question).trim();

  const scan = (words, kind) => {
    const out = [];
    for (const w of words) {
      if (!w.token || w.token.length < 2) continue;
      const junk = AUTO_JUNK.test(w.table);
      let weak = false;
      let hitLen = 0;
      if (q.includes(w.token)) hitLen = w.token.length;
      else if (w.token.length >= 3 && q.includes(w.token.slice(0, -1))) {
        hitLen = w.token.length - 1;
        weak = true;
      }
      if (!hitLen) continue;
      out.push({
        token: w.token,
        kind,
        landing: w.landing,
        table: w.table,
        evidence: (weak ? '模糊命中（词尾不同）：' : '') + w.evidence + (junk ? '（备份/临时表，建议换正表）' : ''),
        _score: hitLen - (junk ? 10 : 0) + (w.table === AUTO_HUB ? 2 : 0),
      });
    }
    return out.sort((a, b) => b._score - a._score);
  };

  const dimHits = scan(lex.dims, 'dim');
  const metricHitsRaw = scan(lex.metrics, 'metric');
  const timeHits = scan(lex.times, 'time');

  const seenToken = new Set();
  const dims = dimHits.filter((h) => !seenToken.has(h.token) && seenToken.add(h.token)).slice(0, 3);
  const seenLanding = new Set();
  const metsAll = metricHitsRaw.filter((h) => !seenLanding.has(h.landing) && seenLanding.add(h.landing));
  const center = dims[0]?.table ?? metsAll[0]?.table ?? null;
  const mets = metsAll.filter((m) => m.table === center).slice(0, 6);
  const entities = [...new Set([...dimHits, ...metricHitsRaw, ...timeHits].map((h) => h.table))];

  let sql = '';
  if (center) {
    const sel = [];
    const groupCols = [];
    for (const d of dims) {
      const col = d.landing.split('.')[2];
      sel.push(`t."${col}" AS "${d.token}"`);
      groupCols.push(`t."${col}"`);
    }
    sel.push('COUNT(*) AS "行数"');
    for (const m of mets) {
      const col = m.landing.split('.')[2];
      sel.push(`SUM(t."${col}") AS "${m.token}合计"`);
    }
    const lines = ['SELECT', sel.map((s) => '  ' + s).join(',\n'), `FROM ${center} t`];
    if (groupCols.length) {
      lines.push('GROUP BY', '  ' + groupCols.join(',\n  '), 'ORDER BY', '  ' + groupCols[0]);
    }
    sql = lines.join('\n') + ';';
  }

  const notes = [];
  if (!center) notes.push('没有命中任何维度/度量词 —— 请换说法，或先给相关表/列补中文注释');
  else if (entities.length > 1) notes.push(`命中 ${entities.length} 张表，SQL 只用中心表 —— 跨表聚合需人工确认关联键后再 JOIN`);
  if (timeHits.length) notes.push(`命中时间词但未生成过滤条件 —— 时间窗口请自行加 WHERE`);

  const result = {
    dsn,
    question: q,
    center,
    sql,
    dims: dims.map(({ token, landing, evidence }) => ({ token, landing, evidence })),
    metrics: mets.map(({ token, landing, evidence }) => ({ token, landing, evidence })),
    times: timeHits.map(({ token, landing, evidence }) => ({ token, landing, evidence })),
    entities,
    notes,
  };

  if (opts.save !== false) {
    result.trace_id = saveOntologyTrace({
      dsn,
      question: q,
      status: 'auto',
      center_entity: center,
      fanout_edges: 0,
      sql,
      steps: [
        { step_no: 1, kind: 'identify', title: '自动解析（Agent API）', detail: `命中 ${dims.length + mets.length + timeHits.length} 条`, edited: 0 },
        { step_no: 2, kind: 'center', title: '定主表', detail: center ? `中心表 ${center}` : '未命中', edited: 0 },
        { step_no: 4, kind: 'compose', title: '组装输出', sql, detail: 'API 自动生成', edited: 0 },
      ],
      hits: [...dims, ...mets, ...timeHits].map((h, i) => ({
        ord: i,
        token: h.token,
        kind: h.kind,
        target: h.landing,
        evidence: h.evidence,
      })),
    }).id;
  }
  return result;
}

/**
 * 导出本体为 Markdown（为后续工作/上下文准备）。
 * 内容：整体说明（口径与可信度）+ 实体清单 + 关系清单（含依据）。
 * 文件落在 rpx/docs/ontology-<dsn>.md，同时返回内容便于前端直接展示。
 */
export async function exportOntologyMd(dsn) {
  const g = await buildOntologyGraph(dsn);
  if (!g.has_model || !g.ontology) throw new Error(`数据源 ${dsn} 没有可导出的本体`);
  const { entities, edges } = g.ontology;
  const viewDeps = g.viewDeps || {};
  const views = entities.filter((e) => e.kind === 'view');
  const tables = entities.filter((e) => e.kind !== 'view');

  // 源码笔记：实体↔表映射 + 业务组说明（E:/project/gzxt-main 静态分析的沉淀）
  let code = null;
  try {
    code = JSON.parse(fs.readFileSync(path.resolve(__dirname, '../examples/library/.ontology-code-notes.json'), 'utf8'));
  } catch {
    code = null;
  }
  const codeEntity = (id) => {
    if (!code?.entities) return '';
    const tbl = id.split('.').slice(1).join('.');
    const hit = code.entities[tbl.toLowerCase()];
    return hit ? `${hit.cls}（${hit.pkg}）` : '';
  };

  const by = { '1:N': 0, '源码': 0, '候选': 0 };
  edges.forEach((e) => {
    by[e.cardinality] = (by[e.cardinality] || 0) + 1;
  });
  const now = new Date().toLocaleString('zh-CN', { hour12: false });
  const relCount = (id) => edges.filter((x) => x.from === id || x.to === id).length;

  const lines = [];
  lines.push(`# 本体 · 实体与关系（${dsn}）`);
  lines.push('');
  lines.push(`> 生成时间：${now}　|　数据源：\`${dsn}\`　|　口径：主库 \`service.*\` 全部基表`);
  lines.push('> 来源：连库现算 —— **真实外键** + **gzxt-main 源码确认**（Java SQL join，证据带 file:line）+ **同名列候选**（代码列恰为另一表主键才推断）');
  lines.push('> 关系可信度：真外键 > 源码确认 > 同名列候选；候选关系使用前建议人工确认。');
  lines.push('');
  lines.push('## 整体说明');
  lines.push('');
  lines.push(`- **实体 ${entities.length} 个**：主库 service.* 全部基表 ${tables.length} 个 + 视图/物化视图 ${views.length} 个，名称取库内注释（无注释则显示表名）。`);
  lines.push(`- **关系 ${edges.length} 条**：真外键 ${by['1:N']} · 源码确认 ${by['源码']} · 同名列候选 ${by['候选']}。`);
  lines.push('- **维度枢纽**：`dwxxb`（单位信息表）——源码中 join 48 处（41 处经 current_dwxxb CTE 按 dwdm 相连），几乎全部工资业务表靠它取单位信息。');
  lines.push('- **五类人员工资主表**：t101（在职在编）/ tcgz1（事业在职）/ tcltgz1（离退）/ t103 / tgygz，各自配有 *lsb 历史留痕表（按 主键+身份证+上月ym 相连做环比）。');
  lines.push('- **对齐键约定**：dwdm=单位代码、sfzh=身份证号、ym=工资月份（t101↔jcjxb 按人+单位+月三键对齐）。');
  lines.push('');
  lines.push('## 源码补充说明（gzxt-main 代码分析）');
  lines.push('');
  if (code) {
    lines.push(`> 分析对象：\`${code.source}\`。Spring Boot + JPA 多模块（gongzi-service=工资主模块：44 实体 / 43 service / 38 repository；SQL 多写在 service 层字符串里）。`);
    lines.push('');
    for (const grp of code.groups || []) {
      lines.push(`### ${grp.name}`);
      lines.push('');
      lines.push(`涉及表：${(grp.tables || []).map((t) => `\`${t}\``).join('、')}`);
      lines.push('');
      lines.push(grp.note);
      lines.push('');
    }
    if (Array.isArray(code.ima_sources) && code.ima_sources.length) {
      lines.push('### 资料来源（ima「gz」知识库）');
      lines.push('');
      for (const s of code.ima_sources) {
        lines.push(`- ${s}`);
      }
      lines.push('');
    }
    // 数据校验规则（业务闸门）
    const vr = code.validation_rules;
    if (vr && Array.isArray(vr.groups)) {
      lines.push(`### 数据校验规则（业务闸门，出自《${(vr.source || '').split('（')[0]}》）`);
      lines.push('');
      for (const grp of vr.groups) {
        lines.push(`**${grp.name}**`);
        lines.push('');
        for (const r of grp.rules || []) {
          lines.push(`- ${r}`);
        }
        lines.push('');
      }
    }
    // 主页单位上报（dashboard 真实口径；统计视图 v_tj 是最终权威答案）
    const db = code.dashboard;
    if (db) {
      lines.push(`### 单位上报情况（dashboard 口径，出自 ${db.source || 'gzxt-main 源码'}）`);
      lines.push('');
      lines.push(`- **状态机**：${(db.states && db.states.zt) || ''}`);
      lines.push(`- **财政类型**：${(db.states && db.states.lx) || ''}`);
      lines.push(`- **状态真列**：${db.columns || ''}`);
      lines.push(`- **判定规则**：${db.gates || ''}`);
      lines.push('');
      if (db.summary_sql) {
        lines.push('**单位上报统计 · 汇总口径（dashboard 原始实现，5 行 × 6 列）**');
        lines.push('');
        lines.push('```sql');
        lines.push(
          db.summary_sql.replace(
            /^dashboard 单位上报统计（5 行汇总 × 6 列，用户提供的原始 SQL）：\n/,
            '',
          ),
        );
        lines.push('```');
        lines.push('');
      }
      if (db.classic_sql) {
        lines.push('**经典问题：按所属镇街统计单位上报情况 —— 分镇区 SQL（统计视图 v_tj）**');
        lines.push('');
        lines.push('```sql');
        lines.push(
          db.classic_sql.replace(
            /^按所属镇街统计单位上报情况 —— 分镇区的答案查统计视图 service\.v_tj（UNION ALL 三分类：1\.工资 \/ 2\.基础绩效 \/ 3\.税金，按 sszq 分组透视 已确认\/待处理\/下载\/总数）：\n/,
            '',
          ),
        );
        lines.push('```');
        lines.push('');
      }
    }
  } else {
    lines.push('（未找到 .ontology-code-notes.json 源码笔记文件）');
    lines.push('');
  }
  lines.push('## 实体清单');
  lines.push('');
  if (code?.entities) {
    lines.push('| 实体 | 中文名（表注释） | 类型 | 代码实体类 | 关系数 |');
    lines.push('|---|---|---|---|---|');
    for (const e of entities) {
      lines.push(`| ${e.id} | ${e.name === e.id ? '—' : e.name} | ${e.kind === 'view' ? '视图' : '表'} | ${codeEntity(e.id) || '—'} | ${relCount(e.id)} |`);
    }
  } else {
    lines.push('| 实体 | 中文名（表注释） | 类型 | 关系数 |');
    lines.push('|---|---|---|---|');
    for (const e of entities) {
      lines.push(`| ${e.id} | ${e.name === e.id ? '—' : e.name} | ${e.kind === 'view' ? '视图' : '表'} | ${relCount(e.id)} |`);
    }
  }
  lines.push('');
  // 视图清单：视图 + 依赖的基表/视图（pg_rewrite 真实派生关系）
  if (views.length) {
    lines.push(`## 视图清单（${views.length} 个，含物化视图）`);
    lines.push('');
    lines.push('> 依赖关系来自 pg_rewrite/pg_depend（视图定义真实引用），每视图列出其派生来源。');
    lines.push('');
    lines.push('| 视图 | 中文名（注释） | 依赖的基表/视图 |');
    lines.push('|---|---|---|');
    for (const v of views) {
      const deps = viewDeps[v.id] || [];
      const shown = deps.slice(0, 12).map((d) => `\`${d}\``).join('、');
      lines.push(`| ${v.id} | ${v.name === v.id ? '—' : v.name} | ${shown}${deps.length > 12 ? ` …（共 ${deps.length} 个）` : ''} |`);
    }
    lines.push('');
  }
  lines.push('## 关系清单');
  lines.push('');
  lines.push('| 1 端 | N 端 | 类型 | 依据 |');
  lines.push('|---|---|---|---|');
  for (const e of edges) {
    lines.push(`| ${e.from} | ${e.to} | ${e.cardinality} | ${e.evidence || '—'} |`);
  }
  lines.push('');
  lines.push('## 源码证据索引（gzxt-main）');
  lines.push('');
  lines.push('- `ConfirmService.java:105/312/328/336/343` —— t101↔dwxxb、t101↔jcjxb、四对主表→历史表');
  lines.push('- `DwCzallService.java:616` —— jcjxb↔dwxxb');
  lines.push('- `TaxService.java:87` —— tax↔dwxxb');
  lines.push('- `GzTfsqbService.java:89` —— gz_tfsqb↔dwxxb');
  lines.push('');
  const md = lines.join('\n');

  const outDir = path.resolve(__dirname, '../docs');
  fs.mkdirSync(outDir, { recursive: true });
  const outPath = path.join(outDir, `ontology-${dsn}.md`);
  fs.writeFileSync(outPath, md, 'utf8');
  return { path: outPath, md };
}

// ---------------------------------------------------------------------------
// NVIDIA Auto-Ontology 体系：自动逆向推导、阶段准入认证 (Candidate -> Certified) 与 YAML 规范导出
// ---------------------------------------------------------------------------

/**
 * 自动逆向发现本体 (Automated Baseline Ontology Discovery)：
 * 借鉴 NVIDIA Auto-Ontology 阶段 1 流水线：
 * 扫描数据库表、主外键、列类型与列注释，自动推导出 Entities、Measures、Dimensions 及关系拓扑图。
 */
export async function autoDiscoverOntology(dsn, options = {}) {
  const regPath = path.resolve(__dirname, '../examples/library/.datasources.json');
  let d;
  try {
    d = JSON.parse(fs.readFileSync(regPath, 'utf8'))[dsn];
  } catch {
    d = null;
  }
  if (!d) {
    // 降级检查 demo_data/datasources.json
    try {
      d = JSON.parse(fs.readFileSync(path.resolve(__dirname, '../demo_data/datasources.json'), 'utf8'))[dsn];
    } catch {
      d = null;
    }
  }
  if (!d) throw new Error(`数据源不存在: ${dsn}`);

  const entities = [];
  const measures = [];
  const dimensions = [];
  const relationships = [];

  const MEASURE_KEYWORDS = /(amount|price|salary|fee|cost|val|qty|cnt|count|total|sum|yfgz|sfgz|gz|money|rate|ratio|金额|工资|总额|数量|费用|差价|毛利)/i;
  const TIME_KEYWORDS = /(date|time|quarter|month|year|ym|created_at|updated_at|day|时间|日期|月份|季度|年份)/i;
  const ID_KEYWORDS = /(_id$|_code$|代码$|编号$|^id$)/i;

  if (d.driver === 'postgres') {
    if (!d.host || !Number(d.port)) throw new Error('主机/端口未配置');
    const conn = new PgConn(d);
    try {
      await conn.connect();
      const allTables = await conn.query(
        `select table_name, table_type from information_schema.tables
         where table_schema = '${ONLY_SCHEMA}'
         order by table_name`,
      );
      const cols = await conn.query(
        `select table_name, column_name, data_type, is_nullable
         from information_schema.columns
         where table_schema = '${ONLY_SCHEMA}'`,
      );
      const pks = await conn.query(
        `select tc.table_name, kcu.column_name
         from information_schema.table_constraints tc
         join information_schema.key_column_usage kcu
           on tc.constraint_name = kcu.constraint_name and tc.table_schema = kcu.table_schema
         where tc.constraint_type = 'PRIMARY KEY' and tc.table_schema = '${ONLY_SCHEMA}'`,
      );
      const fks = await conn.query(
        `select tc.table_name as fk_table, ccu.table_name as ref_table, kcu.column_name as fk_col
         from information_schema.table_constraints tc
         join information_schema.key_column_usage kcu
           on tc.constraint_name = kcu.constraint_name and tc.table_schema = kcu.table_schema
         join information_schema.constraint_column_usage ccu
           on ccu.constraint_name = tc.constraint_name and ccu.table_schema = tc.table_schema
         where tc.constraint_type = 'FOREIGN KEY' and tc.table_schema = '${ONLY_SCHEMA}'`,
      );

      let comments = new Map();
      try {
        const cm = await conn.query(
          `select c.relname as tbl, d.description as comment
           from pg_class c
           join pg_namespace n on n.oid = c.relnamespace
           left join pg_description d on d.objoid = c.oid and d.objsubid = 0
           where n.nspname = '${ONLY_SCHEMA}' and c.relkind in ('r', 'v', 'm')`,
        );
        comments = new Map(cm.rows.map((r) => [r[0], r[1]]));
      } catch {
        // 注释拿不到忽略
      }

      const pkMap = new Map();
      for (const [tbl, col] of pks.rows) {
        if (!pkMap.has(tbl)) pkMap.set(tbl, []);
        pkMap.get(tbl).push(col);
      }

      const colsByTable = new Map();
      for (const [tbl, col, type, nullable] of cols.rows) {
        if (!colsByTable.has(tbl)) colsByTable.set(tbl, []);
        colsByTable.get(tbl).push({ col, type, nullable });
      }

      // 组装 Entities
      for (const [tbl, tblType] of allTables.rows) {
        const id = `${ONLY_SCHEMA}.${tbl}`;
        const name = comments.get(tbl) || tbl;
        const kind = tblType === 'VIEW' ? 'view' : 'table';
        const pk = pkMap.get(tbl) || ['id'];
        entities.push({
          id,
          name,
          table: id,
          kind,
          primary_key: pk,
          status: 'confirmed',
          columns_count: (colsByTable.get(tbl) || []).length,
        });

        // 识别度量与维度
        for (const { col, type } of colsByTable.get(tbl) || []) {
          const isNum = /(int|numeric|decimal|float|double|bigint)/i.test(type);
          const isDate = /(date|timestamp|time)/i.test(type) || TIME_KEYWORDS.test(col);
          const isKey = ID_KEYWORDS.test(col);

          if (isNum && !isKey && MEASURE_KEYWORDS.test(col)) {
            measures.push({
              id: `${tbl}_${col}`,
              label: `${col} (${name})`,
              entity: id,
              column: col,
              data_type: type,
              default_aggregation: /rate|ratio|percent|比例|率/i.test(col) ? 'AVG' : 'SUM',
              expression: `SUM(${tbl}."${col}")`,
              status: 'candidate',
            });
          } else if (isDate) {
            dimensions.push({
              id: `${tbl}_${col}`,
              label: `${col} (${name})`,
              entity: id,
              column: col,
              kind: 'time',
              grain: /month|ym|月/i.test(col) ? 'month' : /year|年/i.test(col) ? 'year' : 'day',
              status: 'candidate',
            });
          } else if (!isKey) {
            dimensions.push({
              id: `${tbl}_${col}`,
              label: `${col} (${name})`,
              entity: id,
              column: col,
              kind: 'categorical',
              status: 'candidate',
            });
          }
        }
      }

      // 组装外键边
      for (const [fkTable, refTable, fkCol] of fks.rows) {
        relationships.push({
          from: `${ONLY_SCHEMA}.${refTable}`,
          to: `${ONLY_SCHEMA}.${fkTable}`,
          cardinality: '1:N',
          join_key: `${refTable}.id = ${fkTable}.${fkCol}`,
          status: 'confirmed',
          evidence: `外键约束 (${fkTable}.${fkCol} ➔ ${refTable})`,
        });
      }
    } finally {
      conn.end();
    }
  } else {
    // SQLite 处理分支（包括 demo-sqlite）
    let dbFile = d.database || 'demo_data/report_demo.db';
    if (dbFile.startsWith('sqlite://')) dbFile = dbFile.replace('sqlite://', '');
    const absPath = path.resolve(__dirname, '..', dbFile);
    if (!fs.existsSync(absPath)) throw new Error(`SQLite 库文件不存在: ${absPath}`);

    const sdb = new DatabaseSync(absPath);
    try {
      const tblRows = sdb.prepare("SELECT name, type FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'").all();
      for (const { name: tbl, type: tblType } of tblRows) {
        const colInfos = sdb.prepare(`PRAGMA table_info("${tbl}")`).all();
        const pkCols = colInfos.filter((c) => c.pk > 0).map((c) => c.name);
        const fkInfos = sdb.prepare(`PRAGMA foreign_key_list("${tbl}")`).all();

        entities.push({
          id: tbl,
          name: tbl === 'sales' ? '销售流水表 (sales)' : tbl === 'emp' ? '员工档案表 (emp)' : tbl,
          table: tbl,
          kind: tblType,
          primary_key: pkCols.length ? pkCols : ['id'],
          status: 'confirmed',
          columns_count: colInfos.length,
        });

        for (const col of colInfos) {
          const colName = col.name;
          const colType = String(col.type || '').toUpperCase();
          const isNum = /(INT|REAL|NUM|FLOAT|DEC|DOUB)/i.test(colType);
          const isDate = TIME_KEYWORDS.test(colName);
          const isKey = ID_KEYWORDS.test(colName);

          if (isNum && !isKey && MEASURE_KEYWORDS.test(colName)) {
            measures.push({
              id: `${tbl}_${colName}`,
              label: colName === 'amount' ? '销售金额 (元)' : colName === 'qty' ? '销售数量 (件)' : colName,
              entity: tbl,
              column: colName,
              data_type: colType,
              default_aggregation: 'SUM',
              expression: `SUM(${tbl}."${colName}")`,
              status: 'candidate',
            });
          } else if (isDate) {
            dimensions.push({
              id: `${tbl}_${colName}`,
              label: colName === 'quarter' ? '销售季度' : colName === 'month' ? '业务月份' : colName,
              entity: tbl,
              column: colName,
              kind: 'time',
              grain: colName === 'quarter' ? 'quarter' : 'month',
              status: 'candidate',
            });
          } else if (!isKey) {
            dimensions.push({
              id: `${tbl}_${colName}`,
              label: colName === 'product' ? '产品品类' : colName === 'region' ? '销售大区' : colName === 'title' ? '职位头衔' : colName,
              entity: tbl,
              column: colName,
              kind: 'categorical',
              status: 'candidate',
            });
          }
        }

        // 外键
        for (const fk of fkInfos) {
          relationships.push({
            from: fk.table,
            to: tbl,
            cardinality: '1:N',
            join_key: `${fk.table}.${fk.to || 'id'} = ${tbl}.${fk.from}`,
            status: 'confirmed',
            evidence: `外键关联 (${tbl}.${fk.from} ➔ ${fk.table})`,
          });
        }
      }

      // SQLite 同名候选边推断 (若 sales.emp_id 且 emp 表存在)
      const tableNames = tblRows.map((t) => t.name);
      if (tableNames.includes('sales') && tableNames.includes('emp')) {
        const hasDirectFk = relationships.some((r) => r.from === 'emp' && r.to === 'sales');
        if (!hasDirectFk) {
          relationships.push({
            from: 'emp',
            to: 'sales',
            cardinality: '1:N',
            join_key: 'emp.id = sales.emp_id',
            status: 'candidate',
            evidence: '同名外键候选推断 (sales.emp_id ➔ emp.id)',
          });
        }
      }
    } finally {
      sdb.close();
    }
  }

  const result = {
    ok: true,
    dsn,
    framework: 'NVIDIA Auto-Ontology / RPX Semantic Reasoning Engine',
    spec_version: 'nvidia-auto-ontology/v1.0',
    discovered_at: new Date().toISOString(),
    summary: {
      entities_count: entities.length,
      measures_count: measures.length,
      dimensions_count: dimensions.length,
      relationships_count: relationships.length,
      confirmed_relationships: relationships.filter((r) => r.status === 'confirmed').length,
      candidate_relationships: relationships.filter((r) => r.status === 'candidate').length,
    },
    entities,
    measures,
    dimensions,
    relationships,
  };

  // 缓存推导结果
  const cachePath = path.resolve(__dirname, `../examples/library/.ontology-${dsn}-discovered.json`);
  try {
    fs.mkdirSync(path.dirname(cachePath), { recursive: true });
    fs.writeFileSync(cachePath, JSON.stringify(result, null, 2) + '\n', 'utf8');
  } catch {
    // 忽略缓存写入错误
  }

  return result;
}

/**
 * 导出 NVIDIA Auto-Ontology 官方标准 YAML 格式
 */
export async function exportNvidiaOntologyYaml(dsn) {
  const discovered = await autoDiscoverOntology(dsn);

  const lines = [];
  lines.push('# ==============================================================================');
  lines.push('# NVIDIA Auto-Ontology Semantic Catalog (v1.0 Specification)');
  lines.push('# Generated by Report Studio / RPX Autonomous Ontology Engine');
  lines.push(`# Target Datasource: ${dsn}`);
  lines.push(`# Generated At: ${discovered.discovered_at}`);
  lines.push('# ==============================================================================');
  lines.push('');
  lines.push('version: "1.0"');
  lines.push(`catalog_name: "${dsn.replace(/[^a-zA-Z0-9_]/g, '_')}_semantic_catalog"`);
  lines.push('');
  lines.push('metadata:');
  lines.push(`  datasource_id: "${dsn}"`);
  lines.push('  engine: "rpx-auto-ontology"');
  lines.push('  compliance_policy: "staged_certification"');
  lines.push('');

  lines.push('entities:');
  for (const e of discovered.entities) {
    lines.push(`  - id: "${e.id}"`);
    lines.push(`    name: "${e.name}"`);
    lines.push(`    table: "${e.table}"`);
    lines.push(`    type: "${e.kind === 'view' ? 'view_entity' : 'table_entity'}"`);
    lines.push(`    primary_key: [${(e.primary_key || []).map((k) => `"${k}"`).join(', ')}]`);
    lines.push(`    status: "${e.status || 'confirmed'}"`);
  }
  lines.push('');

  lines.push('measures:');
  for (const m of discovered.measures) {
    lines.push(`  - id: "${m.id}"`);
    lines.push(`    label: "${m.label}"`);
    lines.push(`    entity: "${m.entity}"`);
    lines.push(`    column: "${m.column}"`);
    lines.push(`    default_aggregation: "${m.default_aggregation || 'SUM'}"`);
    lines.push(`    expression: "${m.expression}"`);
    lines.push(`    status: "${m.status || 'candidate'}"`);
  }
  lines.push('');

  lines.push('dimensions:');
  for (const d of discovered.dimensions) {
    lines.push(`  - id: "${d.id}"`);
    lines.push(`    label: "${d.label}"`);
    lines.push(`    entity: "${d.entity}"`);
    lines.push(`    column: "${d.column}"`);
    lines.push(`    type: "${d.kind || 'categorical'}"`);
    if (d.grain) lines.push(`    grain: "${d.grain}"`);
    lines.push(`    status: "${d.status || 'candidate'}"`);
  }
  lines.push('');

  lines.push('relationships:');
  for (const r of discovered.relationships) {
    lines.push(`  - from: "${r.from}"`);
    lines.push(`    to: "${r.to}"`);
    lines.push(`    cardinality: "${r.cardinality === '1:N' ? 'one_to_many' : 'many_to_one'}"`);
    lines.push(`    join_keys: ["${r.join_key}"]`);
    lines.push(`    evidence: "${r.evidence || 'inferred'}"`);
    lines.push(`    status: "${r.status || 'candidate'}"`);
  }
  lines.push('');

  const yamlContent = lines.join('\n');
  const outDir = path.resolve(__dirname, '../docs');
  fs.mkdirSync(outDir, { recursive: true });
  const outPath = path.join(outDir, `ontology-${dsn}-nvidia.yaml`);
  fs.writeFileSync(outPath, yamlContent, 'utf8');

  return { ok: true, path: outPath, yaml: yamlContent };
}

/**
 * 认证或调整本体条目 (Candidate -> Certified)
 */
export async function certifyOntologyItem(dsn, payload) {
  const { item_type, id, action } = payload;
  const certPath = path.resolve(__dirname, `../examples/library/.ontology-${dsn}-certified.json`);
  let cert = { entities: {}, measures: {}, dimensions: {}, relationships: {} };
  try {
    cert = JSON.parse(fs.readFileSync(certPath, 'utf8'));
  } catch {
    // 忽略
  }
  if (!cert[item_type]) cert[item_type] = {};
  cert[item_type][id] = {
    status: action === 'reject' ? 'rejected' : 'confirmed',
    updated_at: new Date().toISOString(),
  };
  fs.mkdirSync(path.dirname(certPath), { recursive: true });
  fs.writeFileSync(certPath, JSON.stringify(cert, null, 2) + '\n', 'utf8');
  return { ok: true, dsn, item_type, id, status: cert[item_type][id].status };
}

