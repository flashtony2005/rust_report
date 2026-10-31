import test from 'node:test';
import assert from 'node:assert/strict';
import { ReportAgent } from './reportAgent';
import { INITIAL_BIZMODELS } from '../data/seedData';

test('ReportAgent triage: green branch direct generation for explicit request', () => {
  const bm = INITIAL_BIZMODELS['demo-sqlite'];
  const res = ReportAgent.evaluateTriage('华东各品类销售额交叉表', 'demo-sqlite', bm);

  assert.equal(res.status, 'ready');
  assert.equal(res.statusLabel, '🟢 可以做到');
  assert.ok(res.spec);
  assert.equal(res.spec.layout, 'crosstab');
  assert.equal(res.spec.shape, 'crosstab');
  assert.ok(res.caliberInfo);
  assert.ok(res.caliberInfo.metricName.includes('销售额'));
});

test('ReportAgent triage: yellow branch triggers clarification for ambiguous margin', () => {
  const bm = INITIAL_BIZMODELS['demo-sqlite'];
  const res = ReportAgent.evaluateTriage('统计各品类毛利情况', 'demo-sqlite', bm);

  assert.equal(res.status, 'clarify');
  assert.equal(res.statusLabel, '🟡 可以做到 · 要先补');
  assert.ok(res.clarifications && res.clarifications.length > 0);
  const marginQ = res.clarifications.find((q) => q.id === 'margin_caliber');
  assert.ok(marginQ, 'Should ask user to clarify margin caliber');
});

test('ReportAgent triage: yellow branch resolves to green with user options override', () => {
  const bm = INITIAL_BIZMODELS['demo-sqlite'];
  const res = ReportAgent.evaluateTriage('统计各品类毛利情况', 'demo-sqlite', bm, {
    margin_caliber: 'comprehensive',
    time_scope: 'cur_month',
  });

  assert.equal(res.status, 'ready');
  assert.equal(res.statusLabel, '🟢 可以做到');
  assert.ok(res.caliberInfo);
  assert.ok(res.caliberInfo.formula.includes('cost'));
});

test('ReportAgent triage: red branch rejects out-of-vocabulary impossible request', () => {
  const bm = INITIAL_BIZMODELS['demo-sqlite'];
  const res = ReportAgent.evaluateTriage('查询2026年量子纠缠对冲损益表', 'demo-sqlite', bm);

  assert.equal(res.status, 'infeasible');
  assert.equal(res.statusLabel, '🔴 不可以做到');
  assert.ok(res.rejectionReason);
  assert.ok(res.rejectionReason.includes('量子'));
  assert.ok(res.suggestedQueries && res.suggestedQueries.length > 0);
});

test('ReportAgent execution: executes report and produces valid runs with artifacts', () => {
  const bm = INITIAL_BIZMODELS['demo-sqlite'];
  const triage = ReportAgent.evaluateTriage('华东各品类销售额交叉表', 'demo-sqlite', bm);
  const { requirement, run } = ReportAgent.executeReportFromTriage('req-test-1', triage, 'demo-sqlite');

  assert.equal(run.status, 'succeeded');
  assert.ok(run.univerSnapshot);
  assert.ok(run.artifacts['report.csv']);
  assert.ok(run.artifacts['report.html']);
  assert.ok(run.artifacts['snapshot.json']);
  assert.ok(requirement.analysis.calibers.length > 0);
});
