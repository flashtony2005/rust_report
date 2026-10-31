import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  artifactUrl,
  getArtifactJson,
  getReportLedger,
  getRuns,
  type RecentRun,
} from '../services/api';
import { fmtTime, reasonOf } from '../services/reports';
import { WorkbookViewer } from '../components/WorkbookViewer';
import type { WorkbookData } from '../services/workbook';

interface Props {
  /** 给了就只看这一份报表的运行（从报表详情跳过来时用） */
  reportId?: string;
  reportName?: string;
  onOpenReport: (id: string) => void;
}

type Filter = 'all' | 'failed' | 'succeeded';

/**
 * 运行记录 —— 从 master 的 `RunsView` 移植形态，数据换成台账 `rpt_run`。
 *
 * master 的 RunsView 读的是浏览器内存里的 `store.getRuns()`，运行记录随着刷新消失；
 * 这里是库里的行，刷新、重启、换机器都还在（那是它存在的意义）。
 *
 * 详情区做了两件事，都对着**真产物**：
 * - 列产物（名称 / 大小 / 直链）—— 来自 `rpt_artifact`
 * - `workbook.json` 直接渲染成表格（`WorkbookViewer` 吃引擎产物结构）
 *   失败运行没有产物，就只显示错误码与错误文案。
 */
export const RunsView: React.FC<Props> = ({ reportId, reportName, onOpenReport }) => {
  const [filter, setFilter] = useState<Filter>('all');
  const [runs, setRuns] = useState<RecentRun[]>([]);
  const [total, setTotal] = useState(0);
  const [failedTotal, setFailedTotal] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);
  const [artifacts, setArtifacts] = useState<Array<{ name: string; bytes: number | null }>>([]);
  const [workbook, setWorkbook] = useState<WorkbookData | null>(null);
  const [wbError, setWbError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await getRuns({
        status: filter === 'all' ? undefined : filter,
        report: reportId,
        limit: 200,
      });
      setRuns(res.runs.items);
      setTotal(res.runs.total);
      setFailedTotal(res.runs.failed);
      setError(null);
    } catch (e) {
      setError(reasonOf(e));
      setRuns([]);
    } finally {
      setLoading(false);
    }
  }, [filter, reportId]);

  useEffect(() => {
    void load();
  }, [load]);

  const current = useMemo(() => runs.find((r) => r.run_id === selected) ?? null, [runs, selected]);

  // 选中一条 ⇒ 拉它的产物；有 workbook.json 就直接渲染
  useEffect(() => {
    if (!current) {
      setArtifacts([]);
      setWorkbook(null);
      setWbError(null);
      return;
    }
    let alive = true;
    setArtifacts([]);
    setWorkbook(null);
    setWbError(null);
    (async () => {
      try {
        const { ledger } = await getReportLedger(current.report_id);
        const mine = ledger.artifacts.filter((a) => a.run_id === current.run_id);
        if (!alive) return;
        setArtifacts(mine.map((a) => ({ name: a.name, bytes: a.bytes })));
        if (mine.some((a) => a.name === 'workbook.json')) {
          try {
            const data = await getArtifactJson<WorkbookData>(
              current.report_id,
              current.run_id,
              'workbook.json',
            );
            if (alive) setWorkbook(data);
          } catch (e) {
            if (alive) setWbError(reasonOf(e));
          }
        }
      } catch (e) {
        if (alive) setWbError(reasonOf(e));
      }
    })();
    return () => {
      alive = false;
    };
  }, [current]);

  const fmtMs = (ms: number | null) =>
    ms == null ? '—' : ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Runs</div>
          <h1>{reportName ? `${reportName} · 运行记录` : '运行记录'}</h1>
          <div className="sub">
            台账 `rpt_run`：{total} 次运行
            {failedTotal > 0 ? ` · ${failedTotal} 次失败` : ' · 没有失败'}；
            每一行都是引擎真跑过的一次，产物可直链下载
          </div>
        </div>
        <button type="button" className="btn" onClick={() => void load()}>
          刷新
        </button>
      </div>

      <div className="row" style={{ gap: 10, margin: '0 0 16px', flexWrap: 'wrap' }}>
        <div className="chip-toggles" style={{ margin: 0 }}>
          {(
            [
              ['all', '全部'],
              ['succeeded', '成功'],
              ['failed', '失败'],
            ] as Array<[Filter, string]>
          ).map(([v, label]) => (
            <button
              key={v}
              className={`tog ${filter === v ? 'on' : ''}`}
              onClick={() => setFilter(v)}
              type="button"
            >
              {label}
            </button>
          ))}
        </div>
        {reportId ? (
          <button className="btn small" onClick={() => onOpenReport(reportId)} type="button">
            回报表详情 →
          </button>
        ) : null}
      </div>

      {error ? (
        <div className="callout-line" style={{ borderColor: 'var(--err)' }}>
          {error}
        </div>
      ) : null}

      <section className="card pad">
        <div className="tiny" style={{ marginBottom: 8 }}>
          共 {total} 条{filter === 'all' ? '' : `（${filter === 'failed' ? '失败' : '成功'}）`}
          {runs.length < total ? ` · 显示最新 ${runs.length} 条` : ''}
        </div>
        <div style={{ maxHeight: '52vh', overflow: 'auto' }}>
        <table className="tbl">
          <thead>
            <tr>
              <th style={{ width: '26%' }}>报表</th>
              <th style={{ width: '12%' }}>状态</th>
              <th style={{ width: '10%' }}>耗时</th>
              <th style={{ width: '16%' }}>产物</th>
              <th style={{ width: '22%' }}>完成时间</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {runs.map((r) => (
              <tr key={r.run_id} style={selected === r.run_id ? { background: 'var(--paper-2)' } : undefined}>
                <td>
                  <b>{r.report_name || r.report_id}</b>
                  <div className="tiny mono" title={r.run_id}>
                    修订 {(r.revision ?? '').slice(0, 8) || '—'}
                  </div>
                </td>
                <td>
                  <span className={`badge ${r.status === 'succeeded' ? 'ran' : 'failed'}`}>
                    {r.status === 'succeeded' ? '成功' : r.status === 'failed' ? '失败' : r.status}
                  </span>
                  {r.error_code ? <div className="tiny mono">{r.error_code}</div> : null}
                </td>
                <td className="tiny">{fmtMs(r.duration_ms)}</td>
                <td className="tiny">
                  {r.artifact_count} 个 / {formatBytes(r.artifact_bytes)}
                </td>
                <td className="tiny">{fmtTime(r.finished_at ?? r.started_at ?? undefined)}</td>
                <td>
                  <div className="row" style={{ gap: 6 }}>
                    <button
                      className="btn small"
                      type="button"
                      onClick={() => {
                        const next = selected === r.run_id ? null : r.run_id;
                        setSelected(next);
                        if (next) {
                          // 产物区在表格下面：60 行的时候不滚过去，用户以为「点了没反应」
                          window.setTimeout(() => {
                            document
                              .getElementById('run-artifacts')
                              ?.scrollIntoView({ behavior: 'smooth', block: 'start' });
                          }, 120);
                        }
                      }}
                    >
                      {selected === r.run_id ? '收起' : '看产物'}
                    </button>
                    <button
                      className="btn small"
                      type="button"
                      onClick={() => onOpenReport(r.report_id)}
                    >
                      报表
                    </button>
                  </div>
                </td>
              </tr>
            ))}
            {runs.length === 0 ? (
              <tr>
                <td colSpan={6} className="empty">
                  {loading ? '读取台账…' : filter === 'failed' ? '没有失败的运行' : '还没有运行记录'}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
        </div>
      </section>

      {current ? (
        <>
          <div className="section-title" id="run-artifacts">
            产物 <span className="tiny">{current.run_id.slice(-12)}</span>
          </div>
          <section className="card pad">
            {artifacts.length ? (
              <div className="list">
                {artifacts.map((a) => (
                  <div className="list-row" key={a.name}>
                    <div className="grow">
                      <div className="t mono">{a.name}</div>
                      <div className="s">{formatBytes(a.bytes)}</div>
                    </div>
                    <a
                      className="btn small"
                      href={artifactUrl(current.report_id, current.run_id, a.name)}
                      target="_blank"
                      rel="noreferrer"
                    >
                      下载 / 打开
                    </a>
                  </div>
                ))}
              </div>
            ) : (
              <div className="empty">
                这次运行没有产物
                {current.error_code ? `（${current.error_code}：${current.error_message ?? ''}）` : ''}
              </div>
            )}
          </section>

          {workbook ? (
            <>
              <div className="section-title">
                工作表 <span className="tiny">workbook.json，只读渲染</span>
              </div>
              <WorkbookViewer
                snapshot={workbook}
                title={`${current.report_name || current.report_id} · 产物`}
                height="560px"
              />
            </>
          ) : wbError ? (
            <div className="callout-line" style={{ borderColor: 'var(--warn)' }}>
              工作表读不出来：{wbError}
            </div>
          ) : null}
        </>
      ) : null}
    </>
  );
};

function formatBytes(n: number | null | undefined): string {
  if (n == null) return '—';
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
