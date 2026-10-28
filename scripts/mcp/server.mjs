#!/usr/bin/env node
// rpx-mcp —— 报表引擎的 MCP 服务壳（零依赖，ESM）。
//
// 定位：把「接数据源 → 选模板 → 讲需求 → 分析 → 确认规格 → 引擎出报表」六步旅程
// 封装成 5 个 MCP 工具。本壳只做三件事：
//   1. MCP 协议编解码（stdio，NDJSON，一行一报文）；
//   2. 参数校验 + 安全收口（RPX_ROOT 白名单、exports 白名单、超时）；
//   3. 参数 → rpx.exe 命令行翻译，JSON 信封透传。
// 全部计算都在 rpx.exe 里，本文件不含任何报表逻辑。
//
// 环境变量：
//   RPX_ROOT          必填。AI 可触达的目录白名单根（模板库、数据、产物都在其内）
//   RPX_LIBRARY       主模板库目录；缺省 $RPX_ROOT/library
//   RPX_PRESETS_LIBRARY  第二模板库（预制模板集）；缺省 $RPX_ROOT/examples/presets。
//                      两个库在 list_templates 里合并检索，单库故障不拖垮整体。
//   RPX_BIN           rpx.exe 路径；缺省 <repo>/target/release/rpx.exe
//   RPX_MCP_TIMEOUT_MS  子进程超时；缺省 120000
//
// 手动自测（NDJSON，一行一条）：
//   {"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
//   {"jsonrpc":"2.0","id":2,"method":"tools/list"}
//   {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_templates","arguments":{"query":"账龄"}}}
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, '..', '..');

const RPX_ROOT = process.env.RPX_ROOT ? path.resolve(process.env.RPX_ROOT) : null;
const RPX_LIBRARY = process.env.RPX_LIBRARY
  ? path.resolve(process.env.RPX_LIBRARY)
  : RPX_ROOT
    ? path.join(RPX_ROOT, 'library')
    : null;
// 第二模板库（预制模板集）。缺省指 $RPX_ROOT/examples/presets，便于开箱即用；
// 单库故障（目录缺失/清单损坏）不拖垮整体检索，只归入 diagnostics。
const RPX_PRESETS_LIBRARY = process.env.RPX_PRESETS_LIBRARY
  ? path.resolve(process.env.RPX_PRESETS_LIBRARY)
  : RPX_ROOT
    ? path.join(RPX_ROOT, 'examples', 'presets')
    : null;
const RPX_BIN = process.env.RPX_BIN
  ? path.resolve(process.env.RPX_BIN)
  : path.join(REPO_ROOT, 'target', 'release', 'rpx.exe');
const TIMEOUT_MS = Number.parseInt(process.env.RPX_MCP_TIMEOUT_MS || '', 10) || 120_000;

const PROTOCOL_VERSION = '2024-11-05';
const SERVER_INFO = { name: 'rpx-mcp', version: '0.1.0' };
const EXPORT_WHITELIST = ['csv', 'xlsx', 'html'];
const NAME_RE = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

// ── 安全：一切路径收口在 RPX_ROOT 内 ─────────────────────────────────────────

class ToolError extends Error {
  constructor(code, message, extra = {}) {
    super(message);
    this.code = code;
    this.extra = extra;
  }
}

function requireRoot() {
  if (!RPX_ROOT) {
    throw new ToolError('RPX_ROOT_UNSET', '服务未配置 RPX_ROOT（AI 可触达目录白名单根）');
  }
  return RPX_ROOT;
}

/** 把调用方给的（通常相对的）路径解析到 RPX_ROOT 内；越界即拒。 */
function resolveInRoot(rel, what = '路径') {
  const root = requireRoot();
  if (typeof rel !== 'string' || rel.trim() === '') {
    throw new ToolError('INVALID_PARAMS', `${what}不能为空`);
  }
  const abs = path.resolve(root, rel);
  if (abs !== root && !abs.startsWith(root + path.sep)) {
    throw new ToolError('PATH_OUTSIDE_ROOT', `${what}越出 RPX_ROOT 白名单：${rel}`);
  }
  return abs;
}

// ── 子进程：逐参数传递，天然免疫 shell 注入与引号裂开 ─────────────────────────

/** 通用 spawn；bin 与 args 分离，超时 kill。测试可用假 bin 验证超时路径。 */
export function runBin(bin, args, { timeoutMs = TIMEOUT_MS } = {}) {
  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(bin, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    } catch (e) {
      resolve({ spawnError: e });
      return;
    }
    let stdout = '';
    let stderr = '';
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, timeoutMs);
    child.stdout.on('data', (d) => {
      stdout += d;
    });
    child.stderr.on('data', (d) => {
      stderr += d;
    });
    child.on('error', (e) => {
      clearTimeout(timer);
      resolve({ spawnError: e });
    });
    child.on('close', (code) => {
      clearTimeout(timer);
      resolve({ code, stdout, stderr, timedOut });
    });
  });
}

/** 取 stdout 最后一个非空行当 JSON 信封（CLI 契约：一行一信封）。 */
function parseEnvelope(stdout) {
  const lines = stdout.split('\n').map((l) => l.trim()).filter(Boolean);
  const last = lines.at(-1);
  if (!last) return null;
  try {
    return JSON.parse(last);
  } catch {
    return null;
  }
}

/** 跑 rpx.exe 并把信封翻译成工具结果；rpx 侧失败 → isError 透传诊断。 */
async function callRpx(args) {
  const r = await runBin(RPX_BIN, args);
  if (r.spawnError) {
    throw new ToolError('RPX_SPAWN_FAILED', `无法启动 rpx.exe（${RPX_BIN}）：${r.spawnError.message}`);
  }
  if (r.timedOut) {
    throw new ToolError('RPX_TIMEOUT', `rpx.exe 超过 ${TIMEOUT_MS}ms 被终止：${args.join(' ')}`);
  }
  const env = parseEnvelope(r.stdout);
  if (!env) {
    const tail = (r.stderr || r.stdout || '').trim().split('\n').slice(-3).join(' | ');
    throw new ToolError('RPX_BAD_ENVELOPE', `rpx.exe 输出不是合法 JSON 信封（exit ${r.code}）：${tail.slice(0, 300)}`);
  }
  if (!env.ok) {
    const diag = env.error || {};
    throw new ToolError(diag.code || 'RPX_FAILED', diag.message || 'rpx.exe 执行失败', {
      command: env.command,
      stage: diag.stage,
      hint: diag.hint,
      json_pointer: diag.json_pointer,
    });
  }
  return env.data;
}

// ── Studio（语义层 / 查询管线）HTTP 客户端 ──────────────────────────────────
//
// 为什么 MCP 转发 Studio 而不是再调 rpx.exe：语义模型、查询 IR、规划器、SQL 编译器
// 与审计都在 Studio（Python，见 studio/）。文档 §6 要求「MCP、CLI 与 Web 共用同一套
// 核心」，所以这里只做**接口面**，不重复实现逻辑。
const STUDIO_BASE = (process.env.RPX_STUDIO_BASE || 'http://127.0.0.1:8778').replace(/\/$/, '');

function studioError(res, body, status) {
  const e = body && body.error;
  if (typeof e === 'string') return new ToolError('STUDIO_FAILED', e, { response: body });
  if (e && typeof e === 'object') {
    return new ToolError(e.code || 'STUDIO_FAILED', e.message || 'Studio 执行失败', {
      response: body,
    });
  }
  if (body && body.validation && body.validation.ok === false) {
    return new ToolError('QUERY_INVALID', '查询未通过校验（见 validation.errors）', {
      response: body,
    });
  }
  return new ToolError('STUDIO_FAILED', `HTTP ${status}`);
}

async function studioFetch(path, init) {
  let res;
  try {
    res = await fetch(`${STUDIO_BASE}${path}`, init);
  } catch (e) {
    throw new ToolError(
      'STUDIO_UNREACHABLE',
      `连不上 Report Studio（${STUDIO_BASE}）：请先启动 ` +
      `python studio/server.py --port 8778（或设 RPX_STUDIO_BASE）。${e.message}`,
    );
  }
  const body = await res.json().catch(() => ({}));
  if (!res.ok || body.ok === false) throw studioError(res, body, res.status);
  return body;
}

function studioGet(path) {
  return studioFetch(path, { method: 'GET' });
}

function studioPost(path, payload) {
  return studioFetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload || {}),
  });
}

function needStr(a, key) {
  const v = a[key];
  if (typeof v !== 'string' || !v.trim()) {
    throw new ToolError('INVALID_PARAMS', `${key} 必填（字符串）`);
  }
  return v.trim();
}

/** 中文二元滑窗 + 英文词 —— 与 Studio 金样例检索同口径，只用于术语命中。 */
function termsTokens(text) {
  const raw = String(text || '').toLowerCase();
  const out = new Set();
  for (const chunk of raw.split(/[^0-9a-z\u4e00-\u9fff]+/)) {
    if (!chunk) continue;
    if (/^[0-9a-z]+$/.test(chunk)) {
      out.add(chunk);
    } else {
      for (let i = 0; i < chunk.length - 1; i++) out.add(chunk.slice(i, i + 2));
      if (chunk.length === 1) out.add(chunk);
    }
  }
  return [...out];
}

// ── 工具实现 ─────────────────────────────────────────────────────────────────

function clampLimit(v, fallback) {
  if (v === undefined || v === null) return fallback;
  if (!Number.isInteger(v) || v < 1 || v > 500) {
    throw new ToolError('INVALID_PARAMS', 'limit 必须是 1..=500 的整数');
  }
  return v;
}

const tools = {
  async list_templates(a) {
    // 合并多个模板库（RPX_LIBRARY + RPX_PRESETS_LIBRARY）：各自取全量后在工具层
    // 去重、排序、重新分页。report list 只扫单个根目录的直接子目录，无法一次跨库，
    // 所以合并放在壳层；单库故障归入 diagnostics，不拖垮整体检索。
    const libraries = [RPX_LIBRARY, RPX_PRESETS_LIBRARY].filter(
      (p) => typeof p === 'string' && p.length > 0,
    );
    if (libraries.length === 0) {
      throw new ToolError('LIBRARY_UNSET', '未配置任何模板库（RPX_LIBRARY / RPX_PRESETS_LIBRARY）');
    }
    const query = a.query;
    const tag = a.tag;
    if (query !== undefined && typeof query !== 'string') {
      throw new ToolError('INVALID_PARAMS', 'query 必须是字符串');
    }
    if (tag !== undefined && typeof tag !== 'string') {
      throw new ToolError('INVALID_PARAMS', 'tag 必须是字符串');
    }
    const offset = a.offset;
    if (offset !== undefined && (!Number.isInteger(offset) || offset < 0)) {
      throw new ToolError('INVALID_PARAMS', 'offset 必须是非负整数');
    }
    // 合并多库后默认取尽（CLI 上限 500），避免无 limit 调用时静默截掉后半库模板；
    // 显式传 limit 仍按分页语义工作。
    const limit = clampLimit(a.limit, 500);

    const collected = [];
    const diagnostics = [];
    for (const lib of libraries) {
      const args = ['report', 'list', '--library', lib];
      if (query !== undefined) args.push('--query', query);
      if (tag !== undefined) args.push('--tag', tag);
      args.push('--offset', '0');
      // 取全量；分页在合并后重做。CLI 把 --limit 约束在 1..=500（clap 值范围），
      // 用上限 500 一次取尽；若单库超 500，合并会截断（当前两库均远小于此）。
      args.push('--limit', '500');
      let data;
      try {
        data = await callRpx(args);
      } catch (e) {
        // 单个库故障（目录缺失/清单损坏/引擎异常）不要拖垮整体；归入 diagnostics。
        diagnostics.push({
          severity: 'warning',
          code: e.code || 'LIBRARY_ERR',
          library: lib,
          message: e.message,
        });
        continue;
      }
      for (const it of data.items || []) collected.push(it);
      if (Array.isArray(data.diagnostics)) diagnostics.push(...data.diagnostics);
    }

    // 跨库去重（按 id，保留首次出现）；排序后重新分页。
    const seen = new Set();
    const merged = [];
    for (const it of collected) {
      const id = it.id;
      if (id == null) {
        merged.push(it);
        continue;
      }
      if (seen.has(id)) continue;
      seen.add(id);
      merged.push(it);
    }
    // 跨库合并：有 query 时按 **match_score 降序**（CLI 的命中计数，见
    // `library/search.rs`）再按 id；无 query 维持 id 升序。
    merged.sort((x, y) => {
      if (query !== undefined && query !== '') {
        const sx = Number(x.match_score ?? 0);
        const sy = Number(y.match_score ?? 0);
        if (sy !== sx) return sy - sx;
      }
      return String(x.id ?? '').localeCompare(String(y.id ?? ''));
    });

    const off = Number.isInteger(offset) ? offset : 0;
    const pageItems = merged.slice(off, off + limit);
    const next = off + pageItems.length;
    // 信封形状与 report list 完全一致：returned/total 为数字，含 next_offset；
    // 不额外塞 offset/limit，避免消费方按 CLI 契约解析时踩类型坑。
    return {
      items: pageItems,
      returned: pageItems.length,
      total: merged.length,
      next_offset: next < merged.length ? next : null,
      diagnostics,
    };
  },

  async inspect_dataset(a) {
    const file = resolveInRoot(a.file, '数据文件');
    const args = ['dataset', 'inspect', file, '--limit', String(clampLimit(a.limit, 5))];
    return callRpx(args);
  },

  async analyze_spec(a) {
    const spec = a.spec;
    if (!spec || typeof spec !== 'object' || Array.isArray(spec)) {
      throw new ToolError('INVALID_PARAMS', 'spec 必须是规格对象');
    }
    const dataDir = resolveInRoot(a.data_dir ?? '.', 'data_dir');
    // spec.dataset.file 相对 data_dir 解析，同样必须收口在 root 内
    const rel = spec.dataset?.file;
    if (typeof rel === 'string' && rel) resolveInRoot(path.join(dataDir, rel), 'spec.dataset.file');
    const specPath = writeSpecFile(spec);
    return callRpx(['spec', 'analyze', specPath, '--data-dir', dataDir]);
  },

  async compile_spec(a) {
    const spec = a.spec;
    if (!spec || typeof spec !== 'object' || Array.isArray(spec)) {
      throw new ToolError('INVALID_PARAMS', 'spec 必须是规格对象');
    }
    const name = a.name ?? `spec-${stamp()}`;
    if (!NAME_RE.test(name)) {
      throw new ToolError('INVALID_PARAMS', `name 只能含字母数字与 . _ -（且不以点开头）：${name}`);
    }
    const root = requireRoot();
    const dataDir = resolveInRoot(a.data_dir ?? '.', 'data_dir');
    const rel = spec.dataset?.file;
    if (typeof rel === 'string' && rel) resolveInRoot(path.join(dataDir, rel), 'spec.dataset.file');
    const specPath = writeSpecFile(spec);
    const out = path.join(root, 'compiled', `${name}.json`);
    fs.mkdirSync(path.dirname(out), { recursive: true });
    return callRpx(['spec', 'compile', specPath, '--out', out, '--data-dir', dataDir]);
  },

  async run_report(a) {
    if (typeof a.template !== 'string' || !a.template.trim()) {
      throw new ToolError('INVALID_PARAMS', 'template 不能为空');
    }
    const template = resolveInRoot(a.template, '模板');
    const dataDir =
      a.data_dir === undefined
        ? path.dirname(template)
        : resolveInRoot(a.data_dir, 'data_dir');
    // exports 白名单在壳层判死，不出子进程
    const exports = a.exports ?? ['csv'];
    if (!Array.isArray(exports) || exports.length === 0) {
      throw new ToolError('INVALID_PARAMS', 'exports 必须是非空数组');
    }
    for (const e of exports) {
      if (!EXPORT_WHITELIST.includes(e)) {
        throw new ToolError(
          'EXPORT_NOT_ALLOWED',
          `导出格式 ${e} 不在白名单（${EXPORT_WHITELIST.join(' / ')}）`,
        );
      }
    }
    // out-dir 必须不存在（引擎用 CREATE_NEW 建目录）；这里只起名不落盘
    const outDir = uniqueOutDir();
    const args = ['run', template, '--out-dir', outDir, '--data-dir', dataDir];
    // SQL 数据集（模板 datasets[].sql）必须带数据源注册表；文件数据集不需要。
    if (a.datasources !== undefined) {
      args.push('--datasources', resolveInRoot(a.datasources, 'datasources'));
    }
    for (const e of exports) args.push('--export', e);
    const data = await callRpx(args);
    // 产物清单：把请求的格式映射到 artifacts/ 下的真实文件，存在才列
    const productName = { csv: 'report.csv', xlsx: 'report.xlsx', html: 'report.html' };
    const products = [];
    for (const e of exports) {
      const p = path.join(outDir, 'artifacts', productName[e]);
      let size = null;
      try {
        size = fs.statSync(p).size;
      } catch {
        size = null;
      }
      products.push({ format: e, path: p, exists: size !== null, bytes: size });
    }
    data.products = products;
    return data;
  },

  // ── 语义层 / 查询管线（§6.1）：全部转发 Studio，不在 MCP 里重复实现 ──────
  async inspect_data_model(a) {
    const dsn = needStr(a, 'dsn');
    const body = await studioGet(`/api/datasources/${encodeURIComponent(dsn)}/bizmodel`);
    return body.bizmodel;
  },

  async search_business_terms(a) {
    const dsn = needStr(a, 'dsn');
    const q = (a.query || '').trim();
    const body = await studioGet(`/api/datasources/${encodeURIComponent(dsn)}/bizmodel`);
    const model = body.bizmodel || {};
    const tokens = termsTokens(q);
    const hit = (text) => {
      const t = String(text || '').toLowerCase();
      if (!tokens.length) return true;
      return tokens.some((k) => t.includes(k));
    };
    // aliases 也要命中：库里列名常是 amount，业务人员说的是「销售额」（§3.1）
    const ali = (x) => (x.aliases || []).some((w) => hit(w));
    const metrics = (model.metrics || [])
      .filter((m) => hit(m.id) || hit(m.label) || hit(m.expression) || ali(m))
      .map((m) => ({
        id: m.id, label: m.label, aliases: m.aliases || [], expression: m.expression,
        status: m.status, unit: m.format || null, entity: m.entity,
      }));
    const dimensions = (model.dimensions || [])
      .filter((d) => hit(d.id) || hit(d.label) || hit(d.column) || ali(d))
      .map((d) => ({
        id: d.id, label: d.label, aliases: d.aliases || [], kind: d.kind,
        values: d.values || null, status: d.status, entity: d.entity,
      }));
    const entities = (model.entities || [])
      .filter((e) => hit(e.id) || hit(e.label) || hit(e.table))
      .map((e) => ({ id: e.id, table: e.table, grain: e.grain, status: e.status }));
    return {
      query: q, metrics, dimensions, entities,
      rules: model.rules || [],
      open_questions: model.open_questions || [],
      hint: metrics.length || dimensions.length
        ? '口径以 status=confirmed 为准；candidate/draft 是机器推断，使用前应向用户确认'
        : '没命中术语 —— 换更短的说法（如只传「销售额」/「区域」），或先 inspect_data_model 看全量',
    };
  },

  async build_report_plan(a) {
    const dsn = needStr(a, 'dsn');
    let query = a.query;
    let llm = null;
    let clarification = null;
    if (!query) {
      const intent = needStr(a, 'intent');
      const u = await studioPost(`/api/datasources/${encodeURIComponent(dsn)}/understand`, {
        intent, model: a.model,
      });
      query = u.query;
      llm = u.llm || null;
      if (u.needs_clarification) {
        // ⚠ 不静默往下走：未决歧义交回给调用方（agent），由它问用户。
        clarification = { ambiguities: u.ambiguities || [], validation: u.validation };
      }
    }
    const p = await studioPost(`/api/datasources/${encodeURIComponent(dsn)}/plan`, { query });
    return {
      query, plan: p.plan, validation: p.validation, llm,
      needs_clarification: !!clarification,
      ...(clarification || {}),
    };
  },

  async compile_report_query(a) {
    const dsn = needStr(a, 'dsn');
    if (!a.query || typeof a.query !== 'object') {
      throw new ToolError('INVALID_PARAMS', 'query 必填（rpx-query/v1 对象；可由 build_report_plan 得到）');
    }
    const body = await studioPost(`/api/datasources/${encodeURIComponent(dsn)}/sql`, { query: a.query });
    return { sql: body.sql, plan: body.plan, validation: body.validation };
  },

  async explain_report(a) {
    const dsn = needStr(a, 'dsn');
    if (!a.query || typeof a.query !== 'object') {
      throw new ToolError('INVALID_PARAMS', 'query 必填（rpx-query/v1 对象）');
    }
    const body = await studioPost(`/api/datasources/${encodeURIComponent(dsn)}/sql`, { query: a.query });
    const defs = body.sql?.definitions || [];
    const lines = defs.map((d) =>
      `· ${d.label || d.metric} = ${d.expression}（口径状态：${d.status || 'unknown'}）`);
    const warns = (body.sql?.warnings || []).map((w) => `· ${w.what}`);
    const dq = (body.plan?.dimensions || [])
      .filter((d) => d.grain_risk && d.grain_risk !== 'ok')
      .map((d) => `· 维度 ${d.ref} 存在重复计数风险（${d.grain_risk}）`);
    return {
      text: [
        '【指标口径】', lines.join('\n') || '（无指标）',
        warns.length ? '\n【编译警告】\n' + warns.join('\n') : '',
        dq.length ? '\n【粒度风险】\n' + dq.join('\n') : '',
        body.sql?.guardrails ? '\n【护栏】' + JSON.stringify(body.sql.guardrails) : '',
      ].filter(Boolean).join('\n'),
      definitions: defs,
      guardrails: body.sql?.guardrails || null,
      warnings: body.sql?.warnings || [],
      plan: body.plan,
    };
  },
};

// ── 临时文件与唯一目录 ───────────────────────────────────────────────────────

function stamp() {
  return new Date().toISOString().replace(/[:.]/g, '-').replace('T', '_').slice(0, 19);
}

function rand() {
  return Math.random().toString(36).slice(2, 8);
}

/** spec 对象落成临时文件供 CLI 读取；只落在 $RPX_ROOT/.specs/ 内。 */
function writeSpecFile(spec) {
  const root = requireRoot();
  const dir = path.join(root, '.specs');
  fs.mkdirSync(dir, { recursive: true });
  const p = path.join(dir, `spec-${stamp()}-${rand()}.json`);
  fs.writeFileSync(p, `${JSON.stringify(spec, null, 2)}\n`, 'utf8');
  return p;
}

function uniqueOutDir() {
  const root = requireRoot();
  for (let i = 0; i < 200; i++) {
    const dir = path.join(root, 'runs', `run-${stamp()}-${rand()}`);
    if (!fs.existsSync(dir)) return dir;
  }
  throw new ToolError('OUT_DIR_CONFLICT', '无法分配不冲突的 out-dir');
}

// ── 工具描述（T8：AI 只凭这些描述走完全程） ──────────────────────────────────

const TOOLS = [
  {
    name: 'list_templates',
    description:
      '列出报表模板库，可按口语关键词/标签检索。旅程第二步「选模板」用这个：' +
      'query 直接传用户的**整句业务说法**（如「帮我找一下应收账款账龄表」）—— 引擎按' +
      '中文二元滑窗 + 多字段命中计数排序，无需先拆词；命中后看 meta.keywords、' +
      'meta.examples 与 meta.shape 决定推荐哪份；只有完全无命中时才换更短的词重试。',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: '口语化检索词（中文可直接传，无编码坑）' },
        tag: { type: 'string', description: '精确标签，如 crosstab / detail / chart' },
        limit: { type: 'integer', minimum: 1, maximum: 500, default: 500, description: '缺省取尽合并后的全部模板（最多 500）' },
        offset: { type: 'integer', minimum: 0, default: 0 },
      },
    },
  },
  {
    name: 'inspect_dataset',
    description:
      '探查上传的数据文件（CSV/TSV/Excel）：返回列名、行数、样本行与命中的编码/分隔符。' +
      '旅程第一步「接数据源」用这个：拿到列名后才能写规格；注意返回的 encoding 字段，' +
      '若是 GBK 而你要的数据是中文乱码，先把文件转存为 UTF-8。',
    inputSchema: {
      type: 'object',
      properties: {
        file: { type: 'string', description: '数据文件路径（相对 RPX_ROOT 或绝对路径）' },
        limit: { type: 'integer', minimum: 1, maximum: 500, default: 5, description: '样本行数' },
      },
      required: ['file'],
    },
  },
  {
    name: 'analyze_spec',
    description:
      '分析一份结构化需求规格（spec）：返回计算口径（calibers）、对数据的探查发现（findings）、' +
      '已做的决策（decisions，带出处）与未决问题（open_questions）。旅程第四步「分析」用这个。' +
      'open_questions 非空时必须先转述给用户拍板，不要擅自默认；规格有错会返回 SPEC_INVALID ' +
      '并带 json_pointer 指出出错位置。',
    inputSchema: {
      type: 'object',
      properties: {
        spec: { type: 'object', description: '规格对象（schema=rpx-spec/v1）' },
        data_dir: { type: 'string', description: 'spec.dataset.file 的基准目录，缺省 RPX_ROOT' },
      },
      required: ['spec'],
    },
  },
  {
    name: 'compile_spec',
    description:
      '把规格编译成可执行模板（确定性：同规格必得同字节）。产物落到 RPX_ROOT/compiled/ 下，' +
      '返回 geometry（版面尺寸）与 lint 告警。旅程第五步「确认规格」之后用这个；' +
      '编译出的模板路径可直接交给 run_report。',
    inputSchema: {
      type: 'object',
      properties: {
        spec: { type: 'object', description: '规格对象（schema=rpx-spec/v1）' },
        name: { type: 'string', description: '产物名（不含路径与扩展名），缺省自动生成' },
        data_dir: { type: 'string', description: 'spec.dataset.file 的基准目录，缺省 RPX_ROOT' },
      },
      required: ['spec'],
    },
  },
  {
    name: 'run_report',
    description:
      '执行模板生成报表：自动分配唯一产物目录，返回 run 摘要、结果/工作簿路径与导出文件清单。' +
      '旅程第六步「引擎出报表」用这个。template 可以是 compile_spec 的产物路径，' +
      '也可以是库内模板的 template.json 相对路径。',
    inputSchema: {
      type: 'object',
      properties: {
        template: { type: 'string', description: '模板 JSON 路径（相对 RPX_ROOT 或绝对路径）' },
        data_dir: { type: 'string', description: '数据集目录；缺省取模板所在目录' },
        datasources: {
          type: 'string',
          description:
            '数据源注册表 JSON 路径（相对 RPX_ROOT）。模板用 datasets[].sql 取数时必须给：' +
            '{"名字": "postgres://..."} 或 {"名字": {"url","user","pass"}}；文件数据集不用给',
        },
        exports: {
          type: 'array',
          items: { type: 'string', enum: EXPORT_WHITELIST },
          default: ['csv'],
          description: '导出格式白名单',
        },
      },
      required: ['template'],
    },
  },
  {
    name: 'inspect_data_model',
    description:
      '查看数据源已确认的业务模型：实体（表/粒度）、关系（基数/证据）、指标（口径表达式/状态）、' +
      '维度（类型/取值/状态）、业务规则与未决问题。做任何分析前先看它 —— ' +
      'status=confirmed 是人工确认的事实，candidate/draft 是机器推断（后者必须向用户确认后才可用）。',
    inputSchema: {
      type: 'object',
      properties: {
        dsn: { type: 'string', description: '数据源 id（Studio「数据源」页里的那个）' },
      },
      required: ['dsn'],
    },
  },
  {
    name: 'search_business_terms',
    description:
      '在业务模型里按中文说法检索指标/维度/实体（如「销售额」「区域」「订单数」）。' +
      '用户提到业务词但不确定对应哪个字段时用这个；返回每条的口径表达式与 status，' +
      'confirmed 才代表口径已定，draft/candidate 要回去问用户。',
    inputSchema: {
      type: 'object',
      properties: {
        dsn: { type: 'string' },
        query: { type: 'string', description: '业务说法（整句也行，内部按词命中）' },
      },
      required: ['dsn'],
    },
  },
  {
    name: 'build_report_plan',
    description:
      '把自然语言需求变成结构化查询与执行计划：内部先走理解层（LLM 输出 rpx-query/v1 草案并' +
      '过验算），再规划 JOIN 路径与粒度（检查重复计数）。有未决歧义时返回 needs_clarification=true' +
      '与 ambiguities —— **必须先问用户**，不要把 resolved_as 补上继续跑。' +
      '已知查询结构时可直接传 query 跳过 LLM。',
    inputSchema: {
      type: 'object',
      properties: {
        dsn: { type: 'string' },
        intent: { type: 'string', description: '用户的业务问句（自然语言）' },
        query: { type: 'object', description: '（可选）直接给 rpx-query/v1，跳过 LLM 起草' },
        model: { type: 'string', description: '（可选）指定 LLM 模型，如 deepseek-v4-pro' },
      },
      required: ['dsn'],
    },
  },
  {
    name: 'compile_report_query',
    description:
      '把 rpx-query/v1 编译成方言 SQL + 绑定参数（不执行）。返回 sql.sql / sql.params / ' +
      'sql.definitions（口径回执）/ sql.guardrails（只读校验、LIMIT 等）。' +
      '要跑数请改用 Studio 的 execute（或 run_report）。',
    inputSchema: {
      type: 'object',
      properties: {
        dsn: { type: 'string' },
        query: { type: 'object', description: 'rpx-query/v1（可由 build_report_plan 得到）' },
      },
      required: ['dsn', 'query'],
    },
  },
  {
    name: 'explain_report',
    description:
      '解释一个查询「这个数是怎么算出来的」：每个指标的口径表达式与确认状态、编译警告、' +
      '粒度重复计数风险、只读护栏。用户问「销售额怎么算的 / 为什么是这个数」时用；' +
      '注意相关性不等于因果 —— 只解释计算，不要编因果结论。',
    inputSchema: {
      type: 'object',
      properties: {
        dsn: { type: 'string' },
        query: { type: 'object', description: 'rpx-query/v1' },
      },
      required: ['dsn', 'query'],
    },
  },
];

// ── JSON-RPC 协议层 ──────────────────────────────────────────────────────────

function send(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`);
}

function sendError(id, code, message, data) {
  const error = { code, message };
  if (data !== undefined) error.data = data;
  process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, error })}\n`);
}

function toolResult(payload) {
  return { content: [{ type: 'text', text: JSON.stringify(payload, null, 2) }] };
}

function toolErrorResult(err) {
  const payload = {
    error: {
      code: err.code || 'TOOL_FAILED',
      message: err.message,
      ...(err.extra || {}),
    },
  };
  return { content: [{ type: 'text', text: JSON.stringify(payload, null, 2) }], isError: true };
}

async function dispatch(method, params) {
  switch (method) {
    case 'initialize':
      return {
        protocolVersion: PROTOCOL_VERSION,
        capabilities: { tools: { listChanged: false } },
        serverInfo: SERVER_INFO,
        instructions:
          'rpx-mcp 把报表引擎封装成 5 个工具，支撑六步旅程：接数据源（inspect_dataset）→ ' +
          '选模板（list_templates）→ 讲需求（用户口述）→ 分析（analyze_spec）→ ' +
          '确认规格（compile_spec）→ 引擎出报表（run_report）。' +
          'analyze_spec 返回 open_questions 时先与用户确认再编译；' +
          '所有路径参数相对 RPX_ROOT，越界会被拒绝。',
      };
    case 'ping':
      return {};
    case 'tools/list':
      return { tools: TOOLS };
    case 'tools/call': {
      const name = params?.name;
      const args = params?.arguments ?? {};
      if (typeof name !== 'string' || !tools[name]) {
        // 协议层错误：工具名未知/缺失不属于「工具执行失败」
        throw { code: -32602, message: `未知工具：${name}` };
      }
      if (!args || typeof args !== 'object' || Array.isArray(args)) {
        throw new ToolError('INVALID_PARAMS', 'arguments 必须是对象');
      }
      const tool = tools[name];
      try {
        return toolResult(await tool(args));
      } catch (e) {
        if (e instanceof ToolError) return toolErrorResult(e);
        throw e;
      }
    }
    default:
      throw { code: -32601, message: `方法不存在：${method}` };
  }
}

async function handleLine(line) {
  const text = line.trim();
  if (!text) return;
  let msg;
  try {
    msg = JSON.parse(text);
  } catch {
    sendError(null, -32700, 'JSON 解析失败');
    return;
  }
  if (!msg || typeof msg !== 'object' || Array.isArray(msg) || msg.jsonrpc !== '2.0') {
    sendError(null, -32600, '不是合法的 JSON-RPC 2.0 请求');
    return;
  }
  const { id, method, params } = msg;
  if (id === undefined) {
    // 通知（如 notifications/initialized）：不回应
    return;
  }
  try {
    send(id, await dispatch(method, params));
  } catch (e) {
    if (e && typeof e.code === 'number') {
      sendError(id, e.code, e.message);
    } else {
      sendError(id, -32603, `内部错误：${e?.message || e}`);
    }
  }
}

const rl = createInterface({ input: process.stdin, terminal: false });
rl.on('line', handleLine);
process.stdin.resume();

// 直接运行时才启动服务；被 import 时只暴露函数给测试。
const invokedDirectly =
  process.argv[1] && fs.realpathSync(process.argv[1]) === fs.realpathSync(fileURLToPath(import.meta.url));
if (!invokedDirectly) {
  rl.close();
  process.stdin.pause();
}
