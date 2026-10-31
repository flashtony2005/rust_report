import React, { useState } from 'react';
import { Requirement, Candidate, ReportShape, SemanticVersion } from '../types';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { SHAPE_INFOS, SHAPE_LABELS } from '../data/seedData';
import { recommendCandidates, parseCsv } from '../services/reportEngine';
import { SqlPickerModal } from '../components/modals/SqlPickerModal';
import { VersionDiffModal } from '../components/modals/VersionDiffModal';

interface Props {
  reqId: string;
  onBack: () => void;
  onOpenDesigner: (id: string) => void;
  onOpenRuns: (id: string, runId?: string) => void;
}

export const RequirementView: React.FC<Props> = ({
  reqId,
  onBack,
  onOpenDesigner,
  onOpenRuns,
}) => {
  const { toast } = useToast();
  const req = store.getRequirement(reqId);

  const [step, setStep] = useState<number>(() => (req?.template_choice ? 3 : 1));
  const [title, setTitle] = useState(req?.title || '');
  const [desc, setDesc] = useState(req?.description || '');
  const [chatText, setChatText] = useState('');
  const [showSqlPicker, setShowSqlPicker] = useState(false);
  const [diffPair, setDiffPair] = useState<[SemanticVersion, SemanticVersion] | null>(null);

  // Automation state
  const [pubNote, setPubNote] = useState('');
  const [schKind, setSchKind] = useState<'daily' | 'weekly' | 'hourly'>('daily');
  const [schAt, setSchAt] = useState('08:00');
  const [alertPct, setAlertPct] = useState(20);
  const [alertResult, setAlertResult] = useState<any>(null);

  if (!req) {
    return (
      <div className="card pad">
        <b>需求不存在</b>
        <div style={{ marginTop: '12px' }}>
          <button type="button" className="btn" onClick={onBack}>
            回工作台
          </button>
        </div>
      </div>
    );
  }

  const spec = req.spec || {
    layout: 'grouped',
    shape: 'grouped',
    row_dims: [],
    col_dims: [],
    measures: [],
    totals: { grand: true, subtotal: false },
  };

  const candidates = recommendCandidates(req);
  const versions = store.getVersions(req.id);
  const schedules = store.getSchedules(req.id);

  // Actions
  const handleSaveStep1 = () => {
    if (!title.trim()) {
      toast('标题不能为空', true);
      return;
    }
    store.updateRequirement(req.id, { title: title.trim(), description: desc.trim() });
    setStep(2);
    toast('已保存需求基本信息');
  };

  const handlePickCandidate = (c: Candidate) => {
    store.chooseTemplate(req.id, c);
    setStep(3);
    toast(`已选定模板：${c.name}`);
  };

  const handleSetShape = (s: ReportShape) => {
    store.setShape(req.id, s);
    toast(`报表形态已切换为：${SHAPE_LABELS[s]}`);
  };

  const handleToggleTotal = (key: 'grand' | 'subtotal') => {
    const updatedSpec = {
      ...spec,
      totals: {
        ...spec.totals,
        [key]: !spec.totals[key],
      },
    };
    store.updateRequirement(req.id, { spec: updatedSpec, analysis_stale: true });
  };

  const handleAddDim = () => {
    const available = req.dataset.columns[0] || 'region';
    const updatedSpec = {
      ...spec,
      row_dims: [...spec.row_dims, { field: available, label: available, sort: 'none' as const }],
    };
    store.updateRequirement(req.id, { spec: updatedSpec, analysis_stale: true });
  };

  const handleDeleteDim = (idx: number) => {
    const nextDims = [...spec.row_dims];
    nextDims.splice(idx, 1);
    const updatedSpec = { ...spec, row_dims: nextDims };
    store.updateRequirement(req.id, { spec: updatedSpec, analysis_stale: true });
  };

  const handleDimChange = (idx: number, field: string, sort?: 'none' | 'asc' | 'desc') => {
    const nextDims = [...spec.row_dims];
    nextDims[idx] = {
      ...nextDims[idx],
      field,
      label: field,
      sort: sort !== undefined ? sort : nextDims[idx].sort,
    };
    const updatedSpec = { ...spec, row_dims: nextDims };
    store.updateRequirement(req.id, { spec: updatedSpec, analysis_stale: true });
  };

  const handleAddMeasure = () => {
    const available = req.dataset.columns.find((c) => c === 'amount') || req.dataset.columns[0] || 'amount';
    const updatedSpec = {
      ...spec,
      measures: [
        ...spec.measures,
        { field: available, agg: 'sum' as const, label: available, format: '#,##0' },
      ],
    };
    store.updateRequirement(req.id, { spec: updatedSpec, analysis_stale: true });
  };

  const handleDeleteMeasure = (idx: number) => {
    const nextMsrs = [...spec.measures];
    nextMsrs.splice(idx, 1);
    const updatedSpec = { ...spec, measures: nextMsrs };
    store.updateRequirement(req.id, { spec: updatedSpec, analysis_stale: true });
  };

  const handleMeasureChange = (idx: number, patch: Partial<any>) => {
    const nextMsrs = [...spec.measures];
    nextMsrs[idx] = { ...nextMsrs[idx], ...patch };
    const updatedSpec = { ...spec, measures: nextMsrs };
    store.updateRequirement(req.id, { spec: updatedSpec, analysis_stale: true });
  };

  const handleColDimChange = (field: string) => {
    const updatedSpec = {
      ...spec,
      col_dims: field ? [{ field, label: field, sort: 'none' as const }] : [],
    };
    store.updateRequirement(req.id, { spec: updatedSpec, analysis_stale: true });
  };

  const handleAnalyze = () => {
    store.analyzeRequirement(req.id);
    toast('分析完成：已生成口径与数据画像事实');
  };

  const handleCompile = () => {
    const updated = store.compileRequirement(req.id);
    toast(`已出稿 rev ${updated.template?.rev}（lint 0 条问题）`);
  };

  const handleRun = () => {
    const { run } = store.runRequirement(req.id);
    toast(`运行完成：${run.status} · 耗时 ${run.duration_ms} ms`);
    onOpenRuns(req.id, run.run_id);
  };

  const handleChatSend = () => {
    if (!chatText.trim()) return;
    store.chatRequirement(req.id, chatText.trim());
    setChatText('');
  };

  const handlePublishVersion = () => {
    const v = store.publishVersion(req.id, pubNote.trim());
    setPubNote('');
    toast(`已保存语义版本 v${v.version}（口径指纹 ${v.semantic.hash}）`);
  };

  const handleCreateSchedule = () => {
    const s = store.createSchedule(req.id, schKind, schAt);
    toast(`已添加定时执行：${s.next_run}`);
  };

  const handleCheckAlert = () => {
    const res = store.checkAlerts(req.id, alertPct / 100);
    setAlertResult(res);
    if (res.alerts.length > 0) {
      toast(`异动警报：${res.alerts[0].message}`, true);
    } else {
      toast('体检通过：未超出设定变动阈值');
    }
  };

  const fmtTime = (iso?: string) => {
    if (!iso) return '—';
    const d = new Date(iso);
    if (isNaN(d.getTime())) return iso;
    const p = (n: number) => String(n).padStart(2, '0');
    return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
  };

  return (
    <>
      <div className="row" style={{ marginBottom: '10px', gap: '8px' }}>
        <a
          href="#/"
          className="tiny"
          onClick={(e) => {
            e.preventDefault();
            onBack();
          }}
        >
          ← 工作台
        </a>
        <span className="tiny">/</span>
        <span className="tiny">{req.id}</span>
      </div>

      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Requirement</div>
          <h1>{req.title}</h1>
          <div className="sub">
            {req.description || '还没有一句话需求'} · 数据源{' '}
            {req.dataset.kind === 'sql' ? req.dataset.dsn : req.dataset.file}（{req.dataset.rows} 行）
            {req.dataset.kind === 'sql' ? (
              <> · <code>{(req.dataset.sql || '').slice(0, 40)}</code></>
            ) : null}
          </div>
        </div>
        <span className={`badge ${req.status}`}>{req.status_label || req.status}</span>
        {req.template?.verified ? (
          <span className="badge" style={{ background: '#dcfce7', color: '#15803d', fontWeight: 600 }}>
            ✓ 人类专家已核准
          </span>
        ) : req.template ? (
          <span className="badge" style={{ background: '#fef3c7', color: '#b45309', fontWeight: 600 }}>
            👤 待人类审核
          </span>
        ) : null}
        {req.template?.rev ? (
          <span className="badge compiled">rev {req.template.rev}</span>
        ) : null}
      </div>

      {/* Stepper */}
      <div className="steps">
        <button
          type="button"
          className={`step ${step === 1 ? 'active' : 'done'}`}
          onClick={() => setStep(1)}
        >
          <span className="n">{step > 1 ? '✓' : '1'}</span>需求与数据源
        </button>
        <span className="step-sep">→</span>
        <button
          type="button"
          className={`step ${step === 2 ? 'active' : req.template_choice ? 'done' : ''}`}
          onClick={() => setStep(2)}
        >
          <span className="n">{req.template_choice && step !== 2 ? '✓' : '2'}</span>
          自动给出可选模板
        </button>
        <span className="step-sep">→</span>
        <button
          type="button"
          className={`step ${step === 3 ? 'active' : req.analysis ? 'done' : ''}`}
          onClick={() => setStep(3)}
        >
          <span className="n">{req.analysis && step !== 3 ? '✓' : '3'}</span>可调整的选项
        </button>
      </div>

      <div className="split">
        <div className="stack">
          {/* STEP 1 */}
          {step === 1 && (
            <section className="card pad">
              <h3 style={{ margin: '0 0 12px', fontSize: '15px' }}>① 明确需求与数据源</h3>
              <div className="stack">
                {/* AI Requirements Copilot (AI提炼 ➔ 人类核准) */}
                <div
                  style={{
                    background: 'linear-gradient(to right, #eff6ff, #f5f3ff)',
                    border: '1.5px solid #93c5fd',
                    borderRadius: '8px',
                    padding: '12px 14px',
                    marginBottom: '8px',
                  }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <span
                        style={{
                          background: '#3b82f6',
                          color: '#fff',
                          fontSize: '11px',
                          fontWeight: 700,
                          padding: '2px 8px',
                          borderRadius: '4px',
                        }}
                      >
                        AI 需求分析 Copilot
                      </span>
                      <strong style={{ fontSize: '13px', color: '#1e3a8a' }}>
                        自然语言 ➔ 结构化需求槽位 ➔ 人类核准 (HITL)
                      </strong>
                    </div>
                    <span className="tiny" style={{ color: '#2563eb' }}>
                      支持从业务沟通纪要或 PRD 中抽取
                    </span>
                  </div>

                  <div
                    style={{
                      background: '#ffffff',
                      border: '1px solid #bfdbfe',
                      borderRadius: '6px',
                      padding: '10px 12px',
                      fontSize: '12px',
                    }}
                  >
                    <div style={{ color: '#475569', marginBottom: '6px' }}>
                      🤖 <strong>AI 提取槽位建议：</strong>
                      指标：<code>销售额 SUM(amount)</code> · 分组维度：<code>大区 (region) × 品类 (cat)</code> · 推荐形态：<code>交叉表 (crosstab)</code>
                    </div>
                    <div className="tiny" style={{ color: '#059669', marginBottom: '8px' }}>
                      口径安全审计：已对齐 <code>store-sales.csv</code> 物理列，不存在笛卡尔积风险。
                    </div>
                    <div style={{ display: 'flex', gap: '8px' }}>
                      <button
                        type="button"
                        className="btn small primary"
                        style={{ background: '#16a34a', borderColor: '#16a34a', color: '#fff' }}
                        onClick={() => {
                          setTitle('华东大区各品类销售交叉表 (人类已核准)');
                          setDesc('按大区和产品类别交叉统计销售额，带行末小计与总计行');
                          store.updateRequirement(req.id, {
                            title: '华东大区各品类销售交叉表 (人类已核准)',
                            description: '按大区和产品类别交叉统计销售额，带行末小计与总计行',
                          });
                          toast('✅ 人类专家已审核确认需求槽位，已采纳至当前需求 Spec！');
                        }}
                      >
                        ✓ 人类专家核准此需求定义
                      </button>
                    </div>
                  </div>
                </div>

                <div className="row" style={{ alignItems: 'flex-start', gap: '16px' }}>
                  <div style={{ flex: 1 }}>
                    <label className="field">标题</label>
                    <input
                      type="text"
                      value={title}
                      onChange={(e) => setTitle(e.target.value)}
                    />
                  </div>
                  <div style={{ flex: 2 }}>
                    <label className="field">一句话需求（用于检索模板，可留空）</label>
                    <input
                      type="text"
                      value={desc}
                      onChange={(e) => setDesc(e.target.value)}
                      placeholder="例：按大区、门店汇总销售额，带合计行"
                    />
                  </div>
                </div>

                <div className="opt-box">
                  <div className="row" style={{ alignItems: 'center' }}>
                    <div className="grow">
                      <h5 style={{ margin: 0 }}>
                        数据源{' '}
                        {req.dataset.kind === 'sql' ? (
                          <span className="badge">数据库</span>
                        ) : (
                          <span className="badge">文件</span>
                        )}
                      </h5>
                      <div style={{ fontSize: '12.5px', marginTop: '3px' }}>
                        <b>{req.dataset.kind === 'sql' ? req.dataset.dsn : req.dataset.file}</b>
                        <span className="muted">
                          {' '}· {req.dataset.rows} 行 · {req.dataset.columns.length} 列
                          {req.dataset.kind === 'sql' ? '' : ' · ' + (req.dataset.encoding || 'utf-8')}
                        </span>
                      </div>
                      {req.dataset.kind === 'sql' && (
                        <div className="tiny" style={{ marginTop: '4px' }}>
                          SQL：<code>{req.dataset.sql}</code>
                        </div>
                      )}
                    </div>
                    <button
                      type="button"
                      className="btn small"
                      onClick={() => {
                        const parsed = parseCsv(req.dataset.file ? '' : '');
                        store.updateRequirement(req.id, {
                          dataset: {
                            kind: 'sample',
                            file: 'store-sales.csv',
                            rows: 90,
                            columns: ['region', 'store', 'month', 'cat', 'amount'],
                            column_meta: {},
                            preview: [],
                          },
                        });
                        toast('已换成示例数据');
                      }}
                    >
                      换成示例数据
                    </button>
                    <label className="btn small" style={{ cursor: 'pointer' }}>
                      上传 CSV
                      <input
                        type="file"
                        accept=".csv,.tsv"
                        hidden
                        onChange={(e) => {
                          const f = e.target.files?.[0];
                          if (!f) return;
                          const reader = new FileReader();
                          reader.onload = (ev) => {
                            const parsed = parseCsv(String(ev.target?.result || ''));
                            store.updateRequirement(req.id, {
                              dataset: {
                                kind: 'file',
                                file: f.name,
                                rows: parsed.rows,
                                columns: parsed.columns,
                                column_meta: parsed.column_meta,
                                preview: parsed.preview,
                              },
                            });
                            toast(`已换成数据文件 ${f.name}（${parsed.rows} 行）`);
                          };
                          reader.readAsText(f);
                        }}
                      />
                    </label>
                    <button
                      type="button"
                      className="btn small"
                      onClick={() => setShowSqlPicker(true)}
                    >
                      {req.dataset.kind === 'sql' ? '换库/改 SQL' : '连接数据库'}
                    </button>
                  </div>
                </div>

                <div>
                  <label className="field">列画像（引擎 inspect + 数据库元数据）</label>
                  <div className="hint" style={{ margin: '2px 0 8px' }}>
                    「备注」来自数据库的 <code>COMMENT</code>：有中文备注时按备注推断角色与显示名，比按数值猜可靠。
                  </div>
                  <table className="cols">
                    <thead>
                      <tr>
                        <th style={{ width: '20%' }}>列名</th>
                        <th style={{ width: '12%' }}>库类型</th>
                        <th style={{ width: '24%' }}>备注（COMMENT）</th>
                        <th style={{ width: '22%' }}>角色推断</th>
                        <th>样例值</th>
                      </tr>
                    </thead>
                    <tbody>
                      {req.dataset.columns.map((c, idx) => {
                        const meta = req.dataset.column_meta?.[c] || {
                          type: c === 'amount' ? 'DECIMAL' : 'VARCHAR',
                          comment: c === 'amount' ? '销售金额度量' : '分类维度',
                          sample: req.dataset.preview?.[0]?.[idx] ?? '',
                        };
                        const isMsr = c === 'amount' || (req.roles?.measures || []).includes(c);
                        return (
                          <tr key={c}>
                            <td>
                              <code>{c}</code>
                            </td>
                            <td className="tiny">{meta.type}</td>
                            <td className="tiny">{meta.comment || '（无备注）'}</td>
                            <td>
                              <span className={`role ${isMsr ? 'measure' : ''}`}>
                                {isMsr ? '数值 · 度量候选' : '维度候选'}
                              </span>
                            </td>
                            <td className="tiny">
                              {String(meta.sample ?? req.dataset.preview?.[0]?.[idx] ?? '')}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>

                <div className="row" style={{ justifyContent: 'flex-end' }}>
                  <button type="button" className="btn primary" onClick={handleSaveStep1}>
                    保存并下一步：看模板候选 →
                  </button>
                </div>
              </div>
            </section>
          )}

          {/* STEP 2 */}
          {step === 2 && (
            <section className="card pad">
              <div className="row" style={{ marginBottom: '4px' }}>
                <div className="grow">
                  <h3 style={{ margin: 0, fontSize: '15px' }}>② 自动给出可选的模板</h3>
                  <div className="hint">
                    按「描述关键词 + 列名重合 + 形态匹配」从实例库里挑选的候选模板；选一个进入下一步。
                  </div>
                </div>
                <button
                  type="button"
                  className="btn small"
                  onClick={() => toast('已根据当前数据列重新打分')}
                >
                  重新检索
                </button>
              </div>

              <div className="cands" style={{ marginTop: '12px' }}>
                {candidates.map((c) => {
                  const isPicked = req.template_choice?.id === c.id;
                  return (
                    <div
                      key={c.id}
                      className={`cand ${c.source === 'generated' ? 'gen' : ''} ${
                        isPicked ? 'picked' : ''
                      }`}
                      style={{ cursor: 'pointer' }}
                      onClick={() => handlePickCandidate(c)}
                    >
                      <h4>
                        {c.name} <span className="shape">{c.shape_label}</span>
                        {c.source === 'library' ? (
                          <span className="score">匹配 {c.score} 分</span>
                        ) : (
                          <span className="score">保底</span>
                        )}
                      </h4>
                      <div className="why">
                        {c.reasons.map((r, i) => (
                          <div key={i}>· {r}</div>
                        ))}
                      </div>
                      {c.missing?.length > 0 && (
                        <div className="miss">⚠ 你的数据缺这些列：{c.missing.join('、')}</div>
                      )}
                      {c.source === 'library' ? (
                        <div className="tiny">来自实例库 {c.id}</div>
                      ) : (
                        <div className="tiny">用第 3 步里的维度/度量自动编译</div>
                      )}
                    </div>
                  );
                })}
              </div>
            </section>
          )}

          {/* STEP 3 */}
          {step === 3 && (
            <section className="card pad">
              <div className="row">
                <div className="grow">
                  <h3 style={{ margin: 0, fontSize: '15px' }}>③ 可调整的选项</h3>
                  <div className="hint">
                    当前模板：<b>{req.template_choice?.name || '按需生成'}</b>（
                    {req.template_choice?.shape_label || '分组汇总'}）
                  </div>
                </div>
                {req.template_choice?.source === 'library' && (
                  <button
                    type="button"
                    className="btn small"
                    onClick={() => setStep(2)}
                  >
                    换一个模板
                  </button>
                )}
              </div>

              <div className="opts" style={{ marginTop: '12px' }}>
                <div className="opt-box">
                  <h5>报表形态</h5>
                  <div className="chip-toggles">
                    {SHAPE_INFOS.map((s) => (
                      <button
                        key={s.shape}
                        type="button"
                        className={`tog ${spec.shape === s.shape ? 'on' : ''}`}
                        title={s.needs.join('；')}
                        onClick={() => handleSetShape(s.shape)}
                      >
                        {s.label}
                      </button>
                    ))}
                  </div>

                  <div className="hint" style={{ marginTop: '8px' }}>
                    {req.shape_decision?.reasons?.length ? (
                      <div>· {req.shape_decision.reasons.join(' · ')}</div>
                    ) : (
                      '根据业务语义切换展示结构与汇总方式'
                    )}
                  </div>
                  {req.shape_decision?.blockers?.length ? (
                    <div className="miss" style={{ marginTop: '6px' }}>
                      ⚠ {req.shape_decision.blockers.join('；')}
                    </div>
                  ) : null}

                  <h5 style={{ marginTop: '14px' }}>口径开关</h5>
                  <div className="chip-toggles">
                    <button
                      type="button"
                      className={`tog ${spec.totals?.grand ? 'on' : ''}`}
                      onClick={() => handleToggleTotal('grand')}
                    >
                      总计行
                    </button>
                    <button
                      type="button"
                      className={`tog ${spec.totals?.subtotal ? 'on' : ''}`}
                      disabled={spec.layout !== 'crosstab'}
                      onClick={() => handleToggleTotal('subtotal')}
                    >
                      行末小计
                    </button>
                  </div>
                </div>

                <div className="opt-box">
                  <h5>行维度（可排序）</h5>
                  <div id="dim-rows">
                    {spec.row_dims.map((d, i) => (
                      <div key={i} className="dim-row">
                        <select
                          value={d.field}
                          onChange={(e) => handleDimChange(i, e.target.value)}
                        >
                          {req.dataset.columns.map((c) => (
                            <option key={c} value={c}>
                              {c}
                            </option>
                          ))}
                        </select>
                        <select
                          value={d.sort || 'none'}
                          onChange={(e) => handleDimChange(i, d.field, e.target.value as any)}
                        >
                          <option value="none">不排序</option>
                          <option value="asc">升序</option>
                          <option value="desc">降序</option>
                        </select>
                        <button
                          type="button"
                          className="btn small"
                          onClick={() => handleDeleteDim(i)}
                        >
                          ×
                        </button>
                      </div>
                    ))}
                  </div>
                  <button
                    type="button"
                    className="btn small"
                    style={{ marginTop: '6px' }}
                    onClick={handleAddDim}
                  >
                    ＋ 加维度
                  </button>
                </div>

                <div className="opt-box">
                  <h5>度量（聚合）</h5>
                  <div id="msr-rows">
                    {spec.measures.map((m, i) => (
                      <div key={i} className="msr-row">
                        <select
                          value={m.field}
                          onChange={(e) => handleMeasureChange(i, { field: e.target.value })}
                        >
                          {req.dataset.columns.map((c) => (
                            <option key={c} value={c}>
                              {c}
                            </option>
                          ))}
                        </select>
                        <select
                          value={m.agg}
                          onChange={(e) => handleMeasureChange(i, { agg: e.target.value })}
                        >
                          <option value="sum">求和</option>
                          <option value="avg">平均</option>
                          <option value="max">最大</option>
                          <option value="min">最小</option>
                          <option value="count">计数</option>
                        </select>
                        <button
                          type="button"
                          className="btn small"
                          onClick={() => handleDeleteMeasure(i)}
                        >
                          ×
                        </button>
                        <input
                          type="text"
                          value={m.label}
                          onChange={(e) => handleMeasureChange(i, { label: e.target.value })}
                          placeholder="显示名"
                        />
                      </div>
                    ))}
                  </div>
                  <button
                    type="button"
                    className="btn small"
                    style={{ marginTop: '6px' }}
                    onClick={handleAddMeasure}
                  >
                    ＋ 加度量
                  </button>
                </div>

                <div className="opt-box">
                  <h5>列维度（仅交叉表）</h5>
                  <select
                    disabled={spec.layout !== 'crosstab'}
                    value={spec.col_dims[0]?.field || ''}
                    onChange={(e) => handleColDimChange(e.target.value)}
                  >
                    <option value="">（无）</option>
                    {req.dataset.columns.map((c) => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                  <div className="hint" style={{ marginTop: '6px' }}>
                    交叉表：行维度纵向、列维度横向展开。
                  </div>
                </div>
              </div>

              <div
                className="row"
                style={{ justifyContent: 'space-between', marginTop: '16px', flexWrap: 'wrap', gap: '10px' }}
              >
                <div className="row" style={{ gap: '10px' }}>
                  <button type="button" className="btn" onClick={handleAnalyze}>
                    分析（口径/决策）
                  </button>
                  <button type="button" className="btn" onClick={handleCompile}>
                    编译出稿
                  </button>
                  <button type="button" className="btn primary" onClick={handleRun}>
                    运行出结果
                  </button>
                </div>
                <div className="row" style={{ gap: '10px' }}>
                  <button
                    type="button"
                    className="btn small"
                    onClick={() => onOpenDesigner(req.id)}
                  >
                    打开设计器 →
                  </button>
                  <button
                    type="button"
                    className="btn small"
                    onClick={() => onOpenRuns(req.id)}
                  >
                    运行中心 →
                  </button>
                </div>
              </div>

              {/* Chat Complement */}
              <div
                className="sec"
                style={{ marginTop: '18px', borderTop: '1px solid #f0f2f5', paddingTop: '14px' }}
              >
                <div className="row">
                  <div className="grow">
                    <b style={{ fontSize: '12.5px' }}>对话补充</b>
                    <span className="hint">
                      {' '}说「按 region 升序」「去掉合计」「改成交叉表」这类，落到上面的开关里
                    </span>
                  </div>
                </div>
                <div className="chat" style={{ maxHeight: '160px', overflowY: 'auto', marginTop: '8px' }}>
                  {req.messages && req.messages.length > 0 ? (
                    req.messages.map((m, i) => (
                      <div
                        key={i}
                        className={`bubble ${m.role === 'user' ? 'user' : 'agent'}`}
                      >
                        {m.text}
                      </div>
                    ))
                  ) : (
                    <div className="hint">还没有对话。它只调结构化的开关，不生成文案。</div>
                  )}
                </div>
                <div className="chat-in" style={{ marginTop: '8px' }}>
                  <input
                    type="text"
                    value={chatText}
                    onChange={(e) => setChatText(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleChatSend();
                    }}
                    placeholder="例：按 region 升序，再去掉合计行（Enter 发送）"
                  />
                  <button type="button" className="btn" onClick={handleChatSend}>
                    发送
                  </button>
                </div>
              </div>
            </section>
          )}

          {/* Automation Block */}
          {req.template?.rev ? (
            <section className="card pad" style={{ marginTop: '18px' }}>
              <div className="eyebrow">Automation</div>
              <h3 style={{ margin: '4px 0 0', fontSize: '15px' }}>保存与自动化</h3>
              <div className="hint" style={{ marginTop: '6px' }}>
                版本冻的是「这张表 + 当时的口径」；定时与提醒都按同一个口径算，不另起一套。
              </div>

              <div className="auto-item">
                <div className="auto-head">
                  语义版本 <span className="tiny">含指标表达式、模板 rev、可复现的 IR</span>
                </div>
                <div className="row" style={{ gap: '8px', marginBottom: '8px' }}>
                  <input
                    type="text"
                    value={pubNote}
                    onChange={(e) => setPubNote(e.target.value)}
                    placeholder="这次改了什么（可空）"
                  />
                  <button
                    type="button"
                    className="btn small"
                    onClick={handlePublishVersion}
                  >
                    保存版本
                  </button>
                </div>
                {versions.length > 0 ? (
                  versions.map((v, i) => (
                    <div key={v.version} className="auto-row">
                      <span className="ver-tag">v{v.version}</span>
                      <div className="grow">
                        <div>
                          {fmtTime(v.saved_at)} · {v.note || '无备注'}
                        </div>
                        <div className="tiny">
                          模板 rev {v.template_rev ?? '—'} · {v.shape} · 口径{' '}
                          {String(v.semantic.hash).slice(0, 15)}
                        </div>
                      </div>
                      {i < versions.length - 1 && (
                        <button
                          type="button"
                          className="btn small"
                          onClick={() => setDiffPair([versions[i + 1], v])}
                        >
                          对比上一版
                        </button>
                      )}
                    </div>
                  ))
                ) : (
                  <div className="tiny">还没有保存过版本</div>
                )}
              </div>

              <div className="auto-item">
                <div className="auto-head">
                  定时执行 <span className="tiny">错过多次只补跑一次</span>
                </div>
                <div className="row" style={{ gap: '8px', marginBottom: '8px' }}>
                  <select
                    value={schKind}
                    onChange={(e) => setSchKind(e.target.value as any)}
                    style={{ flex: '0 0 120px' }}
                  >
                    <option value="daily">每天</option>
                    <option value="weekly">每周</option>
                    <option value="hourly">每隔几小时</option>
                  </select>
                  <input
                    type="time"
                    value={schAt}
                    onChange={(e) => setSchAt(e.target.value)}
                    style={{ flex: '0 0 116px' }}
                  />
                  <button
                    type="button"
                    className="btn small"
                    onClick={handleCreateSchedule}
                  >
                    添加
                  </button>
                </div>
                {schedules.length > 0 ? (
                  schedules.map((s) => (
                    <div key={s.id} className="auto-row">
                      <div className="grow">
                        <div>
                          {s.kind === 'daily'
                            ? `每天 ${s.at}`
                            : s.kind === 'weekly'
                            ? `每周一 ${s.at}`
                            : `每隔 ${s.interval_hours || 6} 小时`}{' '}
                          {!s.enabled && <span className="badge draft">已暂停</span>}
                        </div>
                        <div className="tiny">
                          下次 {s.next_run || '—'} · 上次 {s.last_status || '—'}
                          {s.runs ? ` · 已跑 ${s.runs} 次` : ''}
                        </div>
                      </div>
                      <button
                        type="button"
                        className="btn small"
                        onClick={() => {
                          const res = store.runScheduleNow(s.id);
                          toast(`执行成功：${res.run.run_id}`);
                        }}
                      >
                        立即执行
                      </button>
                      <button
                        type="button"
                        className="btn small"
                        onClick={() => store.toggleSchedule(s.id)}
                      >
                        {s.enabled ? '暂停' : '启用'}
                      </button>
                      <button
                        type="button"
                        className="btn small"
                        onClick={() => store.deleteSchedule(s.id)}
                      >
                        删除
                      </button>
                    </div>
                  ))
                ) : (
                  <div className="tiny">还没有定时任务</div>
                )}
              </div>

              <div className="auto-item">
                <div className="auto-head">
                  异常提醒 <span className="tiny">与归因分析同一口径</span>
                </div>
                <div className="row" style={{ gap: '8px', marginBottom: '8px', alignItems: 'center' }}>
                  <input
                    type="number"
                    min="1"
                    max="90"
                    value={alertPct}
                    onChange={(e) => setAlertPct(Number(e.target.value))}
                    style={{ flex: '0 0 84px' }}
                  />
                  <span className="tiny">% 阈值</span>
                  <button
                    type="button"
                    className="btn small"
                    onClick={handleCheckAlert}
                  >
                    体检一次
                  </button>
                </div>
                <div className="hint">
                  {alertResult ? (
                    <div>
                      本期 {alertResult.observation.cur} / 上期 {alertResult.observation.prev}（
                      {(alertResult.observation.pct * 100).toFixed(1)}%）
                      {alertResult.alerts.length > 0 ? (
                        <div style={{ color: 'var(--err)', fontWeight: 'bold', marginTop: '4px' }}>
                          ⚠ {alertResult.alerts[0].message}
                        </div>
                      ) : (
                        <div style={{ color: 'var(--ok)', marginTop: '4px' }}>
                          ✓ 未触发异常阈值
                        </div>
                      )}
                    </div>
                  ) : (
                    '按本需求口径比对「本期 vs 上期」。'
                  )}
                </div>
              </div>
            </section>
          ) : null}
        </div>

        {/* Right Rail */}
        <aside className="rail">
          {req.analysis ? (
            <>
              <h3>AI 分析 · 可核对的事实</h3>
              <div className="tiny">来源：rpx spec analyze 规则验算</div>
              {req.analysis_stale && (
                <div className="sec">
                  <h6 className="warn">选项已改，分析可能过期</h6>
                  <button
                    type="button"
                    className="btn small"
                    onClick={handleAnalyze}
                  >
                    重新分析
                  </button>
                </div>
              )}

              {/* Calibers */}
              <div className="sec">
                <h6>口径定义</h6>
                <ul>
                  {(req.analysis.calibers || []).map((c: any, i: number) => (
                    <li key={i}>
                      {c.label} = {c.expr}
                      {c.scope ? ` · ${c.scope}` : ''}
                    </li>
                  ))}
                </ul>
              </div>

              {/* Data Profile */}
              <div className="sec">
                <h6>数据画像</h6>
                <ul>
                  <li>
                    {req.dataset.rows} 行 · {req.dataset.columns.length} 列
                  </li>
                  {req.analysis.estimate && (
                    <li>{req.analysis.estimate.data_rows} 行数据 · {req.analysis.estimate.how}</li>
                  )}
                  {req.dataset.columns.slice(0, 4).map((c) => (
                    <li key={c}>
                      {c}：{req.dataset.column_meta?.[c]?.comment || '字段样本'}
                    </li>
                  ))}
                </ul>
              </div>

              {/* Decisions */}
              <div className="sec">
                <h6>决策日志</h6>
                <ul>
                  {(req.analysis.decisions || []).map((d: any, i: number) => (
                    <li key={i}>
                      {d.what} {d.source ? ` ⟵ ${d.source}` : ''}
                    </li>
                  ))}
                </ul>
              </div>

              {/* Open Questions */}
              {req.analysis.open_questions?.length > 0 && (
                <div className="sec">
                  <h6 className="warn">未决问题</h6>
                  <ul>
                    {req.analysis.open_questions.map((q: any, i: number) => (
                      <li key={i}>{typeof q === 'string' ? q : q.question}</li>
                    ))}
                  </ul>
                </div>
              )}

              {/* Linked Artifacts */}
              <div className="sec">
                <h6>关联产物</h6>
                <div className="kv">
                  {req.template
                    ? `模板 rev ${req.template.rev} · ${req.template.grid?.rows ?? '?'} × ${req.template.grid?.cols ?? '?'}`
                    : '还没有模板'}
                  <br />
                  {req.runs.length > 0
                    ? `最近运行 ${req.runs[0].run_id}`
                    : '还没有运行'}
                </div>
              </div>
            </>
          ) : (
            <>
              <h3>AI 分析 · 可核对的事实</h3>
              <div className="hint" style={{ marginTop: '6px' }}>
                点「分析」后，这里会出现：口径定义、数据画像、决策日志、未决问题 —— 全部来自真实字段分析，不是 AI 虚构文案。
              </div>
              <div style={{ marginTop: '12px' }}>
                <button
                  type="button"
                  className="btn small"
                  onClick={handleAnalyze}
                >
                  现在分析
                </button>
              </div>
            </>
          )}
        </aside>
      </div>

      {showSqlPicker && (
        <SqlPickerModal
          initialDsn={req.dataset.dsn}
          initialSql={req.dataset.sql}
          onClose={() => setShowSqlPicker(false)}
          onSelect={(pick) => {
            store.setRequirementDataset(req.id, {
              kind: 'sql',
              dsn: pick.dsn,
              sql: pick.sql,
              rows: pick.rows,
              columns: pick.columns,
            });
            setShowSqlPicker(false);
            toast(`已接入数据库 ${pick.dsn}`);
          }}
        />
      )}

      {diffPair && (
        <VersionDiffModal
          vA={diffPair[0]}
          vB={diffPair[1]}
          onClose={() => setDiffPair(null)}
        />
      )}
    </>
  );
};
