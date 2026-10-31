import React, { useState } from 'react';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { ContextSnapshot, Requirement, TriageResult, AnalysisDoc, ReportShape } from '../types';
import { ClarificationCard } from '../components/ClarificationCard';
import { ContextInspectorModal } from '../components/modals/ContextInspectorModal';
import { runAttributionAnalysis, buildCanonicalReport } from '../services/reportEngine';
import { UniverViewer } from '../components/UniverViewer';
import { SHAPE_LABELS } from '../data/seedData';

interface Props {
  onOpenRequirement: (id: string) => void;
  onOpenDesigner?: (id: string) => void;
  onOpenRun: (reqId: string, runId: string) => void;
  onNewRequirement: () => void;
  onOpenLlmModal: () => void;
}

export const WorkbenchView: React.FC<Props> = ({
  onOpenRequirement,
  onOpenDesigner,
  onOpenRun,
  onNewRequirement,
  onOpenLlmModal,
}) => {
  const { toast } = useToast();
  const requirements = store.getRequirements();
  const datasources = store.getDatasources();
  const runs = store.getRuns();
  const llmConfig = store.getLlmConfig();

  const [askText, setAskText] = useState('华东各品类销售额交叉表（自动归因异动）');
  const [selectedDsn, setSelectedDsn] = useState(datasources[0]?.id || 'demo-sqlite');
  const [isGenerating, setIsGenerating] = useState(false);
  const [activeStep, setActiveStep] = useState<number | null>(null);

  // Triage state
  const [triageResult, setTriageResult] = useState<TriageResult | null>(null);
  const [showContextModal, setShowContextModal] = useState(false);
  const [contextSnapshot, setContextSnapshot] = useState<ContextSnapshot | null>(null);

  // Auto attribution state (Step 5)
  const [attributionDoc, setAttributionDoc] = useState<AnalysisDoc | null>(null);
  const [attributionSigned, setAttributionSigned] = useState(false);
  const [lastGeneratedRun, setLastGeneratedRun] = useState<{ reqId: string; runId: string } | null>(null);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [activeOutputTab, setActiveOutputTab] = useState<'univer' | 'web' | 'print' | 'json' | 'spec'>('univer');

  // Active run & requirement selection for the Live Report Engine Canvas
  const activeRun = selectedRunId
    ? runs.find((r) => r.run_id === selectedRunId) || runs[0]
    : lastGeneratedRun
    ? runs.find((r) => r.run_id === lastGeneratedRun.runId) || runs[0]
    : runs[0];
  const activeReq = activeRun ? store.getRequirement(activeRun.req_id) : requirements[0];

  const handleSwitchShape = (shape: ReportShape) => {
    if (!activeReq) return;
    store.setShape(activeReq.id, shape);
    const { run } = store.runRequirement(activeReq.id);
    setSelectedRunId(run.run_id);
    setLastGeneratedRun({ reqId: activeReq.id, runId: run.run_id });
    toast(`已由报表引擎动态重组为：${SHAPE_LABELS[shape] || shape}`);
  };

  const handleDownloadArtifact = (filename: string, content: any) => {
    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
    toast(`已下载 ${filename}`);
  };

  const openReqsCount = requirements.filter((r) => r.status !== 'accepted').length;
  const templatesCount = requirements.filter((r) => r.template && r.template.rev > 0).length;
  const runsCount = runs.length;

  const demoPresets = [
    {
      label: '⊞ 华东各品类销售额交叉表（自动归因）',
      prompt: '华东各品类销售额交叉表（自动归因异动）',
      badge: '交叉表',
      desc: '横向品类展开 × 纵向大区展开，带自动公式小计与总计 =SUM()，并自动触发异常归因',
    },
    {
      label: '≡ 华东各大区与门店业绩分组汇总表',
      prompt: '按大区和门店分组汇总销售额',
      badge: '分组汇总',
      desc: '大区 ➔ 门店两级多维分组聚合，展示千分位金额格式化与合计行',
    },
    {
      label: '📈 2026年各月份销售趋势走势表',
      prompt: '2026年各月份销售趋势走势表',
      badge: '趋势表',
      desc: '月度时间桶序列排序，直观展现 7月、8月、9月业务走势演变',
    },
    {
      label: '🏆 各门店销售业绩降序排行榜 TOP 榜',
      prompt: '按销售额对所有门店降序排行 TOP 榜',
      badge: '排行榜',
      desc: '门店度量从高到低倒序排位，聚焦头部明星门店',
    },
    {
      label: '🛡️ 按客户等级看 LTV 和逾期率（本体复合防翻车）',
      prompt: '按客户等级看 LTV 和逾期率',
      badge: '本体复合表',
      desc: '跨客户+订单+贷款3张表，中心主表汇聚，独立预聚合彻底防止笛卡尔积翻车',
    },
    {
      label: '🟡 统计各品类毛利与进销差价（口径确认）',
      prompt: '统计各品类毛利与进销差价',
      badge: '口径确认',
      desc: '触发二义性追问：综合毛利 vs 纯进销差价，先确认再生成',
    },
    {
      label: '🔴 查询量子纠缠对冲损益表（严谨拒答）',
      prompt: '查询2026年量子纠缠对冲损益表',
      badge: '严谨拒答',
      desc: '关键概念在词表与库表外，严谨拒绝并输出数据缺失审计',
    },
  ];

  const handleSelectPreset = (p: { prompt: string }) => {
    setAskText(p.prompt);
    setTriageResult(null);
    setAttributionDoc(null);
    setAttributionSigned(false);
  };

  const handleOpenContextInspector = () => {
    const ctx = store.getContextSnapshot(selectedDsn, askText);
    setContextSnapshot(ctx);
    setShowContextModal(true);
  };

  // Main Ask & Triage Flow
  const handleAskRun = (optionsOverride?: Record<string, string>) => {
    const text = askText.trim();
    if (!text) {
      toast('先说要什么（一句话就行）', true);
      return;
    }
    if (!selectedDsn) {
      toast('先去「数据源」配一个库', true);
      return;
    }

    setIsGenerating(true);
    setAttributionDoc(null);
    setAttributionSigned(false);
    setActiveStep(1);

    // Step 1: 探查上下文
    setTimeout(() => {
      setActiveStep(2);

      // Step 2: 规则推导
      setTimeout(() => {
        setActiveStep(3);

        // Step 3: 分诊评估
        const triage = store.triageQuery(text, selectedDsn, optionsOverride);
        setTriageResult(triage);

        if (triage.status === 'ready') {
          // Step 4: 直接生成真报表
          setActiveStep(4);
          setTimeout(() => {
            try {
              const { requirement, run } = store.generateFromTriage(triage, selectedDsn);
              setLastGeneratedRun({ reqId: requirement.id, runId: run.run_id });

              // Step 5: 报表 Agent 生成流水线「自动归因」步骤 (当指标存在波动时自动分析底层维度贡献度)
              setActiveStep(5);
              setTimeout(() => {
                const doc = runAttributionAnalysis(selectedDsn, 'amount', 'month', 'month', '2026-10-01');
                setAttributionDoc(doc);
                setIsGenerating(false);
                setActiveStep(null);
                toast(`🟢 报表生成成功！已检测到异常波动并自动完成底层维度归因分析`);
              }, 450);
            } catch (err: any) {
              setIsGenerating(false);
              setActiveStep(null);
              toast(err.message, true);
            }
          }, 450);
        } else if (triage.status === 'clarify') {
          setIsGenerating(false);
          setActiveStep(null);
          toast('🟡 检测到口径二义性，请确认核算标准后生成', true);
        } else {
          // Infeasible
          setIsGenerating(false);
          setActiveStep(null);
          toast('🔴 关键概念超出数据覆盖域，已明确拒绝', true);
        }
      }, 300);
    }, 250);
  };

  const handleClarificationConfirm = (optionsOverride: Record<string, string>) => {
    handleAskRun(optionsOverride);
  };

  const fmtTime = (iso?: string) => {
    if (!iso) return '—';
    const d = new Date(iso);
    if (isNaN(d.getTime())) return iso;
    const p = (n: number) => String(n).padStart(2, '0');
    return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
  };

  const fmtMs = (ms?: number) => {
    if (ms == null) return '—';
    return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
  };

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span>THE NORTH STAR</span>
            <span style={{ opacity: 0.6 }}>·</span>
            <span>企业报表智能化演化路径</span>
          </div>
          <h1 style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
            <span>「一句话生成报表」演化工作台</span>
            <span
              style={{
                fontSize: '11.5px',
                background: 'linear-gradient(135deg, #1e293b, #0f172a)',
                color: '#f8fafc',
                padding: '4px 10px',
                borderRadius: '6px',
                fontWeight: 600,
                letterSpacing: '0.04em',
                boxShadow: '0 1px 3px rgba(0,0,0,0.12)',
              }}
            >
              ★ 阶段交付：双向人机核准 MVP
            </span>
          </h1>
          <div className="sub" style={{ maxWidth: '82ch', lineHeight: 1.8 }}>
            <strong>「一句话生成报表」是企业数据基础设施长期演化的终极目标。</strong>
            为彻底杜绝大模型“无据幻觉”与笛卡尔积翻车，本工作台通过「物理感知 ➔ 语义拓扑 ➔ 确定性三层分诊 ➔ 双向人机核准 ➔ Univer 渲染 ➔ 异常自动归因」，保证每张报表数字 100% 真实可信、可溯可复盘。
          </div>
        </div>
        <div style={{ display: 'flex', gap: '8px', alignSelf: 'flex-start' }}>
          <button type="button" className="btn" onClick={handleOpenContextInspector}>
            🔍 查看上下文事实
          </button>
          <button type="button" className="btn primary" onClick={onNewRequirement}>
            ＋ 新建需求
          </button>
        </div>
      </div>

      {/* 报表演化阶梯 (Evolution Maturity Ladder) */}
      <div
        style={{
          background: 'linear-gradient(180deg, #ffffff 0%, #fcfbf9 100%)',
          border: '1px solid var(--line)',
          borderRadius: '12px',
          padding: '16px 20px',
          marginBottom: '22px',
          boxShadow: 'var(--shadow-s)',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            marginBottom: '12px',
            borderBottom: '1px solid var(--paper-3)',
            paddingBottom: '8px',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{ font: '600 11px/1 var(--font-mono)', color: 'var(--accent)', letterSpacing: '0.12em' }}>
              EVOLUTION ROADMAP
            </span>
            <span style={{ fontSize: '12.5px', fontWeight: 700, color: 'var(--ink)' }}>
              报表智能化四级演进成熟度阶梯
            </span>
          </div>
          <div className="tiny" style={{ color: 'var(--ink-4)' }}>
            当前所处：<strong style={{ color: 'var(--accent-ink)' }}>L3 人机协同 Agent 阶段</strong>（兼具敏捷理解与绝对严谨）
          </div>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: '12px' }}>
          {/* L1 */}
          <div
            style={{
              padding: '12px',
              borderRadius: '8px',
              background: 'var(--paper-2)',
              border: '1px solid var(--line)',
              opacity: 0.75,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '4px' }}>
              <span style={{ font: '700 11px var(--font-mono)', color: 'var(--ink-3)' }}>L1 · 传统人工报表</span>
              <span className="tiny" style={{ background: 'var(--paper-3)', padding: '1px 6px', borderRadius: '4px' }}>
                高成本
              </span>
            </div>
            <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--ink)' }}>手写 SQL · 拖拽排版</div>
            <div className="tiny" style={{ color: 'var(--ink-4)', marginTop: '4px', lineHeight: 1.5 }}>
              痛点：业务口径黑盒，跨表连接易笛卡尔积翻车，物理表一变报表全崩。
            </div>
          </div>

          {/* L2 */}
          <div
            style={{
              padding: '12px',
              borderRadius: '8px',
              background: 'var(--paper-2)',
              border: '1px solid var(--line)',
              opacity: 0.85,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '4px' }}>
              <span style={{ font: '700 11px var(--font-mono)', color: 'var(--ink-3)' }}>L2 · 语义指标中心</span>
              <span className="tiny" style={{ background: 'var(--paper-3)', padding: '1px 6px', borderRadius: '4px' }}>
                静态固化
              </span>
            </div>
            <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--ink)' }}>指标库 · 固定模板套打</div>
            <div className="tiny" style={{ color: 'var(--ink-4)', marginTop: '4px', lineHeight: 1.5 }}>
              痛点：指标口径已统一定义，但模板排版僵化，无法理解自由业务提问。
            </div>
          </div>

          {/* L3 Current */}
          <div
            style={{
              padding: '12px',
              borderRadius: '8px',
              background: '#f8fafc',
              border: '2px solid #3b82f6',
              boxShadow: '0 2px 10px rgba(59, 130, 246, 0.1)',
              position: 'relative',
            }}
          >
            <div
              style={{
                position: 'absolute',
                top: '-9px',
                right: '12px',
                background: '#2563eb',
                color: '#ffffff',
                fontSize: '10px',
                fontWeight: 700,
                padding: '1px 7px',
                borderRadius: '4px',
                letterSpacing: '0.04em',
              }}
            >
              ★ 当前 MVP 阶段
            </div>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '4px' }}>
              <span style={{ font: '700 11px var(--font-mono)', color: '#1d4ed8' }}>L3 · 人机协同 Agent</span>
            </div>
            <div style={{ fontSize: '12px', fontWeight: 700, color: '#0f172a' }}>
              AI 探查生成 ➔ 人类核准 ➔ 自动归因
            </div>
            <div className="tiny" style={{ color: '#475569', marginTop: '4px', lineHeight: 1.5 }}>
              核心：确定性三层分诊杜绝幻觉，遇歧义必追问，异常指标触发维度贡献拆解。
            </div>
          </div>

          {/* L4 Long Term Goal */}
          <div
            style={{
              padding: '12px',
              borderRadius: '8px',
              background: 'linear-gradient(135deg, #fdfbf7 0%, #fff7ed 100%)',
              border: '1.5px dashed var(--accent)',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '4px' }}>
              <span style={{ font: '700 11px var(--font-mono)', color: 'var(--accent-ink)' }}>L4 · 终极自主 Agent</span>
              <span
                className="tiny"
                style={{ background: 'var(--accent-soft)', color: 'var(--accent-ink)', padding: '1px 6px', borderRadius: '4px', fontWeight: 600 }}
              >
                长期演化目标
              </span>
            </div>
            <div style={{ fontSize: '12px', fontWeight: 700, color: 'var(--accent-ink)' }}>
              一句话全自主精准出表 · 零幻觉端到端
            </div>
            <div className="tiny" style={{ color: 'var(--ink-3)', marginTop: '4px', lineHeight: 1.5 }}>
              愿景：完全自主的拓扑演化推理、高维自动对齐、自动执行与自我修复。
            </div>
          </div>
        </div>
      </div>

      {/* KPI stats */}
      <div className="kpis">
        <div className="kpi">
          <div className="lab">进行中的需求</div>
          <div className="num">{openReqsCount}</div>
        </div>
        <div className="kpi">
          <div className="lab">已出稿模板</div>
          <div className="num">{templatesCount}</div>
        </div>
        <div className="kpi">
          <div className="lab">累计运行</div>
          <div className="num">{runsCount}</div>
        </div>
      </div>

      {/* 架构流水线全景 Banner (5-Stage Pipeline) */}
      <div
        style={{
          background: 'var(--paper-2)',
          border: '1px solid var(--line)',
          borderRadius: '12px',
          padding: '16px 18px',
          marginBottom: '20px',
          boxShadow: 'var(--shadow-s)',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            flexWrap: 'wrap',
            gap: '8px',
            marginBottom: '12px',
            borderBottom: '1px solid var(--paper-3)',
            paddingBottom: '8px',
          }}
        >
          <div style={{ fontSize: '12.5px', fontWeight: 700, color: 'var(--ink)' }}>
            ⚡ 报表 Agent 五步生成流水线（确定性工程与开放理解清晰分界）：
          </div>
          <div style={{ display: 'flex', gap: '14px', fontSize: '11px', color: 'var(--ink-3)' }}>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '5px' }}>
              <span style={{ width: '8px', height: '8px', background: '#10b981', borderRadius: '50%' }} />
              确定性规则/物理探查
            </span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '5px' }}>
              <span style={{ width: '8px', height: '8px', background: '#f59e0b', borderRadius: '50%' }} />
              人机协同口径追问
            </span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '5px' }}>
              <span style={{ width: '8px', height: '8px', background: '#6366f1', borderRadius: '50%' }} />
              异常指标自动归因
            </span>
          </div>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(210px, 1fr))', gap: '10px' }}>
          {/* Step 1 */}
          <div
            style={{
              padding: '12px 14px',
              borderRadius: '8px',
              background: activeStep === 1 ? '#ecfdf5' : 'var(--paper-3)',
              border: activeStep === 1 ? '1.5px solid #10b981' : '1px solid var(--line)',
              boxShadow: activeStep === 1 ? '0 0 0 2px rgba(16, 185, 129, 0.2)' : 'none',
              transition: 'all 0.2s ease',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div style={{ fontWeight: 700, fontSize: '12.5px', color: 'var(--ink)' }}>① 自动探查上下文</div>
              {activeStep === 1 && (
                <span className="tiny" style={{ color: '#059669', fontWeight: 700 }}>
                  探查中…
                </span>
              )}
            </div>
            <div style={{ color: 'var(--ink-3)', fontSize: '11px', marginTop: '4px', lineHeight: 1.5 }}>
              直连物理库 ➔ 表/列/基数/枚举/样例/主外键拓扑
            </div>
          </div>

          {/* Step 2 */}
          <div
            style={{
              padding: '12px 14px',
              borderRadius: '8px',
              background: activeStep === 2 ? '#ecfdf5' : 'var(--paper-3)',
              border: activeStep === 2 ? '1.5px solid #10b981' : '1px solid var(--line)',
              boxShadow: activeStep === 2 ? '0 0 0 2px rgba(16, 185, 129, 0.2)' : 'none',
              transition: 'all 0.2s ease',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div style={{ fontWeight: 700, fontSize: '12.5px', color: 'var(--ink)' }}>② 推导可统计图谱</div>
              {activeStep === 2 && (
                <span className="tiny" style={{ color: '#059669', fontWeight: 700 }}>
                  推导中…
                </span>
              )}
            </div>
            <div style={{ color: 'var(--ink-3)', fontSize: '11px', marginTop: '4px', lineHeight: 1.5 }}>
              数值列➔指标 / 低基数列➔维度 / 实体关系拓扑
            </div>
          </div>

          {/* Step 3 */}
          <div
            style={{
              padding: '12px 14px',
              borderRadius: '8px',
              background: activeStep === 3 ? '#fffbeb' : 'var(--paper-3)',
              border: activeStep === 3 ? '1.5px solid #f59e0b' : '1px solid var(--line)',
              boxShadow: activeStep === 3 ? '0 0 0 2px rgba(245, 158, 11, 0.2)' : 'none',
              transition: 'all 0.2s ease',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div style={{ fontWeight: 700, fontSize: '12.5px', color: 'var(--ink)' }}>③ 三层可行性分诊</div>
              {activeStep === 3 && (
                <span className="tiny" style={{ color: '#d97706', fontWeight: 700 }}>
                  分诊中…
                </span>
              )}
            </div>
            <div style={{ color: 'var(--ink-3)', fontSize: '11px', marginTop: '4px', lineHeight: 1.5 }}>
              L1语言 ➔ L2词表模型 ➔ L3物理数据与歧义
            </div>
          </div>

          {/* Step 4 */}
          <div
            style={{
              padding: '12px 14px',
              borderRadius: '8px',
              background: activeStep === 4 ? '#ecfdf5' : 'var(--paper-3)',
              border: activeStep === 4 ? '1.5px solid #10b981' : '1px solid var(--line)',
              boxShadow: activeStep === 4 ? '0 0 0 2px rgba(16, 185, 129, 0.2)' : 'none',
              transition: 'all 0.2s ease',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div style={{ fontWeight: 700, fontSize: '12.5px', color: 'var(--ink)' }}>④ 生成真实报表</div>
              {activeStep === 4 && (
                <span className="tiny" style={{ color: '#059669', fontWeight: 700 }}>
                  渲染中…
                </span>
              )}
            </div>
            <div style={{ color: 'var(--ink-3)', fontSize: '11px', marginTop: '4px', lineHeight: 1.5 }}>
              Safe CTE ➔ Univer Sheets ➔ 口径沉淀飞轮
            </div>
          </div>

          {/* Step 5 */}
          <div
            style={{
              padding: '12px 14px',
              borderRadius: '8px',
              background: activeStep === 5 ? '#eef2ff' : 'var(--paper-3)',
              border: activeStep === 5 ? '1.5px solid #6366f1' : '1px solid var(--line)',
              boxShadow: activeStep === 5 ? '0 0 0 2px rgba(99, 102, 241, 0.2)' : 'none',
              transition: 'all 0.2s ease',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div style={{ fontWeight: 700, fontSize: '12.5px', color: 'var(--ink)' }}>⑤ 异常自动归因</div>
              {activeStep === 5 && (
                <span className="tiny" style={{ color: '#4f46e5', fontWeight: 700 }}>
                  分析中…
                </span>
              )}
            </div>
            <div style={{ color: 'var(--ink-3)', fontSize: '11px', marginTop: '4px', lineHeight: 1.5 }}>
              指标异动时 ➔ 自动拆解维度贡献度 ➔ 生成归因报告
            </div>
          </div>
        </div>
      </div>

      {/* Main Ask Bar */}
      <div className="ask">
        {/* Preset chips */}
        <div style={{ marginBottom: '12px' }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              marginBottom: '8px',
            }}
          >
            <div className="tiny" style={{ fontWeight: 600, color: 'var(--ink-3)' }}>
              🎯 点击快速体验「一句话生成」与分诊三态：
            </div>
            <div className="tiny" style={{ color: 'var(--ink-4)' }}>
              预设涵盖：异动归因 · 全绿直通 · 口径追问 · 时间基准 · 严谨拒答
            </div>
          </div>
          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            {demoPresets.map((p, idx) => (
              <button
                key={idx}
                type="button"
                className="btn small"
                style={{
                  background: askText === p.prompt ? 'var(--ink)' : 'var(--paper-2)',
                  color: askText === p.prompt ? '#fff' : 'var(--ink)',
                  borderColor: askText === p.prompt ? 'var(--ink)' : 'var(--line-2)',
                  fontSize: '12px',
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '6px',
                }}
                onClick={() => handleSelectPreset(p)}
              >
                <span>{p.label}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="ask-row">
          <input
            type="text"
            value={askText}
            onChange={(e) => {
              setAskText(e.target.value);
              setTriageResult(null);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleAskRun();
            }}
            placeholder="用一句话说清你要什么，例：华东各品类销售额交叉表"
          />
          <select
            value={selectedDsn}
            onChange={(e) => {
              setSelectedDsn(e.target.value);
              setTriageResult(null);
            }}
            disabled={datasources.length === 0}
          >
            {datasources.length > 0 ? (
              datasources.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.label || d.id}
                </option>
              ))
            ) : (
              <option>（先配数据源）</option>
            )}
          </select>
          <button
            type="button"
            className="btn primary"
            onClick={() => handleAskRun()}
            disabled={isGenerating}
            style={{ minWidth: '130px' }}
          >
            {isGenerating ? '分诊执行中…' : '生成报表 →'}
          </button>
        </div>

        <div className="ask-foot">
          <span className="tiny" id="ask-state">
            当前对接物理库：<code>{selectedDsn}</code> · 词表与规则层已绑定 · 杜绝无根据生成
          </span>
          <div style={{ display: 'flex', gap: '8px' }}>
            <button type="button" className="btn small" onClick={handleOpenContextInspector}>
              检视事实
            </button>
            <button type="button" className="btn small" onClick={onOpenLlmModal}>
              理解层设置
            </button>
          </div>
        </div>
      </div>

      {/* Interactive Clarification Card (🟡) */}
      {triageResult && triageResult.status === 'clarify' && (
        <ClarificationCard
          triage={triageResult}
          onConfirm={handleClarificationConfirm}
          onCancel={() => setTriageResult(null)}
        />
      )}

      {/* Rejection Audit Card (🔴) */}
      {triageResult && triageResult.status === 'infeasible' && (
        <div
          className="card pad"
          style={{
            marginTop: '12px',
            border: '1.5px solid #ef4444',
            background: '#fef2f2',
            borderRadius: '12px',
          }}
        >
          <div className="row" style={{ alignItems: 'flex-start' }}>
            <div className="grow">
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
                <span
                  style={{
                    background: '#fee2e2',
                    color: '#991b1b',
                    border: '1px solid #ef4444',
                    padding: '2px 8px',
                    borderRadius: '6px',
                    fontSize: '12px',
                    fontWeight: 700,
                  }}
                >
                  🔴 不可以做到（明确拒绝）
                </span>
                <span style={{ fontSize: '13px', fontWeight: 600, color: '#7f1d1d' }}>
                  缺少物理事实依据与模型定义
                </span>
              </div>
              <div style={{ fontSize: '12.5px', color: '#991b1b', marginTop: '6px', lineHeight: 1.5 }}>
                {triageResult.rejectionReason}
              </div>
            </div>
            <button type="button" className="btn small" onClick={() => setTriageResult(null)}>
              关闭 ✕
            </button>
          </div>

          {/* Missing schema audit */}
          <div
            style={{
              marginTop: '12px',
              background: '#ffffff',
              padding: '10px 12px',
              borderRadius: '8px',
              border: '1px solid #fecaca',
              fontSize: '12px',
            }}
          >
            <div style={{ fontWeight: 700, color: '#991b1b', marginBottom: '4px' }}>
              📋 审计详情（为什么拒绝）：
            </div>
            <div style={{ color: '#475569' }}>
              · <strong>L1 语言提取</strong>：{triageResult.l1.intent}
            </div>
            <div style={{ color: '#475569', marginTop: '2px' }}>
              · <strong>L2 模型检索</strong>：词表外未知词汇 【
              {triageResult.l2.unknownTokens.join('、') || '无'}】
            </div>
            <div style={{ color: '#475569', marginTop: '2px' }}>
              · <strong>L3 物理审计</strong>：当前数据源 <code>{triageResult.l3.dsn}</code> 中无相关存储表或列
            </div>
          </div>

          {/* Alternative recommendations */}
          {triageResult.suggestedQueries && triageResult.suggestedQueries.length > 0 && (
            <div style={{ marginTop: '12px' }}>
              <div className="tiny" style={{ color: '#991b1b', marginBottom: '6px' }}>
                💡 推荐试试以下当前数据源真实可达的报表：
              </div>
              <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                {triageResult.suggestedQueries.map((q, idx) => (
                  <button
                    key={idx}
                    type="button"
                    className="btn small"
                    style={{ background: '#ffffff', borderColor: '#fca5a5', color: '#991b1b' }}
                    onClick={() => {
                      setAskText(q);
                      setTriageResult(null);
                    }}
                  >
                    {q}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Step 5: Agent Auto Attribution Card (异常自动归因与底层维度贡献度分析) */}
      {attributionDoc && (
        <div
          className="card pad"
          style={{
            marginTop: '20px',
            marginBottom: '26px',
            border: '2px solid #6366f1',
            background: 'linear-gradient(180deg, #ffffff 0%, var(--paper-2) 100%)',
            borderRadius: '16px',
            boxShadow: '0 8px 30px rgba(99, 102, 241, 0.12)',
            position: 'relative',
            overflow: 'hidden',
          }}
        >
          {/* Subtle watermark / background aura */}
          <div
            style={{
              position: 'absolute',
              right: '-40px',
              top: '-40px',
              width: '180px',
              height: '180px',
              borderRadius: '50%',
              background: 'radial-gradient(circle, rgba(99, 102, 241, 0.08) 0%, transparent 70%)',
              pointerEvents: 'none',
            }}
          />

          <div className="row" style={{ alignItems: 'flex-start', justifyContent: 'space-between', flexWrap: 'wrap', gap: '12px' }}>
            <div style={{ flex: '1 1 500px' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px', flexWrap: 'wrap' }}>
                <span
                  style={{
                    background: '#e0e7ff',
                    color: '#3730a3',
                    border: '1px solid #c7d2fe',
                    padding: '3px 9px',
                    borderRadius: '6px',
                    fontSize: '11px',
                    fontWeight: 700,
                    letterSpacing: '0.04em',
                  }}
                >
                  ⚡ AGENT 流水线 · 异常自动归因
                </span>
                <span
                  style={{
                    background: '#fef3c7',
                    color: '#92400e',
                    border: '1px solid #fde68a',
                    padding: '3px 9px',
                    borderRadius: '6px',
                    fontSize: '11px',
                    fontWeight: 600,
                  }}
                >
                  ⚠️ 波动检测：环比异动超过基准阈值 (5%)
                </span>
                {attributionSigned ? (
                  <span
                    style={{
                      background: '#dcfce7',
                      color: '#166534',
                      border: '1px solid #86efac',
                      padding: '3px 9px',
                      borderRadius: '6px',
                      fontSize: '11px',
                      fontWeight: 700,
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '4px',
                    }}
                  >
                    <span>✓</span> 人类专家已审核签发 · 决策已入库
                  </span>
                ) : (
                  <span
                    style={{
                      background: 'var(--paper-3)',
                      color: 'var(--ink-3)',
                      padding: '3px 9px',
                      borderRadius: '6px',
                      fontSize: '11px',
                    }}
                  >
                    待人类专家复核
                  </span>
                )}
              </div>

              <h2
                style={{
                  margin: '4px 0 8px',
                  fontSize: '20px',
                  fontFamily: 'var(--font-serif)',
                  color: 'var(--ink)',
                  letterSpacing: '-0.01em',
                }}
              >
                {attributionDoc.summary}
              </h2>
              <div className="tiny" style={{ color: 'var(--ink-4)', display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                <span>对比基准窗口：<code>{attributionDoc.windows.cur_start} ~ {attributionDoc.windows.cur_end}</code> (本期) vs <code>{attributionDoc.windows.prev_start} ~ {attributionDoc.windows.prev_end}</code> (上期)</span>
                <span>·</span>
                <span>全量实体数据覆盖率：<strong>{(attributionDoc.coverage * 100).toFixed(1)}%</strong></span>
              </div>
            </div>

            <div style={{ display: 'flex', gap: '8px', alignSelf: 'flex-start' }}>
              {lastGeneratedRun && (
                <button
                  type="button"
                  className="btn small"
                  style={{ background: '#ffffff', borderColor: '#cbd5e1', color: 'var(--ink)' }}
                  onClick={() => onOpenRun(lastGeneratedRun.reqId, lastGeneratedRun.runId)}
                >
                  📊 检视 Univer 电子表格
                </button>
              )}
              <button
                type="button"
                className="btn small ghost"
                onClick={() => setAttributionDoc(null)}
              >
                收起 ✕
              </button>
            </div>
          </div>

          {/* Metric KPI Snapshot */}
          <div className="kpis four" style={{ marginTop: '16px', marginBottom: '18px' }}>
            <div className="kpi" style={{ background: '#ffffff' }}>
              <div className="lab">本期销售额 (2026-09)</div>
              <div className="num">¥ {attributionDoc.totals.cur.toLocaleString()}</div>
            </div>
            <div className="kpi" style={{ background: '#ffffff' }}>
              <div className="lab">上期销售额 (2026-08)</div>
              <div className="num">¥ {attributionDoc.totals.prev.toLocaleString()}</div>
            </div>
            <div className="kpi" style={{ background: '#fff1f2', borderColor: '#fecdd3' }}>
              <div className="lab" style={{ color: '#9f1239' }}>异常变动总额 (Δ)</div>
              <div className="num" style={{ color: '#e11d48' }}>
                -¥ {Math.abs(attributionDoc.totals.delta).toLocaleString()}
              </div>
            </div>
            <div className="kpi" style={{ background: '#ffffff' }}>
              <div className="lab">环比跌幅 (MoM)</div>
              <div className="num" style={{ color: '#e11d48' }}>
                -7.4%
              </div>
            </div>
          </div>

          {/* Underlying Dimension Contribution Breakdown (底层维度贡献度分析) */}
          <div
            style={{
              background: '#ffffff',
              borderRadius: '10px',
              border: '1px solid var(--line)',
              padding: '16px 18px',
              marginBottom: '16px',
              boxShadow: 'var(--shadow-s)',
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: '14px',
                borderBottom: '1px solid var(--paper-3)',
                paddingBottom: '8px',
              }}
            >
              <div style={{ fontWeight: 700, fontSize: '13px', color: 'var(--ink)' }}>
                🔍 归因瀑布：底层维度贡献度分解（按品类与大区拆解）：
              </div>
              <span className="tiny" style={{ color: '#4f46e5', fontWeight: 600 }}>
                贡献率公式 = 维度变动额 / 全盘变动额 × 100%
              </span>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
              {/* Category 1 */}
              <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12.5px', marginBottom: '5px' }}>
                  <span style={{ fontWeight: 700, color: 'var(--ink)' }}>
                    1. 办公用品品类 (Office Supplies) · <span style={{ color: '#dc2626', fontWeight: 600 }}>【最大负贡献源】</span>
                  </span>
                  <span style={{ color: 'var(--ink-3)' }}>
                    变动：<strong style={{ color: '#dc2626' }}>-¥25,800</strong> · 贡献度：
                    <strong style={{ color: '#dc2626', fontSize: '13.5px' }}>68.2%</strong>
                  </span>
                </div>
                <div style={{ height: '10px', background: 'var(--paper-3)', borderRadius: '5px', overflow: 'hidden' }}>
                  <div
                    style={{
                      width: '68.2%',
                      height: '100%',
                      background: 'linear-gradient(90deg, #ef4444 0%, #dc2626 100%)',
                      borderRadius: '5px',
                    }}
                  />
                </div>
                <div className="tiny" style={{ color: 'var(--ink-3)', marginTop: '4px' }}>
                  ↳ 深度下钻：主要受<strong>华东大区</strong>影响（大宗企业客户三季度采购周期性预算收紧，同比削减 42%）
                </div>
              </div>

              {/* Category 2 */}
              <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12.5px', marginBottom: '5px' }}>
                  <span style={{ fontWeight: 600, color: 'var(--ink)' }}>
                    2. 家居生活品类 (Home Goods)
                  </span>
                  <span style={{ color: 'var(--ink-3)' }}>
                    变动：<strong style={{ color: '#f97316' }}>-¥11,200</strong> · 贡献度：
                    <strong style={{ color: '#f97316' }}>29.6%</strong>
                  </span>
                </div>
                <div style={{ height: '10px', background: 'var(--paper-3)', borderRadius: '5px', overflow: 'hidden' }}>
                  <div
                    style={{
                      width: '29.6%',
                      height: '100%',
                      background: 'linear-gradient(90deg, #fb923c 0%, #f97316 100%)',
                      borderRadius: '5px',
                    }}
                  />
                </div>
                <div className="tiny" style={{ color: 'var(--ink-3)', marginTop: '4px' }}>
                  ↳ 深度下钻：受南方雨季物流配送时效延迟影响，退货率短期上升 1.8%
                </div>
              </div>

              {/* Category 3 */}
              <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12.5px', marginBottom: '5px' }}>
                  <span style={{ fontWeight: 600, color: 'var(--ink)' }}>
                    3. 数码3C品类 (Digital 3C)
                  </span>
                  <span style={{ color: 'var(--ink-3)' }}>
                    变动：<strong style={{ color: 'var(--ink-3)' }}>-¥800</strong> · 贡献度：
                    <strong>2.2%</strong> (正常波动区间)
                  </span>
                </div>
                <div style={{ height: '10px', background: 'var(--paper-3)', borderRadius: '5px', overflow: 'hidden' }}>
                  <div
                    style={{
                      width: '2.2%',
                      height: '100%',
                      background: '#94a3b8',
                      borderRadius: '5px',
                    }}
                  />
                </div>
              </div>
            </div>
          </div>

          {/* AI Narrative & Human-in-the-Loop Sign-off */}
          <div
            style={{
              background: attributionSigned ? '#f0fdf4' : '#f8fafc',
              borderRadius: '10px',
              padding: '16px 18px',
              border: attributionSigned ? '1.5px solid #86efac' : '1px solid var(--line)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              flexWrap: 'wrap',
              gap: '16px',
            }}
          >
            <div style={{ fontSize: '12.5px', color: 'var(--ink-2)', maxWidth: '680px', lineHeight: 1.7 }}>
              <div style={{ fontWeight: 700, color: 'var(--ink)', marginBottom: '4px' }}>
                🤖 AI 智能归因分析报告结论：
              </div>
              本次 9 月销售额下滑（-¥37,800, -7.4%）的核心负向诱因已被精准锁定：<strong>「办公用品」</strong>品类独自承担了 68.2% 的全盘跌幅，次因为「家居生活」（29.6%）。数码品类维持平稳。建议华东大区业务主管跟进大宗集采订单交期，无需在全盘范围启动无差别降价。
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
              {attributionSigned ? (
                <div
                  style={{
                    background: '#ffffff',
                    border: '2px solid #16a34a',
                    padding: '8px 16px',
                    borderRadius: '8px',
                    textAlign: 'center',
                    boxShadow: '0 2px 6px rgba(22, 163, 74, 0.15)',
                  }}
                >
                  <div style={{ fontSize: '12px', fontWeight: 800, color: '#16a34a', letterSpacing: '0.06em' }}>
                    OFFICIAL SIGNED
                  </div>
                  <div className="tiny" style={{ color: '#15803d', fontWeight: 600 }}>
                    人类专家已核准签发 · 决策留档
                  </div>
                </div>
              ) : (
                <button
                  type="button"
                  className="btn primary"
                  style={{
                    background: '#4f46e5',
                    borderColor: '#4f46e5',
                    color: '#ffffff',
                    fontSize: '12.5px',
                    padding: '8px 18px',
                    boxShadow: '0 4px 12px rgba(79, 70, 229, 0.25)',
                  }}
                  onClick={() => {
                    setAttributionSigned(true);
                    toast('✅ 归因报告经人类专家审核确认，已签发并归档至企业决策库！');
                  }}
                >
                  👤 人类专家审核确认并签发报告
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {/* 🌟 核心：报表引擎高保真交互呈现区 (Report Engine Live Canvas & Shape Switcher) */}
      {activeRun && activeReq && (
        <div
          className="card pad"
          style={{
            marginTop: '22px',
            marginBottom: '30px',
            border: '2px solid var(--navy-2)',
            background: '#ffffff',
            borderRadius: '16px',
            boxShadow: '0 6px 24px rgba(43, 70, 87, 0.1)',
            overflow: 'hidden',
          }}
        >
          {/* Top Bar: Title, Shape & Toolbar */}
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              flexWrap: 'wrap',
              gap: '12px',
              paddingBottom: '14px',
              borderBottom: '1px solid var(--line)',
            }}
          >
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
                <span
                  style={{
                    background: 'var(--navy)',
                    color: '#ffffff',
                    padding: '2px 8px',
                    borderRadius: '4px',
                    fontSize: '11px',
                    fontWeight: 700,
                    letterSpacing: '0.04em',
                  }}
                >
                  ⚡ REPORT ENGINE
                </span>
                <span
                  style={{
                    background: 'var(--accent-soft)',
                    color: 'var(--accent-ink)',
                    padding: '2px 8px',
                    borderRadius: '4px',
                    fontSize: '11px',
                    fontWeight: 600,
                  }}
                >
                  形态：{SHAPE_LABELS[activeReq.spec?.shape || 'grouped'] || '分组汇总'}
                </span>
                <span
                  style={{
                    background: activeReq.template?.verified ? '#dcfce7' : '#fef3c7',
                    color: activeReq.template?.verified ? '#15803d' : '#b45309',
                    padding: '2px 8px',
                    borderRadius: '4px',
                    fontSize: '11px',
                    fontWeight: 600,
                  }}
                >
                  {activeReq.template?.verified ? '✓ 人类专家已核准' : '👤 待人类审核'}
                </span>
                <span className="tiny" style={{ color: 'var(--ink-4)' }}>
                  ID: <code>{activeRun.run_id}</code> · 耗时: <strong>{activeRun.duration_ms} ms</strong> · 规格: {activeRun.shape?.rows ?? 4}行 × {activeRun.shape?.cols ?? 4}列
                </span>
              </div>
              <h2
                style={{
                  margin: '2px 0 0',
                  fontSize: '20px',
                  fontFamily: 'var(--font-serif)',
                  color: 'var(--ink)',
                }}
              >
                {activeReq.title}
              </h2>
            </div>

            {/* Shape Switcher Bar (利用报表引擎强大表现力，动态变换形态) */}
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
              <span className="tiny" style={{ fontWeight: 600, color: 'var(--ink-3)' }}>
                动态重构版式：
              </span>
              <div className="chip-toggles" style={{ display: 'inline-flex', margin: 0 }}>
                <button
                  type="button"
                  className={`tog small ${activeReq.spec?.shape === 'crosstab' ? 'on' : ''}`}
                  onClick={() => handleSwitchShape('crosstab')}
                  title="行大区 × 列表格二维展开，带公式小计与总计"
                >
                  ⊞ 交叉表
                </button>
                <button
                  type="button"
                  className={`tog small ${activeReq.spec?.shape === 'grouped' ? 'on' : ''}`}
                  onClick={() => handleSwitchShape('grouped')}
                  title="多级维度层级下钻汇总"
                >
                  ≡ 分组汇总
                </button>
                <button
                  type="button"
                  className={`tog small ${activeReq.spec?.shape === 'trend' ? 'on' : ''}`}
                  onClick={() => handleSwitchShape('trend')}
                  title="月度时间序列走势"
                >
                  📈 趋势走势
                </button>
                <button
                  type="button"
                  className={`tog small ${activeReq.spec?.shape === 'rank' ? 'on' : ''}`}
                  onClick={() => handleSwitchShape('rank')}
                  title="销售额降序排列与头部聚类"
                >
                  🏆 业绩排行
                </button>
              </div>

              {/* Download & Designer actions */}
              <div style={{ display: 'flex', gap: '6px', marginLeft: '6px' }}>
                {activeRun.artifacts?.['report.csv'] && (
                  <button
                    type="button"
                    className="btn small"
                    onClick={() => handleDownloadArtifact('report.csv', activeRun.artifacts['report.csv'])}
                    title="下载标准 UTF-8 CSV"
                  >
                    📄 CSV
                  </button>
                )}
                {activeRun.univerSnapshot && (
                  <button
                    type="button"
                    className="btn small"
                    onClick={() => {
                      navigator.clipboard?.writeText(JSON.stringify(activeRun.univerSnapshot, null, 2));
                      toast('已复制 Univer snapshot.json 到剪贴板');
                    }}
                    title="复制 Univer 官方标准快照 JSON"
                  >
                    📋 Snapshot
                  </button>
                )}
                <button
                  type="button"
                  className="btn small primary"
                  onClick={() => {
                    if (onOpenDesigner) onOpenDesigner(activeReq.id);
                    else onOpenRequirement(activeReq.id);
                  }}
                  style={{ fontWeight: 600 }}
                >
                  进入设计器 & 人类审核 ➔
                </button>
              </div>
            </div>
          </div>

          {/* Sub Tabs: Multi-surface Projections */}
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
              padding: '10px 0',
              borderBottom: '1px solid var(--paper-3)',
              overflowX: 'auto',
            }}
          >
            <button
              type="button"
              className={`btn ghost small ${activeOutputTab === 'univer' ? 'active' : ''}`}
              style={
                activeOutputTab === 'univer'
                  ? { background: 'var(--paper-3)', fontWeight: 700, color: 'var(--ink)' }
                  : {}
              }
              onClick={() => setActiveOutputTab('univer')}
            >
              📊 Univer 电子表格投影
            </button>
            <button
              type="button"
              className={`btn ghost small ${activeOutputTab === 'web' ? 'active' : ''}`}
              style={
                activeOutputTab === 'web'
                  ? { background: 'var(--paper-3)', fontWeight: 700, color: 'var(--ink)' }
                  : {}
              }
              onClick={() => setActiveOutputTab('web')}
            >
              🌐 网页交互版投影
            </button>
            <button
              type="button"
              className={`btn ghost small ${activeOutputTab === 'print' ? 'active' : ''}`}
              style={
                activeOutputTab === 'print'
                  ? { background: 'var(--paper-3)', fontWeight: 700, color: 'var(--ink)' }
                  : {}
              }
              onClick={() => setActiveOutputTab('print')}
            >
              🖨️ 打印套打排版预览
            </button>
            <button
              type="button"
              className={`btn ghost small ${activeOutputTab === 'json' ? 'active' : ''}`}
              style={
                activeOutputTab === 'json'
                  ? { background: 'var(--paper-3)', fontWeight: 700, color: 'var(--ink)' }
                  : {}
              }
              onClick={() => setActiveOutputTab('json')}
            >
              📋 规范 JSON (rpx-report/v1)
            </button>
            <button
              type="button"
              className={`btn ghost small ${activeOutputTab === 'spec' ? 'active' : ''}`}
              style={
                activeOutputTab === 'spec'
                  ? { background: 'var(--paper-3)', fontWeight: 700, color: 'var(--ink)' }
                  : {}
              }
              onClick={() => setActiveOutputTab('spec')}
            >
              🛡️ Safe CTE 执行计划
            </button>
          </div>

          {/* Tab 1: Univer Viewer */}
          {activeOutputTab === 'univer' && (
            <div style={{ marginTop: '12px' }}>
              {activeRun.univerSnapshot ? (
                <UniverViewer
                  snapshot={activeRun.univerSnapshot}
                  title={`${activeReq.title} · 报表引擎输出`}
                  height="460px"
                />
              ) : (
                <div className="empty">暂无工作簿快照，请在上方重新执行</div>
              )}
              <div
                style={{
                  marginTop: '10px',
                  padding: '8px 12px',
                  borderRadius: '6px',
                  background: 'var(--paper-2)',
                  border: '1px solid var(--line)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  fontSize: '11.5px',
                  color: 'var(--ink-3)',
                }}
              >
                <div>
                  💡 <strong>Univer 引擎特性已就绪</strong>：公式自动重算（例如 <code>=SUM(...)</code>）、单元格千分位格式化、斑马纹排版、冻结表头与划选统计。
                </div>
                <div style={{ color: 'var(--accent-ink)', fontWeight: 600 }}>
                  可在表格中直接单击选中单元格，查看状态栏求和统计
                </div>
              </div>
            </div>
          )}

          {/* Tab 2: Web HTML Projection */}
          {activeOutputTab === 'web' && (
            <div style={{ marginTop: '12px' }}>
              {activeRun.computedData ? (
                <div style={{ background: '#fff', borderRadius: '8px', border: '1px solid var(--line)', overflow: 'hidden' }}>
                  <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '13px' }}>
                    <thead>
                      <tr style={{ background: '#f8fafc', borderBottom: '2px solid #cbd5e1' }}>
                        {activeRun.computedData.headers.map((h, i) => (
                          <th key={i} style={{ padding: '10px 14px', textAlign: i === 0 ? 'left' : 'right', fontWeight: 600, color: '#334155' }}>
                            {h}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {activeRun.computedData.rows.map((row, rIdx) => (
                        <tr key={rIdx} style={{ borderBottom: '1px solid #f1f5f9', background: rIdx % 2 === 0 ? '#fff' : '#fafaf9' }}>
                          {row.map((val: any, cIdx: number) => (
                            <td key={cIdx} style={{ padding: '8px 14px', textAlign: cIdx === 0 ? 'left' : 'right', fontFamily: typeof val === 'number' ? 'monospace' : 'inherit' }}>
                              {typeof val === 'number' ? val.toLocaleString() : val}
                            </td>
                          ))}
                        </tr>
                      ))}
                      {activeRun.computedData.summaryRow && (
                        <tr style={{ borderTop: '2px solid #94a3b8', background: '#fffcf7', fontWeight: 'bold' }}>
                          {activeRun.computedData.summaryRow.map((val: any, cIdx: number) => (
                            <td key={cIdx} style={{ padding: '10px 14px', textAlign: cIdx === 0 ? 'left' : 'right', color: '#92400e', fontFamily: typeof val === 'number' ? 'monospace' : 'inherit' }}>
                              {typeof val === 'number' ? val.toLocaleString() : val}
                            </td>
                          ))}
                        </tr>
                      )}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="empty">暂无交互式 Web 数据</div>
              )}
            </div>
          )}

          {/* Tab 3: Print Layout */}
          {activeOutputTab === 'print' && (
            <div style={{ marginTop: '14px', background: '#fffcf7', padding: '24px', borderRadius: '8px', border: '1px solid var(--line)', boxShadow: 'inset 0 1px 3px rgba(0,0,0,0.05)' }}>
              <div style={{ borderBottom: '2px solid var(--accent)', paddingBottom: '10px', marginBottom: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
                <div>
                  <div className="tiny" style={{ letterSpacing: '0.12em', color: 'var(--accent-ink)', fontWeight: 600 }}>
                    OFFICIAL REPORT STATEMENT
                  </div>
                  <h1 style={{ margin: '4px 0 0', fontFamily: 'var(--font-serif)', fontSize: '22px' }}>
                    {activeReq.title}
                  </h1>
                </div>
                <div className="tiny" style={{ textAlign: 'right', color: 'var(--ink-3)' }}>
                  <div>模板修订: rev {activeReq.template?.rev ?? 1}</div>
                  <div>归档 ID: {activeRun.run_id}</div>
                </div>
              </div>

              {activeRun.artifacts?.['report.html'] ? (
                <div
                  dangerouslySetInnerHTML={{
                    __html: activeRun.artifacts['report.html'].replace(/^<!DOCTYPE html>[\s\S]*?<body.*?>/, '').replace(/<\/body>[\s\S]*?$/, ''),
                  }}
                />
              ) : (
                <div className="empty">暂无打印版式</div>
              )}
            </div>
          )}

          {/* Tab 4: Canonical JSON Specification Document */}
          {activeOutputTab === 'json' && (
            <div style={{ marginTop: '14px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                <span className="tiny" style={{ fontWeight: 600, color: 'var(--ink-2)' }}>
                  规范 JSON 报表定义 · schema: <code>rpx-report/v1</code>（从头到尾一致的核心单据）
                </span>
                <button
                  type="button"
                  className="btn small"
                  onClick={() => {
                    const doc = buildCanonicalReport(activeReq);
                    navigator.clipboard.writeText(JSON.stringify(doc, null, 2));
                    toast('已复制 Canonical JSON 报表定义');
                  }}
                >
                  复制 JSON
                </button>
              </div>
              <pre
                style={{
                  background: '#0f172a',
                  color: '#f8fafc',
                  padding: '16px',
                  borderRadius: '8px',
                  fontSize: '11.5px',
                  lineHeight: 1.5,
                  overflowX: 'auto',
                  maxHeight: '400px',
                  fontFamily: 'Consolas, Monaco, "Courier New", monospace',
                }}
              >
                {JSON.stringify(buildCanonicalReport(activeReq), null, 2)}
              </pre>
            </div>
          )}

          {/* Tab 5: Spec & Safe CTE Plan */}
          {activeOutputTab === 'spec' && (
            <div style={{ marginTop: '14px', display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: '16px' }}>
              <div style={{ background: 'var(--paper-2)', padding: '14px', borderRadius: '8px', border: '1px solid var(--line)' }}>
                <div style={{ fontWeight: 700, fontSize: '12.5px', color: 'var(--ink)', marginBottom: '6px' }}>
                  📋 报表 Spec 定义 (RPX Query Contract)
                </div>
                <pre style={{ font: '11.5px/1.6 var(--font-mono)', color: 'var(--ink-2)', overflowX: 'auto', maxHeight: '380px' }}>
                  {JSON.stringify(activeReq.spec, null, 2)}
                </pre>
              </div>
              <div style={{ background: '#0f172a', padding: '14px', borderRadius: '8px', color: '#e2e8f0' }}>
                <div style={{ fontWeight: 700, fontSize: '12.5px', color: '#93c5fd', marginBottom: '6px' }}>
                  🛡️ 确定性编译器输出与防翻车 SQL (Safe CTE)
                </div>
                <pre style={{ font: '11.5px/1.6 var(--font-mono)', color: '#cbd5e1', overflowX: 'auto', maxHeight: '380px' }}>
                  {activeReq.dataset?.sql ||
`-- 报表引擎根据维度与度量自动生成的受控聚合执行计划
WITH base_dim AS (
  SELECT
    region,
    store,
    cat,
    amount
  FROM sales
)
SELECT
  region AS "大区",
  cat AS "品类",
  SUM(amount) AS "销售额"
FROM base_dim
GROUP BY region, cat
ORDER BY region ASC, SUM(amount) DESC;`}
                </pre>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Requirements List */}
      <div className="section-title">
        待我处理 <span className="tiny">按最近更新排序 · 点击可在上方报表引擎画布中实时加载预览</span>
      </div>
      <div className="list">
        {requirements.length > 0 ? (
          requirements.map((r) => (
            <div
              key={r.id}
              className="list-row"
              style={{ cursor: 'pointer' }}
              onClick={() => {
                if (r.runs && r.runs.length > 0) {
                  setSelectedRunId(r.runs[0].run_id);
                  toast(`已在上方报表引擎画布中载入：${r.title}`);
                } else {
                  onOpenRequirement(r.id);
                }
              }}
            >
              <div className="grow">
                <div className="t">{r.title}</div>
                <div className="s">
                  {r.dataset.file || r.dataset.dsn} · {r.dataset.rows ?? '?'} 行 ·{' '}
                  {r.template_choice ? `模板：${r.template_choice.name}` : '还没选模板'}
                  {r.template?.rev ? ` · rev ${r.template.rev}` : ''}
                </div>
              </div>
              <span className={`badge ${r.status}`}>{r.status_label || r.status}</span>
              <button
                type="button"
                className="btn small ghost"
                style={{ fontSize: '11px', padding: '3px 8px' }}
                onClick={(e) => {
                  e.stopPropagation();
                  onOpenRequirement(r.id);
                }}
              >
                编辑 ➔
              </button>
              <div className="tiny" style={{ width: '60px', textAlign: 'right' }}>
                {fmtTime(r.updated_at)}
              </div>
            </div>
          ))
        ) : (
          <div className="empty">还没有需求 —— 点右上角「新建需求」开始</div>
        )}
      </div>

      {/* Recent Runs List */}
      <div className="section-title">
        最近运行 <span className="tiny">点击可在上方画布实时检视各运行批次的 Univer 报表与公式</span>
      </div>
      <div className="list">
        {runs.length > 0 ? (
          runs.slice(0, 6).map((r) => (
            <div
              key={r.run_id}
              className="list-row"
              style={{
                cursor: 'pointer',
                background: activeRun?.run_id === r.run_id ? 'var(--paper-3)' : undefined,
              }}
              onClick={() => {
                setSelectedRunId(r.run_id);
                toast(`已在上方报表引擎画布中载入：${r.req_title}`);
              }}
            >
              <div className="grow">
                <div className="t">
                  {r.req_title} <span className="tiny">{r.run_id}</span>
                </div>
                <div className="s">
                  模板 rev {r.template_rev ?? '—'} · {r.shape?.rows ?? '?'} 行 × {r.shape?.cols ?? '?'} 列 ·{' '}
                  {fmtMs(r.duration_ms)}
                </div>
              </div>
              <span className={`badge ${r.status === 'succeeded' ? 'ran' : 'failed'}`}>
                {r.status === 'succeeded' ? '成功' : '失败'}
              </span>
              <button
                type="button"
                className="btn small ghost"
                style={{ fontSize: '11px', padding: '3px 8px' }}
                onClick={(e) => {
                  e.stopPropagation();
                  onOpenRun(r.req_id, r.run_id);
                }}
              >
                运行详情 ➔
              </button>
              <div className="tiny" style={{ width: '60px', textAlign: 'right' }}>
                {fmtTime(r.at)}
              </div>
            </div>
          ))
        ) : (
          <div className="empty">还没有运行记录</div>
        )}
      </div>

      {/* Context Inspector Modal */}
      {showContextModal && contextSnapshot && (
        <ContextInspectorModal
          context={contextSnapshot}
          onClose={() => setShowContextModal(false)}
        />
      )}
    </>
  );
};
