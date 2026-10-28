import React from 'react';

export type NavView = 'workbench' | 'datasources' | 'bizmodel' | 'analysis' | 'runs';

interface SidebarProps {
  currentView: NavView;
  onNavigate: (view: NavView) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({ currentView, onNavigate }) => {
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
          <div className="tiny">需求 → 模板 → 结果</div>
        </div>
      </div>
      <button
        className={`nav-item ${currentView === 'workbench' ? 'active' : ''}`}
        onClick={() => onNavigate('workbench')}
      >
        工作台
      </button>
      <button
        className={`nav-item ${currentView === 'datasources' ? 'active' : ''}`}
        onClick={() => onNavigate('datasources')}
      >
        数据源
      </button>
      <button
        className={`nav-item ${currentView === 'bizmodel' ? 'active' : ''}`}
        onClick={() => onNavigate('bizmodel')}
      >
        业务知识 (BizModel)
      </button>
      <button
        className={`nav-item ${currentView === 'analysis' ? 'active' : ''}`}
        onClick={() => onNavigate('analysis')}
      >
        归因分析
      </button>
      <button
        className={`nav-item ${currentView === 'runs' ? 'active' : ''}`}
        onClick={() => onNavigate('runs')}
      >
        运行记录
      </button>
      <div className="side-foot">
        单人工作区 · 本地引擎
        <br />
        <span id="rpx-state" style={{ color: '#eef3f6' }}>
          rpx ● 就绪
        </span>
      </div>
    </aside>
  );
};
