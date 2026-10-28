import React, { useState, useMemo } from 'react';
import { BizModel, OntologyResolutionResult } from '../types';
import { OntologyResolver } from '../services/ontologyResolver';
import { UniverViewer } from './UniverViewer';
import { useToast } from './Toast';

interface Props {
  model: BizModel;
  onApplyAsRequirement?: (result: OntologyResolutionResult) => void;
}

export const OntologyResolverView: React.FC<Props> = ({ model }) => {
  const { toast } = useToast();
  const [intentInput, setIntentInput] = useState('按客户等级统计 LTV 与逾期率');
  const [codeTab, setCodeTab] = useState<'polars' | 'sql'>('polars');
  const [activePreset, setActivePreset] = useState('按客户等级统计 LTV 与逾期率');

  // Compute resolution
  const resolution: OntologyResolutionResult = useMemo(() => {
    return OntologyResolver.resolve(model, intentInput);
  }, [model, intentInput]);

  const presets = [
    {
      label: '🎯 按客户等级统计 LTV 与逾期率',
      prompt: '按客户等级统计 LTV 与逾期率',
      desc: '跨 3 实体 (Customer + Order + Loan) 典型 1:N 扇出场景',
    },
    {
      label: '🌐 按所属大区分析授信总额与贷款逾期率',
      prompt: '按所属大区分析授信总额与贷款逾期率',
      desc: '区域维度投影 + 金融借贷风险粒度对齐',
    },
  ];

  const handleSelectPreset = (p: { label: string; prompt: string }) => {
    setActivePreset(p.prompt);
    setIntentInput(p.prompt);
    toast(`已载入业务需求: ${p.prompt}`);
  };

  return (
    <div className="space-y-6">
      {/* 顶部架构理念 Banner */}
      <div className="bg-gradient-to-r from-slate-900 via-indigo-950 to-slate-900 border border-indigo-500/30 rounded-xl p-5 text-white shadow-xl relative overflow-hidden">
        <div className="absolute right-0 top-0 bottom-0 w-96 bg-[radial-gradient(circle_at_top_right,_var(--tw-gradient-stops))] from-indigo-500/20 via-transparent to-transparent pointer-events-none" />
        <div className="relative z-10 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <div className="flex items-center space-x-2 text-indigo-400 font-mono text-xs font-semibold tracking-wider uppercase mb-1">
              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              <span>Ontology 2.0 Semantic Core & Execution Engine</span>
            </div>
            <h2 className="text-xl font-bold tracking-tight text-white flex items-center space-x-2">
              <span>本体解析器：实体拓扑 DAG + 粒度对齐 + 向量计算管道</span>
            </h2>
            <p className="text-slate-300 text-sm mt-1 max-w-3xl leading-relaxed">
              将业务意图自动解析为实体图连通树，从数学与代数层杜绝多对多/多对一关联中的
              <span className="text-amber-300 font-medium"> 扇出陷阱 (Fan-Out Trap)</span>。静态编译为 Rust 原生 Polars 向量化表达式与最优 CTE SQL，直通 Univer 电子表格渲染。
            </p>
          </div>
          <div className="flex items-center space-x-2 self-start md:self-center">
            <span className="px-3 py-1.5 rounded-lg bg-emerald-500/20 border border-emerald-500/40 text-emerald-300 text-xs font-mono font-medium flex items-center space-x-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
              <span>Grain Alignment: ACTIVE</span>
            </span>
            <span className="px-3 py-1.5 rounded-lg bg-indigo-500/20 border border-indigo-500/40 text-indigo-300 text-xs font-mono font-medium">
              Zero Fan-Out Guarantee
            </span>
          </div>
        </div>
      </div>

      {/* 实体关系拓扑 DAG 可视化卡片 */}
      <div className="bg-white border border-slate-200 rounded-xl p-5 shadow-sm">
        <div className="flex items-center justify-between mb-4 pb-3 border-b border-slate-100">
          <div>
            <h3 className="font-semibold text-slate-800 text-base flex items-center space-x-2">
              <span>实体关系拓扑图 (Entity Relation DAG)</span>
              <span className="text-xs bg-slate-100 text-slate-600 px-2 py-0.5 rounded font-mono">
                {model.entities.length} 实体 / {(model.relationships || []).length} 关系边
              </span>
            </h3>
            <p className="text-xs text-slate-500 mt-0.5">
              绿框节点表示当前需求激活的实体；箭头标明关联基数与主外键对齐路径
            </p>
          </div>
          <div className="flex items-center space-x-3 text-xs">
            <span className="flex items-center space-x-1.5 text-slate-600">
              <span className="w-3 h-3 rounded bg-emerald-100 border border-emerald-500" />
              <span>当前解析命中实体</span>
            </span>
            <span className="flex items-center space-x-1.5 text-slate-600">
              <span className="w-3 h-3 rounded bg-slate-50 border border-slate-300" />
              <span>未涉及实体</span>
            </span>
          </div>
        </div>

        {/* 简洁现代的 DAG 拓扑节点连线卡片 */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 p-4 bg-slate-50/70 border border-slate-200/80 rounded-xl items-center relative">
          {/* Node 1: Customer (Root) */}
          <div
            className={`p-4 rounded-xl border-2 transition-all relative ${
              resolution.resolvedEntities.includes('customer')
                ? 'bg-emerald-50/50 border-emerald-500 shadow-sm shadow-emerald-500/10'
                : 'bg-white border-slate-200'
            }`}
          >
            <div className="flex items-center justify-between mb-1.5">
              <span className="text-xs font-bold uppercase tracking-wider text-emerald-700 font-mono flex items-center space-x-1">
                <span>[Root Entity]</span>
              </span>
              <span className="text-[10px] px-1.5 py-0.5 bg-emerald-100 text-emerald-800 rounded font-mono font-medium">
                Grain: customer.id
              </span>
            </div>
            <div className="font-semibold text-slate-800 text-sm">Customer 客户主体</div>
            <div className="text-xs text-slate-500 mt-1">主键: id | 维度: level, region</div>
            <div className="mt-2.5 pt-2 border-t border-slate-200/60 flex flex-wrap gap-1">
              <span className="text-[10px] bg-slate-200/60 text-slate-700 px-1.5 py-0.5 rounded font-mono">
                level: VIP/白金/黄金/普通
              </span>
            </div>
          </div>

          {/* Connectors & Arrows */}
          <div className="flex flex-col justify-center items-center space-y-4 py-2">
            <div className="flex items-center space-x-2 text-xs font-mono text-slate-600 bg-white px-3 py-1.5 rounded-lg border border-slate-200 shadow-xs">
              <span className="text-indigo-600 font-bold">1 : N</span>
              <span>➔ Order (外键 customer_id)</span>
            </div>
            <div className="flex items-center space-x-2 text-xs font-mono text-slate-600 bg-white px-3 py-1.5 rounded-lg border border-slate-200 shadow-xs">
              <span className="text-indigo-600 font-bold">1 : N</span>
              <span>➔ Loan (外键 customer_id)</span>
            </div>
            <div className="text-[11px] text-amber-700 bg-amber-50 border border-amber-200 px-3 py-1 rounded text-center">
              ⚠️ 检测到分支扇出：触发 Grain Alignment 隔离聚合
            </div>
          </div>

          {/* Child Nodes: Order & Loan */}
          <div className="space-y-3">
            {/* Order Entity */}
            <div
              className={`p-3 rounded-lg border-2 transition-all ${
                resolution.resolvedEntities.includes('order')
                  ? 'bg-emerald-50/50 border-emerald-500'
                  : 'bg-white border-slate-200'
              }`}
            >
              <div className="flex items-center justify-between">
                <span className="font-semibold text-slate-800 text-xs">Order 交易订单</span>
                <span className="text-[10px] px-1.5 py-0.5 bg-indigo-50 text-indigo-700 rounded font-mono">
                  1:N 子实体
                </span>
              </div>
              <div className="text-xs text-slate-500 mt-0.5">度量: amount, discount</div>
              <div className="text-[11px] text-emerald-700 font-mono mt-1">
                LTV = SUM(amount) - SUM(discount)
              </div>
            </div>

            {/* Loan Entity */}
            <div
              className={`p-3 rounded-lg border-2 transition-all ${
                resolution.resolvedEntities.includes('loan')
                  ? 'bg-emerald-50/50 border-emerald-500'
                  : 'bg-white border-slate-200'
              }`}
            >
              <div className="flex items-center justify-between">
                <span className="font-semibold text-slate-800 text-xs">Loan 信贷授信</span>
                <span className="text-[10px] px-1.5 py-0.5 bg-indigo-50 text-indigo-700 rounded font-mono">
                  1:N 子实体
                </span>
              </div>
              <div className="text-xs text-slate-500 mt-0.5">度量: principal, is_overdue</div>
              <div className="text-[11px] text-emerald-700 font-mono mt-1">
                OverdueRate = COUNT_IF(is_overdue=1) / COUNT(id)
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* 报表需求意图输入 & 预设测试 */}
      <div className="bg-white border border-slate-200 rounded-xl p-5 shadow-sm space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div>
            <h3 className="font-semibold text-slate-800 text-base">报表请求意图与语义解析</h3>
            <p className="text-xs text-slate-500 mt-0.5">
              输入自然语言或分析需求，本体解析器实时计算 DAG 连接树与无偏聚合代数
            </p>
          </div>
          <div className="flex items-center space-x-2">
            <span className="text-xs text-slate-500 font-medium">推荐预设用例:</span>
            {presets.map((p, idx) => (
              <button
                key={idx}
                onClick={() => handleSelectPreset(p)}
                className={`text-xs px-2.5 py-1 rounded-md transition-all ${
                  activePreset === p.prompt
                    ? 'bg-indigo-600 text-white font-medium shadow-xs'
                    : 'bg-slate-100 hover:bg-slate-200 text-slate-700'
                }`}
              >
                {p.label}
              </button>
            ))}
          </div>
        </div>

        <div className="flex items-center space-x-2">
          <div className="relative flex-1">
            <input
              type="text"
              value={intentInput}
              onChange={(e) => setIntentInput(e.target.value)}
              placeholder="例如：按客户等级统计 LTV 与逾期率"
              className="w-full text-sm px-4 py-2.5 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-indigo-500 font-medium text-slate-800 bg-slate-50/50"
            />
          </div>
          <button
            onClick={() => toast('本体解析器重新推导拓扑完毕')}
            className="px-4 py-2.5 bg-indigo-600 hover:bg-indigo-700 text-white text-sm font-medium rounded-lg shadow-sm transition-colors flex items-center space-x-1.5 whitespace-nowrap"
          >
            <span>重新解析</span>
          </button>
        </div>

        {/* 扇出陷阱规避诊断 */}
        {resolution.fanOutProtected && (
          <div className="bg-emerald-50 border border-emerald-200 rounded-lg p-3 text-xs text-emerald-900 space-y-1">
            <div className="font-semibold flex items-center space-x-1.5 text-emerald-800">
              <span className="w-2 h-2 rounded-full bg-emerald-500" />
              <span>本体解析器防护机制生效：扇出规避 (Fan-Out & Chasm Trap Prevention)</span>
            </div>
            {resolution.fanOutPreventionNotes.map((note, nIdx) => (
              <div key={nIdx} className="text-emerald-700 pl-3.5 leading-relaxed">
                • {note}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* 执行计划：Polars 向量表达式 vs ANSI SQL */}
      <div className="bg-white border border-slate-200 rounded-xl shadow-sm overflow-hidden">
        <div className="border-b border-slate-200 bg-slate-50/80 px-5 py-3 flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <span className="font-semibold text-slate-800 text-sm">静态编译生成执行计划 (Execution Plan)</span>
            <span className="text-xs text-slate-500">| 高性能列式代数</span>
          </div>
          <div className="flex items-center space-x-1 bg-slate-200 p-0.5 rounded-lg">
            <button
              onClick={() => setCodeTab('polars')}
              className={`text-xs px-3 py-1 rounded-md font-mono transition-all ${
                codeTab === 'polars'
                  ? 'bg-white text-indigo-700 font-bold shadow-xs'
                  : 'text-slate-600 hover:text-slate-900'
              }`}
            >
              Rust + Polars 向量化表达式
            </button>
            <button
              onClick={() => setCodeTab('sql')}
              className={`text-xs px-3 py-1 rounded-md font-mono transition-all ${
                codeTab === 'sql'
                  ? 'bg-white text-indigo-700 font-bold shadow-xs'
                  : 'text-slate-600 hover:text-slate-900'
              }`}
            >
              Grain-Aligned CTE SQL
            </button>
          </div>
        </div>

        <div className="p-4 bg-slate-950 text-slate-200 font-mono text-xs overflow-x-auto max-h-72">
          <pre className="leading-relaxed">
            <code>{codeTab === 'polars' ? resolution.polarsCode : resolution.grainAlignedSql}</code>
          </pre>
        </div>
      </div>

      {/* 极速计算结果 ➔ Univer 电子表格实时交互渲染 */}
      <div className="bg-white border border-slate-200 rounded-xl shadow-sm overflow-hidden">
        <div className="px-5 py-3.5 border-b border-slate-200 bg-slate-50 flex items-center justify-between">
          <div>
            <div className="flex items-center space-x-2">
              <span className="font-semibold text-slate-800 text-sm">
                Univer 电子表格高保真呈现
              </span>
              <span className="text-[11px] bg-emerald-100 text-emerald-800 px-2 py-0.5 rounded font-mono font-medium">
                计算管道完成: {resolution.outputData.rows.length} 维度分组
              </span>
            </div>
            <p className="text-xs text-slate-500 mt-0.5">
              Polars 向量张量直接投影至 Univer 标准 snapshot.json，可任意选区、查看统计与公式
            </p>
          </div>
          <button
            onClick={() => {
              const jsonStr = JSON.stringify(resolution.univerSnapshot, null, 2);
              navigator.clipboard?.writeText(jsonStr);
              toast('已复制 Univer snapshot.json 到剪贴板');
            }}
            className="text-xs px-3 py-1.5 border border-slate-300 hover:border-slate-400 bg-white text-slate-700 rounded-md font-medium shadow-2xs transition-colors flex items-center space-x-1"
          >
            <span>📋 复制 Snapshot JSON</span>
          </button>
        </div>

        <div className="p-4">
          <UniverViewer
            snapshot={resolution.univerSnapshot}
            title={`客户等级 LTV 与逾期率分析表 (Ontology 编译执行)`}
          />
        </div>
      </div>
    </div>
  );
};
