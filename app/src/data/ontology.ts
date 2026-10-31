/**
 * 本体（Ontology）示例夹具 —— **演示用，不是真实数据源**。
 *
 * 为什么放在前端：本体来自 `rpx-bizmodel/v1`，目前只有 Python studio 能发现/维护它，
 * Rust 侧还没有 `GET /bizmodels/{dsn}`（见 `app/src/components/PendingBackend.tsx`）。
 * 所以这一页的"大脑"用一份固定夹具跑通**四步推理的界面与逻辑**，
 * 后端端点到位后把 `resolve()` 的入参换成真模型即可，界面不用改。
 *
 * 数据规模刻意做小（8 客户 / 11 订单 / 9 贷款），且**客户 C001 有 2 笔贷款** ——
 * 这是扇出（fan-out）翻车的必要条件：1 个客户 × 2 订单 × 2 贷款 = 4 行，
 * 订单金额与贷款本金都会被重复计入。少于这个形状，第 3 步就没有可看的东西。
 */

export interface OntologyField {
  name: string;
  label: string;
  type: 'text' | 'number' | 'flag';
}

export interface OntologyEntity {
  id: string;
  name: string;
  rows: number;
  /** 主键字段（预聚合按它分组）。 */
  key: string;
  fields: OntologyField[];
}

export interface OntologyEdge {
  from: string;
  to: string;
  /** `1:N` = 一对多（放大边）；`N:1` = 多对一（不放大）。 */
  cardinality: '1:N' | 'N:1';
  on: string;
}

export interface OntologyMetric {
  id: string;
  label: string;
  entity: string;
  /** 给人看的口径表达式（与生成 SQL 里的写法保持一致）。 */
  expr: string;
  kind: 'sum' | 'ratio';
}

export interface OntologyDim {
  id: string;
  label: string;
  entity: string;
  column: string;
}

export interface Ontology {
  entities: OntologyEntity[];
  edges: OntologyEdge[];
  metrics: OntologyMetric[];
  dims: OntologyDim[];
}

export const ONTOLOGY: Ontology = {
  entities: [
    {
      id: 'customer',
      name: '客户',
      rows: 8,
      key: 'id',
      fields: [
        { name: 'id', label: '客户号', type: 'text' },
        { name: 'name', label: '客户名', type: 'text' },
        { name: 'level', label: '等级', type: 'text' },
        { name: 'region', label: '所属大区', type: 'text' },
      ],
    },
    {
      id: 'order',
      name: '订单',
      rows: 11,
      key: 'id',
      fields: [
        { name: 'id', label: '订单号', type: 'text' },
        { name: 'customer_id', label: '客户号', type: 'text' },
        { name: 'amount', label: '订单金额', type: 'number' },
        { name: 'discount', label: '折扣', type: 'number' },
      ],
    },
    {
      id: 'loan',
      name: '贷款',
      rows: 9,
      key: 'id',
      fields: [
        { name: 'id', label: '贷款号', type: 'text' },
        { name: 'customer_id', label: '客户号', type: 'text' },
        { name: 'principal', label: '本金', type: 'number' },
        { name: 'is_overdue', label: '是否逾期', type: 'flag' },
      ],
    },
  ],
  edges: [
    { from: 'customer', to: 'order', cardinality: '1:N', on: 'customer.id = order.customer_id' },
    { from: 'customer', to: 'loan', cardinality: '1:N', on: 'customer.id = loan.customer_id' },
  ],
  metrics: [
    {
      id: 'ltv',
      label: '客户价值 LTV',
      entity: 'order',
      expr: 'SUM(amount) - SUM(discount)',
      kind: 'sum',
    },
    {
      id: 'overdue_rate',
      label: '逾期率',
      entity: 'loan',
      expr: 'SUM(is_overdue) / COUNT(id)',
      kind: 'ratio',
    },
    {
      id: 'principal',
      label: '授信总额',
      entity: 'loan',
      expr: 'SUM(principal)',
      kind: 'sum',
    },
  ],
  dims: [
    { id: 'level', label: '客户等级', entity: 'customer', column: 'level' },
    { id: 'region', label: '所属大区', entity: 'customer', column: 'region' },
  ],
};

export interface CustomerRow {
  id: string;
  name: string;
  level: string;
  region: string;
}
export interface OrderRow {
  id: string;
  customer_id: string;
  amount: number;
  discount: number;
}
export interface LoanRow {
  id: string;
  customer_id: string;
  principal: number;
  is_overdue: 0 | 1;
}

export const CUSTOMERS: CustomerRow[] = [
  { id: 'C001', name: '阿里巴巴华东采购部', level: 'VIP', region: '华东' },
  { id: 'C002', name: '腾讯深圳分公司', level: 'VIP', region: '华南' },
  { id: 'C003', name: '京东物流华北仓', level: '白金', region: '华北' },
  { id: 'C004', name: '美团上海运营中心', level: '白金', region: '华东' },
  { id: 'C005', name: '网易杭州园区', level: '黄金', region: '华东' },
  { id: 'C006', name: '字节跳动北京总部', level: '黄金', region: '华北' },
  { id: 'C007', name: '顺丰速运广州枢纽', level: '普通', region: '华南' },
  { id: 'C008', name: '滴滴出行华北研发', level: '普通', region: '华北' },
];

export const ORDERS: OrderRow[] = [
  { id: 'O101', customer_id: 'C001', amount: 320000, discount: 15000 },
  { id: 'O102', customer_id: 'C001', amount: 180000, discount: 5000 },
  { id: 'O103', customer_id: 'C002', amount: 490000, discount: 10000 },
  { id: 'O104', customer_id: 'C003', amount: 150000, discount: 8000 },
  { id: 'O105', customer_id: 'C003', amount: 110000, discount: 9500 },
  { id: 'O106', customer_id: 'C004', amount: 240000, discount: 5000 },
  { id: 'O107', customer_id: 'C005', amount: 98000, discount: 2000 },
  { id: 'O108', customer_id: 'C005', amount: 65000, discount: 2800 },
  { id: 'O109', customer_id: 'C006', amount: 160000, discount: 4000 },
  { id: 'O110', customer_id: 'C007', amount: 22000, discount: 1000 },
  { id: 'O111', customer_id: 'C008', amount: 15000, discount: 1400 },
];

export const LOANS: LoanRow[] = [
  { id: 'L501', customer_id: 'C001', principal: 500000, is_overdue: 0 },
  { id: 'L509', customer_id: 'C001', principal: 200000, is_overdue: 0 },
  { id: 'L502', customer_id: 'C002', principal: 800000, is_overdue: 0 },
  { id: 'L503', customer_id: 'C003', principal: 200000, is_overdue: 0 },
  { id: 'L504', customer_id: 'C004', principal: 300000, is_overdue: 1 },
  { id: 'L505', customer_id: 'C005', principal: 100000, is_overdue: 0 },
  { id: 'L506', customer_id: 'C006', principal: 120000, is_overdue: 0 },
  { id: 'L507', customer_id: 'C007', principal: 50000, is_overdue: 1 },
  { id: 'L508', customer_id: 'C008', principal: 40000, is_overdue: 0 },
];
