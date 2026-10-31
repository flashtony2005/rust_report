import React, { useEffect, useMemo, useState } from 'react';
import {
  fmtDuration,
  fmtTime,
  layoutOf,
  library,
  stateBadge,
  useLibrary,
} from '../services/reports';

interface Props {
  onOpenReport: (id: string) => void;
}

/**
 * 报表库 = **真数据**：`GET /reports`（59 份，来自 examples/library）。
 *
 * 这一页只做三件事：找、看状态、点进去跑。所有数字都来自服务端，
 * 不在这里算任何东西（`manifest` 与 `display_run` 是服务端算好的）。
 */
export const LibraryView: React.FC<Props> = ({ onOpenReport }) => {
  const state = useLibrary();
  const [text, setText] = useState(state.query);
  const [tag, setTag] = useState(state.tag);

  useEffect(() => {
    // 首次进入才拉；之后由搜索/切标签触发（避免每次热更新都打服务端）
    if (state.status === 'loading' && state.items.length === 0 && !state.error) {
      void library.load();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const stats = useMemo(() => {
    let fresh = 0;
    let stale = 0;
    let never = 0;
    let lastAt = '';
    for (const it of state.items) {
      if (it.result_state === 'current_revision') fresh += 1;
      else if (it.result_state === 'older_revision') stale += 1;
      else never += 1;
      const at = it.display_run?.finished_at;
      if (at && at > lastAt) lastAt = at;
    }
    return { fresh, stale, never, lastAt };
  }, [state.items]);

  const tags = useMemo(() => {
    const seen = new Map<string, number>();
    for (const it of state.items) {
      for (const t of it.manifest?.tags ?? []) seen.set(t, (seen.get(t) ?? 0) + 1);
    }
    return [...seen.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12);
  }, [state.items]);

  const submit = (e?: React.FormEvent) => {
    e?.preventDefault();
    void library.load({ query: text.trim(), tag });
  };

  const pickTag = (next: string) => {
    const value = next === tag ? '' : next;
    setTag(value);
    void library.load({ query: text.trim(), tag: value });
  };

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Library</div>
          <h1>报表库</h1>
          <div className="sub">
            服务端 8090 上的实例库：点开一份就跑一次真引擎，产物（workbook / csv / xlsx）由服务端落盘
          </div>
        </div>
        <button
          type="button"
          className="btn"
          onClick={() => void library.load()}
          disabled={state.status === 'loading'}
        >
          {state.status === 'loading' ? '读取中…' : '刷新'}
        </button>
      </div>

      <div className="kpis four">
        <div className="kpi">
          <div className="lab">库内报表</div>
          <div className="num">{state.total}</div>
        </div>
        <div className="kpi">
          <div className="lab">结果最新</div>
          <div className="num">{stats.fresh}</div>
        </div>
        <div className="kpi">
          <div className="lab">结果过期</div>
          <div className="num">{stats.stale}</div>
        </div>
        <div className="kpi">
          <div className="lab">从未运行</div>
          <div className="num">{stats.never}</div>
        </div>
      </div>

      <form className="ask" onSubmit={submit}>
        <div className="ask-row">
          <input
            type="text"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="按名称 / 描述 / 标签找，例：资产负债表、交叉表、工资"
          />
          <button type="submit" className="btn primary">
            检索
          </button>
        </div>
        <div className="ask-foot">
          <span className="tiny">
            检索在服务端做（CJK 二元滑窗 + 多字段命中计数），不是前端过滤；
            {state.query ? ` 当前条件「${state.query}」` : ' 当前是全部'}
            {state.total ? ` · 命中 ${state.total} 份` : ''}
          </span>
        </div>
      </form>

      {tags.length > 0 ? (
        <div className="row" style={{ flexWrap: 'wrap', gap: 8, marginTop: 14 }}>
          {tags.map(([t, n]) => (
            <button
              key={t}
              type="button"
              className={`btn small ${tag === t ? 'primary' : ''}`}
              onClick={() => pickTag(t)}
              title={`${n} 份`}
            >
              {t} <span className="tiny">{n}</span>
            </button>
          ))}
        </div>
      ) : null}

      {state.status === 'error' ? (
        <div className="card pad" style={{ marginTop: 18 }}>
          <b>读不到报表库</b>
          <div className="muted" style={{ marginTop: 6, lineHeight: 1.8 }}>{state.error}</div>
          <div style={{ marginTop: 12 }}>
            <button type="button" className="btn" onClick={() => void library.load()}>
              重试
            </button>
          </div>
        </div>
      ) : null}

      <div className="section-title">
        报表 <span className="tiny">{state.status === 'loading' ? '读取中…' : `共 ${state.total} 份`}</span>
      </div>

      <div className="list">
        {state.items.length > 0 ? (
          state.items.map((it) => {
            const badge = stateBadge(it.result_state);
            const run = it.display_run ?? null;
            const { pages } = layoutOf(run);
            const artifacts = Object.keys(run?.artifacts ?? {});
            return (
              <div
                key={it.id}
                className="list-row"
                onClick={() => onOpenReport(it.id)}
                title={`打开 ${it.id}`}
              >
                <div className="grow">
                  <div className="t">
                    {it.manifest?.name || it.id}
                    <span className="tiny" style={{ marginLeft: 8 }}>{it.id}</span>
                  </div>
                  <div className="s">
                    {it.manifest?.description || '（无描述）'}
                  </div>
                  <div className="tiny" style={{ marginTop: 4 }}>
                    {(it.manifest?.tags ?? []).map((t) => `#${t}`).join(' ')}
                    {artifacts.length ? ` · 产物 ${artifacts.join(' / ')}` : ' · 还没有产物'}
                  </div>
                </div>
                <span className={`badge ${badge.cls}`}>{badge.label}</span>
                <div className="tiny" style={{ width: 148, textAlign: 'right', lineHeight: 1.7 }}>
                  {run?.finished_at ? (
                    <>
                      {fmtTime(run.finished_at)}
                      <br />
                      {pages != null ? `${pages} 页 · ` : ''}
                      {fmtDuration(run.started_at, run.finished_at)}
                    </>
                  ) : (
                    '还没跑过'
                  )}
                </div>
              </div>
            );
          })
        ) : state.status === 'loading' ? (
          <div className="empty">读取中…</div>
        ) : state.status === 'error' ? (
          <div className="empty">服务端没给数据（见上方的错误原因）</div>
        ) : (
          <div className="empty">没有匹配的报表 —— 换个词，或清空检索条件</div>
        )}
      </div>

      <div className="hint" style={{ marginTop: 14, lineHeight: 1.9 }}>
        最近一次运行：{stats.lastAt ? fmtTime(stats.lastAt) : '—'}。
        「结果过期」= 模板修订变了、还没重跑，点进去点「运行」即可。
      </div>
    </>
  );
};
