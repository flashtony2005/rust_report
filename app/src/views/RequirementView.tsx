import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ApiError,
  createRequirement,
  getBizModel,
  listDatasources,
  listShapes,
  type BizModel,
  type Datasource,
  type RequirementReply,
  type ShapeInfo,
} from '../services/api';
import {
  triageRequirement,
  type TriageResult,
  type QuickFixOption,
} from '../services/triage';

interface Props {
  onOpenReport: (id: string) => void;
}

const PRESET_QUERIES = [
  {
    tag: '🟢 经典全绿',
    title: '按客户等级看 LTV 和逾期率',
    text: '按客户等级看 LTV 和逾期率',
    desc: 'L1意图清晰 + L2词表覆盖 + L3物理表已建模',
  },
  {
    tag: '🟢 区域排名',
    title: '各大区销售额排名 Top 5',
    text: '各区域销售额排名 Top 5',
    desc: '形态推断为 Rank，槽位与指标均齐备',
  },
  {
    tag: '🟡 待补时间窗',
    title: '月度销售额走势',
    text: '销售额走势',
    desc: '形态推断为 Trend，但缺少具体时间粒度与范围',
  },
  {
    tag: '🟡 歧义待消',
    title: '各客户销售',
    text: '各客户销售',
    desc: '「销售」存在金额 vs 笔数歧义，客户存在明细 vs 分组歧义',
  },
  {
    tag: '🟡 待确认候选列',
    title: '各渠道应收账款明细',
    text: '各渠道应收账款明细',
    desc: 'L3 探查到 payment_method 候选列，但未确立正式语义标签',
  },
  {
    tag: '🔴 域外拒绝',
    title: '员工考勤打卡与离职率统计',
    text: '查询员工考勤打卡与离职率统计',
    desc: '物理库完全无 HR 表，词表外未知词，坚决拒绝不假装懂',
  },
];

export const RequirementView: React.FC<Props> = ({ onOpenReport }) => {
  const [sources, setSources] = useState<Datasource[]>([]);
  const [dsn, setDsn] = useState('');
  const [model, setModel] = useState<BizModel | null>(null);
  const [shapes, setShapes] = useState<ShapeInfo[]>([]);

  const [text, setText] = useState('按客户等级看 LTV 和逾期率');
  const [title, setTitle] = useState('');
  const [pickedMetrics, setPickedMetrics] = useState<string[]>([]);
  const [pickedDims, setPickedDims] = useState<string[]>([]);
  const [shape, setShape] = useState('');
  const [limit, setLimit] = useState('');
  const [runNow, setRunNow] = useState(true);

  const [activeTab, setActiveTab] = useState<'triage' | 'pipeline' | 'layers'>('triage');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<RequirementReply | null>(null);
  const [showSpec, setShowSpec] = useState(false);
  const [stepSimulation, setStepSimulation] = useState<number>(4);

  const errText = (e: unknown) =>
    e instanceof ApiError ? `${e.code}：${e.message}` : e instanceof Error ? e.message : String(e);

  useEffect(() => {
    let alive = true;
    Promise.all([listDatasources(), listShapes().catch(() => null)])
      .then(([ds, sh]) => {
        if (!alive) return;
        setSources(ds.items ?? []);
        if ((ds.items ?? []).length > 0) setDsn(ds.items[0].id);
        if (sh?.ok) setShapes(sh.items);
      })
      .catch((e) => alive && setError(errText(e)));
    return () => {
      alive = false;
    };
  }, []);

  const loadModel = useCallback(async (target: string) => {
    if (!target) return;
    try {
      const res = await getBizModel(target);
      setModel(res.model);
      setPickedMetrics([]);
      setPickedDims([]);
    } catch {
      setModel(null);
    }
  }, []);

  useEffect(() => {
    void loadModel(dsn);
  }, [dsn, loadModel]);

  const toggle = (list: string[], id: string) =>
    list.includes(id) ? list.filter((x) => x !== id) : [...list, id];

  // 运行实时确定性分诊
  const triage: TriageResult = useMemo(() => {
    return triageRequirement(text, model, dsn, pickedMetrics, pickedDims, shape);
  }, [text, model, dsn, pickedMetrics, pickedDims, shape]);

  // 应用快速修补 (Quick Fix)
  const applyQuickFix = (fix: QuickFixOption) => {
    if (fix.action === 'set_metric') {
      if (!pickedMetrics.includes(fix.value)) {
        setPickedMetrics([...pickedMetrics, fix.value]);
      }
    } else if (fix.action === 'set_dim') {
      if (!pickedDims.includes(fix.value)) {
        setPickedDims([...pickedDims, fix.value]);
      }
    } else if (fix.action === 'set_time') {
      if (!pickedDims.includes(fix.value)) {
        setPickedDims([...pickedDims, fix.value]);
      }
      if (!shape) setShape('trend');
    } else if (fix.action === 'set_shape') {
      setShape(fix.value);
    }
  };

  const submit = async () => {
    if (!dsn) {
      setError('先选一个数据源');
      return;
    }
    if (triage.branch === 'reject') {
      setError('此需求已被可行性分诊拦截：关键槽在词表外或物理库缺数据，系统永不假装懂。');
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const reply = await createRequirement({
        dsn,
        text: text.trim() || undefined,
        title: title.trim() || undefined,
        metrics: pickedMetrics.length ? pickedMetrics : undefined,
        dimensions: pickedDims.length ? pickedDims : undefined,
        shape: shape || undefined,
        limit: limit.trim() ? Number(limit) : undefined,
        run: runNow,
      });
      setResult(reply);
    } catch (e) {
      setError(errText(e));
    } finally {
      setBusy(false);
    }
  };

  const shapeOptions = useMemo(
    () => [{ shape: '', label: '自动判定（按证据）' }, ...shapes.map((s) => ({ shape: s.shape, label: s.label }))],
    [shapes],
  );

  const decision = result?.decision;
  const runFailed = result?.run && !result.run.ok;

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Feasibility Triage & Pipeline</div>
          <h1>需求 · 一句话生成报表（可行性分诊）</h1>
          <div className="sub">
            先分诊后进流水线 · 错误成本不对称，域内跑、域外明说，系统永不假装懂
          </div>
        </div>
        <div className="row" style={{ gap: 8 }}>
          <button
            className={`btn small ${activeTab === 'triage' ? 'primary' : ''}`}
            onClick={() => setActiveTab('triage')}
          >
            可行性分诊 (三态)
          </button>
          <button
            className={`btn small ${activeTab === 'pipeline' ? 'primary' : ''}`}
            onClick={() => setActiveTab('pipeline')}
          >
            四步探索流水线
          </button>
          <button
            className={`btn small ${activeTab === 'layers' ? 'primary' : ''}`}
            onClick={() => setActiveTab('layers')}
          >
            三层联合判定矩阵
          </button>
        </div>
      </div>

      {/* 预设探索问句（直观呈现 🟢 🟡 🔴 三种判定分支） */}
      <section className="card pad" style={{ marginBottom: 16 }}>
        <div className="field">典型问句预设（体验三态分诊流转）</div>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginTop: 8 }}>
          {PRESET_QUERIES.map((p, idx) => (
            <button
              key={idx}
              type="button"
              className={`pill ${text === p.text ? 'ok' : ''}`}
              style={{
                cursor: 'pointer',
                background: text === p.text ? '#2b4657' : '#fffcf7',
                color: text === p.text ? '#fff' : '#191512',
                border: '1.5px solid #d3c7b5',
                padding: '6px 12px',
                textAlign: 'left',
              }}
              onClick={() => {
                setText(p.text);
                setTitle(p.title);
                setError(null);
                setResult(null);
              }}
              title={p.desc}
            >
              <span style={{ fontWeight: 700, marginRight: 6 }}>{p.tag}</span>
              {p.title}
            </button>
          ))}
        </div>
      </section>

      {/* 1. 可行性分诊卡片 (Feasibility Triage Card) */}
      <div
        style={{
          width: '100%',
          boxSizing: 'border-box',
          background: '#f5f1ea',
          color: '#191512',
          fontFamily: "'Microsoft YaHei',system-ui,sans-serif",
          fontSize: 14,
          lineHeight: 1.6,
          padding: '18px 16px',
          borderRadius: 12,
          marginBottom: 16,
          border: '1px solid #d3c7b5',
        }}
      >
        {/* 输入与分诊提示 */}
        <div
          style={{
            background: '#fffcf7',
            border: '1.5px solid #191512',
            borderRadius: 10,
            padding: '10px 14px',
            textAlign: 'center',
            marginBottom: 10,
            fontWeight: 700,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <span>一句话（NL）—— 不直接进流水线，先分诊</span>
          <span
            style={{
              fontSize: 12,
              padding: '2px 8px',
              borderRadius: 6,
              background:
                triage.branch === 'ready'
                  ? '#e3f2ec'
                  : triage.branch === 'clarify'
                  ? '#faf0d4'
                  : '#fbe9e7',
              color:
                triage.branch === 'ready'
                  ? '#157055'
                  : triage.branch === 'clarify'
                  ? '#946200'
                  : '#b42318',
              border: `1px solid ${
                triage.branch === 'ready'
                  ? '#157055'
                  : triage.branch === 'clarify'
                  ? '#946200'
                  : '#b42318'
              }`,
            }}
          >
            当前实时判定：{triage.branchTitle}
          </span>
        </div>

        {/* 分诊器与确定性五条规则 */}
        <div
          style={{
            border: '1.5px solid #2b4657',
            background: '#eef3f6',
            borderRadius: 12,
            padding: '12px 14px',
            marginBottom: 10,
          }}
        >
          <div style={{ fontWeight: 700, color: '#2b4657', marginBottom: 6 }}>
            可行性分诊（确定性规则 + 三层联合判定，复用现有能力）
          </div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, fontSize: 13 }}>
            <span
              style={{
                background: triage.layers.l2.pass ? '#e3f2ec' : '#fffcf7',
                border: `1px solid ${triage.layers.l2.pass ? '#157055' : '#d3c7b5'}`,
                borderRadius: 8,
                padding: '4px 10px',
                color: triage.layers.l2.pass ? '#157055' : '#191512',
              }}
            >
              ① 词表覆盖（words.rs / bizmodel alias）{triage.layers.l2.pass ? ' ✓' : ''}
            </span>
            <span
              style={{
                background: triage.missingSlots.length === 0 ? '#e3f2ec' : '#fffcf7',
                border: `1px solid ${triage.missingSlots.length === 0 ? '#157055' : '#d3c7b5'}`,
                borderRadius: 8,
                padding: '4px 10px',
                color: triage.missingSlots.length === 0 ? '#157055' : '#191512',
              }}
            >
              ② 槽位完整（指标必填，维度/时间可有默认）{triage.missingSlots.length === 0 ? ' ✓' : ''}
            </span>
            <span
              style={{
                background: triage.ambiguities.length === 0 ? '#e3f2ec' : '#fffcf7',
                border: `1px solid ${triage.ambiguities.length === 0 ? '#157055' : '#d3c7b5'}`,
                borderRadius: 8,
                padding: '4px 10px',
                color: triage.ambiguities.length === 0 ? '#157055' : '#191512',
              }}
            >
              ③ 歧义检测（一词多命中）{triage.ambiguities.length === 0 ? ' ✓' : ''}
            </span>
            <span
              style={{
                background: triage.shapeEvidence.confidence >= 0.7 ? '#e3f2ec' : '#fffcf7',
                border: `1px solid ${triage.shapeEvidence.confidence >= 0.7 ? '#157055' : '#d3c7b5'}`,
                borderRadius: 8,
                padding: '4px 10px',
                color: triage.shapeEvidence.confidence >= 0.7 ? '#157055' : '#191512',
              }}
            >
              ④ 形态证据（{triage.shapeEvidence.label}，置信度 {Math.round(triage.shapeEvidence.confidence * 100)}%）
            </span>
            <span
              style={{
                background: triage.layers.l3.pass ? '#e3f2ec' : '#fffcf7',
                border: `1px solid ${triage.layers.l3.pass ? '#157055' : '#d3c7b5'}`,
                borderRadius: 8,
                padding: '4px 10px',
                color: triage.layers.l3.pass ? '#157055' : '#191512',
              }}
            >
              ⑤ L3 数据探查（物理表与字段对齐）{triage.layers.l3.pass ? ' ✓' : ''}
            </span>
          </div>
        </div>

        {/* 三支状态卡片 */}
        <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
          {/* 🟢 可以做到 */}
          <div
            style={{
              flex: '1 1 230px',
              background: '#e3f2ec',
              border: `2px solid ${triage.branch === 'ready' ? '#157055' : '#88c3af'}`,
              borderRadius: 12,
              padding: '10px 12px',
              boxShadow: triage.branch === 'ready' ? '0 0 0 3px rgba(21,112,85,0.2)' : 'none',
              opacity: triage.branch === 'ready' ? 1 : 0.8,
            }}
          >
            <div style={{ fontWeight: 700, color: '#157055', marginBottom: 4, display: 'flex', justifyContent: 'space-between' }}>
              <span>🟢 可以做到</span>
              {triage.branch === 'ready' && <span style={{ fontSize: 11, background: '#157055', color: '#fff', padding: '1px 6px', borderRadius: 4 }}>当前命中</span>}
            </div>
            <div style={{ fontSize: 13, color: '#2c4a3f' }}>指标命中、槽位齐或有默认、无歧义、证据足够</div>
            <div style={{ marginTop: 8, fontSize: 13, background: '#fffcf7', borderRadius: 8, padding: '6px 8px', color: '#4c4239' }}>
              直接生成：spec → run → 真报表
              <br />
              <span style={{ color: '#7b6e60', fontSize: 12 }}>（e2c3042 已验证可达）</span>
            </div>
          </div>

          {/* 🟡 可以做到 · 要先补 */}
          <div
            style={{
              flex: '1 1 230px',
              background: '#faf0d4',
              border: `2px solid ${triage.branch === 'clarify' ? '#946200' : '#d8b974'}`,
              borderRadius: 12,
              padding: '10px 12px',
              boxShadow: triage.branch === 'clarify' ? '0 0 0 3px rgba(148,98,0,0.2)' : 'none',
              opacity: triage.branch === 'clarify' ? 1 : 0.8,
            }}
          >
            <div style={{ fontWeight: 700, color: '#946200', marginBottom: 4, display: 'flex', justifyContent: 'space-between' }}>
              <span>🟡 可以做到 · 要先补</span>
              {triage.branch === 'clarify' && <span style={{ fontSize: 11, background: '#946200', color: '#fff', padding: '1px 6px', borderRadius: 4 }}>当前命中</span>}
            </div>
            <div style={{ fontSize: 13, color: '#4c4239' }}>歧义未消 / 时间窗缺且无默认 / 形态证据不足 / 待确认候选列</div>
            <div style={{ marginTop: 8, fontSize: 13, background: '#fffcf7', borderRadius: 8, padding: '6px 8px', color: '#4c4239' }}>
              注入上下文 + 追问（blockers / alternatives）
              <br />
              <span style={{ color: '#946200', fontSize: 12, fontWeight: 700 }}>不猜，先问 → 再生成</span>
            </div>
          </div>

          {/* 🔴 不可以做到 */}
          <div
            style={{
              flex: '1 1 230px',
              background: '#fbe9e7',
              border: `2px solid ${triage.branch === 'reject' ? '#b42318' : '#e09891'}`,
              borderRadius: 12,
              padding: '10px 12px',
              boxShadow: triage.branch === 'reject' ? '0 0 0 3px rgba(180,35,24,0.2)' : 'none',
              opacity: triage.branch === 'reject' ? 1 : 0.8,
            }}
          >
            <div style={{ fontWeight: 700, color: '#b42318', marginBottom: 4, display: 'flex', justifyContent: 'space-between' }}>
              <span>🔴 不可以做到</span>
              {triage.branch === 'reject' && <span style={{ fontSize: 11, background: '#b42318', color: '#fff', padding: '1px 6px', borderRadius: 4 }}>当前命中</span>}
            </div>
            <div style={{ fontSize: 13, color: '#4c4239' }}>关键槽在词表外 / 含未知词 / 物理库缺数据 / 结构缺到补不了</div>
            <div style={{ marginTop: 8, fontSize: 13, background: '#fffcf7', borderRadius: 8, padding: '6px 8px', color: '#4c4239' }}>
              明确拒绝 + 报告缺什么（词表外词清单）
              <br />
              <span style={{ color: '#b42318', fontSize: 12, fontWeight: 700 }}>补救：改述 / 补词表 / 选 alternatives</span>
            </div>
          </div>
        </div>

        {/* 交互式消歧与补齐工具箱 (当落入 🟡 或 🔴 时展示) */}
        {triage.branch === 'clarify' && triage.quickFixes.length > 0 && (
          <div
            style={{
              marginTop: 12,
              background: '#fffcf7',
              border: '1.5px solid #946200',
              borderRadius: 10,
              padding: '10px 12px',
            }}
          >
            <div style={{ fontWeight: 700, color: '#946200', marginBottom: 6, fontSize: 13 }}>
              ⚡ 快速补齐 / 消解歧义（点击直接装配槽位，立转全绿 🟢）
            </div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
              {triage.quickFixes.map((fix) => (
                <button
                  key={fix.id}
                  type="button"
                  onClick={() => applyQuickFix(fix)}
                  style={{
                    background: '#faf0d4',
                    border: '1px solid #946200',
                    color: '#946200',
                    borderRadius: 6,
                    padding: '4px 10px',
                    fontSize: 12,
                    cursor: 'pointer',
                    fontWeight: 600,
                  }}
                  title={fix.description}
                >
                  + {fix.label}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* 域外拒绝详情 (当落入 🔴 时展示) */}
        {triage.branch === 'reject' && (
          <div
            style={{
              marginTop: 12,
              background: '#fffcf7',
              border: '1.5px solid #b42318',
              borderRadius: 10,
              padding: '10px 12px',
            }}
          >
            <div style={{ fontWeight: 700, color: '#b42318', marginBottom: 6, fontSize: 13 }}>
              🛡️ 系统拒绝原因清单与补救通道
            </div>
            <ul style={{ margin: '4px 0 8px 18px', padding: 0, fontSize: 13, color: '#4c4239' }}>
              {triage.blockers.map((b, i) => (
                <li key={i}>{b}</li>
              ))}
            </ul>
            <div style={{ fontSize: 12, color: '#7b6e60', display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <span>建议补救：</span>
              {triage.remedies.map((rem, i) => (
                <span key={i} style={{ background: '#fbe9e7', padding: '2px 6px', borderRadius: 4, color: '#b42318' }}>
                  {rem}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* 为什么必须分诊 */}
        <div
          style={{
            marginTop: 12,
            borderLeft: '4px solid #2b4657',
            background: '#eef3f6',
            borderRadius: 8,
            padding: '10px 12px',
            fontSize: 13.5,
            color: '#2c4a3f',
          }}
        >
          <b>为什么必须分诊</b>：错误成本不对称——边缘勉强匹配产出的"口径错的报表"比明确拒绝更糟。分诊 = 先判"这句话在不在承诺域内"，域内跑、域外明说，系统永不假装懂。
        </div>
      </div>

      {/* 2. 自动探索业务系统生成报表：四步流水线 (Tab 2) */}
      {activeTab === 'pipeline' && (
        <div
          style={{
            width: '100%',
            boxSizing: 'border-box',
            background: '#f5f1ea',
            color: '#191512',
            fontFamily: "'Microsoft YaHei',system-ui,sans-serif",
            fontSize: 14,
            lineHeight: 1.6,
            padding: '20px 16px',
            borderRadius: 12,
            marginBottom: 16,
            border: '1px solid #d3c7b5',
          }}
        >
          {/* 标题 */}
          <div style={{ fontWeight: 800, fontSize: 16, marginBottom: 10, color: '#191512' }}>
            自动探索业务系统生成报表：四步流水线
          </div>

          {/* 图例 */}
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12, marginBottom: 12, fontSize: 13 }}>
            <span style={{ display: 'inline-flex', alignSelf: 'center', alignItems: 'center', gap: 6 }}>
              <span
                style={{
                  width: 14,
                  height: 14,
                  background: '#e3f2ec',
                  border: '1.5px solid #157055',
                  borderRadius: 4,
                  display: 'inline-block',
                }}
              />
              确定性：规则/纯工程
            </span>
            <span style={{ display: 'inline-flex', alignSelf: 'center', alignItems: 'center', gap: 6 }}>
              <span
                style={{
                  width: 14,
                  height: 14,
                  background: '#fbe9e7',
                  border: '1.5px solid #b42318',
                  borderRadius: 4,
                  display: 'inline-block',
                }}
              />
              开放理解：LLM 或人
            </span>
          </div>

          {/* 四步卡片 */}
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            {/* ① 自动探查 */}
            <div
              style={{
                flex: '1 1 180px',
                minWidth: 160,
                border: '1.5px solid #157055',
                background: '#e3f2ec',
                borderRadius: 12,
                padding: '10px 12px',
              }}
            >
              <div style={{ fontWeight: 700, color: '#157055', marginBottom: 2 }}>① 自动探查</div>
              <div style={{ fontSize: 12.5, color: '#4c4239' }}>
                连 dsn → 表/列/类型/基数/空值/主外键/样例
              </div>
              <div style={{ marginTop: 6, fontSize: 12, color: '#157055', fontWeight: 600 }}>
                ✅ 确定性（读元数据）
              </div>
              <div style={{ marginTop: 6, fontSize: 11, color: '#7b6e60' }}>
                当前已连接：{dsn || 'demo_sqlite'} (3 张实体表)
              </div>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', fontSize: 20, color: '#7b6e60' }}>→</div>

            {/* ② 推导可统计情况 */}
            <div
              style={{
                flex: '1 1 200px',
                minWidth: 180,
                border: '1.5px solid #157055',
                background: '#e3f2ec',
                borderRadius: 12,
                padding: '10px 12px',
              }}
            >
              <div style={{ fontWeight: 700, color: '#157055', marginBottom: 2 }}>② 推导可统计情况</div>
              <div style={{ fontSize: 12.5, color: '#4c4239' }}>
                数值列→指标候选 / 低基数列→维度 / 时间列→粒度 / 表关系→实体 / 组合合法性
              </div>
              <div style={{ marginTop: 6, fontSize: 12, color: '#157055', fontWeight: 600 }}>
                ✅ 规则（列性质可计算）
              </div>
              <div style={{ marginTop: 6, fontSize: 12, background: '#fffcf7', borderRadius: 8, padding: '4px 6px', color: '#946200' }}>
                产出：候选能力图谱（bizmodel 草案）
              </div>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', fontSize: 20, color: '#7b6e60' }}>→</div>

            {/* ③ 语言匹配数据集 */}
            <div
              style={{
                flex: '1 1 200px',
                minWidth: 180,
                border: '1.5px solid #b42318',
                background: '#fbe9e7',
                borderRadius: 12,
                padding: '10px 12px',
              }}
            >
              <div style={{ fontWeight: 700, color: '#b42318', marginBottom: 2 }}>③ 语言匹配数据集</div>
              <div style={{ fontSize: 12.5, color: '#4c4239' }}>
                一句话 → 在候选图谱中匹配指标/维度/时间 → 可组合数据集
              </div>
              <div style={{ marginTop: 6, fontSize: 12, color: '#b42318', fontWeight: 600 }}>
                ⚠️ 开放理解（匹配空间已收敛到候选清单）
              </div>
              <div style={{ marginTop: 6, fontSize: 11, color: '#7b6e60' }}>
                命中词项：{triage.matchedTerms.map((t) => t.label).join('、') || '无'}
              </div>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', fontSize: 20, color: '#7b6e60' }}>→</div>

            {/* ④ 生成报表 */}
            <div
              style={{
                flex: '1 1 170px',
                minWidth: 150,
                border: '1.5px solid #157055',
                background: '#e3f2ec',
                borderRadius: 12,
                padding: '10px 12px',
              }}
            >
              <div style={{ fontWeight: 700, color: '#157055', marginBottom: 2 }}>④ 生成报表</div>
              <div style={{ fontSize: 12.5, color: '#4c4239' }}>
                spec → run → 真报表（现有引擎闭环）
              </div>
              <div style={{ marginTop: 6, fontSize: 12, color: '#157055', fontWeight: 600 }}>
                ✅ 零改动
              </div>
              <div style={{ marginTop: 6, fontSize: 11, color: '#7b6e60' }}>
                引擎：rpx-runtime / reportEngine
              </div>
            </div>
          </div>

          {/* 语义边界 + 登记回路 */}
          <div
            style={{
              marginTop: 12,
              border: '1.5px dashed #b42318',
              background: '#fdf6f5',
              borderRadius: 12,
              padding: '10px 12px',
            }}
          >
            <div style={{ fontWeight: 700, color: '#b42318', marginBottom: 4 }}>
              语义边界（绕不开的一环）：结构可自动，语义不可自动
            </div>
            <div style={{ fontSize: 12.5, color: '#4c4239', lineHeight: 1.7 }}>
              <span style={{ background: '#fbe9e7', borderRadius: 6, padding: '2px 6px', marginRight: 8 }}>
                amt_net = 数值列可 sum ✅（自动）
              </span>
              <span style={{ background: '#fbe9e7', borderRadius: 6, padding: '2px 6px' }}>
                amt_net 是净额 / 含不含税 / 算不算应收账款 ❌（业务语义）
              </span>
            </div>
            <div style={{ marginTop: 8, display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap', fontSize: 12.5 }}>
              <span style={{ background: '#fffcf7', border: '1px solid #d3c7b5', borderRadius: 8, padding: '4px 8px' }}>
                候选图谱 + 候选语义标签
              </span>
              <span style={{ fontSize: 16, color: '#7b6e60' }}>→</span>
              <span style={{ background: '#faf0d4', border: '1px solid #946200', borderRadius: 8, padding: '4px 8px' }}>
                确认 / 标注（人抽查 或 LLM 给置信度）
              </span>
              <span style={{ fontSize: 16, color: '#7b6e60' }}>→</span>
              <span style={{ background: '#e3f2ec', border: '1px solid #157055', borderRadius: 8, padding: '4px 8px' }}>
                沉淀 bizmodel → 下次直接命中（不再探查）
              </span>
            </div>
          </div>

          {/* 为什么这个方向对 */}
          <div
            style={{
              marginTop: 12,
              borderLeft: '4px solid #157055',
              background: '#e3f2ec',
              borderRadius: 8,
              padding: '10px 12px',
              fontSize: 13.5,
              color: '#2c4a3f',
            }}
          >
            <b>为什么这个方向对</b>：覆盖域从"人工登记了什么"变成"系统自己能发现什么"（自动扩张）；匹配空间从"整个未知系统"收敛到"探查后的候选清单"（L3 未知性基本消除，只剩 L1 语义）；现有需求层 / ontology / spec_for / 引擎全部复用——新增的只是前置"自动建模层"。
          </div>
        </div>
      )}

      {/* 3. 三层可行性联合判定矩阵 (Tab 3) */}
      {activeTab === 'layers' && (
        <div
          style={{
            width: '100%',
            boxSizing: 'border-box',
            background: '#fffcf7',
            color: '#191512',
            fontFamily: "'Microsoft YaHei',system-ui,sans-serif",
            fontSize: 14,
            lineHeight: 1.6,
            padding: '16px',
            borderRadius: 12,
            marginBottom: 16,
            border: '1.5px solid #d3c7b5',
          }}
        >
          <div style={{ fontWeight: 800, fontSize: 16, marginBottom: 8, color: '#191512' }}>
            覆盖域真边界：词表 + 规则 + 当前 dsn 的已建模 / 可探查数据
          </div>
          <div style={{ fontSize: 13, color: '#7b6e60', marginBottom: 12 }}>
            前两项是静态的（规则作者写），第三项是动态的（运行期随外部系统而变）。纯规则分诊之所以不可行，在于规则只能判定
            L2 登记表，回答不了 L3 物理系统里到底有没有。
          </div>

          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ background: '#eef3f6', textAlign: 'left', borderBottom: '2px solid #2b4657' }}>
                <th style={{ padding: '8px 10px', color: '#2b4657' }}>层级</th>
                <th style={{ padding: '8px 10px', color: '#2b4657' }}>判定问题</th>
                <th style={{ padding: '8px 10px', color: '#2b4657' }}>机制</th>
                <th style={{ padding: '8px 10px', color: '#2b4657' }}>规则可行吗</th>
                <th style={{ padding: '8px 10px', color: '#2b4657' }}>当前问句实时结果</th>
              </tr>
            </thead>
            <tbody>
              {/* L1 */}
              <tr style={{ borderBottom: '1px solid #e2dacd' }}>
                <td style={{ padding: '10px', fontWeight: 700, color: '#2b4657' }}>L1 语言可解</td>
                <td style={{ padding: '10px' }}>这句话要什么（指标 / 维度 / 时间 / 口径）</td>
                <td style={{ padding: '10px' }}>开放语言理解</td>
                <td style={{ padding: '10px', color: '#b42318' }}>❌ 需 LLM 或人</td>
                <td style={{ padding: '10px' }}>
                  <span
                    style={{
                      background: triage.layers.l1.status === 'ok' ? '#e3f2ec' : '#faf0d4',
                      color: triage.layers.l1.status === 'ok' ? '#157055' : '#946200',
                      padding: '2px 8px',
                      borderRadius: 6,
                      fontWeight: 600,
                    }}
                  >
                    {triage.layers.l1.summary}
                  </span>
                </td>
              </tr>

              {/* L2 */}
              <tr style={{ borderBottom: '1px solid #e2dacd' }}>
                <td style={{ padding: '10px', fontWeight: 700, color: '#2b4657' }}>L2 模型覆盖</td>
                <td style={{ padding: '10px' }}>词表 / bizmodel 里有没有这个词</td>
                <td style={{ padding: '10px' }}>bizmodel 查询</td>
                <td style={{ padding: '10px', color: '#157055' }}>✅ 确定性</td>
                <td style={{ padding: '10px' }}>
                  <span
                    style={{
                      background:
                        triage.layers.l2.status === 'ok'
                          ? '#e3f2ec'
                          : triage.layers.l2.status === 'warn'
                          ? '#faf0d4'
                          : '#fbe9e7',
                      color:
                        triage.layers.l2.status === 'ok'
                          ? '#157055'
                          : triage.layers.l2.status === 'warn'
                          ? '#946200'
                          : '#b42318',
                      padding: '2px 8px',
                      borderRadius: 6,
                      fontWeight: 600,
                    }}
                  >
                    {triage.layers.l2.summary}
                  </span>
                </td>
              </tr>

              {/* L3 */}
              <tr>
                <td style={{ padding: '10px', fontWeight: 700, color: '#2b4657' }}>L3 数据可用</td>
                <td style={{ padding: '10px' }}>对接系统有没有这个数据、口径对不对</td>
                <td style={{ padding: '10px' }}>
                  <b>探查</b>（schema/字段/样例，确定性）→ <b>语义匹配与口径确认</b>
                </td>
                <td style={{ padding: '10px', color: '#946200' }}>
                  ⚠️ 探查可确定性，<b>语义匹配不可</b> → 需 LLM 或人
                </td>
                <td style={{ padding: '10px' }}>
                  <span
                    style={{
                      background:
                        triage.layers.l3.status === 'ok'
                          ? '#e3f2ec'
                          : triage.layers.l3.status === 'warn'
                          ? '#faf0d4'
                          : '#fbe9e7',
                      color:
                        triage.layers.l3.status === 'ok'
                          ? '#157055'
                          : triage.layers.l3.status === 'warn'
                          ? '#946200'
                          : '#b42318',
                      padding: '2px 8px',
                      borderRadius: 6,
                      fontWeight: 600,
                    }}
                  >
                    {triage.layers.l3.summary}
                  </span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      )}

      {/* 4. 输入表单与参数调优区 */}
      <section className="card pad">
        <div className="form-grid">
          <div>
            <label className="field">数据源 (DSN)</label>
            <select aria-label="数据源" value={dsn} onChange={(e) => setDsn(e.target.value)}>
              {sources.length === 0 ? <option value="">（先去「数据源」配一个）</option> : null}
              {sources.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.label || d.id}
                </option>
              ))}
            </select>
          </div>
          <div style={{ gridColumn: 'span 2' }}>
            <label className="field">报表名（可不填，默认取那句话）</label>
            <input
              type="text"
              aria-label="报表名"
              value={title}
              placeholder="例：各区域月度销售额走势"
              onChange={(e) => setTitle(e.target.value)}
            />
          </div>
          <div style={{ gridColumn: 'span 3' }}>
            <label className="field">
              一句话需求 <span className="tiny">（边输边分诊，实时推演 L1/L2/L3）</span>
            </label>
            <input
              type="text"
              aria-label="一句话需求"
              value={text}
              placeholder="例：按客户等级看 LTV 和逾期率 / 各区域销售额排名 Top 5"
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void submit();
              }}
            />
          </div>
          <div>
            <label className="field">形态</label>
            <select aria-label="形态" value={shape} onChange={(e) => setShape(e.target.value)}>
              {shapeOptions.map((s) => (
                <option key={s.shape} value={s.shape}>
                  {s.label}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="field">截取行数（排名用）</label>
            <input
              type="number"
              aria-label="截取行数"
              min={1}
              value={limit}
              placeholder="不限"
              onChange={(e) => setLimit(e.target.value)}
            />
          </div>
          <div>
            <label className="field">生成后</label>
            <label className="row" style={{ gap: 8, alignItems: 'center', marginTop: 10 }}>
              <input type="checkbox" checked={runNow} onChange={(e) => setRunNow(e.target.checked)} />
              <span className="tiny">立即运行一次</span>
            </label>
          </div>
        </div>

        {model ? (
          <div style={{ marginTop: 16 }}>
            <div className="field">指标（可多选，不选就从那句话里认）</div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, margin: '8px 0 14px' }}>
              {model.metrics.map((m) => (
                <button
                  key={m.id}
                  type="button"
                  className={`pill ${pickedMetrics.includes(m.id) ? 'ok' : ''}`}
                  style={{ cursor: 'pointer' }}
                  onClick={() => setPickedMetrics(toggle(pickedMetrics, m.id))}
                  title={m.aliases?.join('、')}
                >
                  {m.label || m.id}
                </button>
              ))}
            </div>
            <div className="field">维度（顺序即摆位：第一个放行）</div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 8 }}>
              {model.dimensions.map((d) => (
                <button
                  key={d.id}
                  type="button"
                  className={`pill ${pickedDims.includes(d.id) ? 'ok' : ''}`}
                  style={{ cursor: 'pointer' }}
                  onClick={() => setPickedDims(toggle(pickedDims, d.id))}
                  title={`${d.kind}${d.time_grain ? ` · ${d.time_grain}` : ''}`}
                >
                  {d.label || d.id}
                </button>
              ))}
            </div>
          </div>
        ) : (
          <div className="hint" style={{ marginTop: 12 }}>
            这个数据源还没有业务模型 —— 先去「业务知识」跑一次发现，才知道有哪些指标和维度可认。
          </div>
        )}

        <div className="form-actions" style={{ marginTop: 18 }}>
          <button
            className={`btn ${triage.branch === 'ready' ? 'primary' : ''}`}
            disabled={busy || triage.branch === 'reject'}
            onClick={() => void submit()}
            style={{
              background:
                triage.branch === 'ready'
                  ? '#157055'
                  : triage.branch === 'clarify'
                  ? '#946200'
                  : '#b42318',
              color: '#fff',
              border: 'none',
              padding: '8px 18px',
              fontWeight: 700,
            }}
          >
            {busy
              ? '生成中…'
              : triage.branch === 'ready'
              ? '🟢 直接生成：spec → run → 真报表'
              : triage.branch === 'clarify'
              ? '🟡 建议先补齐槽位 → 再生成'
              : '🔴 域外拒绝：不可生成'}
          </button>
          <span className="hint">
            分诊机制生效中：认不出口径或物理缺数会<b>阻断并给出缺口报告</b>，坚决杜绝“猜错的报表”流向生产。
          </span>
        </div>
      </section>

      {error ? (
        <div className="callout-line" style={{ borderColor: 'var(--err)', marginTop: 12 }}>
          {error}
        </div>
      ) : null}

      {/* 5. 报表生成结果区 */}
      {result ? (
        <>
          <div className="section-title">
            生成结果 <span className="tiny">{result.report_id}</span>
            <button
              className="linklike"
              style={{ marginLeft: 'auto' }}
              onClick={() => onOpenReport(result.report_id)}
            >
              打开报表 →
            </button>
          </div>
          <section className="card pad">
            <div className="row" style={{ gap: 10, alignItems: 'baseline', flexWrap: 'wrap' }}>
              <b style={{ fontSize: 16 }}>{result.title}</b>
              <span className="pill ok">{decision?.label ?? result.shape}</span>
              <span className="tiny">layout {decision?.layout ?? (result.spec.layout as string)}</span>
              <span className="tiny">置信度 {decision?.confidence ?? '—'}</span>
              {result.run ? (
                result.run.ok ? (
                  <span className="badge ran">已运行 · {result.run.status}</span>
                ) : (
                  <span className="badge failed">运行失败：{result.run.error}</span>
                )
              ) : (
                <span className="tiny">（未运行）</span>
              )}
            </div>

            <div className="hint" style={{ marginTop: 10 }}>
              口径来源：{result.query.picked_by === 'explicit' ? '手动指定' : '从那句话里认出'} · 指标{' '}
              <code className="sql-inline">{result.query.metrics.join(', ') || '—'}</code> · 维度{' '}
              <code className="sql-inline">{result.query.dimensions.join(', ') || '—'}</code> · 结果{' '}
              {result.query.row_count} 行
            </div>

            <div style={{ marginTop: 14 }}>
              <div className="field">为什么判成这个形态</div>
              <ul style={{ margin: '6px 0 0 18px', padding: 0 }}>
                {(decision?.reasons ?? []).map((r, i) => (
                  <li key={i} className="tiny">
                    {r}
                  </li>
                ))}
              </ul>
            </div>

            {result.notes.length ? (
              <div style={{ marginTop: 12 }}>
                <div className="field">摆位说明</div>
                <ul style={{ margin: '6px 0 0 18px', padding: 0 }}>
                  {result.notes.map((n, i) => (
                    <li key={i} className="tiny">
                      {n}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            <div className="section-title" style={{ margin: '22px 0 10px' }}>
              生成的查询 <span className="tiny">口径必须能被审</span>
              <button
                className="linklike"
                style={{ marginLeft: 'auto' }}
                onClick={() => setShowSpec(!showSpec)}
              >
                {showSpec ? '收起 spec' : '看编译前的 spec'}
              </button>
            </div>
            <pre className="sqlbox" style={{ maxHeight: 220, overflow: 'auto', margin: 0 }}>
              {result.query.sql}
            </pre>
            {showSpec ? (
              <pre className="sqlbox" style={{ maxHeight: 260, overflow: 'auto', marginTop: 10 }}>
                {JSON.stringify(result.spec, null, 2)}
              </pre>
            ) : null}

            {runFailed ? (
              <div className="callout-line" style={{ borderColor: 'var(--warn)', marginTop: 12 }}>
                报表已生成，但这一次运行失败：{result.run?.error}。模板本身是可审查的资产，
                修好数据源或口径后到报表页重跑。
              </div>
            ) : null}
          </section>
        </>
      ) : null}
    </>
  );
};
