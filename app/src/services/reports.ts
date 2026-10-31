/**
 * 报表库状态 —— **唯一的数据源是 8090 上的 Rust 服务端**。
 *
 * 这里没有 mock、没有 seed、没有本地算数：拿不到就是「拿不到」，
 * 页面显示错误原因而不是演示数据（红线见 docs/merge-master-analysis.md §6.1）。
 *
 * 状态用 `useSyncExternalStore` 订阅，避免每个视图各写一套 loading/error 逻辑。
 */
import { useCallback, useSyncExternalStore } from 'react';
import {
  ApiError,
  getReport,
  listReports,
  listRuns,
  runReport,
  type ReportDetail,
  type ReportSummary,
  type RunRecord,
} from './api';

/** 把各种异常压成一句人话：HTTP 错误带 code，网络错误带 "Failed to fetch"。 */
export function reasonOf(e: unknown): string {
  if (e instanceof ApiError) {
    if (e.status === 401) return '需要 Bearer token：服务端启了 --auth-token';
    return `${e.code}（${e.status}）：${e.message}`;
  }
  if (e instanceof TypeError) return '连不上服务端（8090 没起？或被代理拦了）';
  return e instanceof Error ? e.message : String(e);
}

// ---------------------------------------------------------------------------
// 库列表
// ---------------------------------------------------------------------------

export interface LibraryState {
  status: 'loading' | 'ready' | 'error';
  items: ReportSummary[];
  total: number;
  error: string | null;
  /** 服务端检索词（走 library::search 的 CJK 分词，不是前端过滤）。 */
  query: string;
  tag: string;
}

class LibraryStore {
  private state: LibraryState = {
    status: 'loading',
    items: [],
    total: 0,
    error: null,
    query: '',
    tag: '',
  };
  private listeners = new Set<() => void>();
  /** 请求序号：晚到的旧响应不许覆盖新结果（搜索连续输入时必然发生）。 */
  private seq = 0;

  getSnapshot = (): LibraryState => this.state;

  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  private patch(part: Partial<LibraryState>): void {
    this.state = { ...this.state, ...part };
    this.listeners.forEach((fn) => fn());
  }

  async load(next: { query?: string; tag?: string } = {}): Promise<void> {
    const query = next.query ?? this.state.query;
    const tag = next.tag ?? this.state.tag;
    const seq = ++this.seq;
    this.patch({ status: 'loading', error: null, query, tag });
    try {
      // 500 是服务端硬上限（files::page 的 limit 校验），别传更大
      const page = await listReports({ query, tag, limit: 500 });
      if (seq !== this.seq) return;
      this.patch({
        status: 'ready',
        items: page.items,
        total: page.total ?? page.items.length,
        error: null,
      });
    } catch (e) {
      if (seq !== this.seq) return;
      this.patch({ status: 'error', error: reasonOf(e), items: [], total: 0 });
    }
  }
}

export const library = new LibraryStore();

export function useLibrary(): LibraryState {
  return useSyncExternalStore(library.subscribe, library.getSnapshot, library.getSnapshot);
}

// ---------------------------------------------------------------------------
// 单张报表（详情 + 运行记录 + 触发运行）
// ---------------------------------------------------------------------------

export interface ReportState {
  status: 'loading' | 'ready' | 'error';
  detail: ReportDetail | null;
  runs: RunRecord[];
  runsTotal: number;
  error: string | null;
  /** 正在跑（按钮要禁用并显示进度，不允许重复点）。 */
  running: boolean;
  /** 最近一次运行的错误（与 detail 的错误分开：运行失败不该让整页空白）。 */
  runError: string | null;
  /** 刚跑完那一次的结果（页数/耗时），用于提示条。 */
  lastRun: RunRecord | null;
}

/** 空态用**同一个冻结对象**：`useSyncExternalStore` 要求快照稳定，
 *  每次 `get()` 都 new 一个会让 React 认为「状态一直在变」并报
 *  “The result of getSnapshot should be cached”。 */
const EMPTY_REPORT: ReportState = Object.freeze({
  status: 'loading',
  detail: null,
  runs: [],
  runsTotal: 0,
  error: null,
  running: false,
  runError: null,
  lastRun: null,
});

class ReportStore {
  private cache = new Map<string, ReportState>();
  private listeners = new Set<() => void>();

  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  get(id: string): ReportState {
    return this.cache.get(id) ?? EMPTY_REPORT;
  }

  private set(id: string, part: Partial<ReportState>): void {
    const cur = this.cache.get(id) ?? EMPTY_REPORT;
    this.cache.set(id, { ...cur, ...part });
    this.listeners.forEach((fn) => fn());
  }

  /** 详情 + 运行记录一起拉；任一失败都把原因写进 `error`，页面据此显示。 */
  async open(id: string): Promise<void> {
    this.set(id, { status: 'loading', error: null });
    try {
      const [detail, runs] = await Promise.all([
        getReport(id),
        listRuns(id, { limit: 50 }),
      ]);
      this.set(id, {
        status: 'ready',
        detail,
        runs: runs.items,
        runsTotal: runs.total ?? runs.items.length,
      });
    } catch (e) {
      this.set(id, { status: 'error', error: reasonOf(e) });
    }
  }

  async run(id: string, exports: string[] = []): Promise<void> {
    this.set(id, { running: true, runError: null, lastRun: null });
    try {
      const res = (await runReport(id, { exports })) as { run?: RunRecord };
      this.set(id, { running: false, lastRun: res?.run ?? null });
      // 跑完必须重取：result_state 由「当前修订有没有跑过」决定，别在前端猜
      await this.open(id);
    } catch (e) {
      this.set(id, { running: false, runError: reasonOf(e) });
    }
  }
}

export const reports = new ReportStore();

export function useReport(id: string): ReportState {
  const snapshot = useCallback(() => reports.get(id), [id]);
  return useSyncExternalStore(reports.subscribe, snapshot, snapshot);
}

// ---------------------------------------------------------------------------
// 展示辅助（纯函数，视图共用，避免各写一套格式）
// ---------------------------------------------------------------------------

export function fmtTime(iso?: string): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function fmtDuration(start?: string, end?: string): string {
  if (!start || !end) return '—';
  const ms = new Date(end).getTime() - new Date(start).getTime();
  if (!Number.isFinite(ms) || ms < 0) return '—';
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(2)} s`;
}

/** 运行体的 `layout.page_count` / `rows` / `cols`：报告页数最常被问，单独取。 */
export function layoutOf(run?: RunRecord | null): {
  pages: number | null;
  rows: number | null;
  cols: number | null;
} {
  const layout = (run?.layout ?? {}) as Record<string, unknown>;
  const num = (v: unknown) => (typeof v === 'number' ? v : null);
  return {
    pages: num(layout.page_count),
    rows: num(layout.rows),
    cols: num(layout.cols),
  };
}

/** `result_state` → 徽章文案与样式（`library.rs:218` 的三个取值）。 */
export function stateBadge(state?: string | null): { label: string; cls: string } {
  switch (state) {
    case 'current_revision':
      return { label: '结果最新', cls: 'ran' };
    case 'older_revision':
      return { label: '结果过期', cls: 'analyzed' };
    case 'none':
      return { label: '未运行', cls: 'draft' };
    default:
      return { label: '未知', cls: 'draft' };
  }
}
