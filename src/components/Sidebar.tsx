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
          <div className="tiny" style={{ letterSpacing: '0.08em', opacity: 0.9 }}>
            一句话生成报表 · 演化引擎
          </div>
        </div>
      </div>
      <button
        className={`nav-item ${currentView === 'workbench' ? 'active' : ''}`}
        onClick={() => onNavigate('workbench')}
      >
        <span style={{ opacity: 0.85 }}>⚡</span> 工作台 (Workbench)
      </button>
      <button
        className={`nav-item ${currentView === 'datasources' ? 'active' : ''}`}
        onClick={() => onNavigate('datasources')}
      >
        <span style={{ opacity: 0.85 }}>🗄️</span> 数据源 (Datasources)
      </button>
      <button
        className={`nav-item ${currentView === 'bizmodel' ? 'active' : ''}`}
        onClick={() => onNavigate('bizmodel')}
      >
        <span style={{ opacity: 0.85 }}>🧠</span> 业务知识 (BizModel)
      </button>
      <button
        className={`nav-item ${currentView === 'analysis' ? 'active' : ''}`}
        onClick={() => onNavigate('analysis')}
      >
        <span style={{ opacity: 0.85 }}>🔍</span> 归因分析 (Attribution)
      </button>
      <button
        className={`nav-item ${currentView === 'runs' ? 'active' : ''}`}
        onClick={() => onNavigate('runs')}
      >
        <span style={{ opacity: 0.85 }}>📋</span> 运行记录 (Archive)
      </button>
      <div className="side-foot">
        <div style={{ fontWeight: 600, color: '#f8fafc', marginBottom: '2px' }}>
          企业级安全沙箱 · 本地执行
        </div>
        <div style={{ fontSize: '10.5px', color: '#94a3b8' }}>
          双向人机核准 · Safe CTE · 杜绝笛卡尔积
        </div>
        <div style={{ marginTop: '6px', display: 'flex', alignItems: 'center', gap: '6px' }}>
          <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: '#10b981' }} />
          <span id="rpx-state" style={{ color: '#e2e8f0', fontSize: '11px', fontWeight: 600 }}>
            RPX Engine 就绪
          </span>
        </div>
      </div>
    </aside>
  );
};
