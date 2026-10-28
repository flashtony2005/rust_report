import { BizModel, OntologyResolutionResult, IWorkbookData } from '../types';

// Mock multi-entity tabular dataset for instant in-memory vector execution
const MOCK_CUSTOMERS = [
  { id: 'C001', name: '阿里巴巴华东采购部', level: 'VIP', region: '华东' },
  { id: 'C002', name: '腾讯深圳分公司', level: 'VIP', region: '华南' },
  { id: 'C003', name: '京东物流华北仓', level: '白金', region: '华北' },
  { id: 'C004', name: '美团上海运营中心', level: '白金', region: '华东' },
  { id: 'C005', name: '网易杭州园区', level: '黄金', region: '华东' },
  { id: 'C006', name: '字节跳动北京总部', level: '黄金', region: '华北' },
  { id: 'C007', name: '顺丰速运广州枢纽', level: '普通', region: '华南' },
  { id: 'C008', name: '滴滴出行华北研发', level: '普通', region: '华北' },
];

const MOCK_ORDERS = [
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

const MOCK_LOANS = [
  { id: 'L501', customer_id: 'C001', principal: 500000, is_overdue: 0 },
  { id: 'L502', customer_id: 'C002', principal: 800000, is_overdue: 0 },
  { id: 'L503', customer_id: 'C003', principal: 200000, is_overdue: 0 },
  { id: 'L504', customer_id: 'C004', principal: 300000, is_overdue: 1 },
  { id: 'L505', customer_id: 'C005', principal: 100000, is_overdue: 0 },
  { id: 'L506', customer_id: 'C006', principal: 120000, is_overdue: 0 },
  { id: 'L507', customer_id: 'C007', principal: 50000, is_overdue: 1 },
  { id: 'L508', customer_id: 'C008', principal: 40000, is_overdue: 0 },
];

export class OntologyResolver {
  /**
   * Resolves a natural language reporting intent into a mathematically pure execution plan
   */
  static resolve(
    model: BizModel,
    intent: string,
    overrideDims?: string[],
    overrideMetrics?: string[]
  ): OntologyResolutionResult {
    const normIntent = intent.toLowerCase();

    // 1. Identify Target Dimensions
    let dimIds: string[] = overrideDims || [];
    if (dimIds.length === 0) {
      if (normIntent.includes('等级') || normIntent.includes('level') || normIntent.includes('客户')) {
        dimIds.push('customer_level');
      }
      if (normIntent.includes('区域') || normIntent.includes('地区') || normIntent.includes('大区')) {
        dimIds.push('customer_region');
      }
      if (dimIds.length === 0) dimIds.push('customer_level');
    }

    // 2. Identify Target Metrics
    let metricIds: string[] = overrideMetrics || [];
    if (metricIds.length === 0) {
      if (normIntent.includes('ltv') || normIntent.includes('价值') || normIntent.includes('销售') || normIntent.includes('金额')) {
        metricIds.push('customer_ltv');
      }
      if (normIntent.includes('逾期') || normIntent.includes('逾期率') || normIntent.includes('风险') || normIntent.includes('overdue')) {
        metricIds.push('overdue_rate');
      }
      if (normIntent.includes('贷款') || normIntent.includes('借款') || normIntent.includes('本金')) {
        metricIds.push('total_loan');
      }
      if (metricIds.length === 0) {
        metricIds.push('customer_ltv', 'overdue_rate');
      }
    }

    // 3. Topology DAG Analysis
    const rootEntity = 'customer';
    const requiredEntities = new Set<string>([rootEntity]);

    if (metricIds.includes('customer_ltv')) requiredEntities.add('order');
    if (metricIds.includes('overdue_rate') || metricIds.includes('total_loan')) requiredEntities.add('loan');

    const resolvedEntities = Array.from(requiredEntities);

    // Discover Join Paths
    const joinPath: OntologyResolutionResult['joinPath'] = [];
    if (requiredEntities.has('order')) {
      joinPath.push({
        from: 'customer',
        to: 'order',
        cardinality: '1:N',
        joinOn: 'customer.id = order.customer_id',
      });
    }
    if (requiredEntities.has('loan')) {
      joinPath.push({
        from: 'customer',
        to: 'loan',
        cardinality: '1:N',
        joinOn: 'customer.id = loan.customer_id',
      });
    }

    // Detect Fan-out Risk
    const hasMultipleOneToMany = joinPath.filter((p) => p.cardinality === '1:N').length > 1;
    const fanOutNotes: string[] = [];

    if (hasMultipleOneToMany) {
      fanOutNotes.push(
        '检测到多分支 1:N 拓扑（Customer ➔ Order 与 Customer ➔ Loan）。',
        '若采用简单 SQL 多表直接 LEFT JOIN，会导致笛卡尔积乘积膨胀（Fan-Out Trap），导致 LTV 与逾期单数翻倍虚增。',
        '本体解析器（Ontology Resolver）启动【粒度对齐 (Grain Alignment)】：各子实体在 customer_id 原生粒度预聚合为独立 CTE/LazyFrame，再对齐合并。'
      );
    }

    // 4. Generate Rust + Polars Vector Code
    const polarsCode = `// [Ontology 2.0] Rust 原生 Polars 向量化惰性计算管道
use polars::prelude::*;

pub fn execute_grain_aligned_report(
    df_customer: LazyFrame,
    df_order: LazyFrame,
    df_loan: LazyFrame,
) -> Result<DataFrame, PolarsError> {
    // 阶段 1: Order 原生粒度聚合（杜绝 Fan-out 乘积膨胀）
    let order_agg = df_order
        .group_by([col("customer_id")])
        .agg([
            (col("amount").sum() - col("discount").sum()).alias("ltv"),
            col("id").count().alias("order_count"),
        ]);

    // 阶段 2: Loan 原生粒度聚合
    let loan_agg = df_loan
        .group_by([col("customer_id")])
        .agg([
            (col("is_overdue").cast(DataType::Float64).sum() / col("id").count())
                .alias("overdue_rate"),
            col("principal").sum().alias("total_loan"),
        ]);

    // 阶段 3: 零拷贝 Arrow 拓扑连接与目标维度分组汇总
    df_customer
        .join(order_agg, [col("id")], [col("customer_id")], JoinType::Left.into())
        .join(loan_agg, [col("id")], [col("customer_id")], JoinType::Left.into())
        .group_by([col("level")])
        .agg([
            col("id").count().alias("customer_count"),
            col("ltv").sum().alias("total_ltv"),
            col("ltv").mean().alias("avg_ltv"),
            col("overdue_rate").mean().alias("avg_overdue_rate"),
        ])
        .sort(["total_ltv"], SortMultipleOptions::new().with_order_descending(true))
        .collect()
}`;

    // 5. Generate Grain-Aligned ANSI SQL
    const grainAlignedSql = `-- [Ontology 2.0] 语义拓扑编译生成的最优 Grain-Aligned SQL
WITH
-- 1. 订单实体在 customer_id 粒度局部预聚合
order_grain AS (
    SELECT 
        customer_id,
        SUM(amount) - SUM(discount) AS ltv,
        COUNT(id) AS order_count
    FROM orders
    GROUP BY customer_id
),
-- 2. 借贷实体在 customer_id 粒度局部预聚合
loan_grain AS (
    SELECT 
        customer_id,
        CAST(SUM(is_overdue) AS REAL) / NULLIF(COUNT(id), 0) AS overdue_rate,
        SUM(principal) AS total_loan
    FROM loans
    GROUP BY customer_id
)
-- 3. 根实体连接并按目标维度投影
SELECT 
    c.level AS customer_level,
    COUNT(c.id) AS customer_count,
    ROUND(SUM(COALESCE(o.ltv, 0)), 2) AS total_ltv,
    ROUND(AVG(COALESCE(o.ltv, 0)), 2) AS avg_ltv,
    ROUND(AVG(COALESCE(l.overdue_rate, 0)) * 100, 2) || '%' AS avg_overdue_rate
FROM customers c
LEFT JOIN order_grain o ON c.id = o.customer_id
LEFT JOIN loan_grain l ON c.id = l.customer_id
GROUP BY c.level
ORDER BY total_ltv DESC;`;

    // 6. Execute in-memory calculation
    // Calculate customer metrics
    const custAggMap = new Map<string, { ltv: number; isOverdue: number; loanCount: number }>();
    MOCK_CUSTOMERS.forEach((c) => {
      custAggMap.set(c.id, { ltv: 0, isOverdue: 0, loanCount: 0 });
    });

    MOCK_ORDERS.forEach((o) => {
      const entry = custAggMap.get(o.customer_id);
      if (entry) entry.ltv += o.amount - o.discount;
    });

    MOCK_LOANS.forEach((l) => {
      const entry = custAggMap.get(l.customer_id);
      if (entry) {
        entry.isOverdue += l.is_overdue;
        entry.loanCount += 1;
      }
    });

    // Group by level
    const levelMap = new Map<string, { count: number; totalLtv: number; rates: number[] }>();
    MOCK_CUSTOMERS.forEach((c) => {
      const entry = custAggMap.get(c.id)!;
      const rate = entry.loanCount > 0 ? entry.isOverdue / entry.loanCount : 0;
      if (!levelMap.has(c.level)) {
        levelMap.set(c.level, { count: 0, totalLtv: 0, rates: [] });
      }
      const lvl = levelMap.get(c.level)!;
      lvl.count += 1;
      lvl.totalLtv += entry.ltv;
      lvl.rates.push(rate);
    });

    const headers = ['客户等级', '客户数量', '累计生命周期价值 (LTV)', '平均客单 LTV', '平均贷款逾期率'];
    const rows: any[][] = [];

    const sortedLevels = Array.from(levelMap.entries()).sort((a, b) => b[1].totalLtv - a[1].totalLtv);

    sortedLevels.forEach(([level, stats]) => {
      const avgLtv = stats.totalLtv / (stats.count || 1);
      const avgRate = stats.rates.length > 0 ? stats.rates.reduce((a, b) => a + b, 0) / stats.rates.length : 0;
      rows.push([
        level,
        stats.count,
        `¥ ${stats.totalLtv.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`,
        `¥ ${avgLtv.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`,
        `${(avgRate * 100).toFixed(1)}%`,
      ]);
    });

    // 7. Generate Univer Snapshot
    const cellData: Record<string, Record<string, any>> = {};

    // Row 0: Headers
    cellData['0'] = {};
    headers.forEach((h, colIdx) => {
      cellData['0'][String(colIdx)] = {
        v: h,
        t: 1,
        s: {
          bl: 1,
          bg: { rgb: '#1e293b' },
          cl: { rgb: '#ffffff' },
          ht: 2,
          vt: 2,
          bd: {
            b: { s: 1, cl: { rgb: '#475569' } },
            t: { s: 1, cl: { rgb: '#475569' } },
            l: { s: 1, cl: { rgb: '#475569' } },
            r: { s: 1, cl: { rgb: '#475569' } },
          },
        },
      };
    });

    // Data Rows
    rows.forEach((r, rowIdx) => {
      const rKey = String(rowIdx + 1);
      cellData[rKey] = {};
      r.forEach((val, colIdx) => {
        const isNum = typeof val === 'number';
        const isCurrency = typeof val === 'string' && val.startsWith('¥');
        const isPercent = typeof val === 'string' && val.endsWith('%');
        cellData[rKey][String(colIdx)] = {
          v: val,
          t: isNum ? 2 : 1,
          s: {
            bg: { rgb: rowIdx % 2 === 0 ? '#f8fafc' : '#ffffff' },
            ht: colIdx === 0 ? 1 : isPercent || isCurrency ? 3 : 2,
            vt: 2,
            bd: {
              b: { s: 1, cl: { rgb: '#e2e8f0' } },
              t: { s: 1, cl: { rgb: '#e2e8f0' } },
              l: { s: 1, cl: { rgb: '#e2e8f0' } },
              r: { s: 1, cl: { rgb: '#e2e8f0' } },
            },
          },
        };
      });
    });

    // Summary Row
    const sumRowKey = String(rows.length + 1);
    const grandLtv = sortedLevels.reduce((acc, [, s]) => acc + s.totalLtv, 0);
    const grandCount = sortedLevels.reduce((acc, [, s]) => acc + s.count, 0);
    const grandAvgLtv = grandLtv / (grandCount || 1);
    const allRates = sortedLevels.flatMap(([, s]) => s.rates);
    const grandAvgRate = allRates.length > 0 ? allRates.reduce((a, b) => a + b, 0) / allRates.length : 0;

    cellData[sumRowKey] = {
      '0': { v: '总计 (全量对齐)', t: 1, s: { bl: 1, bg: { rgb: '#e0f2fe' }, ht: 1, vt: 2 } },
      '1': { v: grandCount, t: 2, s: { bl: 1, bg: { rgb: '#e0f2fe' }, ht: 2, vt: 2 } },
      '2': {
        v: `¥ ${grandLtv.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`,
        t: 1,
        s: { bl: 1, bg: { rgb: '#e0f2fe' }, ht: 3, vt: 2 },
      },
      '3': {
        v: `¥ ${grandAvgLtv.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`,
        t: 1,
        s: { bl: 1, bg: { rgb: '#e0f2fe' }, ht: 3, vt: 2 },
      },
      '4': {
        v: `${(grandAvgRate * 100).toFixed(1)}%`,
        t: 1,
        s: { bl: 1, bg: { rgb: '#e0f2fe' }, ht: 3, vt: 2 },
      },
    };

    const univerSnapshot: IWorkbookData = {
      id: `workbook-ontology-${Date.now()}`,
      name: 'Ontology 语义解析结果',
      appVersion: '1.2.0',
      locale: 'zhCN',
      sheetOrder: ['sheet-ontology'],
      sheets: {
        'sheet-ontology': {
          id: 'sheet-ontology',
          name: '客户价值与风控报表',
          rowCount: rows.length + 5,
          columnCount: headers.length + 2,
          defaultRowHeight: 28,
          defaultColumnWidth: 160,
          freeze: { ySplit: 1, startRow: 1 },
          cellData,
          columnData: {
            '0': { w: 120 },
            '1': { w: 100 },
            '2': { w: 200 },
            '3': { w: 180 },
            '4': { w: 150 },
          },
        },
      },
    };

    return {
      intent,
      requestedDimensions: dimIds,
      requestedMetrics: metricIds,
      rootEntity,
      resolvedEntities,
      joinPath,
      fanOutProtected: hasMultipleOneToMany,
      fanOutPreventionNotes: fanOutNotes,
      polarsCode,
      grainAlignedSql,
      outputData: {
        headers,
        rows,
      },
      univerSnapshot,
    };
  }
}
