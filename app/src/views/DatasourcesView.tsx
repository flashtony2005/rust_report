import React, { useEffect, useMemo, useState } from 'react';
import {
  ApiError,
  deleteDatasource,
  listDatasources,
  listTables,
  probeDatasource,
  probeRecord,
  saveDatasource,
  type Datasource,
  type Probe,
} from '../services/api';
import { fmtTime } from '../services/reports';

/**
 * 数据源 —— **真连库**的页面（`/datasources*`，Rust 侧 `rpx-semantic`）。
 *
 * 三件事，一件也不能假：
 * 1. 保存的响应里没有口令（服务端只回 `url_masked`）；
 * 2. 「抽样」是真的跑 `select * from 表 limit n`，列画像与主键都来自库；
 * 3. 连不上就把驱动原文摆出来，不显示"0 行"糊过去。
 *
 * 界面按「列表 → 表单 → 探测」三段纵向排：每段只说一件事，字段三列对齐，
 * 数字列右对齐等宽（`.tbl` 已有规则）。
 */

const DRIVERS: Array<{ value: string; label: string; hint: string }> = [
  { value: 'postgres', label: 'PostgreSQL / Kingbase', hint: 'host:port + 库名' },
  { value: 'mysql', label: 'MySQL', hint: 'host:port + 库名' },
  { value: 'sqlite', label: 'SQLite 文件', hint: '只填文件绝对路径' },
];

const emptyForm = (): Partial<Datasource> & { driver: string } => ({
  driver: 'sqlite',
  id: '',
  label: '',
  host: '',
  port: 0,
  database: '',
  user: '',
  password: '',
  options: '',
});

export const DatasourcesView: React.FC = () => {
  const [items, setItems] = useState<Datasource[]>([]);
  const [registry, setRegistry] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [formOpen, setFormOpen] = useState(false);
  const [form, setForm] = useState(emptyForm());
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const [selected, setSelected] = useState<string | null>(null);
  const [tables, setTables] = useState<string[]>([]);
  const [table, setTable] = useState('');
  const [sql, setSql] = useState('');
  const [probe, setProbe] = useState<Probe | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reason = (e: unknown) =>
    e instanceof ApiError ? `${e.code}：${e.message}` : e instanceof Error ? e.message : String(e);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const page = await listDatasources();
      setItems(page.items ?? []);
      setRegistry(page.registry ?? '');
    } catch (e) {
      setError(reason(e));
      setItems([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const current = useMemo(() => items.find((x) => x.id === selected) ?? null, [items, selected]);

  const resetProbe = () => {
    setProbe(null);
    setProbeError(null);
    setTables([]);
    setTable('');
    setSql('');
  };

  const openProbe = async (id: string) => {
    setSelected(id);
    resetProbe();
    // 表清单是"增强项"：拿不到就让用户自己写 SQL，不挡流程
    try {
      const res = await listTables(id);
      setTables(res.items ?? []);
      if (res.items?.length) setTable(res.items[0]);
    } catch {
      setTables([]);
    }
  };

  const runProbe = async (opts: { table?: string; sql?: string }) => {
    if (!selected) return;
    setBusy(true);
    setProbeError(null);
    try {
      const res = await probeDatasource(selected, { limit: 200, ...opts });
      setProbe(res.probe);
    } catch (e) {
      setProbe(null);
      setProbeError(reason(e));
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    setSaving(true);
    setFormError(null);
    try {
      const payload = { ...form };
      if (!payload.id?.trim()) delete payload.id;
      if (!payload.port) delete payload.port;
      const res = await saveDatasource(payload as Datasource & { driver: string });
      setFormOpen(false);
      setForm(emptyForm());
      await load();
      await openProbe(res.item.id);
    } catch (e) {
      setFormError(reason(e));
    } finally {
      setSaving(false);
    }
  };

  const testForm = async () => {
    setSaving(true);
    setFormError(null);
    try {
      const res = await probeRecord(form as Datasource & { driver: string }, { limit: 1 });
      setFormError(`连接成功 · ${res.probe.elapsed_ms} ms`);
    } catch (e) {
      setFormError(reason(e));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (id: string) => {
    if (!window.confirm(`删除数据源 ${id}？注册表里的记录会被移除（库本身不动）。`)) return;
    try {
      await deleteDatasource(id);
      if (selected === id) {
        setSelected(null);
        resetProbe();
      }
      await load();
    } catch (e) {
      setError(reason(e));
    }
  };

  const field = (
    key: keyof Datasource,
    label: string,
    props: { placeholder?: string; type?: string; span?: number; hint?: string } = {},
  ) => (
    <div style={props.span ? { gridColumn: `span ${props.span}` } : undefined}>
      <label className="field">{label}</label>
      <input
        type={props.type ?? 'text'}
        value={(form[key] as string | number | undefined) ?? ''}
        placeholder={props.placeholder}
        onChange={(e) =>
          setForm((f) => ({
            ...f,
            [key]: props.type === 'number' ? Number(e.target.value) : e.target.value,
          }))
        }
      />
      {props.hint ? <div className="tiny" style={{ marginTop: 4 }}>{props.hint}</div> : null}
    </div>
  );

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Sources</div>
          <h1>数据源</h1>
          <div className="sub">注册表在服务端；探测真连一次库 —— 类型、样本、主键都从库里读</div>
        </div>
        <button
          type="button"
          className="btn primary"
          onClick={() => {
            setForm(emptyForm());
            setFormError(null);
            setFormOpen((v) => !v);
          }}
        >
          {formOpen ? '收起表单' : '＋ 添加数据源'}
        </button>
      </div>

      {formOpen ? (
        <section className="card pad" style={{ marginBottom: 18 }}>
          <div className="eyebrow">New / Edit</div>
          <h2 className="step-h" style={{ fontSize: 17 }}>一条连接记录</h2>
          <p className="step-p" style={{ marginBottom: 14 }}>
            口令只写不读：保存后接口再也拿不回它，页面只显示打了码的连接串。
          </p>
          <div className="form-grid">
            <div>
              <label className="field">驱动</label>
              <select
                value={form.driver}
                onChange={(e) => setForm((f) => ({ ...f, driver: e.target.value, port: 0 }))}
              >
                {DRIVERS.map((d) => (
                  <option key={d.value} value={d.value}>{d.label}</option>
                ))}
              </select>
              <div className="tiny" style={{ marginTop: 4 }}>
                {DRIVERS.find((d) => d.value === form.driver)?.hint}
              </div>
            </div>
            {field('id', '标识（可空，自动生成）', { placeholder: 'demo-sqlite' })}
            {field('label', '显示名', { placeholder: '工资库（测试）' })}
            {field('database', form.driver === 'sqlite' ? '文件路径' : '库名', {
              placeholder:
                form.driver === 'sqlite'
                  ? 'F:/project/report/rpx/studio/sample/sales-demo.db'
                  : 'gongzi',
              span: form.driver === 'sqlite' ? 3 : 1,
            })}
            {form.driver === 'sqlite' ? null : (
              <>
                {field('host', '主机', { placeholder: '192.168.1.77' })}
                {field('port', '端口（0=默认）', { type: 'number', placeholder: '5432' })}
                {field('user', '用户名', { placeholder: 'system' })}
                {field('password', '口令（只写不读）', { type: 'password', placeholder: '••••••' })}
                {field('options', '额外参数', { placeholder: 'sslmode=require', span: 2 })}
              </>
            )}
          </div>

          <div className="form-actions">
            <button type="button" className="btn primary" onClick={() => void save()} disabled={saving}>
              {saving ? '处理中…' : '保存'}
            </button>
            <button type="button" className="btn" onClick={() => void testForm()} disabled={saving}>
              测试连接
            </button>
            <button type="button" className="btn ghost" onClick={() => setFormOpen(false)}>
              取消
            </button>
            {formError ? (
              <span className="tiny" style={{ color: formError.startsWith('连接成功') ? 'var(--ok)' : 'var(--err)' }}>
                {formError}
              </span>
            ) : null}
          </div>
        </section>
      ) : null}

      {error ? (
        <div className="card pad" style={{ borderColor: 'var(--err)', marginBottom: 18 }}>
          <b style={{ color: 'var(--err)' }}>读不到注册表</b>
          <div className="muted" style={{ marginTop: 6 }}>{error}</div>
          <div style={{ marginTop: 12 }}>
            <button type="button" className="btn small" onClick={() => void load()}>重试</button>
          </div>
        </div>
      ) : null}

      <div className="section-title">
        已注册 <span className="tiny">{loading ? '读取中…' : `${items.length} 个`}</span>
      </div>

      <div className="list">
        {items.length > 0 ? (
          items.map((ds) => (
            <div
              key={ds.id}
              className="list-row"
              style={{ cursor: 'default', alignItems: 'flex-start' }}
              onClick={() => void openProbe(ds.id)}
            >
              <span className={`pill ${ds.driver === 'sqlite' ? 'entity' : 'dim'}`}>{ds.driver}</span>
              <div className="grow">
                <div className="t">
                  {ds.label || ds.id}
                  <span className="tiny" style={{ marginLeft: 8 }}>{ds.id}</span>
                </div>
                <div className="s mono">{ds.url_masked || '（未配置连接串）'}</div>
                <div className="tiny" style={{ marginTop: 4 }}>
                  更新于 {fmtTime(ds.updated_at)}
                </div>
              </div>
              <button
                type="button"
                className="btn small"
                onClick={(e) => {
                  e.stopPropagation();
                  void openProbe(ds.id);
                }}
              >
                探测
              </button>
              <button
                type="button"
                className="btn small ghost"
                onClick={(e) => {
                  e.stopPropagation();
                  void remove(ds.id);
                }}
              >
                删除
              </button>
            </div>
          ))
        ) : loading ? (
          <div className="empty">读取中…</div>
        ) : (
          <div className="empty">还没有数据源 —— 点右上角「添加数据源」登记第一条</div>
        )}
      </div>

      <div className="hint" style={{ marginTop: 12, lineHeight: 1.9 }}>
        注册表：<code className="sql-inline">{registry || '（未知）'}</code>
        · 口令保存在该文件里（要连库），接口层不外发；文件已排除在版本库之外。
      </div>

      {current ? (
        <section className="card pad" style={{ marginTop: 18 }}>
          <div className="eyebrow">Probe</div>
          <h2 className="step-h" style={{ fontSize: 17 }}>
            探测 <code className="sql-inline">{current.id}</code>
          </h2>
          <p className="step-p" style={{ marginBottom: 12 }}>
            抽样 200 行：看列名、按值判类型、数 NULL、取前三个样本，并读主键。全部只读。
          </p>

          <div className="row" style={{ gap: 10, flexWrap: 'wrap' }}>
            <button
              type="button"
              className="btn small"
              onClick={() => void runProbe({})}
              disabled={busy}
              title="只跑 select 1，验证连通性"
            >
              连接测试
            </button>
            <select
              value={table}
              onChange={(e) => setTable(e.target.value)}
              style={{ flex: '0 0 220px' }}
              disabled={!tables.length || busy}
            >
              {tables.length ? (
                tables.map((t) => <option key={t} value={t}>{t}</option>)
              ) : (
                <option value="">（没取到表清单）</option>
              )}
            </select>
            <button
              type="button"
              className="btn small primary"
              onClick={() => void runProbe({ table })}
              disabled={!table || busy}
            >
              {busy ? '探测中…' : '抽样这张表'}
            </button>
            <span className="tiny">共 {tables.length} 张表</span>
          </div>

          <div className="row" style={{ gap: 10, marginTop: 10 }}>
            <input
              type="text"
              value={sql}
              onChange={(e) => setSql(e.target.value)}
              placeholder="或直接给一条 select（例：select region, count(*) from sales group by 1）"
            />
            <button
              type="button"
              className="btn small"
              onClick={() => void runProbe({ sql })}
              disabled={!sql.trim() || busy}
            >
              跑这条
            </button>
          </div>

          {probeError ? (
            <div className="note risk" style={{ marginTop: 14, color: 'var(--err)' }}>{probeError}</div>
          ) : null}

          {probe ? (
            <>
              <div className="tiny" style={{ marginTop: 14, lineHeight: 1.9 }}>
                实际执行：<code className="sql-inline">{probe.sql}</code>
                <span style={{ marginLeft: 10 }}>
                  样本 {probe.sampled} 行 · {probe.columns.length} 列 · {probe.elapsed_ms} ms
                </span>
                {probe.key.length ? (
                  <span style={{ marginLeft: 10 }}>
                    主键 <code className="sql-inline">{probe.key.join(', ')}</code>
                  </span>
                ) : (
                  <span style={{ marginLeft: 10 }}>未读到主键</span>
                )}
              </div>

              <table className="tbl compact" style={{ marginTop: 10 }}>
                <thead>
                  <tr>
                    <th>列</th>
                    <th>类型</th>
                    <th className="num">NULL</th>
                    <th className="num">去重</th>
                    <th>样本</th>
                  </tr>
                </thead>
                <tbody>
                  {probe.columns.map((c) => (
                    <tr key={c.name}>
                      <td className="mono">{c.name}</td>
                      <td>
                        <span className={`pill ${c.kind === 'number' || c.kind === 'decimal' ? 'metric' : c.kind === 'text' ? 'dim' : ''}`}>
                          {c.kind}
                        </span>
                      </td>
                      <td className="num">{c.nulls}</td>
                      <td className="num">{c.distinct}</td>
                      <td className="muted" style={{ maxWidth: 320, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                        {c.samples.join(' · ') || '—'}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              <div className="section-title" style={{ marginTop: 20 }}>
                样本 <span className="tiny">前 {Math.min(probe.rows.length, 8)} 行 / 共 {probe.sampled} 行</span>
              </div>
              <div className="preview-wrap">
                <table className="tbl dense">
                  <thead>
                    <tr>
                      {probe.columns.map((c) => (
                        <th key={c.name}>{c.name}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {probe.rows.slice(0, 8).map((row, i) => (
                      <tr key={i}>
                        {row.map((cell, j) => (
                          <td
                            key={j}
                            title={cell === null ? 'NULL' : String(cell)}
                            className={typeof cell === 'number' ? 'num' : ''}
                          >
                            {cell === null ? <span className="tiny">NULL</span> : String(cell)}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          ) : null}
        </section>
      ) : null}
    </>
  );
};
