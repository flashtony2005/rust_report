/**
 * Rust 服务端（`rpx-server`，8090）的**唯一** HTTP 出口。
 *
 * 约定三件事，改这个文件前先读：
 *
 * 1. **路径不带端口**：一律 `/api/...`，开发期由 vite 转发（`vite.config.ts` 的 rewrite
 *    把 `/api` 前缀剥掉），生产期由同域反代。代码里出现 `localhost:8090` 就算错。
 * 2. **服务端只有 10 条路由**（`crates/rpx-server/src/http.rs:31-40`）：报表列表/详情/存模板/
 *    运行/运行记录/产物。数据源、业务模型、SQL 编译**还没有** Rust 端点，
 *    对应页面在 P1 之前一律标「待后端」，**不许用前端 mock 冒充**。
 * 3. **计算只发生在服务端**：`runReport()` 返回的是产物清单（run 记录 + 文件），
 *    浏览器不做任何聚合。这条是产品红线（见 docs/merge-master-analysis.md §6.1）。
 */

/** 与服务端一致的错误体：`{"error":{"code","message",...}}`。 */
export class ApiError extends Error {
  code: string;
  status: number;
  detail: unknown;

  constructor(status: number, code: string, message: string, detail?: unknown) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
    this.detail = detail;
  }
}

const TOKEN_KEY = 'rpx.token';

/** `rpx-server --auth-token` 启用后，前端需要带 Bearer。默认无鉴权。 */
export function getToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

export function setToken(token: string | null): void {
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* 隐私模式下 localStorage 会抛，登录态丢失不影响只读浏览 */
  }
}

type Query = Record<string, string | number | boolean | undefined | null>;

function withQuery(path: string, query?: Query): string {
  if (!query) return path;
  const pairs = Object.entries(query)
    .filter(([, v]) => v !== undefined && v !== null && v !== '')
    .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`);
  return pairs.length ? `${path}?${pairs.join('&')}` : path;
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  const token = getToken();
  if (token) headers.set('Authorization', `Bearer ${token}`);
  if (init.body !== undefined && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  const res = await fetch(path, { ...init, headers });
  const text = await res.text();
  const body = text ? safeJson(text) : null;

  if (!res.ok) {
    // 服务端的错误体是 {error:{code,message}}；网络层错误没有体（例如 502 由代理返回）。
    const err = (body as { error?: { code?: string; message?: string } } | null)?.error;
    throw new ApiError(
      res.status,
      err?.code || `HTTP_${res.status}`,
      err?.message || res.statusText || `请求失败（${res.status}）`,
      body,
    );
  }
  return body as T;
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

// ---------------------------------------------------------------------------
// 类型：只写服务端真的会返回的字段（`library::list` / `library::show` / `run`）
// ---------------------------------------------------------------------------

/** 报表库条目（`GET /reports` 的 items[]）。`template` 已被剥掉，只留摘要。 */
export interface ReportSummary {
  id: string;
  manifest: {
    version?: string;
    id?: string;
    name?: string;
    description?: string;
    tags?: string[];
    template?: string;
    data_dir?: string;
  };
  revision: string;
  template_path?: string;
  /**
   * 结果与当前修订的关系（`library.rs:218`）：
   * `current_revision`（跑过且是最新）/ `older_revision`（模板改过没重跑）/ `none`（从没跑过）。
   */
  result_state?: 'current_revision' | 'older_revision' | 'none' | string;
  /** 用来展示的那一次运行（就是「结果」页签要装的那份快照）。 */
  display_run?: RunRecord | null;
  display_run_id?: string | null;
  /** 最近一次尝试（可能失败）；`display_run` 是成功那次。 */
  latest_attempt?: RunRecord | null;
  /** `/reports?query=` 命中时由服务端算出的分数（未搜索则没有该字段）。 */
  match_score?: number;
}

/**
 * 分页体（`files::page`，`crates/rpx-runtime/src/files.rs:97`）：
 * `{items, returned, total, next_offset}` —— `next_offset` 为 null 表示没有下一页。
 * `limit` 必须在 1..=500，越界直接 USAGE 400（不是截断）。
 */
export interface Page<T> {
  items: T[];
  returned?: number;
  total?: number;
  next_offset?: number | null;
  diagnostics?: Array<{ severity: string; code: string; message?: string; report?: string }>;
}

export type ReportPage = Page<ReportSummary>;

/** `GET /reports/{id}`：带完整模板定义（`rpx-spec/v1` 或 legacy `.rpx` 的 JSON 形态）。 */
export interface ReportDetail {
  id: string;
  revision: string;
  manifest: ReportSummary['manifest'];
  template: unknown;
  template_path?: string;
  /** 与 list 同一个 `state()` 产物（`library.rs:208-216`），字段在顶层。 */
  result_state?: ReportSummary['result_state'];
  display_run?: RunRecord | null;
  display_run_id?: string | null;
  latest_attempt?: RunRecord | null;
}

export interface RunRecord {
  run_id: string;
  status?: string;
  started_at?: string;
  finished_at?: string;
  duration_ms?: number;
  /** 产物名 → 运行目录内的相对路径（`{"workbook.json":"artifacts/workbook.json"}`）。 */
  artifacts?: Record<string, string>;
  /** 版面账：`{rows, cols, page_count, paper_w_pt, …}`（引擎算的，别在前端估）。 */
  layout?: Record<string, unknown>;
  revision?: string;
  report_id?: string;
  engine_version?: string;
  parameters?: Record<string, unknown>;
  inputs?: Array<Record<string, unknown>>;
  diagnostics?: Array<Record<string, unknown>>;
  shape?: Record<string, unknown>;
  error?: { code?: string; message?: string } | null;
}

export type RunPage = Page<RunRecord>;

export interface ExecuteOptions {
  /** 报表参数（`${name}` 占位符）；字符串键值。 */
  params?: Record<string, unknown>;
  /** 数据目录覆盖（相对路径基准是模板所在目录，别填 `data/`）。 */
  data_dir?: string;
  /** 额外导出：`csv` / `xlsx` / `print-xlsx` / `html` / `pdf`。 */
  exports?: string[];
  /** 期望的模板修订；给了就按它校验（412 = 源码已变）。缺省取库内当前修订。 */
  revision?: string;
}

// ---------------------------------------------------------------------------
// 语义层：数据源（`datasources_http.rs`）
// ---------------------------------------------------------------------------

/** 一条数据源记录。**响应里永远没有口令**（只有 `url_masked`）。 */
export interface Datasource {
  id: string;
  label?: string;
  driver: 'postgres' | 'mysql' | 'sqlite' | string;
  host?: string;
  port?: number;
  /** sqlite 是文件路径；其余是库名。 */
  database?: string;
  user?: string;
  /** 只在**提交**时出现；服务端从不回传（回传的是空串）。 */
  password?: string;
  options?: string;
  created_at?: string;
  updated_at?: string;
  /** 打了码的连接串，用于让人核对"连的是哪个库"。 */
  url_masked?: string;
  /** Python 侧历史字段（kind / sql / sample_rows…），迁移时原样保留。 */
  [k: string]: unknown;
}

export interface DatasourcePage {
  items: Datasource[];
  total: number;
  /** 注册表文件路径（排障第一信息）。 */
  registry?: string;
}

/** 一列的画像：类型靠值判，不是库里的 `varchar(64)`。 */
export interface ColumnProfile {
  name: string;
  kind: 'number' | 'decimal' | 'text' | 'bool' | 'null' | 'mixed' | string;
  nulls: number;
  distinct: number;
  samples: string[];
}

export interface Probe {
  sql: string;
  columns: ColumnProfile[];
  /** 抽样行（最多 500），值已转成 JSON 可读形态。 */
  rows: unknown[][];
  sampled: number;
  /** 主键列（探测不到就是空数组）。 */
  key: string[];
  elapsed_ms: number;
}

export function listDatasources(): Promise<DatasourcePage> {
  return request<DatasourcePage>('/api/datasources');
}

/** 新建或覆盖（`id` 缺省由服务端生成）。返回的记录不含口令。 */
export function saveDatasource(ds: Partial<Datasource> & { driver: string }): Promise<{
  ok: boolean;
  item: Datasource;
}> {
  return request<{ ok: boolean; item: Datasource }>('/api/datasources', {
    method: 'POST',
    body: JSON.stringify(ds),
  });
}

export function deleteDatasource(id: string): Promise<{ ok: boolean; id: string }> {
  return request<{ ok: boolean; id: string }>(`/api/datasources/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
}

export interface ProbeOptions {
  /** 表名（优先于 sql）：服务端会拼 `select * from 表` 并顺便取主键。 */
  table?: string;
  sql?: string;
  limit?: number;
}

/** 用**已保存**的记录探测；`{}` = 只验连通性（`select 1`）。 */
export function probeDatasource(
  id: string,
  opts: ProbeOptions = {},
): Promise<{ ok: boolean; probe: Probe }> {
  return request<{ ok: boolean; probe: Probe }>(
    `/api/datasources/${encodeURIComponent(id)}/probe`,
    { method: 'POST', body: JSON.stringify(opts) },
  );
}

/** 用**未保存**的记录探测（表单里的「测试连接」）。 */
export function probeRecord(
  record: Partial<Datasource> & { driver: string },
  opts: ProbeOptions = {},
): Promise<{ ok: boolean; probe: Probe }> {
  return request<{ ok: boolean; probe: Probe }>('/api/datasources/probe', {
    method: 'POST',
    body: JSON.stringify({ record, ...opts }),
  });
}

export function listTables(id: string): Promise<{ ok: boolean; items: string[] }> {
  return request<{ ok: boolean; items: string[] }>(
    `/api/datasources/${encodeURIComponent(id)}/tables`,
  );
}

// ---------------------------------------------------------------------------
// 语义层：业务模型（`bizmodels_http.rs`，契约 rpx-bizmodel/v1）
// ---------------------------------------------------------------------------

export interface BizEvidence {
  [k: string]: unknown;
}

/** 一条业务模型条目（实体/关系/指标/维度/规则的公共部分）。 */
export interface BizItem {
  id: string;
  label?: string;
  status: 'draft' | 'candidate' | 'confirmed' | string;
  evidence?: string[];
  [k: string]: unknown;
}

export interface BizModel {
  schema: string;
  version: number;
  source: { dsn: string; generated_at: string; generator: string };
  entities: Array<
    BizItem & {
      table?: string;
      grain?: { key?: string[]; description?: string; status?: string; evidence?: string[] };
    }
  >;
  relationships: Array<
    BizItem & {
      from?: { entity?: string; key?: string[] };
      to?: { entity?: string; key?: string[] };
      cardinality?: string;
    }
  >;
  metrics: Array<
    BizItem & {
      entity?: string;
      column?: string | null;
      expression?: string;
      aliases?: string[];
      format?: string;
      time_field?: string | null;
    }
  >;
  dimensions: Array<
    BizItem & {
      entity?: string;
      column?: string | null;
      kind?: string;
      values?: string[] | null;
      aliases?: string[];
      time_grain?: string | null;
    }
  >;
  rules: Array<BizItem & { expression?: string; applies_to?: string[] }>;
  open_questions: Array<{ question: string; source?: string; evidence?: string[] }>;
}

export interface BizModelReply {
  ok: boolean;
  /** `null` = 还没发现过（不是错误）。 */
  model: BizModel | null;
  counts?: Record<string, number>;
  discovered?: Record<string, number>;
  sampled?: boolean;
  /** 读的来源：`db` 走语义库，`json` 回落文件，`none` 连文件都没有。 */
  source?: 'db' | 'json' | 'none';
  /** 写之后库同步的结果：`stale` 表示文件已写、索引落后（`warning` 里是原因）。 */
  store?: 'db' | 'stale';
  warning?: string;
}

export type BizSection = 'entities' | 'metrics' | 'dimensions' | 'rules';

export function getBizModel(dsn: string): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}`);
}

/** 连库发现（`sample=false` 跳过值域采样，大库上快很多）。 */
export function discoverBizModel(dsn: string, sample = true): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/discover`, {
    method: 'POST',
    body: JSON.stringify({ sample }),
  });
}

/** 人工晋升（唯一能把 status 写成 confirmed 的入口）。 */
export function promoteBizItem(
  dsn: string,
  section: BizSection,
  id: string,
): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/promote`, {
    method: 'POST',
    body: JSON.stringify({ section, id }),
  });
}


// ---------------------------------------------------------------------------
// 语义层：人改资产（表达式先过编译器白名单，人工改动即 confirmed）
// ---------------------------------------------------------------------------

/** 一条业务规则。`applies_to` 为空 = 全局；只有 `confirmed` 才会装配进 SQL。 */
export interface BizRule extends BizItem {
  expression?: string;
  applies_to?: string[];
}

/** 金样例：一句业务问句 + 期望的 IR + 可判定的期望值。 */
export interface GoldenExpect {
  rows?: number;
  columns?: string[];
  first_cell?: string;
  note?: string;
}

export interface Golden {
  id: string;
  question: string;
  query: Record<string, unknown>;
  expect: GoldenExpect;
  created_at: string;
  last_run?: {
    at: string;
    passed: boolean;
    detail: string;
    row_count: number;
    elapsed_ms: number;
  } | null;
}

/** 单条评测结论。`detail` 写清为什么通过/不通过。 */
export interface GoldenOutcome {
  id: string;
  question: string;
  passed: boolean;
  detail: string;
  row_count: number;
  elapsed_ms: number;
  sample: unknown[][];
  columns: string[];
  sql?: string | null;
}

export interface GoldenRunReply {
  ok: boolean;
  outcomes: GoldenOutcome[];
  summary: { total?: number; passed?: number; failed?: number; avg_ms?: number };
  total: number;
}

export interface QueryRunReply {
  ok: boolean;
  columns: string[];
  rows: unknown[][];
  row_count: number;
  elapsed_ms: number;
  compiled: {
    schema: string;
    dialect: string;
    sql: string;
    params: unknown[];
    warnings: Array<{ what: string; source: string }>;
    guardrails: Record<string, unknown>;
  };
  plan?: { warnings?: Array<{ what: string; source: string }>; rules?: Array<{ id: string; label: string }> };
}

export interface MetricInput {
  id: string;
  label?: string;
  expression: string;
  entity?: string;
  column?: string;
  format?: string;
  time_field?: string;
  aliases?: string[];
}

export interface DimensionInput {
  id: string;
  label?: string;
  kind?: string;
  column?: string;
  time_grain?: string;
  values?: string[];
  aliases?: string[];
}

export function upsertMetric(dsn: string, body: MetricInput): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/metric`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function patchDimension(dsn: string, body: DimensionInput): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/dimension`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function upsertRule(
  dsn: string,
  body: { id?: string; label: string; expression: string; applies_to?: string[] },
): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/rule`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function deleteRule(dsn: string, id: string): Promise<BizModelReply> {
  return request<BizModelReply>(
    `/api/bizmodels/${encodeURIComponent(dsn)}/rule/${encodeURIComponent(id)}`,
    { method: 'DELETE' },
  );
}

export function deleteMetric(dsn: string, id: string): Promise<BizModelReply> {
  return request<BizModelReply>(
    `/api/bizmodels/${encodeURIComponent(dsn)}/metric/${encodeURIComponent(id)}`,
    { method: 'DELETE' },
  );
}

export function deleteDimension(dsn: string, id: string): Promise<BizModelReply> {
  return request<BizModelReply>(
    `/api/bizmodels/${encodeURIComponent(dsn)}/dimension/${encodeURIComponent(id)}`,
    { method: 'DELETE' },
  );
}

// ---------------------------------------------------------------------------
// 需求层（`POST /requirements`）：一句话 → 形状决策 → spec → 一份真报表
// ---------------------------------------------------------------------------

export interface ShapeInfo {
  shape: string;
  label: string;
  layout: string;
  needs: string[];
}

export function listShapes(): Promise<{ ok: boolean; items: ShapeInfo[] }> {
  return request('/api/shapes');
}

export interface RequirementRequest {
  dsn: string;
  /** 一句话（用来选形态与标题；不填就看 metrics/dimensions） */
  text?: string;
  title?: string;
  metrics?: string[];
  dimensions?: string[];
  /** 强制形态（不填按证据判） */
  shape?: string;
  limit?: number;
  /** 生成后立即跑一次 */
  run?: boolean;
  id?: string;
}

export interface RequirementReply {
  ok: boolean;
  report_id: string;
  id?: string;
  title: string;
  shape: string;
  decision: {
    shape: string;
    label: string;
    layout: string;
    confidence: string;
    reasons: string[];
    blockers: string[];
    alternatives: Array<{ shape: string; why: string }>;
    ok: boolean;
  };
  spec: Record<string, unknown>;
  notes: string[];
  query: {
    metrics: string[];
    dimensions: string[];
    picked_by: string;
    sql: string;
    columns: string[];
    row_count: number;
  };
  run: { ok: boolean; status?: string; error?: string } | null;
}

export function createRequirement(body: RequirementRequest): Promise<RequirementReply> {
  return request<RequirementReply>('/api/requirements', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

// ---------------------------------------------------------------------------
// 归因分析（`rpx-analysis/v1`）：一次分解 = 若干受控查询
// ---------------------------------------------------------------------------

export interface AnalysisOption {
  id: string;
  label: string;
  status?: string;
  time_field?: string | null;
  time_grain?: string | null;
  kind?: string;
}

export interface AnalysisOptions {
  ok: boolean;
  dsn: string;
  metrics: AnalysisOption[];
  date_dimensions: AnalysisOption[];
  text_dimensions: AnalysisOption[];
  defaults: { metric: string; time_field: string; grain: string; dimensions: string[] };
  budget: { max_steps: number; max_rows: number; top_k: number };
  window: string;
}

export function getAnalysisOptions(dsn: string): Promise<AnalysisOptions> {
  return request<AnalysisOptions>(`/api/analysis/options?dsn=${encodeURIComponent(dsn)}`);
}

/** 一步：IR + 计划 + SQL + 结果（每步都能单独复现）。 */
export interface AnalysisStep {
  id: string;
  n: number;
  kind: 'baseline' | 'split' | 'drill' | string;
  title: string;
  ok: boolean;
  sql?: string | null;
  params?: unknown[] | null;
  columns: string[];
  rows: unknown[][];
  row_count: number;
  error?: { code: string; message: string; pointer?: string } | null;
}

export interface AnalysisFinding {
  kind: 'observed' | 'computed';
  what: string;
  value?: number | null;
  pct?: number | null;
  share?: number | null;
  member?: string;
  sub_member?: string;
  evidence: string[];
}

export interface AnalysisDoc {
  schema: string;
  id: string;
  dsn: string;
  question: string;
  metric: string;
  metric_label: string;
  time_field: string;
  grain: string;
  dimensions: string[];
  window: string;
  windows: { cur_start: string; cur_end: string; prev_start: string; prev_end: string };
  budget: { max_steps: number; max_rows: number; top_k: number };
  totals?: { cur: number | null; prev: number | null; growth: number | null; delta: number | null };
  steps: AnalysisStep[];
  findings: AnalysisFinding[];
  caveats: string[];
  summary: string;
  status: 'ok' | 'partial' | 'failed' | string;
  coverage?: number;
  generated_at: string;
}

export interface AnalysisRequest {
  dsn: string;
  metric?: string;
  time_field?: string;
  grain?: string;
  dimensions?: string[];
  window?: string;
  as_of?: string;
  time_range?: { start: string; end: string };
  budget?: { max_steps?: number; max_rows?: number; top_k?: number };
}

export function runAnalysis(body: AnalysisRequest): Promise<{ ok: boolean; document: AnalysisDoc }> {
  return request<{ ok: boolean; document: AnalysisDoc }>('/api/analysis', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

// ---------------------------------------------------------------------------
// 语义层：结构性编辑（实体粒度 / 关系基数 / 待确认问题）
//
// 这三类是**会改数**的编辑（粒度决定分母、基数决定行数放大），所以只能人来定；
// 后端一律标 `human_edited` 并置 `confirmed`。
// ---------------------------------------------------------------------------

export interface EntityInput {
  id: string;
  label?: string;
  grain_description?: string;
  /** 粒度键（主键列名）。改了等于重新定义"一行代表什么"。 */
  grain_key?: string[];
}

export function patchEntity(dsn: string, body: EntityInput): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/entity`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export interface RelationEnd {
  entity: string;
  key: string[];
}

export function patchRelationship(
  dsn: string,
  body: {
    from: RelationEnd;
    to: RelationEnd;
    cardinality?: string;
    status?: 'draft' | 'candidate' | 'confirmed';
  },
): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/relationship`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

/** 处置待确认问题：`dismiss` 按下标移除（已处理），`add` 补一条人发现的问题。 */
export function askQuestion(
  dsn: string,
  body: {
    action: 'add' | 'dismiss';
    index?: number;
    question?: string;
    source?: string;
    evidence?: string[];
  },
): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/question`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

/** 语义库状态：`enabled:false` 是状态不是错误（本次启动没启用库）。 */
export interface SemanticDbStatus {
  ok: boolean;
  enabled: boolean;
  path?: string;
  counts?: Record<string, number>;
  empty?: string[];
}

export function getSemanticDb(): Promise<SemanticDbStatus> {
  return request<SemanticDbStatus>('/api/semantic/db');
}

/** 加/删同义词。改别名不动口径（`status` 不变），历史结果仍可比。 */
export function editAlias(
  dsn: string,
  body: { section: 'metrics' | 'dimensions'; id: string; add?: string; remove?: string },
): Promise<BizModelReply> {
  return request<BizModelReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/alias`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function listGoldens(
  dsn: string,
): Promise<{ ok: boolean; items: Golden[]; total: number; path?: string }> {
  return request(`/api/bizmodels/${encodeURIComponent(dsn)}/goldens`);
}

export function upsertGolden(
  dsn: string,
  body: { id?: string; question: string; query: Record<string, unknown>; expect: GoldenExpect },
): Promise<{ ok: boolean; item: Golden; total: number }> {
  return request(`/api/bizmodels/${encodeURIComponent(dsn)}/goldens`, {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export function deleteGolden(
  dsn: string,
  id: string,
): Promise<{ ok: boolean; deleted: string; total: number }> {
  return request(
    `/api/bizmodels/${encodeURIComponent(dsn)}/goldens/${encodeURIComponent(id)}`,
    { method: 'DELETE' },
  );
}

/** 全量（或单条）评测：服务端真跑 SQL，结论落盘。 */
export function runGoldens(dsn: string, id?: string): Promise<GoldenRunReply> {
  return request<GoldenRunReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/goldens/run`, {
    method: 'POST',
    body: JSON.stringify(id ? { id } : {}),
  });
}

/** 真跑一次 IR（样例"试跑"与人工核对都用它）。 */
export function runQuery(dsn: string, query: Record<string, unknown>): Promise<QueryRunReply> {
  return request<QueryRunReply>('/api/query/run', {
    method: 'POST',
    body: JSON.stringify({ dsn, query }),
  });
}

/** 只编译：给 SQL 看，不碰数据。 */
export function compileQuery(
  dsn: string,
  query: Record<string, unknown>,
): Promise<QueryRunReply & { validation: unknown; plan: unknown }> {
  return request('/api/query/compile', {
    method: 'POST',
    body: JSON.stringify({ dsn, query }),
  });
}

// ---------------------------------------------------------------------------
// 本体解析：`GET /bizmodels/{dsn}/ontology`（真实模型图谱） + `/ontology*`（四步痕迹落库/复用）
//
// 分工与后端一致：前端是"大脑"（四步确定性规则匹配），后端是"记忆"
// （人确认过的解析结论，下次同样的问句可以直接调出来看）。所以这里只取图谱 +
// 存/取痕迹，**不**让前端重新算四步（任意模型的行级聚合要等服务端 `/query/run`）。
// ---------------------------------------------------------------------------

/** 真实模型暴露的本体图谱（来自 `catalog.model` 真相入口）。 */
export interface OntoEntity {
  id: string;
  name: string;
  table: string | null;
  status: string;
}

export interface OntoEdge {
  from: string;
  to: string;
  /** `1:N` / `N:1` / `1:1` / `N:N` —— 与示例夹具口径一致。 */
  cardinality: string;
  evidence: string;
}

export interface OntoMetric {
  id: string;
  label: string;
  entity: string;
  expression: string | null;
}

export interface OntoDim {
  id: string;
  label: string;
  entity: string;
  column: string | null;
  kind: string | null;
}

export interface OntoGraph {
  entities: OntoEntity[];
  edges: OntoEdge[];
  metrics: OntoMetric[];
  dims: OntoDim[];
}

export interface OntologyGraphReply {
  ok: boolean;
  /** `false` = 该 dsn 还没发现过模型（不是错误），前端退回示例夹具。 */
  has_model: boolean;
  ontology: OntoGraph | null;
}

/** 取某 dsn 的真实本体图谱。`has_model:false` 时 `ontology` 为 `null`。 */
export function fetchOntology(dsn: string): Promise<OntologyGraphReply> {
  return request<OntologyGraphReply>(`/api/bizmodels/${encodeURIComponent(dsn)}/ontology`);
}

/** 四步痕迹里的一步（落库/复用时用的形状，字段名对齐 `onto_step`）。 */
export interface OntoStep {
  step_no: number;
  kind?: string;
  title?: string;
  /** 这一步行级结果（JSON 串，前端存什么后端原样回）。 */
  payload_json?: string;
  sql?: string;
  row_count?: number;
  ok?: number;
  detail?: string;
}

/** 一次命中（字段名对齐 `onto_hit`）。 */
export interface OntoHit {
  ord: number;
  token?: string;
  kind: string;
  target?: string;
  entity_id?: string;
  evidence?: string;
}

export interface OntologySaveRequest {
  dsn: string;
  /** 被解析的那句话（驱动了这次四步）。 */
  question: string;
  status?: string;
  center_entity?: string;
  fanout_edges?: number;
  sql?: string;
  elapsed_ms?: number;
  steps: OntoStep[];
  hits: OntoHit[];
}

export interface OntologySaveReply {
  ok: boolean;
  id: string;
}

/**
 * 保存一次四步解析痕迹（`POST /ontology`）。
 * 语义库未启用时服务端回 `SEMANTIC_DB_OFF`（404），前端据此提示"用 --semantic-db 打开"。
 */
export function saveOntology(body: OntologySaveRequest): Promise<OntologySaveReply> {
  return request<OntologySaveReply>('/api/ontology', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

export interface OntologyListItem {
  id: string;
  question: string;
  status: string;
  center_entity: string | null;
  fanout_edges: number;
  created_at: string;
}

export interface OntologyListReply {
  ok: boolean;
  items: OntologyListItem[];
  returned: number;
  /** 语义库没启用时为 `"disabled"`（不是错误）。 */
  store?: string;
}

/** 列出某 dsn 保存过的解析结论（历史复用）。 */
export function listOntologies(dsn: string, limit = 50): Promise<OntologyListReply> {
  return request<OntologyListReply>(withQuery('/api/ontology', { dsn, limit }));
}

export interface OntologyTrace extends OntologySaveRequest {
  ok: boolean;
  id: string;
  dsn: string;
  created_at: string;
}

/** 取一条完整痕迹（四步 + 命中），复用 / 审计用。 */
export function getOntology(id: string): Promise<OntologyTrace> {
  return request<OntologyTrace>(`/api/ontology/${encodeURIComponent(id)}`);
}

// ---------------------------------------------------------------------------
// 接口：一一对应 http.rs 的 10 条路由
// ---------------------------------------------------------------------------

export function health(): Promise<string> {
  return request<string>('/api/health');
}

export function authStatus(): Promise<{ required: boolean }> {
  return request<{ required: boolean }>('/api/auth/status');
}

/** 报表库列表。`query` 走服务端 CJK 分词检索（不是前端过滤）。 */
export function listReports(
  opts: { query?: string; tag?: string; limit?: number; offset?: number } = {},
): Promise<ReportPage> {
  return request<ReportPage>(withQuery('/api/reports', { ...opts }));
}

export function getReport(id: string): Promise<ReportDetail> {
  return request<ReportDetail>(`/api/reports/${encodeURIComponent(id)}`);
}

/** 存模板：**必须**带修订（`If-Match`），否则服务端回 428。 */
export function saveTemplate(id: string, revision: string, template: unknown): Promise<ReportDetail> {
  return request<ReportDetail>(`/api/reports/${encodeURIComponent(id)}/template`, {
    method: 'PUT',
    headers: { 'If-Match': `"${revision}"` },
    body: JSON.stringify(template),
  });
}

export function listRuns(
  id: string,
  opts: { limit?: number; offset?: number } = {},
): Promise<RunPage> {
  return request<RunPage>(withQuery(`/api/reports/${encodeURIComponent(id)}/runs`, { ...opts }));
}

export function getRun(id: string, run: string): Promise<RunRecord> {
  return request<RunRecord>(
    `/api/reports/${encodeURIComponent(id)}/runs/${encodeURIComponent(run)}`,
  );
}

/**
 * 触发一次运行 —— 设计器「结果」按钮打的这里。
 *
 * 服务端在 Rust 进程里执行引擎，浏览器只等产物；`data_dir` 缺省是模板所在目录。
 */
export function runReport(id: string, opts: ExecuteOptions = {}): Promise<unknown> {
  const body: Record<string, unknown> = {};
  if (opts.params !== undefined) body.params = opts.params;
  if (opts.data_dir !== undefined) body.data_dir = opts.data_dir;
  if (opts.exports !== undefined) body.exports = opts.exports;
  const headers: Record<string, string> = {};
  if (opts.revision) headers['If-Match'] = `"${opts.revision}"`;
  return request<unknown>(`/api/reports/${encodeURIComponent(id)}/runs`, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
}

/** 台账里的一次运行（`rpt_run`）。`error_code` 是失败时唯一好搜的线索。 */
export interface ReportLedgerRun {
  run_id: string;
  revision: string | null;
  status: string;
  finished_at: string | null;
  duration_ms: number | null;
  error_code: string | null;
  error_message: string | null;
}

export interface ReportLedger {
  id: string;
  name: string;
  description: string | null;
  tags: string[] | null;
  current_revision: string | null;
  template_kind: string | null;
  updated_at: string | null;
  revisions: Array<{ revision: string; note: string | null; created_at: string | null }>;
  runs: ReportLedgerRun[];
  artifacts: Array<{ run_id: string; name: string; path: string | null; bytes: number | null }>;
}

/** 台账（`rpt_*`）：改过几版 / 跑过几次 / 产物多大。文件仍是真相，库是索引。 */
export function getReportLedger(id: string): Promise<{ ok: boolean; ledger: ReportLedger }> {
  return request(`/api/reports/${encodeURIComponent(id)}/ledger`);
}

/** 一次跨报表的运行流水项（`GET /runs`）。 */
export interface RecentRun {
  run_id: string;
  report_id: string;
  /** 报表显示名（报表被删过时可能是 null，靠 `report_id` 兜底） */
  report_name: string | null;
  revision: string | null;
  /** succeeded / failed / running */
  status: string | null;
  trigger: string | null;
  started_at: string | null;
  finished_at: string | null;
  duration_ms: number | null;
  error_code: string | null;
  error_message: string | null;
  artifact_count: number;
  artifact_bytes: number;
}

export interface RecentRuns {
  total: number;
  failed: number;
  items: RecentRun[];
}

/**
 * 跨报表的运行流水。
 *
 * 为什么不用 `getReportLedger` 循环：那是「一份报表」的账，库里有 59 份，
 * 循环一次就是 59 个请求；这个问题库里一条 SQL 就能答。
 */
export function getRuns(params: {
  status?: string;
  report?: string;
  limit?: number;
} = {}): Promise<{ ok: boolean; runs: RecentRuns }> {
  const q = new URLSearchParams();
  if (params.status) q.set('status', params.status);
  if (params.report) q.set('report', params.report);
  if (params.limit) q.set('limit', String(params.limit));
  const qs = q.toString();
  return request(`/api/runs${qs ? `?${qs}` : ''}`);
}

/** 取一份产物并解析成 JSON（`workbook.json` 就是这么读的）。 */
export async function getArtifactJson<T = unknown>(
  id: string,
  run: string,
  name: string,
): Promise<T> {
  const token = getToken();
  const res = await fetch(artifactUrl(id, run, name), {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  });
  if (!res.ok) {
    throw new ApiError(res.status, `HTTP_${res.status}`, `产物 ${name} 读不出来（${res.status}）`);
  }
  return (await res.json()) as T;
}

export function artifactUrl(id: string, run: string, name: string): string {
  return `/api/reports/${encodeURIComponent(id)}/runs/${encodeURIComponent(run)}/artifacts/${encodeURIComponent(name)}`;
}

/** 源数据文件（运行目录里的 `source/…`），用于核对「结果里的数从哪来」。 */
export function sourceUrl(id: string, run: string, path: string): string {
  return `/api/reports/${encodeURIComponent(id)}/runs/${encodeURIComponent(run)}/source/${path}`;
}
