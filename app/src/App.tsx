import React, { useEffect, useState } from 'react';
import { Sidebar, type NavView } from './components/Sidebar';
import { ToastProvider } from './components/Toast';
import { WorkbenchView } from './views/WorkbenchView';
import { LibraryView } from './views/LibraryView';
import { RequirementView } from './views/RequirementView';
import { RunsView } from './views/RunsView';
import { ReportView } from './views/ReportView';
import { OntologyView } from './views/OntologyView';
import { DatasourcesView } from './views/DatasourcesView';
import { BizModelView } from './views/BizModelView';
import { AnalysisView } from './views/AnalysisView';
import { DesignerView } from './views/DesignerView';

/**
 * 路由（**根路由是工作台**，与 master 一致）：
 * - `#/`            工作台：该处理什么（最近运行 / 失败 / 一句话找报表）
 * - `#/requirement` 需求：一句话生成报表
 * - `#/library`     报表库：59 份报表，检索 / 运行 / 看产物
 * - `#/runs`        运行记录：跨报表的运行流水（台账 `rpt_run`）
 * - `#/report/:id`  单张报表：模板 / 运行 / 历史 / 产物
 * - `#/design/:id`  设计器（轻量版）：读 template.json 只读渲染 + 少量单元格编辑；重活深链 5288
 * - `#/bizmodel`    业务知识：实体 / 维度 / 指标 / 规则 / 金样例
 * - `#/datasources` 数据源：真连库（注册表 + 探测）
 * - `#/ontology`    本体解析（四步，仍是示例夹具）
 * - `#/analysis`    归因分析：多步受控拆解
 *
 * 与 master 的差异：master 有「需求（Requirement）」这一层（`#/req/:id`），
 * Rust 侧还没有对应端点与表，所以这一版**不假装有**（见 docs/rpx-semantic-port.md）。
 * 等需求层接上再补 `#/req/:id` 与 `#/req/:id/design`。
 */
export const App: React.FC = () => {
  const [route, setRoute] = useState<string>(() => window.location.hash || '#/');

  useEffect(() => {
    const onHash = () => setRoute(window.location.hash || '#/');
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);

  const navigate = (hash: string) => {
    window.location.hash = hash;
  };

  const clean = route.replace(/^#/, '');
  const reportMatch = clean.match(/^\/report\/([^/?]+)/);
  const designMatch = clean.match(/^\/design\/([^/?]+)/);

  let currentNav: NavView = 'workbench';
  if (clean.startsWith('/requirement')) currentNav = 'requirement';
  else if (clean.startsWith('/library')) currentNav = 'library';
  else if (clean.startsWith('/runs')) currentNav = 'runs';
  else if (clean.startsWith('/report/')) currentNav = 'library';
  else if (clean.startsWith('/design/')) currentNav = 'library';
  else if (clean.startsWith('/ontology')) currentNav = 'ontology';
  else if (clean.startsWith('/datasources')) currentNav = 'datasources';
  else if (clean.startsWith('/bizmodel')) currentNav = 'bizmodel';
  else if (clean.startsWith('/analysis')) currentNav = 'analysis';

  const content = () => {
    if (designMatch) {
      const id = decodeURIComponent(designMatch[1]);
      return <DesignerView reportId={id} onBack={() => navigate(`#/report/${encodeURIComponent(id)}`)} />;
    }
    if (reportMatch) {
      const id = decodeURIComponent(reportMatch[1]);
      return (
        <ReportView
          reportId={id}
          onBack={() => navigate('#/library')}
          onDesign={(rid) => navigate(`#/design/${encodeURIComponent(rid)}`)}
        />
      );
    }
    if (clean === '/requirement') {
      return <RequirementView onOpenReport={(id) => navigate(`#/report/${encodeURIComponent(id)}`)} />;
    }
    if (clean === '/library' || clean.startsWith('/library?')) {
      return <LibraryView onOpenReport={(id) => navigate(`#/report/${encodeURIComponent(id)}`)} />;
    }
    if (clean === '/runs') {
      return <RunsView onOpenReport={(id) => navigate(`#/report/${encodeURIComponent(id)}`)} />;
    }
    if (clean === '/ontology') {
      return <OntologyView />;
    }
    if (clean === '/datasources') {
      return <DatasourcesView />;
    }
    if (clean === '/bizmodel') {
      return <BizModelView />;
    }
    if (clean === '/analysis') {
      return <AnalysisView />;
    }
    return (
      <WorkbenchView
        onOpenReport={(id) => navigate(`#/report/${encodeURIComponent(id)}`)}
        onNavigate={navigate}
      />
    );
  };

  return (
    <ToastProvider>
      <div className="app">
        <Sidebar
          currentView={currentNav}
          onNavigate={(view) => {
            if (view === 'workbench') navigate('#/');
            else if (view === 'requirement') navigate('#/requirement');
            else if (view === 'library') navigate('#/library');
            else if (view === 'runs') navigate('#/runs');
            else if (view === 'ontology') navigate('#/ontology');
            else if (view === 'datasources') navigate('#/datasources');
            else if (view === 'bizmodel') navigate('#/bizmodel');
            else if (view === 'analysis') navigate('#/analysis');
          }}
        />
        <main className="main narrow" id="main">
          {content()}
        </main>
      </div>
    </ToastProvider>
  );
};
