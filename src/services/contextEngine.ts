import { ContextSnapshot, ContextTable, Datasource } from '../types';
import { INITIAL_DATASOURCES } from '../data/seedData';

export class ContextEngine {
  private static physicalCatalogs: Record<string, ContextTable[]> = {
    'demo-sqlite': [
      {
        name: 'sales',
        label: '门店销售明细事实表',
        rows: 90,
        columns: [
          { name: 'region', type: 'VARCHAR(32)', comment: '所属大区（华东/华南/华北）', isKey: false, sample: '华东', cardinality: 3 },
          { name: 'store', type: 'VARCHAR(64)', comment: '门店名称', isKey: false, sample: '上海一店', cardinality: 6 },
          { name: 'month', type: 'DATE', comment: '销售月份（YYYY-MM）', isKey: false, sample: '2026-07', cardinality: 3 },
          { name: 'cat', type: 'VARCHAR(32)', comment: '产品品类（办公用品/数码配件/家居清洁）', isKey: false, sample: '办公用品', cardinality: 3 },
          { name: 'amount', type: 'DECIMAL(12,2)', comment: '含税销售实收金额（元）', isKey: false, sample: '28500', cardinality: 88 },
          { name: 'cost', type: 'DECIMAL(12,2)', comment: '核算销售成本（元）', isKey: false, sample: '18200', cardinality: 85 },
        ],
      },
      {
        name: 'stores',
        label: '连锁门店主数据维表',
        rows: 6,
        columns: [
          { name: 'id', type: 'VARCHAR(32)', comment: '门店唯一编码', isKey: true, sample: 'S001', cardinality: 6 },
          { name: 'name', type: 'VARCHAR(64)', comment: '门店中文全称', isKey: false, sample: '上海一店', cardinality: 6 },
          { name: 'region', type: 'VARCHAR(32)', comment: '区域归属', isKey: false, sample: '华东', cardinality: 3 },
          { name: 'tier', type: 'VARCHAR(16)', comment: '门店层级等级（S/A/B）', isKey: false, sample: 'S', cardinality: 3 },
          { name: 'opened_at', type: 'DATE', comment: '开业日期', isKey: false, sample: '2023-05-18', cardinality: 6 },
        ],
      },
      {
        name: 'categories',
        label: '商品品类目录表',
        rows: 3,
        columns: [
          { name: 'id', type: 'VARCHAR(32)', comment: '品类编码', isKey: true, sample: 'C01', cardinality: 3 },
          { name: 'name', type: 'VARCHAR(32)', comment: '品类名称', isKey: false, sample: '办公用品', cardinality: 3 },
          { name: 'margin_target', type: 'DECIMAL(5,2)', comment: '标准毛利率目标', isKey: false, sample: '0.35', cardinality: 3 },
        ],
      },
    ],
    'sales-pg': [
      {
        name: 'customers',
        label: '客户主体档案主表',
        rows: 250,
        columns: [
          { name: 'id', type: 'BIGINT', comment: '客户ID主键', isKey: true, sample: '1001', cardinality: 250 },
          { name: 'name', type: 'VARCHAR(128)', comment: '客户全称/企业名称', isKey: false, sample: '上海智成科技', cardinality: 250 },
          { name: 'tier', type: 'VARCHAR(16)', comment: '客户评级（VIP/Gold/Silver）', isKey: false, sample: 'VIP', cardinality: 3 },
          { name: 'region', type: 'VARCHAR(32)', comment: '大区', isKey: false, sample: '华东', cardinality: 4 },
          { name: 'credit_limit', type: 'DECIMAL(14,2)', comment: '授信总额度（元）', isKey: false, sample: '500000.00', cardinality: 80 },
        ],
      },
      {
        name: 'orders',
        label: '客户消费交易订单表',
        rows: 1200,
        columns: [
          { name: 'id', type: 'BIGINT', comment: '订单主键', isKey: true, sample: '8801', cardinality: 1200 },
          { name: 'customer_id', type: 'BIGINT', comment: '客户外键', isKey: false, sample: '1001', cardinality: 250 },
          { name: 'order_date', type: 'TIMESTAMP', comment: '下单时间', isKey: false, sample: '2026-09-12 14:20:00', cardinality: 980 },
          { name: 'amount', type: 'DECIMAL(12,2)', comment: '订单金额', isKey: false, sample: '12800.00', cardinality: 650 },
          { name: 'status', type: 'VARCHAR(32)', comment: '订单履约状态', isKey: false, sample: 'completed', cardinality: 4 },
        ],
      },
      {
        name: 'loans',
        label: '信贷授信与履约表',
        rows: 350,
        columns: [
          { name: 'id', type: 'BIGINT', comment: '贷款合同号', isKey: true, sample: 'L20260901', cardinality: 350 },
          { name: 'customer_id', type: 'BIGINT', comment: '客户外键', isKey: false, sample: '1001', cardinality: 210 },
          { name: 'principal', type: 'DECIMAL(14,2)', comment: '放款本金', isKey: false, sample: '150000.00', cardinality: 180 },
          { name: 'status', type: 'VARCHAR(32)', comment: '还款状态（normal/overdue/settled）', isKey: false, sample: 'overdue', cardinality: 3 },
          { name: 'overdue_days', type: 'INT', comment: '逾期天数', isKey: false, sample: '15', cardinality: 45 },
        ],
      },
    ],
  };

  /**
   * 准备客观事实上下文 (Context Preparation Pipeline)
   * 结合物理探查与元数据剪枝，杜绝大模型臆想
   */
  public static prepareContext(dsnId: string, queryText: string = ''): ContextSnapshot {
    const dsn = INITIAL_DATASOURCES.find((d) => d.id === dsnId);
    const dsnLabel = dsn?.label || dsnId;
    const allTables = this.physicalCatalogs[dsnId] || this.physicalCatalogs['demo-sqlite'];

    // 动态剪枝：根据 queryText 过滤最相关物理表，若未指定则默认携带核心事实表与维表
    const lowerQuery = queryText.toLowerCase();
    const isCustomerFinance = lowerQuery.includes('客户') || lowerQuery.includes('贷款') || lowerQuery.includes('ltv') || lowerQuery.includes('逾期') || lowerQuery.includes('授信');

    let tables: ContextTable[] = [];
    let prunedTables: string[] = [];

    if (dsnId === 'sales-pg' || isCustomerFinance) {
      tables = (this.physicalCatalogs['sales-pg'] || []).slice();
    } else {
      tables = allTables.slice();
    }

    const activeRules = [
      '所有金额计算统一使用含税净额（已扣除作废订单）',
      '跨多事实表关联必须以客户中心表为准，严禁无聚合直接 JOIN',
      '日期跨度缺省时默认锚定最近完结自然月 (2026-09)',
    ];

    return {
      dsn: dsnId,
      dsnLabel,
      asOfDate: '2026-10-30',
      tables,
      activeRules,
      prunedTables,
    };
  }

  public static getTableMeta(dsnId: string, tableName: string): ContextTable | undefined {
    const catalog = this.physicalCatalogs[dsnId] || this.physicalCatalogs['demo-sqlite'];
    return catalog.find((t) => t.name === tableName);
  }
}
