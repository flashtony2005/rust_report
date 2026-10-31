// rpx-mcp 服务壳的自动化验收（T1–T7）。
//
// 跑法：node scripts/mcp/test_server.mjs
// 前提：target/release/rpx.exe 已构建（cargo build --release -p rpx-cli）。
//
// 覆盖计划见 docs/mcp-plan.md §4：协议层 / 检索 / 数据探查 / 分析 / 编译 /
// 执行 / 安全。中文参数全程走结构化 JSON-RPC —— 「绕开 Git Bash 编码坑」
// 本身就是验收项之一。
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '..', '..');
const NODE = process.execPath;
const SERVER = path.join(HERE, 'server.mjs');
const RPX = path.join(REPO, 'target', 'release', 'rpx.exe');
const LIB = path.join(REPO, 'examples', 'library');
const RMODEL = path.join(REPO, 'examples', 'r-model');
const DATA_DIR = path.join(REPO, 'target', 'mcp-test-data');

// ── 极简测试框架 ─────────────────────────────────────────────────────────────

const results = [];
function check(name, ok, detail = '') {
  results.push({ name, ok, detail });
  const mark = ok ? 'PASS' : 'FAIL';
  console.log(`  [${mark}] ${name}${detail && !ok ? ` — ${detail}` : ''}`);
}
function section(t) {
  console.log(`\n== ${t} ==`);
}
async function throwsTool(code, fn) {
  try {
    await fn();
    return null;
  } catch (e) {
    return e;
  }
}

// ── MCP 客户端 ───────────────────────────────────────────────────────────────

class Client {
  constructor() {
    fs.mkdirSync(DATA_DIR, { recursive: true });
    this.child = spawn(NODE, [SERVER], {
      env: {
        ...process.env,
        RPX_ROOT: REPO,
        RPX_LIBRARY: LIB,
        RPX_PRESETS_LIBRARY: path.join(REPO, 'examples', 'presets'),
        RPX_BIN: RPX,
      },
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    this.waiters = new Map();
    this.nextId = 1;
    this.rl = createInterface({ input: this.child.stdout, terminal: false });
    this.rl.on('line', (line) => {
      const t = line.trim();
      if (!t) return;
      let msg;
      try {
        msg = JSON.parse(t);
      } catch {
        return;
      }
      const w = this.waiters.get(msg.id);
      if (w) {
        this.waiters.delete(msg.id);
        w(msg);
      }
    });
    this.stderr = '';
    this.child.stderr.on('data', (d) => {
      this.stderr += d;
    });
  }
  /** 发请求并等响应；toolError=true 时期待 isError 结果而非异常。 */
  async call(method, params, timeoutMs = 60_000) {
    const id = this.nextId++;
    const line = `${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`;
    const got = new Promise((resolve) => this.waiters.set(id, resolve));
    this.child.stdin.write(line);
    const timer = new Promise((_, rej) =>
      setTimeout(() => rej(new Error(`等待 ${method} 响应超时`)), timeoutMs),
    );
    const msg = await Promise.race([got, timer]);
    if (msg.error) {
      const e = new Error(msg.error.message);
      e.rpc = msg.error;
      throw e;
    }
    return msg.result;
  }
  /** 工具调用：永远 resolve（MCP 语义：失败也是 result + isError）。 */
  async tool(name, args, timeoutMs = 60_000) {
    const r = await this.call('tools/call', { name, arguments: args }, timeoutMs);
    const text = r.content?.[0]?.text ?? '';
    let payload;
    try {
      payload = JSON.parse(text);
    } catch {
      payload = { __unparsable: text.slice(0, 200) };
    }
    return { isError: !!r.isError, payload };
  }
  raw(line) {
    this.child.stdin.write(`${line}\n`);
  }
  close() {
    this.child.kill();
  }
}

// ── 夹具 ─────────────────────────────────────────────────────────────────────

function crosstabSpec() {
  return {
    schema: 'rpx-spec/v1',
    title: '区域品类交叉统计',
    dataset: { name: 'd', file: 'crosstab-merge-cells.csv' },
    layout: 'crosstab',
    measures: [{ field: 'amount', agg: 'sum', label: '金额' }],
    totals: { grand: true, subtotal: true },
    row_dims: [
      { field: 'region', label: '大区' },
      { field: 'store', label: '门店' },
      { field: 'month', label: '月份' },
    ],
    col_dims: [{ field: 'cat', label: '品类' }],
  };
}

function dataFile(name, content) {
  const p = path.join(DATA_DIR, name);
  fs.writeFileSync(p, content, 'utf8');
  return p;
}

// ── T1 协议层 ────────────────────────────────────────────────────────────────

async function t1(c) {
  section('T1 协议层');
  const init = await c.call('initialize', {});
  check('T1.1 initialize 握手', init.protocolVersion === '2024-11-05' && init.serverInfo?.name === 'rpx-mcp' && !!init.capabilities?.tools && typeof init.instructions === 'string');

  const list = await c.call('tools/list');
  const names = (list.tools || []).map((t) => t.name);
  // 10 = 原 5 个（模板/数据集/spec 链）+ §6.1 新增 5 个语义层工具
  // （inspect_data_model / search_business_terms / build_report_plan /
  //   compile_report_query / explain_report）
  const LEGACY = ['list_templates', 'inspect_dataset', 'analyze_spec', 'compile_spec', 'run_report'];
  const SEMANTIC = ['inspect_data_model', 'search_business_terms', 'build_report_plan', 'compile_report_query', 'explain_report'];
  check('T1.2 tools/list 返回 10 个工具（含语义层 5 个）',
    names.length === 10 && [...LEGACY, ...SEMANTIC].every((n) => names.includes(n)));
  const schemaOk = (list.tools || []).every(
    (t) => t.description && t.inputSchema?.type === 'object' && t.inputSchema.properties,
  );
  check('T1.2b 每个工具带描述与入参 schema', schemaOk);

  const e1 = await throwsTool('-32601', () => c.call('no/such/method', {}));
  check('T1.3 未知方法 → -32601', e1?.rpc?.code === -32601, e1?.message);

  c.raw('{"jsonrpc":"2.0","id":999,"method":"tools/call", BROKEN JSON');
  c.raw('not json at all');
  const after = await c.call('ping', {});
  check('T1.4 坏 JSON 不崩进程，后续仍服务', after !== undefined);

  // 批量数组（2024-11- 已移除）：必须是协议错误而不是静默执行。
  // 请求本身不合法 ⇒ id 无法回认，按 JSON-RPC 惯例回 id=null。
  const e3 = await new Promise((resolve) => {
    c.child.stdin.write(`${JSON.stringify([{ jsonrpc: '2.0', id: 998, method: 'ping' }])}\n`);
    const timer = setTimeout(() => resolve(null), 3000);
    const onLine = (line) => {
      try {
        const m = JSON.parse(line);
        if (m.id === null || m.id === 998) {
          clearTimeout(timer);
          c.rl.off('line', onLine);
          resolve(m);
        }
      } catch {}
    };
    c.rl.on('line', onLine);
  });
  check('T1.4b 批量数组 → -32600', e3?.error?.code === -32600, JSON.stringify(e3?.error));
}

// ── T2 list_templates ────────────────────────────────────────────────────────

async function t2(c) {
  section('T2 list_templates（双库合并检索）');
  const all = await c.tool('list_templates', {});
  const allIds = (all.payload.items || []).map((i) => i.id);
  // 主库 29 + 预制库 30，id 零重叠 → 合并后 59
  check('T2.1 双库合并 → 59 条', !all.isError && all.payload.total === 59, `total=${all.payload?.total} err=${all.payload?.error?.message || ''}`);
  check('T2.1b 预制库模板可被检索到', allIds.includes('sales-order-detail') && allIds.includes('fin-ar-aging-summary'), `n=${all.payload?.total}`);

  const aging = await c.tool('list_templates', { query: '账龄' });
  const ids = (aging.payload.items || []).map((i) => i.id);
  // 账龄 同时命中主库 ar-aging 与预制库 fin-ar-aging-summary
  check('T2.2 query=账龄 跨库命中 ar-aging + fin-ar-aging-summary', !aging.isError && ids.includes('ar-aging') && ids.includes('fin-ar-aging-summary'), `ids=${ids.join(',')}`);

  const xt = await c.tool('list_templates', { tag: 'crosstab' });
  const tagged = (xt.payload.items || []).every((i) => (i.manifest?.tags || []).includes('crosstab'));
  // 主库 7 + 预制库 3 = 10；断言全部带标签且至少含一个预制交叉表
  check('T2.3 tag=crosstab → 全部带标签且含预制模板', !xt.isError && tagged && (xt.payload.items || []).length >= 10 && allIds.includes('sales-region-category-crosstab'), `n=${xt.payload.items?.length}`);

  const presetOnly = await c.tool('list_templates', { query: '预算' });
  const pids = (presetOnly.payload.items || []).map((i) => i.id);
  check('T2.4 query=预算 只命中预制库 fin-budget-vs-actual', !presetOnly.isError && pids.length >= 1 && pids.includes('fin-budget-vs-actual'), `ids=${pids.join(',')}`);

  const none = await c.tool('list_templates', { query: '绝不可能存在的词zzz' });
  check('T2.5 无命中 → 空列表不是错误', !none.isError && none.payload.items?.length === 0);

  const pag = await c.tool('list_templates', { limit: 10, offset: 0 });
  check('T2.6 分页参数生效（returned=10 且 total=59）', !pag.isError && (pag.payload.items || []).length === 10 && pag.payload.returned === 10 && pag.payload.total === 59, `returned=${pag.payload?.returned} total=${pag.payload?.total}`);

  const page2 = await c.tool('list_templates', { limit: 10, offset: 10 });
  const same = (pag.payload.items || [])[0]?.id === (page2.payload.items || [])[0]?.id;
  check('T2.7 翻页偏移生效（offset=10 首条不同）', !page2.isError && (page2.payload.items || []).length === 10 && !same, `same=${same}`);
}

// ── T3 inspect_dataset ───────────────────────────────────────────────────────

async function t3(c) {
  section('T3 inspect_dataset');
  const ok = await c.tool('inspect_dataset', { file: 'examples/library/ar-aging/data/ar-aging.csv' });
  check('T3.1 csv → 列名 + 行数 + 样本', !ok.isError && ok.payload.columns?.join(',') === 'customer,amount,days' && ok.payload.rows === 6, JSON.stringify(ok.payload).slice(0, 160));

  const miss = await c.tool('inspect_dataset', { file: 'examples/library/no-such-file.csv' });
  check('T3.2 文件不存在 → FILE_NOT_FOUND 透传', miss.isError && miss.payload.error?.code === 'FILE_NOT_FOUND', miss.payload?.error?.code);

  const two = await c.tool('inspect_dataset', { file: 'examples/library/ar-aging/data/ar-aging.csv', limit: 2 });
  check('T3.3 limit=2 → 只回 2 行样本', !two.isError && two.payload.sample?.returned === 2);
}

// ── T4 analyze_spec ──────────────────────────────────────────────────────────

async function t4(c) {
  section('T4 analyze_spec');
  const spec = crosstabSpec();
  const a = await c.tool('analyze_spec', { spec, data_dir: 'examples/r-model' });
  const an = a.payload.analysis;
  check('T4.1 合法 spec → 四类输出齐全', !a.isError && Array.isArray(an?.calibers) && Array.isArray(an?.findings) && Array.isArray(an?.decisions) && Array.isArray(an?.open_questions), JSON.stringify(a.payload).slice(0, 200));
  check('T4.1b calibers 每条带回指规格的 source', (an?.calibers || []).length > 0 && (an?.calibers || []).every((x) => typeof x.source === 'string'));

  const bad = structuredClone(spec);
  bad.row_dims[0].field = 'regioN';
  const b = await c.tool('analyze_spec', { spec: bad, data_dir: 'examples/r-model' });
  check('T4.2 列名拼错 → SPEC_INVALID + json_pointer', b.isError && b.payload.error?.code === 'SPEC_INVALID' && typeof b.payload.error?.json_pointer === 'string', b.payload?.error?.code);

  const nod = structuredClone(spec);
  delete nod.dataset;
  const n = await c.tool('analyze_spec', { spec: nod, data_dir: 'examples/r-model' });
  check('T4.3 缺 dataset → SPEC_INVALID', n.isError && n.payload.error?.code === 'SPEC_INVALID', n.payload?.error?.code);

  // 分组序分歧：取值序 c,a,b 与字典序不同 ⇒ 必须冒未决问题
  dataFile('order.csv', 'g,amount\nc,1\na,2\nb,3\n');
  const ospec = {
    schema: 'rpx-spec/v1',
    title: '分组序',
    dataset: { name: 'd', file: 'order.csv' },
    layout: 'grouped',
    measures: [{ field: 'amount', agg: 'sum', label: '金额' }],
    row_dims: [{ field: 'g', label: '组' }],
  };
  const o = await c.tool('analyze_spec', { spec: ospec, data_dir: 'target/mcp-test-data' });
  const qs = o.payload.analysis?.open_questions || [];
  check('T4.4 分组序分歧 → open_questions 含「字典序」', !o.isError && qs.length > 0 && qs.some((q) => String(q.question).includes('字典序')), JSON.stringify(qs).slice(0, 200));

  const zh = await c.tool('analyze_spec', { spec, data_dir: 'examples/r-model' });
  const raw = JSON.stringify(zh.payload);
  // Node 的 JSON.stringify 不转义中文；出现 \uXXXX 即说明链路某处按 ASCII 重编码过
  check('T4.5 中文无乱码', !zh.isError && raw.includes('华东') && raw.includes('金额') && !raw.includes('\\u'), raw.slice(0, 120));
}

// ── T5 compile_spec ──────────────────────────────────────────────────────────

async function t5(c) {
  section('T5 compile_spec');
  const spec = crosstabSpec();
  const name = `mcp-t5-${Date.now().toString(36)}`;
  const g1 = await c.tool('compile_spec', { spec, name, data_dir: 'examples/r-model' });
  const out = path.join(REPO, 'compiled', `${name}.json`);
  const geo = g1.payload.geometry;
  check('T5.1 合法 spec → geometry + 模板落盘', !g1.isError && Number.isInteger(geo?.rows) && Number.isInteger(geo?.cols) && geo.rows >= 2 && geo.cols >= 2 && fs.existsSync(out), JSON.stringify(g1.payload).slice(0, 200));

  const bad = structuredClone(spec);
  bad.layout = 'pivot';
  const p = await c.tool('compile_spec', { spec: bad, data_dir: 'examples/r-model' });
  // 实测行为：parse 阶段即拒（SPEC_INVALID），指针指到 $.layout，message 列出受支持形态
  check('T5.2 layout=pivot → 拒绝且指出受支持形态', p.isError && p.payload.error?.code === 'SPEC_INVALID' && p.payload.error?.json_pointer === '$.layout' && String(p.payload.error?.message).includes('crosstab'), p.payload?.error?.code);

  const name2 = `${name}-again`;
  await c.tool('compile_spec', { spec, name: name2, data_dir: 'examples/r-model' });
  const out2 = path.join(REPO, 'compiled', `${name2}.json`);
  const same = fs.existsSync(out) && fs.existsSync(out2) && fs.readFileSync(out).equals(fs.readFileSync(out2));
  check('T5.3 同 spec 编译两次 → 逐字节一致', same);

  return { template: `compiled/${name}.json`, geometry: g1.payload.geometry };
}

// ── T6 run_report ────────────────────────────────────────────────────────────

async function t6(c, compiled) {
  section('T6 run_report');
  const r1 = await c.tool('run_report', { template: compiled.template, data_dir: 'examples/r-model', exports: ['csv'] });
  const csv = (r1.payload.products || []).find((p) => p.format === 'csv');
  check('T6.1 csv 导出 → 产物存在', !r1.isError && csv?.exists, JSON.stringify(r1.payload).slice(0, 200));
  let total = '';
  try {
    total = fs.readFileSync(csv.path, 'utf8');
  } catch {}
  check('T6.1b csv 含总计 125600', total.includes('125600'), total.slice(-120));

  const r2 = await c.tool('run_report', { template: compiled.template, data_dir: 'examples/r-model', exports: ['xlsx'] });
  const xlsx = (r2.payload.products || []).find((p) => p.format === 'xlsx');
  check('T6.2 xlsx 导出 → 存在且 >5KB', !r2.isError && xlsx?.exists && xlsx.bytes > 5000, `bytes=${xlsx?.bytes}`);

  const before = fs.readdirSync(path.join(REPO, 'runs')).length;
  const bad = await c.tool('run_report', { template: compiled.template, data_dir: 'examples/r-model', exports: ['pdf'] });
  const after = fs.readdirSync(path.join(REPO, 'runs')).length;
  check('T6.3 非法 export → 壳层拒绝且不落 run 目录', bad.isError && bad.payload.error?.code === 'EXPORT_NOT_ALLOWED' && before === after, bad.payload?.error?.code);

  const r3 = await c.tool('run_report', { template: compiled.template, data_dir: 'examples/r-model' });
  const r4 = await c.tool('run_report', { template: compiled.template, data_dir: 'examples/r-model' });
  check('T6.4 连续两次 run → out-dir 不冲突', !r3.isError && !r4.isError && r3.payload.out_dir !== r4.payload.out_dir && fs.existsSync(r3.payload.out_dir) && fs.existsSync(r4.payload.out_dir), `${r3.payload.out_dir} vs ${r4.payload.out_dir}`);
}

// ── T7 安全 ──────────────────────────────────────────────────────────────────

async function t7(c) {
  section('T7 安全');
  const esc = await c.tool('inspect_dataset', { file: '../../etc/passwd' });
  check('T7.1 ../ 越界 → 拒绝', esc.isError && esc.payload.error?.code === 'PATH_OUTSIDE_ROOT', esc.payload?.error?.code);

  const abs = await c.tool('inspect_dataset', { file: path.join(REPO, 'examples/library/ar-aging/data/ar-aging.csv') });
  check('T7.2 白名单内绝对路径 → 正常', !abs.isError && abs.payload.columns?.length === 3);

  // Windows 文件名不能含引号，选空格 + 分号 + 双连字符这类 shell 敏感字符
  const weird = 'we ird; --x.csv';
  dataFile(weird, 'a,b\n1,2\n');
  const w = await c.tool('inspect_dataset', { file: `target/mcp-test-data/${weird}` });
  check('T7.3 空格/分号/-- 文件名不裂开', !w.isError && w.payload.columns?.join(',') === 'a,b', JSON.stringify(w.payload).slice(0, 160));

  const escRun = await c.tool('run_report', { template: '../../elsewhere/tpl.json' });
  check('T7.4 run_report 同样收口', escRun.isError && escRun.payload.error?.code === 'PATH_OUTSIDE_ROOT');
}

// ── T6.5 超时（直接测 spawn 层，假慢命令） ────────────────────────────────────

async function tTimeout() {
  section('T6.5 超时');
  const slow = path.join(DATA_DIR, 'slow-bin.mjs');
  fs.writeFileSync(slow, 'setTimeout(() => {}, 60000);\n', 'utf8');
  const { runBin } = await import(pathToFileURL(path.join(HERE, 'server.mjs')).href);
  const r = await runBin(NODE, [slow], { timeoutMs: 900 });
  check('T6.5 超时 → kill + timedOut', r.timedOut === true && r.code === null, `code=${r.code} timedOut=${r.timedOut}`);
}

// ── 主流程 ───────────────────────────────────────────────────────────────────

async function main() {
  console.log(`rpx-mcp 验收：${REPO}`);
  console.log(`rpx.exe：${RPX} ${fs.existsSync(RPX) ? '' : '（缺失！先 cargo build --release -p rpx-cli）'}`);
  fs.mkdirSync(path.join(REPO, 'runs'), { recursive: true });
  const c = new Client();
  try {
    await t1(c);
    await t2(c);
    await t3(c);
    await t4(c);
    const compiled = await t5(c);
    await t6(c, compiled);
    await t7(c);
    await tTimeout();
  } finally {
    c.close();
  }
  const failed = results.filter((r) => !r.ok);
  console.log(`\n结果：${results.length - failed.length}/${results.length} 通过`);
  if (c.stderr.trim()) {
    console.log('server stderr（前 500 字）：');
    console.log(c.stderr.slice(0, 500));
  }
  if (failed.length) {
    console.log('\n失败项：');
    for (const f of failed) console.log(`  - ${f.name}${f.detail ? ` — ${f.detail}` : ''}`);
    process.exit(1);
  }
  console.log('MCP_SERVER_OK');
}

main().catch((e) => {
  console.error('验收脚本自身崩溃：', e);
  process.exit(2);
});
