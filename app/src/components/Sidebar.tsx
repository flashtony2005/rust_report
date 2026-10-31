import React from 'react';
import { useBackendStatus } from '../services/useBackend';

export type NavView =
  | 'workbench'
  | 'requirement'
  | 'library'
  | 'runs'
  | 'bizmodel'
  | 'datasources'
  | 'ontology'
  | 'analysis';

interface SidebarProps {
  currentView: NavView;
  onNavigate: (view: NavView) => void;
}

/** 待后端的页面：标出来，别让人点了才发现是空的。 */
const PENDING: NavView[] = [];

/**
 * 顺序就是工作流：先在**工作台**看该处理什么，再去**报表库**挑一份，跑完在**运行记录**里核对。
 * 业务知识 / 数据源是在此之前要打的地基，本体解析与归因是"想不通时"的工具。
 */
const LABEL: Record<NavView, string> = {
  workbench: '工作台',
  requirement: '需求',
  library: '报表库',
  runs: '运行记录',
  bizmodel: '业务知识',
  datasources: '数据源',
  ontology: '本体解析',
  analysis: '归因分析',
};

export const Sidebar: React.FC<SidebarProps> = ({ currentView, onNavigate }) => {
  // 这一颗灯是唯一真实信号：它绿着才算连着 Rust 引擎。
  const backend = useBackendStatus();
  const dot =
    backend.state === 'ok' ? '#7ddc9a' : backend.state === 'down' ? '#f0a978' : '#9aa7b3';
  const text =
    backend.state === 'ok'
      ? `rpx ${backend.reports ?? '?'} 份`
      : backend.state === 'down'
        ? 'rpx 未连接'
        : 'rpx 连接中';

  return (
    <aside className="side">
      <div className="brand">
        <img
          className="brand-mark"
          src="/logo-mark-light.png"
          alt="AI 报表引擎"
          title="AI 报表引擎（Report Studio 是它的工作台）"
        />
        <div>
          <div className="brand-name">Report Studio</div>
          <div className="tiny">报表库 → 运行 → 产物</div>
        </div>
      </div>

      {(Object.keys(LABEL) as NavView[]).map((view) => (
        <button
          key={view}
          className={`nav-item ${currentView === view ? 'active' : ''}`}
          onClick={() => onNavigate(view)}
          title={
            view === 'ontology'
              ? '四步把一句话拆成安全 SQL（示例夹具）'
              : view === 'datasources'
                ? '真连库：注册表 + 探测（列画像 / 抽样 / 主键）'
                : view === 'bizmodel'
                  ? '从库结构发现的实体/关系/指标/维度（confirmed 只能人工晋升）'
                : PENDING.includes(view)
                  ? '语义层端点还没到 Rust，页面只说明缺口'
                  : '服务端 8090 上的实例库'
          }
        >
          {LABEL[view]}
          {PENDING.includes(view) ? (
            <span className="tiny" style={{ marginLeft: 6, opacity: 0.7 }}>待后端</span>
          ) : null}
        </button>
      ))}

      <div className="side-foot">
        单人工作区 · 本地引擎
        <br />
        <button
          className="linklike"
          style={{ color: dot }}
          onClick={backend.retry}
          title={
            backend.state === 'down'
              ? `点此重试。${backend.error || ''}`
              : 'Rust 服务端（8090）连通性；点此刷新'
          }
        >
          ● {text}
          {backend.state === 'down' ? '（重试）' : ''}
        </button>
        {backend.state === 'down' && backend.error ? (
          <div className="tiny" style={{ color: '#f0a978', marginTop: 4, lineHeight: 1.5 }}>
            {backend.error}
          </div>
        ) : null}
      </div>
    </aside>
  );
};
