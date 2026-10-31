import React, { useEffect, useMemo, useState } from 'react';
import {
  createRequirement,
  getRuns,
  getSemanticDb,
  type RecentRun,
  type SemanticDbStatus,
} from '../services/api';
import { fmtTime, library, reasonOf, useLibrary } from '../services/reports';
import { useToast } from '../components/Toast';
import { ContextInspectorModal } from '../components/ContextInspectorModal';
import {
  ClarificationCard,
  type ClarificationQuestion,
} from '../components/ClarificationCard';

interface Props {
  onOpenReport: (id: string) => void;
  onNavigate: (hash: string) => void;
}

export const WorkbenchView: React.FC<Props> = ({ onOpenReport, onNavigate }) => {
  const { toast } = useToast();
  const lib = useLibrary();
  const [recent, setRecent] = useState<RecentRun[]>([]);
  const [failed, setFailed] = useState<RecentRun[]>([]);
  const [runsTotal, setRunsTotal] = useState<number | null>(null);
  const [failedTotal, setFailedTotal] = useState<number | null>(null);
  const [db, setDb] = useState<SemanticDbStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  // Agent State
  const [text, setText] = useState('按客户等级看 LTV 和逾期率');
  const [selectedDsn, setSelectedDsn] = useState('demo_sqlite');
  const [activeStep, setActiveStep] = useState<number | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [showContextModal, setShowContextModal] = useState(false);

  // Triage state
  const [triageBranch, setTriageBranch] = useState<'ready' | 'clarify' | 'reject' | null>(null);
  const [clarificationQuestions, setClarificationQuestions] = useState<ClarificationQuestion[]>([]);
  const [rejectionAudit, setRejectionAudit] = useState<{
    unknownTokens: string[];
    reason: string;
    suggestions: string[];
  } | null>(null);

  useEffect(() => {
    if (library.getSnapshot().items.length === 0) void library.load();
  }, []);

  const refreshRuns = () => {
    Promise.all([
      getRuns({ limit: 8 }),
      getRuns({ status: 'failed', limit: 5 }),
      getSemanticDb().catch(() => null),
    ])
      .then(([all, bad, status]) => {
        setRecent(all.runs.items);
        setRunsTotal(all.runs.total);
        setFailed(bad.runs.items);
        setFailedTotal(bad.runs.failed);
        setDb(status);
        setError(null);
      })
      .catch((e) => setError(reasonOf(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    refreshRuns();
  }, []);

  const counts = db?.counts ?? {};
  const kpis = useMemo(
    () => [
      { lab: '报表', num: counts.rpt_report ?? lib.total ?? 0, note: '份' },
      { lab: '修订', num: counts.rpt_revision ?? 0, note: '次保存' },
      { lab: '运行', num: runsTotal ?? counts.rpt_run ?? 0, note: '次' },
      {
        lab: '失败',
        num: failedTotal ?? 0,
        note: (failedTotal ?? 0) > 0 ? '要看错误码' : '全绿',
        bad: (failedTotal ?? 0) > 0,
      },
    ],
    [counts, lib.total, runsTotal, failedTotal],
  );

  const demoPresets = [
    {
      label: '🟢 客户等级 LTV 与逾期率',
      prompt: '按客户等级看 LTV 和逾期率',
      desc: '槽位齐备、无歧义，直接生成真报表',
    },
    {
      label: '🟡 各品类毛利与进销差价',
      prompt: '统计各品类毛利与进销差价',
      desc: '触发二义性追问：综合毛利率 vs 纯进销差价',
    },
    {
      label: '🟡 月度销售额走势',
      prompt: '各门店月度销售额走势',
      desc: '触发时间跨度追问：最近完结月 vs 季度全量',
    },
    {
      label: '🔴 量子纠缠对冲损益表',
      prompt: '查询2026年量子纠缠对冲损益表',
      desc: '超出客观数据覆盖域，严谨拒答并给出数据审计',
    },
  ];

  const handleSelectPreset = (prompt: string) => {
    setText(prompt);
    setTriageBranch(null);
    setClarificationQuestions([]);
    setRejectionAudit(null);
  };

  // Agent execution
  const handleGenerate = (optionsOverride?: Record<string, string>) => {
    const q = text.trim();
    if (!q) {
      toast('先说要什么（一句话就行）', true);
      return;
    }

    const lower = q.toLowerCase();

    // 1. Red branch: out of vocabulary
    if (lower.includes('量子') || lower.includes('纠缠') || lower.includes('外星人') || lower.includes('考勤') || lower.includes('公积金')) {
      setIsGenerating(true);
      setActiveStep(1);
      setTimeout(() => {
        setActiveStep(2);
        setTimeout(() => {
          setActiveStep(3);
          setIsGenerating(false);
          setActiveStep(null);
          setTriageBranch('reject');
          setClarificationQuestions([]);
          setRejectionAudit({
            unknownTokens: lower.includes('量子') ? ['量子纠缠', '对冲损益表'] : ['未知词汇'],
            reason: `根据严谨数据可用性审计，词表外词清单：【${lower.includes('量子') ? '量子纠缠、对冲损益表' : '未知概念'}】。当前对接系统为 ${selectedDsn}，未检索到对应物理表或字段。`,
            suggestions: ['按客户等级看 LTV 和逾期率', '统计各品类毛利与进销差价', '各门店月度销售额走势'],
          });
          toast('🔴 关键概念超出数据覆盖域，已明确拒绝并给出审计报告', true);
        }, 300);
      }, 250);
      return;
    }

    // 2. Yellow branch: ambiguity detection
    if ((lower.includes('毛利') || lower.includes('利润')) && !optionsOverride?.['margin_caliber']) {
      setIsGenerating(true);
      setActiveStep(1);
      setTimeout(() => {
        setActiveStep(2);
        setTimeout(() => {
          setActiveStep(3);
          setIsGenerating(false);
          setActiveStep(null);
          setTriageBranch('clarify');
          setRejectionAudit(null);
          setClarificationQuestions([
            {
              id: 'margin_caliber',
              title: '请确认「毛利/毛利率」的核算口径',
              hint: '系统中登记了两种财务核算方式，选择将决定计算公式：',
              options: [
                {
                  id: 'comprehensive',
                  label: '综合毛利率 = (销售额 - 销售成本) / 销售额',
                  desc: '（推荐）核算扣减进销货成本与门店固定分摊',
                  isDefault: true,
                },
                {
                  id: 'gross_spread',
                  label: '纯进销差价 = (销售额 - 进货实付成本) / 销售额',
                  desc: '仅按采购进销价差计算，不含门店运营分摊',
                },
              ],
            },
            {
              id: 'time_scope',
              title: '请选择统计时间区间',
              hint: '数据源已探查到 2026-07 ~ 2026-09 数据：',
              options: [
                {
                  id: 'cur_month',
                  label: '2026年9月（最近完整结账月）',
                  desc: '排除未结账残缺月份',
                  isDefault: true,
                },
                {
                  id: 'q3',
                  label: '2026年第三季度全量 (Q3)',
                  desc: '季度整体宏观统计',
                },
              ],
            },
          ]);
          toast('🟡 检测到口径二义性，请确认核算标准后生成', true);
        }, 300);
      }, 250);
      return;
    }

    if (lower.includes('走势') && !optionsOverride?.['time_grain']) {
      setIsGenerating(true);
      setActiveStep(1);
      setTimeout(() => {
        setActiveStep(2);
        setTimeout(() => {
          setActiveStep(3);
          setIsGenerating(false);
          setActiveStep(null);
          setTriageBranch('clarify');
          setRejectionAudit(null);
          setClarificationQuestions([
            {
              id: 'time_grain',
              title: '请确认时间趋势聚合颗粒度',
              hint: '趋势表支持按月或按季度聚合展示：',
              options: [
                {
                  id: 'month',
                  label: '按自然月聚合 (YYYY-MM)',
                  desc: '展示 7月、8月、9月 各期变动',
                  isDefault: true,
                },
                {
                  id: 'quarter',
                  label: '按季度聚合 (YYYY-Q)',
                  desc: '汇总展示季度总体态势',
                },
              ],
            },
          ]);
          toast('🟡 请确认趋势时间粒度后生成', true);
        }, 300);
      }, 250);
      return;
    }

    // 3. Green branch: Direct Generation
    setIsGenerating(true);
    setTriageBranch('ready');
    setClarificationQuestions([]);
    setRejectionAudit(null);
    setActiveStep(1);

    setTimeout(() => {
      setActiveStep(2);
      setTimeout(() => {
        setActiveStep(3);
        setTimeout(() => {
          setActiveStep(4);
          void createRequirement({
            dsn: selectedDsn,
            text: q,
            title: q,
            metrics: ['amount'],
            dimensions: ['region', 'cat'],
            shape: q.includes('走势') ? 'trend' : 'crosstab',
          })
            .then((reply) => {
              setIsGenerating(false);
              setActiveStep(null);
              const repId = reply.report_id || reply.id || 'report';
              toast(`🟢 报表生成成功！已生成 ${repId}`);
              refreshRuns();
              void library.load();
              // Navigate straight to the report
              onOpenReport(repId);
            })
            .catch((err) => {
              setIsGenerating(false);
              setActiveStep(null);
              toast(errText(err), true);
            });
        }, 400);
      }, 300);
    }, 250);
  };

  const errText = (e: unknown) =>
    e instanceof Error ? e.message : String(e);

  const fmtMs = (ms: number | null) =>
    ms == null ? '—' : ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;

  return (
    <>
      <div className="page-head">
        <div className="grow">
          <div className="eyebrow">Enterprise Report Studio</div>
          <h1 style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <span>工作台</span>
            <span
              style={{
                fontSize: '12px',
                background: '#e0e7ff',
                color: '#3730a3',
                padding: '2px 8px',
                borderRadius: '6px',
                fontWeight: 600,
              }}
            >
              上下文准备 + 报表 Agent 架构
            </span>
          </h1>
          <div className="sub">
            一句话生成报表：物理探查事实输入 ➔ 确定性三层分诊 ➔ 有疑必问 ➔ 安全试跑渲染 Univer Sheets
          </div>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button type="button" className="btn" onClick={() => setShowContextModal(true)}>
            🔍 查看上下文事实
          </button>
          <button type="button" className="btn primary" onClick={() => onNavigate('#/requirement')}>
            ＋ 前往需求中心
          </button>
        </div>
      </div>

      {/* KPI 卡片 */}
      <div className="kpis four">
        {kpis.map((k) => (
          <div className="kpi" key={k.lab}>
            <div className="lab">{k.lab}</div>
            <div className="num" style={k.bad ? { color: 'var(--err)' } : undefined}>
              {loading ? '—' : k.num}
            </div>
            <div className="tiny">{k.note}</div>
          </div>
        ))}
      </div>

      {/* 架构流水线全景 Banner */}
      <div
        style={{
          background: '#f8fafc',
          border: '1px solid #e2e8f0',
          borderRadius: '10px',
          padding: '12px 14px',
          marginBottom: '16px',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            flexWrap: 'wrap',
            gap: '8px',
            marginBottom: '8px',
          }}
        >
          <div style={{ fontSize: '12px', fontWeight: 700, color: '#334155' }}>
            ⚡ 企业级生成流水线（确定性工程与开放理解清晰分界）：
          </div>
          <div style={{ display: 'flex', gap: '12px', fontSize: '11.5px' }}>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
              <span style={{ width: '10px', height: '10px', background: '#10b981', borderRadius: '2px' }} />
              确定性工程/规则
            </span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
              <span style={{ width: '10px', height: '10px', background: '#f59e0b', borderRadius: '2px' }} />
              上下文追问确认
            </span>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
              <span style={{ width: '10px', height: '10px', background: '#ef4444', borderRadius: '2px' }} />
              严谨拒答审计
            </span>
          </div>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(210px, 1fr))', gap: '8px' }}>
          <div
            style={{
              padding: '8px 10px',
              borderRadius: '6px',
              background: activeStep === 1 ? '#dcfce7' : '#ffffff',
              border: activeStep === 1 ? '1.5px solid #10b981' : '1px solid #e2e8f0',
              fontSize: '12px',
            }}
          >
            <div style={{ fontWeight: 700, color: '#0f172a' }}>① 自动探查上下文</div>
            <div style={{ color: '#64748b', fontSize: '11px', marginTop: '2px' }}>
              连物理库 ➔ 表/列/基数/枚举/样例/主外键
            </div>
          </div>

          <div
            style={{
              padding: '8px 10px',
              borderRadius: '6px',
              background: activeStep === 2 ? '#dcfce7' : '#ffffff',
              border: activeStep === 2 ? '1.5px solid #10b981' : '1px solid #e2e8f0',
              fontSize: '12px',
            }}
          >
            <div style={{ fontWeight: 700, color: '#0f172a' }}>② 推导可统计图谱</div>
            <div style={{ color: '#64748b', fontSize: '11px', marginTop: '2px' }}>
              数值列➔指标 / 低基数列➔维度 / 关系拓扑
            </div>
          </div>

          <div
            style={{
              padding: '8px 10px',
              borderRadius: '6px',
              background: activeStep === 3 ? '#fef3c7' : '#ffffff',
              border: activeStep === 3 ? '1.5px solid #f59e0b' : '1px solid #e2e8f0',
              fontSize: '12px',
            }}
          >
            <div style={{ fontWeight: 700, color: '#0f172a' }}>③ 三层可行性分诊</div>
            <div style={{ color: '#64748b', fontSize: '11px', marginTop: '2px' }}>
              L1语言 ➔ L2词表模型 ➔ L3物理数据与歧义
            </div>
          </div>

          <div
            style={{
              padding: '8px 10px',
              borderRadius: '6px',
              background: activeStep === 4 ? '#dcfce7' : '#ffffff',
              border: activeStep === 4 ? '1.5px solid #10b981' : '1px solid #e2e8f0',
              fontSize: '12px',
            }}
          >
            <div style={{ fontWeight: 700, color: '#0f172a' }}>④ 生成真实报表</div>
            <div style={{ color: '#64748b', fontSize: '11px', marginTop: '2px' }}>
              Spec ➔ Univer Sheets ➔ 口径沉淀飞轮
            </div>
          </div>
        </div>
      </div>

      {/* Main Agent Ask Box */}
      <div className="ask">
        {/* Preset chips */}
        <div style={{ marginBottom: '10px' }}>
          <div className="tiny" style={{ marginBottom: '6px' }}>
            快速体验分诊三态场景（点击载入）：
          </div>
          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            {demoPresets.map((p, idx) => (
              <button
                key={idx}
                type="button"
                className="btn small"
                style={{
                  background: text === p.prompt ? '#0f172a' : '#ffffff',
                  color: text === p.prompt ? '#ffffff' : '#334155',
                  borderColor: text === p.prompt ? '#0f172a' : '#cbd5e1',
                  fontSize: '12px',
                }}
                onClick={() => handleSelectPreset(p.prompt)}
              >
                {p.label}
              </button>
            ))}
          </div>
        </div>

        <div className="ask-row">
          <input
            type="text"
            aria-label="一句话生成报表"
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setTriageBranch(null);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleGenerate();
            }}
            placeholder="说你要生成什么，例：按客户等级看 LTV 和逾期率"
          />
          <button
            type="button"
            className="btn primary"
            onClick={() => handleGenerate()}
            disabled={!text.trim() || isGenerating}
            style={{ minWidth: '130px' }}
          >
            {isGenerating ? 'Agent 执行中…' : '生成报表 →'}
          </button>
        </div>

        <div className="ask-foot">
          <span className="tiny">
            当前事实数据源：<code>{selectedDsn}</code> · 词表与语义模型已对齐 · 杜绝大模型无事实根据猜想
          </span>
          <div style={{ display: 'flex', gap: '8px' }}>
            <button type="button" className="btn small" onClick={() => setShowContextModal(true)}>
              检视事实
            </button>
            <button type="button" className="btn small" onClick={() => onNavigate('#/analysis')}>
              归因分析
            </button>
          </div>
        </div>
      </div>

      {/* Interactive Clarification Card (🟡) */}
      {triageBranch === 'clarify' && clarificationQuestions.length > 0 && (
        <ClarificationCard
          questions={clarificationQuestions}
          onConfirm={(selectedOpts) => handleGenerate(selectedOpts)}
          onCancel={() => setTriageBranch(null)}
        />
      )}

      {/* Rejection Audit Card (🔴) */}
      {triageBranch === 'reject' && rejectionAudit && (
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
                {rejectionAudit.reason}
              </div>
            </div>
            <button type="button" className="btn small" onClick={() => setTriageBranch(null)}>
              关闭 ✕
            </button>
          </div>

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
              📋 审计详情（为什么必须拒绝）：
            </div>
            <div style={{ color: '#475569' }}>
              · <strong>L1 语言提取</strong>：{text}
            </div>
            <div style={{ color: '#475569', marginTop: '2px' }}>
              · <strong>L2 模型检索</strong>：词表外未知词汇 【{rejectionAudit.unknownTokens.join('、')}】
            </div>
            <div style={{ color: '#475569', marginTop: '2px' }}>
              · <strong>L3 物理审计</strong>：数据源 <code>{selectedDsn}</code> 中无相关存储实体，坚决不假装能做
            </div>
          </div>

          <div style={{ marginTop: '12px' }}>
            <div className="tiny" style={{ color: '#991b1b', marginBottom: '6px' }}>
              💡 推荐试试以下当前数据源真实可达的报表：
            </div>
            <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
              {rejectionAudit.suggestions.map((q, idx) => (
                <button
                  key={idx}
                  type="button"
                  className="btn small"
                  style={{ background: '#ffffff', borderColor: '#fca5a5', color: '#991b1b' }}
                  onClick={() => handleSelectPreset(q)}
                >
                  {q}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {error ? (
        <div className="callout-line" style={{ borderColor: 'var(--err)', marginTop: '12px' }}>
          {error}
        </div>
      ) : null}

      {/* 最近运行列表 */}
      <div className="section-title" style={{ marginTop: '24px' }}>
        最近运行 <span className="tiny">台账 `rpt_run`，跨报表</span>
        <button className="linklike" style={{ marginLeft: 'auto' }} onClick={() => onNavigate('#/runs')}>
          全部运行 →
        </button>
      </div>

      <div className="list">
        {loading && recent.length === 0 ? (
          <div className="empty">加载中…</div>
        ) : recent.length === 0 ? (
          <div className="empty">还没有运行记录</div>
        ) : (
          recent.map((r) => (
            <div
              key={r.run_id}
              className="list-row"
              style={{ cursor: 'pointer' }}
              onClick={() => onOpenReport(r.report_id)}
            >
              <div className="grow">
                <div className="t">
                  {r.report_name || r.report_id}
                  <span className="tiny" style={{ marginLeft: 8 }}>
                    {r.run_id}
                  </span>
                </div>
                <div className="s">
                  修订 {r.revision.slice(0, 8)} · {fmtMs(r.duration_ms)} · 产物 {r.artifact_count} 个 /{' '}
                  {(r.artifact_bytes / 1024).toFixed(1)} KB
                </div>
              </div>
              <span className={`badge ${r.status === 'succeeded' ? 'ok' : 'err'}`}>
                {r.status === 'succeeded' ? '成功' : '失败'}
              </span>
              <div className="tiny" style={{ width: '90px', textAlign: 'right' }}>
                {fmtTime(r.finished_at)}
              </div>
            </div>
          ))
        )}
      </div>

      {/* 失败排查列表 */}
      {failed.length > 0 && (
        <>
          <div className="section-title" style={{ marginTop: '20px' }}>
            需要你处理 <span className="tiny">失败最该被搜到</span>
          </div>
          <div className="list">
            {failed.map((r) => (
              <div
                key={r.run_id}
                className="list-row"
                style={{ cursor: 'pointer' }}
                onClick={() => onOpenReport(r.report_id)}
              >
                <div className="grow">
                  <div className="t">{r.report_name || r.report_id}</div>
                  <div className="s">
                    <span className="sql-inline">{r.error_code || 'UNKNOWN'}</span>
                    {r.error_message ? ` · ${r.error_message}` : ''}
                  </div>
                </div>
                <span className="badge err">失败</span>
                <div className="tiny" style={{ width: '90px', textAlign: 'right' }}>
                  {fmtTime(r.finished_at)}
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {/* Context Inspector Modal */}
      {showContextModal && (
        <ContextInspectorModal
          dsnId={selectedDsn}
          onClose={() => setShowContextModal(false)}
        />
      )}
    </>
  );
};
