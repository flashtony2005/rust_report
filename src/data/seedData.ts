import { Datasource, Requirement, ShapeInfo, Candidate, Alert, Schedule, SemanticVersion, Run } from '../types';

export const SHAPE_INFOS: ShapeInfo[] = [
  {
    shape: 'detail',
    label: '明细清单',
    layout: 'list',
    needs: ['维度（可不选，不选就是单行汇总）'],
    template_shapes: ['list'],
  },
  {
    shape: 'grouped',
    label: '分组汇总',
    layout: 'grouped',
    needs: ['至少一个维度', '至少一个指标'],
    template_shapes: ['grouped', 'crosstab'],
  },
  {
    shape: 'crosstab',
    label: '交叉表',
    layout: 'crosstab',
    needs: ['两个维度（一个放行、一个放列）', '至少一个指标'],
    template_shapes: ['crosstab', 'grouped'],
  },
  {
    shape: 'trend',
    label: '趋势表',
    layout: 'grouped',
    needs: ['一个日期维度（带 time_grain 出时间桶）', '至少一个指标'],
    template_shapes: ['grouped', 'crosstab'],
  },
  {
    shape: 'rank',
    label: '排名表',
    layout: 'grouped',
    needs: ['按指标排序（sort.by.metric）', '可选 limit 截前 N'],
    template_shapes: ['grouped', 'list'],
  },
  {
    shape: 'compare',
    label: '对比表',
    layout: 'grouped',
    needs: ['comparison（同比/环比/上一期）', 'time_range', '对比列 m<i>_prev'],
    template_shapes: ['grouped', 'crosstab'],
  },
];

export const SHAPE_LABELS: Record<string, string> = {
  detail: '明细清单',
  grouped: '分组汇总',
  crosstab: '交叉表',
  trend: '趋势表',
  rank: '排名表',
  compare: '对比表',
};

// 90 lines sample sales dataset (raw rows matching store-sales.csv)
export const RAW_SAMPLE_CSV = `region,store,month,cat,amount
华东,上海一店,2026-07,办公用品,28500
华东,上海一店,2026-07,数码配件,26500
华东,上海一店,2026-07,家居清洁,31400
华东,上海一店,2026-08,办公用品,38600
华东,上海一店,2026-08,数码配件,20700
华东,上海一店,2026-08,家居清洁,22400
华东,上海一店,2026-09,办公用品,32900
华东,上海一店,2026-09,数码配件,26600
华东,上海一店,2026-09,家居清洁,31200
华东,杭州二店,2026-07,办公用品,37000
华东,杭州二店,2026-07,数码配件,66500
华东,杭州二店,2026-07,家居清洁,41100
华东,杭州二店,2026-08,办公用品,50100
华东,杭州二店,2026-08,数码配件,26800
华东,杭州二店,2026-08,家居清洁,45400
华东,杭州二店,2026-09,办公用品,63800
华东,杭州二店,2026-09,数码配件,60600
华东,杭州二店,2026-09,家居清洁,32300
华东,南京东路店,2026-07,办公用品,24000
华东,南京东路店,2026-07,数码配件,21000
华东,南京东路店,2026-07,家居清洁,28000
华东,南京东路店,2026-08,办公用品,31000
华东,南京东路店,2026-08,数码配件,19500
华东,南京东路店,2026-08,家居清洁,24500
华东,南京东路店,2026-09,办公用品,29000
华东,南京东路店,2026-09,数码配件,25000
华东,南京东路店,2026-09,家居清洁,27000
华南,广州天河店,2026-07,办公用品,31000
华南,广州天河店,2026-07,数码配件,39000
华南,广州天河店,2026-07,家居清洁,26000
华南,广州天河店,2026-08,办公用品,33500
华南,广州天河店,2026-08,数码配件,42000
华南,广州天河店,2026-08,家居清洁,28500
华南,广州天河店,2026-09,办公用品,36000
华南,广州天河店,2026-09,数码配件,48000
华南,广州天河店,2026-09,家居清洁,31000
华南,深圳南山店,2026-07,办公用品,45000
华南,深圳南山店,2026-07,数码配件,78000
华南,深圳南山店,2026-07,家居清洁,39000
华南,深圳南山店,2026-08,办公用品,48000
华南,深圳南山店,2026-08,数码配件,82000
华南,深圳南山店,2026-08,家居清洁,41000
华南,深圳南山店,2026-09,办公用品,52000
华南,深圳南山店,2026-09,数码配件,89000
华南,深圳南山店,2026-09,家居清洁,43000
华北,北京中关村店,2026-07,办公用品,39000
华北,北京中关村店,2026-07,数码配件,56000
华北,北京中关村店,2026-07,家居清洁,31000
华北,北京中关村店,2026-08,办公用品,41000
华北,北京中关村店,2026-08,数码配件,52000
华北,北京中关村店,2026-08,家居清洁,33000
华北,北京中关村店,2026-09,办公用品,44000
华北,北京中关村店,2026-09,数码配件,59000
华北,北京中关村店,2026-09,家居清洁,35000
华北,天津滨海店,2026-07,办公用品,21000
华北,天津滨海店,2026-07,数码配件,29000
华北,天津滨海店,2026-07,家居清洁,18000
华北,天津滨海店,2026-08,办公用品,23000
华北,天津滨海店,2026-08,数码配件,27000
华北,天津滨海店,2026-08,家居清洁,19000
华北,天津滨海店,2026-09,办公用品,25000
华北,天津滨海店,2026-09,数码配件,31000
华北,天津滨海店,2026-09,家居清洁,21000`;

export const PRESET_LIBRARY: Candidate[] = [
  {
    id: 'sales-region-category-crosstab',
    name: '区域品类交叉表',
    shape_label: '交叉表',
    source: 'library',
    score: 95,
    reasons: ['列名完全重合（region、cat、amount）', '描述包含交叉统计与总计行'],
    missing: [],
    fields: ['region', 'cat', 'amount'],
  },
  {
    id: 'store-performance-summary',
    name: '门店业绩分组汇总表',
    shape_label: '分组汇总',
    source: 'library',
    score: 90,
    reasons: ['命中多维分组汇总模式', '包含门店、大区与销售额'],
    missing: [],
    fields: ['region', 'store', 'amount'],
  },
  {
    id: 'sales-monthly-trend',
    name: '月度销售走势分析表',
    shape_label: '趋势表',
    source: 'library',
    score: 85,
    reasons: ['时间维 month 自动聚合到列', '度量金额按月平铺'],
    missing: [],
    fields: ['month', 'region', 'amount'],
  },
  {
    id: 'sales-by-product',
    name: '品类销售排行表',
    shape_label: '排名表',
    source: 'library',
    score: 80,
    reasons: ['按金额降序排序', '单维度高聚焦清单'],
    missing: [],
    fields: ['cat', 'amount'],
  },
  {
    id: 'fin-income-expense-detail',
    name: '销售明细流水清单',
    shape_label: '明细清单',
    source: 'library',
    score: 75,
    reasons: ['无聚合平铺模式', '逐行展示明细'],
    missing: [],
    fields: ['region', 'store', 'month', 'cat', 'amount'],
  },
  {
    id: 'sales-compare-mom',
    name: '销售环比变动对比表',
    shape_label: '对比表',
    source: 'library',
    score: 70,
    reasons: ['本期 vs 上期对比', '增幅计算与异常标记'],
    missing: [],
    fields: ['region', 'month', 'amount'],
  },
  {
    id: 'generated-custom',
    name: '按需求生成新模板',
    shape_label: '按需编译',
    source: 'generated',
    reasons: ['灵活排版：由编译器根据维度与度量自动推导网格'],
    missing: [],
  },
];

export const INITIAL_DATASOURCES: Datasource[] = [
  {
    id: 'demo-sqlite',
    label: '销售演示库 (SQLite 本机)',
    driver: 'sqlite',
    driver_label: 'SQLite 文件',
    database: 'sample/sales-demo.db',
    has_password: false,
    options: 'mode=ro',
  },
  {
    id: 'sales-pg',
    label: '主业务库 (PostgreSQL 只读从库)',
    driver: 'postgres',
    driver_label: 'PostgreSQL / Kingbase',
    host: '10.0.1.18',
    port: 5432,
    database: 'sales_prod_replica',
    user: 'ro_report',
    has_password: true,
    options: 'sslmode=disable',
  },
];

export const INITIAL_SCHEDULES: Schedule[] = [
  {
    id: 'sch-101',
    requirement: 'req-001',
    kind: 'daily',
    at: '08:00',
    enabled: true,
    next_run: '明天 08:00',
    last_status: 'succeeded',
    runs: 14,
  },
  {
    id: 'sch-102',
    requirement: 'req-002',
    kind: 'weekly',
    weekday: 0,
    at: '09:30',
    enabled: true,
    next_run: '下周一 09:30',
    last_status: 'succeeded',
    runs: 4,
  },
];

export const INITIAL_ALERTS: Alert[] = [
  {
    id: 'alt-1',
    req_id: 'req-001',
    dsn: 'demo-sqlite',
    metric: 'amount',
    metric_label: '销售额',
    window: { cur_start: '2026-09-01', cur_end: '2026-09-30' },
    at: '2026-10-20T08:00:00Z',
    level: 'warn',
    message: '华东区 9 月销售额环比下降 14.8%，未达预算下限',
  },
  {
    id: 'alt-2',
    req_id: 'req-002',
    dsn: 'demo-sqlite',
    metric: 'amount',
    metric_label: '销售额',
    window: { cur_start: '2026-09-01', cur_end: '2026-09-30' },
    at: '2026-10-18T09:30:00Z',
    level: 'critical',
    message: '天津滨海店 9 月数码配件销量波动超过 35%',
  },
];

export const INITIAL_VERSIONS: Record<string, SemanticVersion[]> = {
  'req-001': [
    {
      version: 1,
      saved_at: '2026-10-19T10:15:00Z',
      note: '初版：按大区和产品类别交叉汇总',
      template_rev: 1,
      shape: 'crosstab',
      semantic: {
        hash: 'c1a4e98f029b3d1',
        calibers: ['SUM(amount)', 'region', 'cat'],
      },
    },
    {
      version: 2,
      saved_at: '2026-10-22T14:30:00Z',
      note: '增加行末小计与总计行加粗边框',
      template_rev: 2,
      shape: 'crosstab',
      semantic: {
        hash: 'c1a4e98f029b3d1', // same semantic hash, visual change
        calibers: ['SUM(amount)', 'region', 'cat'],
      },
    },
  ],
};

export const INITIAL_BIZMODELS: Record<string, any> = {
  'demo-sqlite': {
    schema: 'rpx-bizmodel/v1',
    version: 1,
    source: {
      dsn: 'demo-sqlite',
      generated_at: '2026-10-23T21:00:00+08:00',
      generator: 'discovery/P0',
    },
    entities: [
      {
        id: 'sales',
        table: 'sales',
        label: '销售明细事实表',
        grain: {
          key: ['id'],
          description: '一行代表一笔门店品类结算销售记录',
          status: 'confirmed',
          evidence: ['primary_key(id)', 'table_comment:门店销售记录'],
        },
        status: 'confirmed',
        evidence: ['table_exists', 'declared_primary_key'],
      },
    ],
    metrics: [
      {
        id: 'sales_amount',
        label: '净销售额',
        entity: 'sales',
        column: 'amount',
        expression: 'SUM(amount) - SUM(COALESCE(discount, 0))',
        filters: ['valid_orders'],
        time_field: 'month',
        grain: 'monthly',
        format: '¥ #,##0.00',
        status: 'confirmed',
        evidence: ['human_confirmed:2026-10-24', 'business_audited'],
        aliases: ['销售额', '金额', '流水', '收入', '营收', 'GMV', '业绩', '销售总额'],
      },
      {
        id: 'sales_qty',
        label: '销售件数',
        entity: 'sales',
        column: 'qty',
        expression: 'SUM(qty)',
        filters: ['valid_orders'],
        time_field: 'month',
        grain: 'item',
        format: '#,##0 件',
        status: 'confirmed',
        evidence: ['column_type:INTEGER', 'human_confirmed:2026-10-24'],
        aliases: ['数量', '销量', '件数', '出库量', '单量'],
      },
      {
        id: 'avg_order_price',
        label: '客单均价',
        entity: 'sales',
        expression: 'SUM(amount) / NULLIF(COUNT(DISTINCT store), 0)',
        filters: ['valid_orders'],
        format: '¥ #,##0.00',
        status: 'candidate',
        evidence: ['algorithm_derived:sales_amount/store_count'],
        aliases: ['客单价', '均价', '店均额'],
      },
      {
        id: 'refund_amount',
        label: '退款金额',
        entity: 'sales',
        expression: 'SUM(refund_amt)',
        filters: [],
        status: 'draft',
        evidence: ['column_missing:sample_only'],
        aliases: ['退货额', '退款'],
      },
    ],
    dimensions: [
      {
        id: 'sales_region',
        entity: 'sales',
        label: '销售大区',
        kind: 'enum',
        values: ['华东', '华南', '华北'],
        status: 'confirmed',
        evidence: ['distinct=3', 'human_confirmed:2026-10-24'],
        aliases: ['大区', '区域', '地区'],
      },
      {
        id: 'sales_store',
        entity: 'sales',
        label: '门店名称',
        kind: 'text',
        values: ['上海一店', '杭州二店', '南京东路店', '广州天河店', '深圳南山店', '北京中关村店', '天津滨海店'],
        status: 'confirmed',
        evidence: ['distinct=7', 'human_confirmed:2026-10-24'],
        aliases: ['门店', '分店', '店铺', '店名'],
      },
      {
        id: 'sales_category',
        entity: 'sales',
        label: '产品品类',
        kind: 'enum',
        values: ['办公用品', '数码配件', '家居清洁'],
        status: 'confirmed',
        evidence: ['distinct=3', 'human_confirmed:2026-10-24'],
        aliases: ['品类', '产品类别', '类目', '商品分类'],
      },
      {
        id: 'sales_month',
        entity: 'sales',
        label: '结算月份',
        kind: 'date',
        time_grain: 'month',
        status: 'confirmed',
        evidence: ['date_format:YYYY-MM', 'human_confirmed:2026-10-24'],
        aliases: ['月份', '按月', '时期', '周期'],
      },
    ],
    rules: [
      {
        id: 'valid_orders',
        label: '有效结算订单规则',
        expression: "amount > 0 AND status <> 'cancelled'",
        applies_to: ['sales'],
        status: 'confirmed',
        evidence: ['human_rule:2026-10-24', '财务对账标准'],
      },
      {
        id: 'exclude_internal_test',
        label: '排除内部测试门店',
        expression: "store NOT LIKE '%测试%'",
        applies_to: ['sales'],
        status: 'candidate',
        evidence: ['convention_rule'],
      },
    ],
    open_questions: [
      {
        question: '天津滨海店部分打折订单是否需要按折前原价统计？',
        source: 'entity:sales.column:discount',
        evidence: ['discount_presence:22_rows'],
      },
    ],
  },
};

export const INITIAL_GOLDENS: any[] = [
  {
    id: 'q01',
    intent: '按区域看销售额',
    query: {
      metrics: [{ ref: 'sales_amount', label: '销售额' }],
      dimensions: [{ ref: 'sales_region' }],
      sort: [{ by: { metric: 'sales_amount' }, direction: 'desc' }],
    },
    expect: {
      rows: 3,
      cols: 2,
      shape: '分组汇总',
      note: '应按大区分组并按销售额降序排列',
    },
  },
  {
    id: 'q02',
    intent: '各个品类卖了多少件',
    query: {
      metrics: [{ ref: 'sales_qty', label: '销售件数' }],
      dimensions: [{ ref: 'sales_category' }],
    },
    expect: {
      rows: 3,
      cols: 2,
      shape: '分组汇总',
      note: '应准确识别“件数”对应 sales_qty',
    },
  },
  {
    id: 'q03',
    intent: '按区域和品类交叉看销售额',
    query: {
      metrics: [{ ref: 'sales_amount', label: '销售额' }],
      dimensions: [{ ref: 'sales_region' }, { ref: 'sales_category' }],
    },
    expect: {
      rows: 3,
      cols: 4,
      shape: '交叉表',
      note: '两个维度 + 一个度量，判定为 crosstab 交叉表',
    },
  },
  {
    id: 'q04',
    intent: '今年第三季度各月走势',
    query: {
      metrics: [{ ref: 'sales_amount', label: '月销售总额' }],
      dimensions: [{ ref: 'sales_month', time_grain: 'month' }],
    },
    expect: {
      rows: 3,
      cols: 2,
      shape: '趋势表',
      note: '时间序列维度自动识别为 trend 趋势形态',
    },
  },
  {
    id: 'q05',
    intent: '门店业绩排行 Top 5',
    query: {
      metrics: [{ ref: 'sales_amount', label: '销售额' }],
      dimensions: [{ ref: 'sales_store' }],
      sort: [{ by: { metric: 'sales_amount' }, direction: 'desc' }],
    },
    expect: {
      rows: 5,
      cols: 2,
      shape: '排名表',
      note: '带排序且限定名次，判定为 rank 排名形态',
    },
  },
];

