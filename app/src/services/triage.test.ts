import test from 'node:test';
import assert from 'node:assert/strict';
import { triageRequirement } from './triage';
import type { BizModel } from './api';

const mockBizModel: BizModel = {
  schema: 'rpx-bizmodel/v1',
  version: 1,
  source: { dsn: 'demo_sqlite', generated_at: '2026-10-30', generator: 'test' },
  entities: [
    { id: 'customer', label: '客户', status: 'confirmed' },
    { id: 'order', label: '订单', status: 'confirmed' },
    { id: 'loan', label: '贷款', status: 'confirmed' },
  ],
  relationships: [],
  metrics: [
    { id: 'ltv', label: 'LTV', expression: 'sum(amount)', status: 'confirmed', aliases: ['客户价值', '累计消费'] },
    { id: 'overdue_rate', label: '逾期率', expression: 'avg(is_overdue)', status: 'confirmed', aliases: ['逾期', '坏账率'] },
    { id: 'sales_amount', label: '销售额', expression: 'sum(amount)', status: 'confirmed', aliases: ['金额', '销售总额'] },
    { id: 'order_count', label: '订单数', expression: 'count(id)', status: 'confirmed', aliases: ['销量', '笔数'] },
  ],
  dimensions: [
    { id: 'customer_level', label: '客户等级', kind: 'category', status: 'confirmed', aliases: ['等级', '客户分类'] },
    { id: 'region', label: '大区', kind: 'category', status: 'confirmed', aliases: ['所属大区', '区域'] },
    { id: 'month', label: '月份', kind: 'time', status: 'confirmed', time_grain: 'month', aliases: ['按月', '月度'] },
  ],
  rules: [],
  open_questions: [],
};

test('🟢 Ready: 按客户等级看 LTV 和逾期率 should be feasible (ready)', () => {
  const result = triageRequirement('按客户等级看 LTV 和逾期率', mockBizModel, 'demo_sqlite');
  assert.equal(result.branch, 'ready');
  assert.equal(result.canGenerateDirectly, true);
  assert.equal(result.layers.l1.pass, true);
  assert.equal(result.layers.l2.pass, true);
  assert.equal(result.layers.l3.pass, true);
  assert.ok(result.matchedTerms.some((t) => t.id === 'ltv'));
  assert.ok(result.matchedTerms.some((t) => t.id === 'overdue_rate'));
  assert.ok(result.matchedTerms.some((t) => t.id === 'customer_level'));
});

test('🟡 Clarify: 销售额走势 lacks time grain and should ask for clarification', () => {
  const result = triageRequirement('销售额走势', mockBizModel, 'demo_sqlite');
  assert.equal(result.branch, 'clarify');
  assert.equal(result.canGenerateDirectly, false);
  assert.ok(result.missingSlots.some((s) => s.slot === 'time_window'));
  assert.ok(result.quickFixes.length > 0);
});

test('🟡 Clarify: 各客户销售 has ambiguity between sales_amount and order_count', () => {
  const result = triageRequirement('各客户销售', mockBizModel, 'demo_sqlite');
  assert.equal(result.branch, 'clarify');
  assert.ok(result.ambiguities.length > 0);
  assert.ok(result.ambiguities.some((a) => a.token === '销售'));
});

test('🔴 Reject: 员工考勤与打卡记录 is out of domain and lacks physical tables', () => {
  const result = triageRequirement('查询员工考勤与打卡记录', mockBizModel, 'demo_sqlite');
  assert.equal(result.branch, 'reject');
  assert.equal(result.canGenerateDirectly, false);
  assert.equal(result.layers.l3.pass, false);
  assert.ok(result.outOfVocabTerms.length > 0);
  assert.ok(result.remedies.length > 0);
});
