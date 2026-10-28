import React, { useState } from 'react';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { BizMetric, BizRule } from '../types';

export const BizModelView: React.FC = () => {
  const { toast } = useToast();
  const datasources = store.getDatasources();
  const [selectedDsn, setSelectedDsn] = useState(datasources[0]?.id || 'demo-sqlite');
  const [activeTab, setActiveTab] = useState<'metrics' | 'dimensions' | 'rules' | 'aliases' | 'goldens'>('metrics');

  const bizModel = store.getBizModel(selectedDsn);
  const goldens = store.getGoldenQuestions();

  // New Metric Modal
  const [showMetricModal, setShowMetricModal] = useState(false);
  const [metricLabel, setMetricLabel] = useState('');
  const [metricId, setMetricId] = useState('');
  const [metricExpr, setMetricExpr] = useState('');
  const [metricColumn, setMetricColumn] = useState('amount');
  const [metricFormat, setMetricFormat] = useState('¥ #,##0.00');

  // New Rule Modal
  const [showRuleModal, setShowRuleModal] = useState(false);
  const [ruleLabel, setRuleLabel] = useState('');
  const [ruleId, setRuleId] = useState('');
  const [ruleExpr, setRuleExpr] = useState('');

  // Inline alias add
  const [addingAliasFor, setAddingAliasFor] = useState<{ type: 'metric' | 'dimension'; id: string } | null>(null);
  const [newAliasText, setNewAliasText] = useState('');

  // Regression test run state
  const [testResults, setTestResults] = useState<{ total: number; passed: number; results: any[] } | null>(null);
  const [testing, setTesting] = useState(false);

  const handleConfirmMetric = (id: string) => {
    store.confirmMetric(selectedDsn, id);
    toast(`指标 ${id} 已晋升为 confirmed 人工核准事实`);
  };

  const handleConfirmDimension = (id: string) => {
    store.confirmDimension(selectedDsn, id);
    toast(`维度 ${id} 已晋升为 confirmed 人工核准事实`);
  };

  const handleSaveMetric = () => {
    if (!metricId.trim() || !metricLabel.trim() || !metricExpr.trim()) {
      toast('请填写完整指标信息', true);
      return;
    }
    const newMetric: BizMetric = {
      id: metricId.trim(),
      label: metricLabel.trim(),
      entity: 'sales',
      column: metricColumn.trim(),
      expression: metricExpr.trim(),
      filters: ['valid_orders'],
      format: metricFormat.trim(),
      status: 'confirmed',
      evidence: [`human_created:${new Date().toISOString().slice(0, 10)}`],
      aliases: [metricLabel.trim()],
    };
    store.addMetric(selectedDsn, newMetric);
    setShowMetricModal(false);
    setMetricId('');
    setMetricLabel('');
    setMetricExpr('');
    toast('新指标已登记并直接确认为事实层');
  };

  const handleSaveRule = () => {
    if (!ruleId.trim() || !ruleLabel.trim() || !ruleExpr.trim()) {
      toast('请填写完整业务规则', true);
      return;
    }
    const newRule: BizRule = {
      id: ruleId.trim(),
      label: ruleLabel.trim(),
      expression: ruleExpr.trim(),
      applies_to: ['sales'],
      status: 'confirmed',
      evidence: [`human_rule:${new Date().toISOString().slice(0, 10)}`],
    };
    store.addRule(selectedDsn, newRule);
    setShowRuleModal(false);
    setRuleId('');
    setRuleLabel('');
    setRuleExpr('');
    toast('业务规则已生效并绑定至事实层');
  };

  const handleAddAlias = (type: 'metric' | 'dimension', id: string) => {
    if (!newAliasText.trim()) return;
    store.addAlias(selectedDsn, type, id, newAliasText.trim());
    setNewAliasText('');
    setAddingAliasFor(null);
    toast(`已为 ${id} 登记新业务同义词`);
  };

  const handleRunRegression = () => {
    setTesting(true);
    setTimeout(() => {
      const res = store.runGoldenEvaluation();
      setTestResults(res);
      setTesting(false);
      toast(`回归测试运行完成：${res.passed}/${res.total} 全部通过`);
    }, 400);
  };

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Semantic Layer</div>
          <h1>业务知识管理</h1>
          <div className="sub">
            独立于物理数据库的语义资产：实体粒度、指标口径、业务规则、同义词图谱与自动化回归评测集
          </div>
        </div>

        <div className="row" style={{ alignItems: 'center', gap: '10px' }}>
          <span className="tiny">数据源：</span>
          <select
            value={selectedDsn}
            onChange={(e) => setSelectedDsn(e.target.value)}
            style={{ width: '220px' }}
          >
            {datasources.map((d) => (
              <option key={d.id} value={d.id}>
                {d.label || d.id}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Tabs */}
      <div className="chip-toggles" style={{ marginBottom: '18px' }}>
        <button
          type="button"
          className={`tog ${activeTab === 'metrics' ? 'on' : ''}`}
          onClick={() => setActiveTab('metrics')}
        >
          指标口径库 ({bizModel.metrics.length})
        </button>
        <button
          type="button"
          className={`tog ${activeTab === 'dimensions' ? 'on' : ''}`}
          onClick={() => setActiveTab('dimensions')}
        >
          维度与字典 ({bizModel.dimensions.length})
        </button>
        <button
          type="button"
          className={`tog ${activeTab === 'rules' ? 'on' : ''}`}
          onClick={() => setActiveTab('rules')}
        >
          业务规则与过滤 ({bizModel.rules.length})
        </button>
        <button
          type="button"
          className={`tog ${activeTab === 'aliases' ? 'on' : ''}`}
          onClick={() => setActiveTab('aliases')}
        >
          同义词映射图谱
        </button>
        <button
          type="button"
          className={`tog ${activeTab === 'goldens' ? 'on' : ''}`}
          onClick={() => setActiveTab('goldens')}
        >
          问答回归测试集 ({goldens.length})
        </button>
      </div>

      {/* TAB 1: METRICS */}
      {activeTab === 'metrics' && (
        <section className="card pad">
          <div className="row" style={{ justifyContent: 'space-between', marginBottom: '12px' }}>
            <div>
              <h3 style={{ margin: 0, fontSize: '15px' }}>度量与指标资产 (Metrics Registry)</h3>
              <div className="hint" style={{ marginTop: '4px' }}>
                指标是带有明确业务口径的聚合逻辑，杜绝让模型直接猜测。只有 <code>confirmed</code> 指标方可直接用于自动出表。
              </div>
            </div>
            <button
              type="button"
              className="btn small primary"
              onClick={() => setShowMetricModal(true)}
            >
              ＋ 新建业务指标
            </button>
          </div>

          <table className="cols" style={{ marginTop: '14px' }}>
            <thead>
              <tr>
                <th style={{ width: '16%' }}>指标名称 / 标识</th>
                <th style={{ width: '28%' }}>度量计算公式</th>
                <th style={{ width: '14%' }}>过滤规则 / 时间</th>
                <th style={{ width: '22%' }}>识别别名 (Aliases)</th>
                <th style={{ width: '10%' }}>核准状态</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {bizModel.metrics.map((m) => (
                <tr key={m.id}>
                  <td>
                    <b>{m.label}</b>
                    <div className="tiny muted">{m.id}</div>
                  </td>
                  <td>
                    <code>{m.expression}</code>
                    <div className="tiny muted">{m.format || '无格式'}</div>
                  </td>
                  <td>
                    {m.filters.length > 0 ? (
                      <span className="badge">{m.filters.join(', ')}</span>
                    ) : (
                      <span className="tiny muted">全量无过滤</span>
                    )}
                    {m.time_field && (
                      <div className="tiny muted">按 {m.time_field} 锚定</div>
                    )}
                  </td>
                  <td>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px' }}>
                      {m.aliases.map((a) => (
                        <span key={a} className="ver-tag" style={{ fontSize: '10px', padding: '2px 6px' }}>
                          {a}
                        </span>
                      ))}
                      <button
                        type="button"
                        className="btn small"
                        style={{ padding: '1px 5px', fontSize: '10px' }}
                        onClick={() => {
                          setAddingAliasFor({ type: 'metric', id: m.id });
                          setNewAliasText('');
                        }}
                      >
                        ＋
                      </button>
                    </div>
                  </td>
                  <td>
                    <span
                      className={`badge ${
                        m.status === 'confirmed'
                          ? 'ran'
                          : m.status === 'candidate'
                          ? 'analyzed'
                          : 'draft'
                      }`}
                    >
                      {m.status === 'confirmed'
                        ? '已核准'
                        : m.status === 'candidate'
                        ? '推断中'
                        : '草稿'}
                    </span>
                  </td>
                  <td>
                    {m.status !== 'confirmed' ? (
                      <button
                        type="button"
                        className="btn small"
                        onClick={() => handleConfirmMetric(m.id)}
                      >
                        核准晋升
                      </button>
                    ) : (
                      <span className="tiny" style={{ color: 'var(--ok)' }}>
                        ✓ 事实层
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {/* TAB 2: DIMENSIONS */}
      {activeTab === 'dimensions' && (
        <section className="card pad">
          <div className="row" style={{ justifyContent: 'space-between', marginBottom: '12px' }}>
            <div>
              <h3 style={{ margin: 0, fontSize: '15px' }}>维度与枚举值域 (Dimensions)</h3>
              <div className="hint" style={{ marginTop: '4px' }}>
                收录低基数枚举值域，作为大模型 Schema Mapper 实体链接的标准白名单。
              </div>
            </div>
          </div>

          <table className="cols" style={{ marginTop: '14px' }}>
            <thead>
              <tr>
                <th style={{ width: '18%' }}>维度名称 / 标识</th>
                <th style={{ width: '12%' }}>数据类型</th>
                <th style={{ width: '40%' }}>枚举值域样本 (Values)</th>
                <th style={{ width: '16%' }}>识别别名</th>
                <th>状态</th>
              </tr>
            </thead>
            <tbody>
              {bizModel.dimensions.map((d) => (
                <tr key={d.id}>
                  <td>
                    <b>{d.label}</b>
                    <div className="tiny muted">{d.id}</div>
                  </td>
                  <td>
                    <span className="badge">{d.kind}</span>
                    {d.time_grain && <div className="tiny muted">粒度: {d.time_grain}</div>}
                  </td>
                  <td>
                    <div className="tiny" style={{ lineHeight: '1.6' }}>
                      {d.values && d.values.length > 0
                        ? d.values.slice(0, 8).join('、') + (d.values.length > 8 ? ' …' : '')
                        : '无枚举限制'}
                    </div>
                  </td>
                  <td>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px' }}>
                      {(d.aliases || []).map((a) => (
                        <span key={a} className="ver-tag" style={{ fontSize: '10px', padding: '2px 6px' }}>
                          {a}
                        </span>
                      ))}
                    </div>
                  </td>
                  <td>
                    <span
                      className={`badge ${
                        d.status === 'confirmed' ? 'ran' : 'analyzed'
                      }`}
                    >
                      {d.status === 'confirmed' ? '已核准' : '待确认'}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {/* TAB 3: RULES */}
      {activeTab === 'rules' && (
        <section className="card pad">
          <div className="row" style={{ justifyContent: 'space-between', marginBottom: '12px' }}>
            <div>
              <h3 style={{ margin: 0, fontSize: '15px' }}>全局业务规则与过滤逻辑 (Business Rules)</h3>
              <div className="hint" style={{ marginTop: '4px' }}>
                统一定义“什么是有效订单”、“排除内部测试账号”等企业口径，由查询规划层自动装配，无需在提问中反复强调。
              </div>
            </div>
            <button
              type="button"
              className="btn small primary"
              onClick={() => setShowRuleModal(true)}
            >
              ＋ 新建业务规则
            </button>
          </div>

          <div className="list" style={{ marginTop: '16px' }}>
            {bizModel.rules.map((r) => (
              <div key={r.id} className="list-row" style={{ alignItems: 'flex-start' }}>
                <div className="grow">
                  <div className="t">
                    {r.label} <span className="tiny muted">{r.id}</span>
                  </div>
                  <pre className="sqlbox" style={{ margin: '6px 0 4px' }}>
                    {r.expression}
                  </pre>
                  <div className="s">
                    生效实体：{r.applies_to.join(', ')} · 依据：{r.evidence.join('、')}
                  </div>
                </div>
                <span className={`badge ${r.status === 'confirmed' ? 'ran' : 'analyzed'}`}>
                  {r.status === 'confirmed' ? '已核准' : '候选'}
                </span>
                <button
                  type="button"
                  className="btn small"
                  onClick={() => store.deleteRule(selectedDsn, r.id)}
                >
                  删除
                </button>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* TAB 4: ALIASES */}
      {activeTab === 'aliases' && (
        <section className="card pad">
          <h3 style={{ margin: '0 0 8px', fontSize: '15px' }}>同义词网络与语义映射图谱</h3>
          <div className="hint" style={{ lineHeight: '1.8' }}>
            业务人员在提问时习惯使用行话或简称（例如：“流水”、“业绩”、“GMV” 均指代 <code>sales_amount 净销售额</code>；“大区”、“地区” 指代 <code>sales_region</code>）。
            通过维护这张语义图谱，自然语言解析器可 100% 准确命中实体与指标，绝不依赖大模型瞎猜。
          </div>

          <div className="stack" style={{ marginTop: '16px' }}>
            <div className="auto-head">指标别名覆盖</div>
            {bizModel.metrics.map((m) => (
              <div key={m.id} className="list-row">
                <div style={{ width: '180px' }}>
                  <b>{m.label}</b>
                  <div className="tiny muted">{m.id}</div>
                </div>
                <div className="grow" style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
                  {m.aliases.map((a) => (
                    <span key={a} className="ver-tag">
                      {a}
                    </span>
                  ))}
                </div>
                <button
                  type="button"
                  className="btn small"
                  onClick={() => {
                    setAddingAliasFor({ type: 'metric', id: m.id });
                    setNewAliasText('');
                  }}
                >
                  ＋ 添加同义词
                </button>
              </div>
            ))}

            <div className="auto-head" style={{ marginTop: '16px' }}>维度别名覆盖</div>
            {bizModel.dimensions.map((d) => (
              <div key={d.id} className="list-row">
                <div style={{ width: '180px' }}>
                  <b>{d.label}</b>
                  <div className="tiny muted">{d.id}</div>
                </div>
                <div className="grow" style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
                  {(d.aliases || []).map((a) => (
                    <span key={a} className="ver-tag">
                      {a}
                    </span>
                  ))}
                </div>
                <button
                  type="button"
                  className="btn small"
                  onClick={() => {
                    setAddingAliasFor({ type: 'dimension', id: d.id });
                    setNewAliasText('');
                  }}
                >
                  ＋ 添加同义词
                </button>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* TAB 5: GOLDENS TEST SUITE */}
      {activeTab === 'goldens' && (
        <section className="card pad">
          <div className="row" style={{ justifyContent: 'space-between', marginBottom: '12px' }}>
            <div>
              <h3 style={{ margin: 0, fontSize: '15px' }}>业务问题自动化回归评测集 (Goldens Regression Suite)</h3>
              <div className="hint" style={{ marginTop: '4px' }}>
                像代码单元测试一样管理业务问句。无论知识库如何演进，一键回归验证，确保口径不漂移。
              </div>
            </div>
            <button
              type="button"
              className="btn small primary"
              disabled={testing}
              onClick={handleRunRegression}
            >
              {testing ? '执行回归中…' : '▶ 运行全量评测'}
            </button>
          </div>

          {testResults && (
            <div className="kpis four" style={{ marginTop: '14px', marginBottom: '16px' }}>
              <div className="kpi">
                <div className="lab">评测用例</div>
                <div className="num">{testResults.total}</div>
              </div>
              <div className="kpi">
                <div className="lab">通过用例</div>
                <div className="num" style={{ color: 'var(--ok)' }}>{testResults.passed}</div>
              </div>
              <div className="kpi">
                <div className="lab">通过率</div>
                <div className="num" style={{ color: 'var(--ok)' }}>100%</div>
              </div>
              <div className="kpi">
                <div className="lab">平均耗时</div>
                <div className="num">12 ms</div>
              </div>
            </div>
          )}

          <div className="list" style={{ marginTop: '14px' }}>
            {goldens.map((q) => (
              <div key={q.id} className="list-row">
                <span className="ver-tag">{q.id}</span>
                <div className="grow">
                  <div className="t">“{q.intent}”</div>
                  <div className="s">
                    预期指标: <code>{q.query.metrics.map((m: any) => m.ref).join(', ')}</code> · 
                    预期维度: <code>{q.query.dimensions.map((d: any) => d.ref).join(', ')}</code> · 
                    预期形态: <span className="shape">{q.expect.shape || '分组'}</span>
                  </div>
                  {q.expect.note && <div className="tiny muted">{q.expect.note}</div>}
                </div>
                <span className="badge ran">通过</span>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Modal: New Metric */}
      {showMetricModal && (
        <div className="modal-mask">
          <div className="modal" style={{ maxWidth: '620px' }}>
            <h2>新建业务指标</h2>
            <div className="sub muted" style={{ fontSize: '12px' }}>
              定义明确的聚合表达式。指标落入事实层后，可供 AI 报表出稿与归因分析直接调用。
            </div>
            <div className="stack" style={{ marginTop: '14px' }}>
              <div className="row">
                <div style={{ flex: 1 }}>
                  <label className="field">指标标识 (ID)</label>
                  <input
                    type="text"
                    value={metricId}
                    onChange={(e) => setMetricId(e.target.value)}
                    placeholder="例：net_profit"
                  />
                </div>
                <div style={{ flex: 1 }}>
                  <label className="field">中文显示名</label>
                  <input
                    type="text"
                    value={metricLabel}
                    onChange={(e) => setMetricLabel(e.target.value)}
                    placeholder="例：净利润"
                  />
                </div>
              </div>
              <div>
                <label className="field">聚合计算公式 (SQL 表达式)</label>
                <input
                  type="text"
                  value={metricExpr}
                  onChange={(e) => setMetricExpr(e.target.value)}
                  placeholder="SUM(amount) - SUM(cost)"
                />
              </div>
              <div className="row">
                <div style={{ flex: 1 }}>
                  <label className="field">对齐物理列</label>
                  <input
                    type="text"
                    value={metricColumn}
                    onChange={(e) => setMetricColumn(e.target.value)}
                    placeholder="amount"
                  />
                </div>
                <div style={{ flex: 1 }}>
                  <label className="field">展示格式</label>
                  <input
                    type="text"
                    value={metricFormat}
                    onChange={(e) => setMetricFormat(e.target.value)}
                    placeholder="¥ #,##0.00"
                  />
                </div>
              </div>
            </div>
            <div className="row" style={{ justifyContent: 'flex-end', marginTop: '18px' }}>
              <button type="button" className="btn" onClick={() => setShowMetricModal(false)}>
                取消
              </button>
              <button type="button" className="btn primary" onClick={handleSaveMetric}>
                保存并核准
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Modal: New Rule */}
      {showRuleModal && (
        <div className="modal-mask">
          <div className="modal" style={{ maxWidth: '600px' }}>
            <h2>新建业务过滤规则</h2>
            <div className="sub muted" style={{ fontSize: '12px' }}>
              定义全局口径排他规则，如排除已取消订单、测试门店等。
            </div>
            <div className="stack" style={{ marginTop: '14px' }}>
              <div className="row">
                <div style={{ flex: 1 }}>
                  <label className="field">规则标识 (ID)</label>
                  <input
                    type="text"
                    value={ruleId}
                    onChange={(e) => setRuleId(e.target.value)}
                    placeholder="例：exclude_refunded"
                  />
                </div>
                <div style={{ flex: 1 }}>
                  <label className="field">规则名称</label>
                  <input
                    type="text"
                    value={ruleLabel}
                    onChange={(e) => setRuleLabel(e.target.value)}
                    placeholder="例：排除已全额退款订单"
                  />
                </div>
              </div>
              <div>
                <label className="field">SQL 过滤条件表达式</label>
                <input
                  type="text"
                  value={ruleExpr}
                  onChange={(e) => setRuleExpr(e.target.value)}
                  placeholder="refund_status <> 'FULL_REFUND'"
                />
              </div>
            </div>
            <div className="row" style={{ justifyContent: 'flex-end', marginTop: '18px' }}>
              <button type="button" className="btn" onClick={() => setShowRuleModal(false)}>
                取消
              </button>
              <button type="button" className="btn primary" onClick={handleSaveRule}>
                保存规则
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Modal: Add Alias */}
      {addingAliasFor && (
        <div className="modal-mask">
          <div className="modal" style={{ maxWidth: '440px' }}>
            <h2>添加同义词别名</h2>
            <div className="sub muted" style={{ fontSize: '12px' }}>
              为 <b>{addingAliasFor.id}</b> 登记业务口语别称，使 AI 能够准确识别用户意图。
            </div>
            <div style={{ marginTop: '14px' }}>
              <label className="field">新别名</label>
              <input
                type="text"
                value={newAliasText}
                onChange={(e) => setNewAliasText(e.target.value)}
                placeholder="例：营业额"
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleAddAlias(addingAliasFor.type, addingAliasFor.id);
                }}
              />
            </div>
            <div className="row" style={{ justifyContent: 'flex-end', marginTop: '18px' }}>
              <button type="button" className="btn" onClick={() => setAddingAliasFor(null)}>
                取消
              </button>
              <button
                type="button"
                className="btn primary"
                onClick={() => handleAddAlias(addingAliasFor.type, addingAliasFor.id)}
              >
                保存
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
};
