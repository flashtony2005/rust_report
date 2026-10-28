import React, { useState, useMemo } from 'react';
import { BizModel, OntologyResolutionResult } from '../types';
import { OntologyResolver } from '../services/ontologyResolver';
import { UniverViewer } from './UniverViewer';
import { useToast } from './Toast';

interface Props {
  model: BizModel;
  onApplyAsRequirement?: (result: OntologyResolutionResult) => void;
}

export const OntologyResolverView: React.FC<Props> = ({ model, onApplyAsRequirement }) => {
  const { toast } = useToast();
  const [intentInput, setIntentInput] = useState('按客户等级看 LTV 和逾期率');
  const [activeStep, setActiveStep] = useState<1 | 2 | 3 | 4>(1);
  const [viewMode, setViewMode] = useState<'step' | 'full'>('step');
  const [codeTab, setCodeTab] = useState<'sql' | 'polars'>('sql');
  const [activePreset, setActivePreset] = useState('按客户等级看 LTV 和逾期率');

  // Compute resolution
  const resolution: OntologyResolutionResult = useMemo(() => {
    return OntologyResolver.resolve(model, intentInput);
  }, [model, intentInput]);

  const presets = [
    {
      label: '🎯 按客户等级看 LTV 和逾期率',
      prompt: '按客户等级看 LTV 和逾期率',
      desc: '跨 3 实体 (客户 + 订单 + 贷款) 典型 1:N 双向分支防翻车场景',
    },
    {
      label: '🌐 按所属大区分析授信总额与逾期率',
      prompt: '按所属大区分析授信总额与逾期率',
      desc: '地理区域分组 + 信贷本金与风险度量对齐',
    },
  ];

  const handleSelectPreset = (p: { label: string; prompt: string }) => {
    setActivePreset(p.prompt);
    setIntentInput(p.prompt);
    toast(`已载入业务需求: ${p.prompt}`);
  };

  const stepsMeta = [
    {
      num: 1,
      title: '第 1 步【识别】',
      sub: '找表与字段',
      summary: '找出要用哪几个表（客户、订单、贷款）及对应度量',
      icon: '🔍',
    },
    {
      num: 2,
      title: '第 2 步【找中心表】',
      sub: '拓扑定主表',
      summary: '确定谁是主表（客户表是公共汇聚中心）',
      icon: '🏛️',
    },
    {
      num: 3,
      title: '第 3 步【分头聚合】',
      sub: '核心防翻车算法',
      summary: '订单自己算自己的，贷款自己算自己的，杜绝笛卡尔积',
      icon: '🛡️',
    },
    {
      num: 4,
      title: '第 4 步【组装输出】',
      sub: '执行与Univer表',
      summary: '拼成最安全 CTE SQL 或 Polars 并直接生成 Univer 表格',
      icon: '📊',
    },
  ];

  return (
    <div className="space-y-6">
      {/* 顶部架构理念 Banner */}
      <div className="bg-gradient-to-r from-slate-900 via-indigo-950 to-slate-900 border border-indigo-500/30 rounded-xl p-5 text-white shadow-xl relative overflow-hidden">
        <div className="absolute right-0 top-0 bottom-0 w-96 bg-[radial-gradient(circle_at_top_right,_var(--tw-gradient-stops))] from-indigo-500/20 via-transparent to-transparent pointer-events-none" />
        <div className="relative z-10 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <div className="flex items-center space-x-2 text-indigo-400 font-mono text-xs font-semibold tracking-wider uppercase mb-1">
              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              <span>Ontology 2.0 语义拓扑解析与执行引擎</span>
            </div>
            <h2 className="text-xl font-bold tracking-tight text-white flex items-center space-x-2">
              <span>本体解析器：4 步直达安全报表</span>
            </h2>
            <p className="text-slate-300 text-sm mt-1 max-w-3xl leading-relaxed">
              从自然语言需求到 Univer 电子表格：自动识别表实体、拓扑判定中心主表、分头预聚合彻底解决
              <span className="text-amber-300 font-medium"> 扇出翻车陷阱 (Fan-Out Trap)</span>，最后拼成最安全的执行计划并渲染！
            </p>
          </div>
          <div className="flex items-center space-x-2 self-start md:self-center">
            {/* 模式切换按钮 */}
            <div className="bg-slate-800/80 p-0.5 rounded-lg border border-slate-700 flex items-center text-xs">
              <button
                onClick={() => setViewMode('step')}
                className={`px-3 py-1.5 rounded-md font-medium transition-all ${
                  viewMode === 'step'
                    ? 'bg-indigo-600 text-white shadow-xs'
                    : 'text-slate-400 hover:text-white'
                }`}
              >
                分步向导模式
              </button>
              <button
                onClick={() => setViewMode('full')}
                className={`px-3 py-1.5 rounded-md font-medium transition-all ${
                  viewMode === 'full'
                    ? 'bg-indigo-600 text-white shadow-xs'
                    : 'text-slate-400 hover:text-white'
                }`}
              >
                全景一览模式
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* 报表需求输入栏 & 预设选择 */}
      <div className="bg-white border border-slate-200 rounded-xl p-5 shadow-sm space-y-3">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <label className="text-sm font-semibold text-slate-800 flex items-center space-x-1.5">
            <span>💬 报表业务需求输入：</span>
            <span className="text-xs font-normal text-slate-500">(输入分析目标，本体大脑自动拆解推导)</span>
          </label>
          <div className="flex items-center space-x-2">
            <span className="text-xs text-slate-400">快捷用例:</span>
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
              placeholder="例如：按客户等级看 LTV 和逾期率"
              className="w-full text-sm px-4 py-2.5 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-indigo-500 font-medium text-slate-800 bg-slate-50/50"
            />
          </div>
          <button
            onClick={() => toast('本体解析器重新完成 4 步推导')}
            className="px-5 py-2.5 bg-indigo-600 hover:bg-indigo-700 text-white text-sm font-medium rounded-lg shadow-sm transition-colors flex items-center space-x-1.5 whitespace-nowrap"
          >
            <span>⚡ 智能解析</span>
          </button>
        </div>
      </div>

      {/* 🌟 核心 STEP 步骤条 (Stepper Navigation) */}
      <div className="bg-white border border-slate-200 rounded-xl p-4 shadow-sm">
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
          {stepsMeta.map((s) => {
            const isActive = activeStep === s.num;
            const isPassed = activeStep > s.num;
            return (
              <button
                key={s.num}
                onClick={() => {
                  setActiveStep(s.num as any);
                  if (viewMode === 'full') setViewMode('step');
                }}
                className={`text-left p-3.5 rounded-xl border-2 transition-all relative overflow-hidden flex flex-col justify-between ${
                  isActive
                    ? 'border-indigo-600 bg-indigo-50/40 shadow-sm shadow-indigo-500/10'
                    : isPassed
                    ? 'border-emerald-300 bg-emerald-50/30 hover:border-emerald-400'
                    : 'border-slate-200 bg-slate-50/40 hover:border-slate-300'
                }`}
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center space-x-2">
                    <span
                      className={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold font-mono ${
                        isActive
                          ? 'bg-indigo-600 text-white'
                          : isPassed
                          ? 'bg-emerald-500 text-white'
                          : 'bg-slate-200 text-slate-600'
                      }`}
                    >
                      {isPassed ? '✓' : s.num}
                    </span>
                    <span className="font-bold text-sm text-slate-900">{s.title}</span>
                  </div>
                  <span className="text-base">{s.icon}</span>
                </div>
                <div>
                  <div className="text-xs font-semibold text-indigo-700 mb-0.5">{s.sub}</div>
                  <div className="text-[11px] text-slate-500 line-clamp-2 leading-tight">{s.summary}</div>
                </div>
                {isActive && (
                  <div className="absolute bottom-0 left-0 right-0 h-1 bg-indigo-600" />
                )}
              </button>
            );
          })}
        </div>

        {/* 步骤流转导航按钮 */}
        {viewMode === 'step' && (
          <div className="flex items-center justify-between pt-3 mt-3 border-t border-slate-100 text-xs">
            <button
              onClick={() => setActiveStep((prev) => Math.max(1, prev - 1) as any)}
              disabled={activeStep === 1}
              className={`px-3 py-1.5 rounded-md border font-medium flex items-center space-x-1 ${
                activeStep === 1
                  ? 'border-slate-200 text-slate-300 cursor-not-allowed bg-slate-50'
                  : 'border-slate-300 text-slate-700 hover:bg-slate-100'
              }`}
            >
              <span>← 上一步</span>
            </button>
            <div className="text-slate-500 font-medium">
              当前演示：<span className="text-indigo-600 font-bold">{stepsMeta[activeStep - 1].title}</span> ({activeStep} / 4)
            </div>
            <button
              onClick={() => setActiveStep((prev) => Math.min(4, prev + 1) as any)}
              disabled={activeStep === 4}
              className={`px-4 py-1.5 rounded-md font-medium text-white shadow-xs flex items-center space-x-1 ${
                activeStep === 4
                  ? 'bg-slate-300 cursor-not-allowed'
                  : 'bg-indigo-600 hover:bg-indigo-700'
              }`}
            >
              <span>下一步 →</span>
            </button>
          </div>
        )}
      </div>

      {/* ========================================================================= */}
      {/* 🚀 STEP 1: 【识别】找出要用哪几个表（客户、订单、贷款）                     */}
      {/* ========================================================================= */}
      {(viewMode === 'full' || activeStep === 1) && (
        <div className="bg-white border border-slate-200 rounded-xl p-6 shadow-sm space-y-5">
          <div className="flex items-start justify-between pb-4 border-b border-slate-100">
            <div>
              <div className="flex items-center space-x-2">
                <span className="px-2 py-0.5 rounded bg-indigo-100 text-indigo-700 text-xs font-mono font-bold">
                  STEP 1
                </span>
                <h3 className="font-bold text-slate-900 text-base">
                  【智能识别】找出要用哪几个表（客户、订单、贷款）与对应字段
                </h3>
              </div>
              <p className="text-xs text-slate-500 mt-1">
                解析器从问句中提取实体词、维度词和度量词，自动关联物理数据表并绑定公式定义。
              </p>
            </div>
            <span className="text-xs bg-emerald-50 text-emerald-700 border border-emerald-200 px-2.5 py-1 rounded-full font-medium">
              ✓ 识别成功：3 张物理表命中
            </span>
          </div>

          {/* 词法拆解槽位展示 */}
          <div>
            <div className="text-xs font-bold text-slate-700 uppercase tracking-wider mb-2 font-mono">
              自然语言词槽识别与映射 (NLP Slot Mapping)
            </div>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              {(resolution.identifiedTokens || []).map((t, idx) => (
                <div key={idx} className="p-3 bg-slate-50 rounded-lg border border-slate-200">
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-bold text-slate-800 bg-white px-2 py-0.5 rounded border border-slate-200">
                      “{t.token}”
                    </span>
                    <span
                      className={`text-[10px] px-1.5 py-0.5 rounded font-mono font-bold ${
                        t.type === 'dim'
                          ? 'bg-sky-100 text-sky-800'
                          : 'bg-emerald-100 text-emerald-800'
                      }`}
                    >
                      {t.type === 'dim' ? '分组维度' : '业务指标'}
                    </span>
                  </div>
                  <div className="text-xs font-semibold text-indigo-900 mt-1">{t.label}</div>
                  <div className="text-[11px] text-slate-500 mt-0.5 font-mono">{t.detail}</div>
                  <div className="mt-2 text-[10px] text-slate-400">
                    物理归属表: <span className="font-bold text-slate-600">{t.targetEntity}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* 3 张物理表卡片卡片 */}
          <div>
            <div className="text-xs font-bold text-slate-700 uppercase tracking-wider mb-2 font-mono">
              所需物理表及字段定义 (Required Physical Tables)
            </div>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              {/* Table 1: Customer */}
              <div className="p-4 rounded-xl border-2 border-emerald-500 bg-emerald-50/20 shadow-xs relative">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-xs font-bold text-emerald-800 font-mono">1. Customer 客户表</span>
                  <span className="text-[10px] px-1.5 py-0.5 bg-emerald-100 text-emerald-800 rounded font-medium">
                    根实体 / 维度承载
                  </span>
                </div>
                <div className="text-xs text-slate-600 mb-2">
                  记录客户档案信息，承载分析分组的核心维度。
                </div>
                <div className="space-y-1.5 bg-white p-2.5 rounded-lg border border-emerald-100 text-xs">
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-slate-700">id (主键 PK)</span>
                    <span className="text-[10px] text-slate-400">唯一标识客户</span>
                  </div>
                  <div className="flex items-center justify-between text-indigo-700 font-semibold bg-indigo-50/60 px-1 py-0.5 rounded">
                    <span className="font-mono">level (客户等级)</span>
                    <span className="text-[10px]">命中维度: VIP/白金等</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-slate-700">region (所属大区)</span>
                    <span className="text-[10px] text-slate-400">华东/华南等</span>
                  </div>
                </div>
              </div>

              {/* Table 2: Order */}
              <div className="p-4 rounded-xl border-2 border-indigo-400 bg-indigo-50/20 shadow-xs relative">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-xs font-bold text-indigo-900 font-mono">2. Order 订单交易表</span>
                  <span className="text-[10px] px-1.5 py-0.5 bg-indigo-100 text-indigo-800 rounded font-medium">
                    1:N 流水表
                  </span>
                </div>
                <div className="text-xs text-slate-600 mb-2">
                  记录客户的消费流水，用于计算客户总生命周期价值 (LTV)。
                </div>
                <div className="space-y-1.5 bg-white p-2.5 rounded-lg border border-indigo-100 text-xs">
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-slate-700">customer_id (外键 FK)</span>
                    <span className="text-[10px] text-indigo-600 font-bold">➔ 关联客户 id</span>
                  </div>
                  <div className="flex items-center justify-between text-emerald-700 font-semibold bg-emerald-50/60 px-1 py-0.5 rounded">
                    <span className="font-mono">amount - discount</span>
                    <span className="text-[10px]">命中度量: LTV净额</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-slate-700">order_date (下单时间)</span>
                    <span className="text-[10px] text-slate-400">流水时间戳</span>
                  </div>
                </div>
              </div>

              {/* Table 3: Loan */}
              <div className="p-4 rounded-xl border-2 border-amber-400 bg-amber-50/20 shadow-xs relative">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-xs font-bold text-amber-900 font-mono">3. Loan 借贷授信表</span>
                  <span className="text-[10px] px-1.5 py-0.5 bg-amber-100 text-amber-800 rounded font-medium">
                    1:N 授信表
                  </span>
                </div>
                <div className="text-xs text-slate-600 mb-2">
                  记录客户的金融信贷与还款风险，用于统计逾期率与放款总额。
                </div>
                <div className="space-y-1.5 bg-white p-2.5 rounded-lg border border-amber-100 text-xs">
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-slate-700">customer_id (外键 FK)</span>
                    <span className="text-[10px] text-amber-700 font-bold">➔ 关联客户 id</span>
                  </div>
                  <div className="flex items-center justify-between text-amber-800 font-semibold bg-amber-50/60 px-1 py-0.5 rounded">
                    <span className="font-mono">is_overdue (0/1)</span>
                    <span className="text-[10px]">命中度量: 逾期率</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-slate-700">principal (贷款本金)</span>
                    <span className="text-[10px] text-slate-400">授信金额</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ========================================================================= */}
      {/* 🏛️ STEP 2: 【找中心表】谁是主表？（客户表是公共中心）                     */}
      {/* ========================================================================= */}
      {(viewMode === 'full' || activeStep === 2) && (
        <div className="bg-white border border-slate-200 rounded-xl p-6 shadow-sm space-y-5">
          <div className="flex items-start justify-between pb-4 border-b border-slate-100">
            <div>
              <div className="flex items-center space-x-2">
                <span className="px-2 py-0.5 rounded bg-indigo-100 text-indigo-700 text-xs font-mono font-bold">
                  STEP 2
                </span>
                <h3 className="font-bold text-slate-900 text-base">
                  【找中心表】谁是主表？（客户表是公共中心）
                </h3>
              </div>
              <p className="text-xs text-slate-500 mt-1">
                分析实体间的外键连通拓扑 DAG，确定以哪个表为公共中心汇聚计算。
              </p>
            </div>
            <span className="text-xs bg-indigo-50 text-indigo-700 border border-indigo-200 px-2.5 py-1 rounded-full font-medium">
              中心裁决：Customer (客户表) 为主表
            </span>
          </div>

          {/* 灵魂三问解析 */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="p-3.5 bg-rose-50/60 border border-rose-200 rounded-xl text-xs space-y-1.5">
              <div className="font-bold text-rose-800 flex items-center space-x-1">
                <span>❌ 能以【订单表】为主表吗？</span>
              </div>
              <p className="text-rose-700 leading-relaxed">
                <b>不能！</b> 订单表里根本没有贷款数据！如果以订单表驱动，很多没有订单但有贷款的客户全部丢失，且订单与贷款完全对不上。
              </p>
            </div>

            <div className="p-3.5 bg-rose-50/60 border border-rose-200 rounded-xl text-xs space-y-1.5">
              <div className="font-bold text-rose-800 flex items-center space-x-1">
                <span>❌ 能以【贷款表】为主表吗？</span>
              </div>
              <p className="text-rose-700 leading-relaxed">
                <b>也不能！</b> 贷款表里根本没有交易订单流水！两张业务事实表之间没有直接外键关系。
              </p>
            </div>

            <div className="p-3.5 bg-emerald-50 border border-emerald-300 rounded-xl text-xs space-y-1.5">
              <div className="font-bold text-emerald-900 flex items-center space-x-1">
                <span>✅ 为什么【客户表】是唯一中心？</span>
              </div>
              <p className="text-emerald-800 leading-relaxed">
                <b>拓扑唯一连通！</b> 订单表与贷款表都拥有外键 <code>customer_id</code> 指向客户表！客户表是它们的公共祖先（Root Entity），聚合基准对齐粒度就是 <code>customer_id</code>。
              </p>
            </div>
          </div>

          {/* 拓扑图可视化 DAG */}
          <div className="p-5 bg-slate-900 text-white rounded-xl relative overflow-hidden">
            <div className="text-xs font-mono text-slate-400 mb-3 flex items-center justify-between">
              <span>实体关系拓扑 DAG (Topological DAG Graph)</span>
              <span className="text-emerald-400 font-bold">粒度对齐中枢: customer.id</span>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 items-center">
              {/* Left Branch: Order */}
              <div className="p-3.5 bg-slate-800/90 border border-indigo-500/40 rounded-xl text-center">
                <div className="text-[10px] font-mono text-indigo-400 uppercase">Child Entity (1:N)</div>
                <div className="text-sm font-bold text-white mt-1">Order 订单表</div>
                <div className="text-xs text-slate-400 mt-1">FK: customer_id ➔ Customer.id</div>
                <div className="mt-2 text-[11px] text-emerald-400 bg-emerald-950/60 border border-emerald-500/30 px-2 py-0.5 rounded font-mono">
                  度量: LTV
                </div>
              </div>

              {/* Center Root: Customer */}
              <div className="p-4 bg-gradient-to-b from-emerald-600 to-emerald-700 border-2 border-emerald-300 rounded-xl text-center shadow-lg relative">
                <div className="absolute -top-2.5 left-1/2 -translate-x-1/2 bg-amber-400 text-slate-900 text-[10px] font-bold px-2 py-0.5 rounded-full uppercase tracking-wider">
                  公共中心主表 (Root Entity)
                </div>
                <div className="text-base font-bold text-white mt-1">Customer 客户表</div>
                <div className="text-xs text-emerald-100 mt-1 font-mono">主键: id | 粒度: customer.id</div>
                <div className="text-xs text-emerald-200 mt-1">承载维度: level (等级), region (大区)</div>
              </div>

              {/* Right Branch: Loan */}
              <div className="p-3.5 bg-slate-800/90 border border-amber-500/40 rounded-xl text-center">
                <div className="text-[10px] font-mono text-amber-400 uppercase">Child Entity (1:N)</div>
                <div className="text-sm font-bold text-white mt-1">Loan 贷款表</div>
                <div className="text-xs text-slate-400 mt-1">FK: customer_id ➔ Customer.id</div>
                <div className="mt-2 text-[11px] text-amber-400 bg-amber-950/60 border border-amber-500/30 px-2 py-0.5 rounded font-mono">
                  度量: 逾期率 (OverdueRate)
                </div>
              </div>
            </div>

            <div className="mt-4 pt-3 border-t border-slate-800 text-center text-xs text-slate-400 font-mono">
              拓扑连通路径：Order (N) ◄── [customer_id] ──► <span className="text-emerald-400 font-bold">Customer (1)</span> ◄── [customer_id] ──► Loan (N)
            </div>
          </div>
        </div>
      )}

      {/* ========================================================================= */}
      {/* 🛡️ STEP 3: 【分头聚合】核心防翻车算法（订单自己算自己的，贷款自己算自己的） */}
      {/* ========================================================================= */}
      {(viewMode === 'full' || activeStep === 3) && (
        <div className="bg-white border border-slate-200 rounded-xl p-6 shadow-sm space-y-5">
          <div className="flex items-start justify-between pb-4 border-b border-slate-100">
            <div>
              <div className="flex items-center space-x-2">
                <span className="px-2 py-0.5 rounded bg-indigo-100 text-indigo-700 text-xs font-mono font-bold">
                  STEP 3
                </span>
                <h3 className="font-bold text-slate-900 text-base">
                  【分头聚合】核心防翻车算法（订单自己算自己的，贷款自己算自己的）
                </h3>
              </div>
              <p className="text-xs text-slate-500 mt-1">
                大数报表中最隐蔽的致命陷阱：<span className="text-rose-600 font-bold">扇出/笛卡尔积陷阱 (Fan-Out / Chasm Trap)</span> 与本体粒度对齐算法解法。
              </p>
            </div>
            <span className="text-xs bg-emerald-50 text-emerald-700 border border-emerald-200 px-2.5 py-1 rounded-full font-medium">
              ✓ 防护激活：0 虚增，0 重复
            </span>
          </div>

          {/* 翻车原因与防翻车算法解析 */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {/* 翻车现场 */}
            <div className="p-4 rounded-xl border-2 border-rose-300 bg-rose-50/30 space-y-3">
              <div className="flex items-center justify-between">
                <div className="font-bold text-rose-800 text-sm flex items-center space-x-1.5">
                  <span className="text-base">💥</span>
                  <span>传统直接多表 JOIN（翻车现场）</span>
                </div>
                <span className="text-[10px] bg-rose-100 text-rose-800 px-2 py-0.5 rounded font-mono font-bold">
                  笛卡尔积膨胀
                </span>
              </div>
              <p className="text-xs text-rose-700 leading-relaxed">
                假设客户“阿里巴巴”有 <b>2 笔订单</b> 和 <b>1 笔贷款</b>。如果写常规 SQL：<br />
                <code className="text-[11px] bg-rose-100 px-1 py-0.5 rounded font-mono">
                  FROM Customer JOIN Order JOIN Loan
                </code><br />
                两表交叉匹配，原本 1 笔 50 万元的贷款会因为被 2 笔订单各自匹配一次，在结果集中<b>变成了 2 行</b>！直接导致贷款金额被算成 100 万元，翻倍虚高！
              </p>
              <div className="overflow-x-auto border border-rose-200 rounded-lg bg-white">
                <table className="w-full text-[11px] text-left">
                  <thead className="bg-rose-100/70 text-rose-900">
                    <tr>
                      <th className="p-1.5">客户</th>
                      <th className="p-1.5">订单金额</th>
                      <th className="p-1.5">贷款本金</th>
                      <th className="p-1.5">翻车错误</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-rose-100 text-slate-700">
                    <tr className="bg-rose-50/20">
                      <td className="p-1.5 font-medium">阿里巴巴</td>
                      <td className="p-1.5">¥ 305,000</td>
                      <td className="p-1.5">¥ 500,000</td>
                      <td className="p-1.5 text-slate-500">匹配第 1 笔</td>
                    </tr>
                    <tr className="bg-rose-100/40">
                      <td className="p-1.5 font-medium">阿里巴巴</td>
                      <td className="p-1.5">¥ 175,000</td>
                      <td className="p-1.5 text-rose-600 font-bold">¥ 500,000 (重复!)</td>
                      <td className="p-1.5 text-rose-600 font-bold">❌ 贷款重复翻倍!</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>

            {/* 安全算法 */}
            <div className="p-4 rounded-xl border-2 border-emerald-400 bg-emerald-50/30 space-y-3">
              <div className="flex items-center justify-between">
                <div className="font-bold text-emerald-900 text-sm flex items-center space-x-1.5">
                  <span className="text-base">🛡️</span>
                  <span>本体分头聚合 (Grain Alignment 安全算法)</span>
                </div>
                <span className="text-[10px] bg-emerald-100 text-emerald-800 px-2 py-0.5 rounded font-mono font-bold">
                  数学级纯净
                </span>
              </div>
              <p className="text-xs text-emerald-800 leading-relaxed">
                <b>解决之道：订单自己算自己的，贷款自己算自己的！</b><br />
                1. 订单表自己按 <code>customer_id</code> 聚合，算出真实的 LTV 净额，<b>浓缩为唯一的 1 行</b>；<br />
                2. 贷款表自己按 <code>customer_id</code> 聚合，算出真实的逾期率与本金，<b>也浓缩为唯一的 1 行</b>；<br />
                3. 两张浓缩完的结果，再和客户表做 1:1 安全连接，绝不产生交叉重复！
              </p>
              <div className="overflow-x-auto border border-emerald-200 rounded-lg bg-white">
                <table className="w-full text-[11px] text-left">
                  <thead className="bg-emerald-100/70 text-emerald-900">
                    <tr>
                      <th className="p-1.5">客户</th>
                      <th className="p-1.5">对齐真实 LTV</th>
                      <th className="p-1.5">真实逾期率</th>
                      <th className="p-1.5">对齐结果</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-emerald-100 text-slate-700">
                    <tr className="bg-emerald-50/20">
                      <td className="p-1.5 font-medium">阿里巴巴</td>
                      <td className="p-1.5 font-bold text-emerald-700">¥ 480,000</td>
                      <td className="p-1.5">0.0%</td>
                      <td className="p-1.5 text-emerald-700 font-bold">✅ 严格 1:1，准确度 100%</td>
                    </tr>
                    <tr className="bg-emerald-50/20">
                      <td className="p-1.5 font-medium">腾讯深圳</td>
                      <td className="p-1.5 font-bold text-emerald-700">¥ 480,000</td>
                      <td className="p-1.5">0.0%</td>
                      <td className="p-1.5 text-emerald-700 font-bold">✅ 订单浓缩1行，贷款1行</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </div>

          {/* 算法执行三部曲 */}
          <div className="bg-slate-50 border border-slate-200 rounded-xl p-4">
            <div className="text-xs font-bold text-slate-800 uppercase tracking-wider mb-2 font-mono">
              分头聚合三阶段执行管道 (Grain Alignment Pipeline)
            </div>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3 text-xs">
              <div className="bg-white p-3 rounded-lg border border-slate-200">
                <div className="font-bold text-indigo-700 mb-1">阶段 1: 订单独立聚合</div>
                <div className="text-slate-500 text-[11px]">
                  在 <code>orders</code> 表中执行 <code>GROUP BY customer_id</code>，计算净 LTV 与订单数。浓缩为 <code>order_grain</code>。
                </div>
              </div>
              <div className="bg-white p-3 rounded-lg border border-slate-200">
                <div className="font-bold text-indigo-700 mb-1">阶段 2: 贷款独立聚合</div>
                <div className="text-slate-500 text-[11px]">
                  在 <code>loans</code> 表中执行 <code>GROUP BY customer_id</code>，计算真实逾期标记与总本金。浓缩为 <code>loan_grain</code>。
                </div>
              </div>
              <div className="bg-white p-3 rounded-lg border border-slate-200">
                <div className="font-bold text-emerald-700 mb-1">阶段 3: 零膨胀汇聚连接</div>
                <div className="text-slate-500 text-[11px]">
                  以 <code>customers</code> 为主表，分别与两个单行 CTE 依据 <code>customer_id</code> 安全连接，再按等级分组统计。
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ========================================================================= */}
      {/* 📊 STEP 4: 【组装输出】拼成最安全的 SQL 或直接生成 Univer 表格！            */}
      {/* ========================================================================= */}
      {(viewMode === 'full' || activeStep === 4) && (
        <div className="bg-white border border-slate-200 rounded-xl p-6 shadow-sm space-y-6">
          <div className="flex items-start justify-between pb-4 border-b border-slate-100">
            <div>
              <div className="flex items-center space-x-2">
                <span className="px-2 py-0.5 rounded bg-indigo-100 text-indigo-700 text-xs font-mono font-bold">
                  STEP 4
                </span>
                <h3 className="font-bold text-slate-900 text-base">
                  【组装输出】拼成最安全 CTE SQL / Polars 并实时生成 Univer 电子表格！
                </h3>
              </div>
              <p className="text-xs text-slate-500 mt-1">
                静态编译为标准 ANSI CTE SQL 与 Rust Polars 表达式，并自动灌入 Univer 电子表格引擎即刻交互。
              </p>
            </div>
            <div className="flex items-center space-x-2">
              {onApplyAsRequirement && (
                <button
                  onClick={() => onApplyAsRequirement(resolution)}
                  className="px-3 py-1.5 bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-medium rounded-md shadow-xs transition-colors flex items-center space-x-1"
                >
                  <span>🚀 转为正式报表需求</span>
                </button>
              )}
            </div>
          </div>

          {/* 执行代码检视器：ANSI CTE SQL vs Rust Polars */}
          <div className="border border-slate-200 rounded-xl overflow-hidden bg-slate-950 text-slate-200">
            <div className="border-b border-slate-800 bg-slate-900 px-4 py-2.5 flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <span className="text-xs font-bold text-slate-200">最优执行计划 (Compiled Execution Plan)</span>
                <span className="text-[10px] text-slate-400">| 零扇出安全保证</span>
              </div>
              <div className="flex items-center space-x-2">
                <div className="flex items-center space-x-1 bg-slate-800 p-0.5 rounded-md">
                  <button
                    onClick={() => setCodeTab('sql')}
                    className={`text-xs px-2.5 py-1 rounded font-mono transition-all ${
                      codeTab === 'sql'
                        ? 'bg-indigo-600 text-white font-bold'
                        : 'text-slate-400 hover:text-white'
                    }`}
                  >
                    Grain-Aligned CTE SQL
                  </button>
                  <button
                    onClick={() => setCodeTab('polars')}
                    className={`text-xs px-2.5 py-1 rounded font-mono transition-all ${
                      codeTab === 'polars'
                        ? 'bg-indigo-600 text-white font-bold'
                        : 'text-slate-400 hover:text-white'
                    }`}
                  >
                    Rust + Polars 向量化表达式
                  </button>
                </div>
                <button
                  onClick={() => {
                    const text = codeTab === 'sql' ? resolution.grainAlignedSql : resolution.polarsCode;
                    navigator.clipboard?.writeText(text);
                    toast(`已复制 ${codeTab.toUpperCase()} 代码`);
                  }}
                  className="text-xs px-2.5 py-1 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded border border-slate-700 transition-colors"
                >
                  📋 复制代码
                </button>
              </div>
            </div>

            <div className="p-4 font-mono text-xs overflow-x-auto max-h-64">
              <pre className="leading-relaxed text-indigo-300">
                <code>{codeTab === 'sql' ? resolution.grainAlignedSql : resolution.polarsCode}</code>
              </pre>
            </div>
          </div>

          {/* 🌟 Univer 电子表格实时嵌入与交互渲染 */}
          <div className="border border-slate-200 rounded-xl overflow-hidden shadow-xs">
            <div className="px-4 py-3 bg-slate-50 border-b border-slate-200 flex items-center justify-between">
              <div>
                <div className="flex items-center space-x-2">
                  <span className="font-bold text-slate-800 text-sm">
                    Univer 电子表格高保真呈现 (真实数据交互)
                  </span>
                  <span className="text-[10px] bg-emerald-100 text-emerald-800 px-2 py-0.5 rounded font-mono font-medium">
                    计算完成: {resolution.outputData.rows.length} 等级分组
                  </span>
                </div>
                <div className="text-xs text-slate-500 mt-0.5">
                  已按分头聚合对齐合并，包含千分位格式化、汇总合计行与冻结表头。可在表格中任意选区与划选统计。
                </div>
              </div>
              <div className="flex items-center space-x-2">
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
            </div>

            <div className="p-4 bg-slate-100/50">
              <UniverViewer
                snapshot={resolution.univerSnapshot}
                title={`客户等级 LTV 与逾期率分析表 (Ontology 编译执行)`}
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
