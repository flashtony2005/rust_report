import React, { useState, useEffect } from 'react';
import { Sidebar, NavView } from './components/Sidebar';
import { WorkbenchView } from './views/WorkbenchView';
import { RequirementView } from './views/RequirementView';
import { DesignerView } from './views/DesignerVIew';
import { RunsView } from './views/RunsView';
import { DatasourcesView } from './views/DatasourcesView';
import { AnalysisView } from './views/AnalysisView';
import { BizModelView } from './views/BizModelView';
import { NewRequirementModal } from './components/modals/NewRequirementModal';
import { LlmSettingsModal } from './components/modals/LlmSettingsModal';
import { store } from './services/store';

export const App: React.FC = () => {
  // Sync state with hash route
  const [route, setRoute] = useState<string>(() => window.location.hash || '#/');
  const [, setTick] = useState(0);

  // Modals
  const [showNewReqModal, setShowNewReqModal] = useState(false);
  const [showLlmModal, setShowLlmModal] = useState(false);

  useEffect(() => {
    const handleHash = () => setRoute(window.location.hash || '#/');
    window.addEventListener('hashchange', handleHash);
    const unsubscribe = store.subscribe(() => setTick((t) => t + 1));
    return () => {
      window.removeEventListener('hashchange', handleHash);
      unsubscribe();
    };
  }, []);

  const navigate = (hash: string) => {
    window.location.hash = hash;
  };

  // Determine current active view for sidebar
  let currentNav: NavView = 'workbench';
  if (route.startsWith('#/datasources')) currentNav = 'datasources';
  else if (route.startsWith('#/bizmodel')) currentNav = 'bizmodel';
  else if (route.startsWith('#/analysis')) currentNav = 'analysis';
  else if (route.startsWith('#/runs')) currentNav = 'runs';

  // Parse route parameters
  const renderContent = () => {
    const clean = route.replace(/^#/, '');

    // Requirement designer: /req/:id/design
    const designMatch = clean.match(/^\/req\/([^/]+)\/design$/);
    if (designMatch) {
      return (
        <DesignerView
          reqId={designMatch[1]}
          onBack={() => navigate(`#/req/${designMatch[1]}`)}
          onOpenRuns={(id, runId) =>
            navigate(`#/req/${id}/runs${runId ? `?run=${runId}` : ''}`)
          }
        />
      );
    }

    // Requirement runs: /req/:id/runs
    const reqRunsMatch = clean.match(/^\/req\/([^/]+)\/runs/);
    if (reqRunsMatch) {
      const q = new URLSearchParams(clean.split('?')[1] || '');
      const initialRun = q.get('run') || undefined;
      return (
        <RunsView
          reqId={reqRunsMatch[1]}
          initialRunId={initialRun}
          onBack={() => navigate(`#/req/${reqRunsMatch[1]}`)}
          onOpenRequirement={(id) => navigate(`#/req/${id}`)}
        />
      );
    }

    // Requirement detail: /req/:id
    const reqMatch = clean.match(/^\/req\/([^/]+)$/);
    if (reqMatch) {
      return (
        <RequirementView
          reqId={reqMatch[1]}
          onBack={() => navigate('#/')}
          onOpenDesigner={(id) => navigate(`#/req/${id}/design`)}
          onOpenRuns={(id, runId) =>
            navigate(`#/req/${id}/runs${runId ? `?run=${runId}` : ''}`)
          }
        />
      );
    }

    // Global pages
    if (clean === '/datasources') {
      return <DatasourcesView />;
    }
    if (clean === '/bizmodel') {
      return <BizModelView />;
    }
    if (clean === '/analysis') {
      return <AnalysisView />;
    }
    if (clean === '/runs') {
      return (
        <RunsView
          onOpenRequirement={(id) => navigate(`#/req/${id}`)}
        />
      );
    }

    // Default: Workbench
    return (
      <WorkbenchView
        onOpenRequirement={(id) => navigate(`#/req/${id}`)}
        onOpenRun={(reqId, runId) => navigate(`#/req/${reqId}/runs?run=${runId}`)}
        onNewRequirement={() => setShowNewReqModal(true)}
        onOpenLlmModal={() => setShowLlmModal(true)}
      />
    );
  };

  return (
    <div className="app">
      <Sidebar
        currentView={currentNav}
        onNavigate={(view) => {
          if (view === 'workbench') navigate('#/');
          else if (view === 'datasources') navigate('#/datasources');
          else if (view === 'bizmodel') navigate('#/bizmodel');
          else if (view === 'analysis') navigate('#/analysis');
          else if (view === 'runs') navigate('#/runs');
        }}
      />
      <main className="main narrow" id="main">
        {renderContent()}
      </main>

      {showNewReqModal && (
        <NewRequirementModal
          onClose={() => setShowNewReqModal(false)}
          onSubmit={(title, desc, datasetOverride) => {
            const req = store.createRequirement(title, desc, datasetOverride);
            setShowNewReqModal(false);
            navigate(`#/req/${req.id}`);
          }}
        />
      )}

      {showLlmModal && (
        <LlmSettingsModal onClose={() => setShowLlmModal(false)} />
      )}
    </div>
  );
};
