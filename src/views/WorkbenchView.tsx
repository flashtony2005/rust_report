import React, { useState } from 'react';
import { store } from '../services/store';
import { useToast } from '../components/Toast';
import { Requirement, Run } from '../types';

interface Props {
  onOpenRequirement: (id: string) => void;
  onOpenRun: (reqId: string, runId: string) => void;
  onNewRequirement: () => void;
  onOpenLlmModal: () => void;
}

export const WorkbenchView: React.FC<Props> = ({
  onOpenRequirement,
  onOpenRun,
  onNewRequirement,
  onOpenLlmModal,
}) => {
  const { toast } = useToast();
  const requirements = store.getRequirements();
  const datasources = store.getDatasources();
  const runs = store.getRuns();
  const alerts = store.getAlerts();
  const llmConfig = store.getLlmConfig();

  const [askText, setAskText] = useState('');
  const [selectedDsn, setSelectedDsn] = useState(datasources[0]?.id || '');
  const [isGenerating, setIsGenerating] = useState(false);

  const openReqsCount = requirements.filter((r) => r.status !== 'accepted').length;
  const templatesCount = requirements.filter((r) => r.template && r.template.rev > 0).length;
  const runsCount = runs.length;

  const handleAskRun = () => {
    if (!askText.trim()) {
      toast('先说要什么（一句话就行）', true);
      return;
    }
    if (!selectedDsn) {
      toast('先去「数据源」配一个库', true);
      return;
    }

    setIsGenerating(true);
    setTimeout(() => {
      try {
        const newReq = store.askAndGenerate(askText.trim(), selectedDsn);
        toast(`已生成 ${newReq.id}（${newReq.shape_decision?.label || '报表'}）`);
        setIsGenerating(false);
        onOpenRequirement(newReq.id);
      } catch (err: any) {
        setIsGenerating(false);
        toast(err.message, true);
      }
    }, 600);
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
          <div className="eyebrow">Workbench</div>
          <h1>工作台</h1>
          <div className="sub">从一句话需求到可打印的报表：需求 → 模板 → 结果都留在这里</div>
        </div>
        <button type="button" className="btn primary" onClick={onNewRequirement}>
          ＋ 新建需求
        </button>
      </div>

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

      <div className="ask">
        <div className="ask-row">
          <input
            type="text"
            value={askText}
            onChange={(e) => setAskText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleAskRun();
            }}
            placeholder="用一句话说清你要什么，例：今年各月销售额走势"
          />
          <select
            value={selectedDsn}
            onChange={(e) => setSelectedDsn(e.target.value)}
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
            onClick={handleAskRun}
            disabled={isGenerating}
          >
            {isGenerating ? '生成中…' : '生成报表 →'}
          </button>
        </div>
        <div className="ask-foot">
          <span className="tiny" id="ask-state">
            {llmConfig.has_key
              ? `理解层：${llmConfig.model} · 输出一律过编译器校验`
              : '理解层：内置规则与方言解析器已就绪 · 点右侧可配置 DeepSeek / OpenAI 兼容 Key'}
          </span>
          <button type="button" className="btn small" onClick={onOpenLlmModal}>
            理解层设置
          </button>
        </div>
      </div>

      <div className="section-title">
        待我处理 <span className="tiny">按最近更新排序</span>
      </div>
      <div className="list">
        {requirements.length > 0 ? (
          requirements.map((r) => (
            <div
              key={r.id}
              className="list-row"
              style={{ cursor: 'pointer' }}
              onClick={() => onOpenRequirement(r.id)}
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
              <div className="tiny" style={{ width: '70px', textAlign: 'right' }}>
                {fmtTime(r.updated_at)}
              </div>
            </div>
          ))
        ) : (
          <div className="empty">还没有需求 —— 点右上角「新建需求」开始</div>
        )}
      </div>

      <div className="section-title">最近运行</div>
      <div className="list">
        {runs.length > 0 ? (
          runs.slice(0, 6).map((r) => (
            <div
              key={r.run_id}
              className="list-row"
              style={{ cursor: 'pointer' }}
              onClick={() => onOpenRun(r.req_id, r.run_id)}
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
              <div className="tiny" style={{ width: '70px', textAlign: 'right' }}>
                {fmtTime(r.at)}
              </div>
            </div>
          ))
        ) : (
          <div className="empty">还没有运行记录</div>
        )}
      </div>

      <div className="section-title">
        异常提醒 <span className="tiny">按需求自己的口径比对本期/上期</span>
      </div>
      <div className="list">
        {alerts.length > 0 ? (
          alerts.slice(0, 5).map((a) => (
            <div key={a.id} className="list-row">
              <div className="grow">
                <div className="t">{a.message}</div>
                <div className="s">
                  {a.dsn || ''} · {a.metric_label || a.metric || ''} ·{' '}
                  {a.window?.cur_start || ''}~{a.window?.cur_end || ''} · {fmtTime(a.at)}
                </div>
              </div>
              <span className={`badge ${a.level === 'critical' ? 'failed' : 'analyzed'}`}>
                {a.level === 'critical' ? '严重' : '预警'}
              </span>
            </div>
          ))
        ) : (
          <div className="empty">没有异常（或还没体检过 —— 在需求页的「异常提醒」里点一次）</div>
        )}
      </div>
    </>
  );
};
