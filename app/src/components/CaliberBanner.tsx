import React, { useState } from 'react';
import { useToast } from './Toast';

export interface CaliberInfo {
  metricName: string;
  formula: string;
  dimensions: string[];
  timeWindow: string;
  safetyGuarantee: string;
}

interface Props {
  dsnId?: string;
  caliber: CaliberInfo;
  reportName?: string;
}

export const CaliberBanner: React.FC<Props> = ({ dsnId = 'demo_sqlite', caliber, reportName = '报表' }) => {
  const { toast } = useToast();
  const [isSaved, setIsSaved] = useState(false);

  const handleSaveToBizModel = () => {
    setIsSaved(true);
    toast(`已将「${caliber.metricName}」统计口径沉淀至 ${dsnId} 业务知识模型`);
  };

  return (
    <div
      style={{
        background: 'linear-gradient(135deg, #ffffff 0%, #faf8f5 100%)',
        border: '1px solid var(--border)',
        borderLeft: '4px solid var(--accent)',
        borderRadius: 'var(--r-m)',
        padding: '16px 20px',
        marginBottom: '20px',
        boxShadow: 'var(--shadow-s)',
        position: 'relative',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          flexWrap: 'wrap',
          gap: '12px',
          paddingBottom: '12px',
          borderBottom: '1px solid var(--border-subtle)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
          <span
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: '10.5px',
              fontWeight: 700,
              letterSpacing: '0.14em',
              textTransform: 'uppercase',
              color: 'var(--accent-ink)',
              background: 'var(--accent-soft)',
              padding: '3px 8px',
              borderRadius: 'var(--r-s)',
              display: 'inline-flex',
              alignItems: 'center',
              gap: '5px',
            }}
          >
            <span style={{ fontSize: '11px' }}>⚖</span> CALIBER SPEC
          </span>
          <span
            style={{
              fontFamily: 'var(--font-serif)',
              fontSize: '16px',
              fontWeight: 700,
              color: 'var(--ink)',
              letterSpacing: '-0.01em',
            }}
          >
            {caliber.metricName}
          </span>
          <span className="tiny" style={{ color: 'var(--ink-4)' }}>
            · 数据源 <code>{dsnId}</code>
          </span>
        </div>

        <div>
          {isSaved ? (
            <span
              style={{
                background: 'var(--ok-bg)',
                color: 'var(--ok)',
                border: '1px solid var(--ok-border)',
                padding: '5px 12px',
                borderRadius: 'var(--r-s)',
                fontSize: '12px',
                fontWeight: 600,
                display: 'inline-flex',
                alignItems: 'center',
                gap: '6px',
              }}
            >
              <span>✓</span> 已沉淀至业务模型
            </span>
          ) : (
            <button
              type="button"
              className="btn small"
              onClick={handleSaveToBizModel}
              title="将经审计确认的口径指标同步写入业务模型飞轮"
              style={{
                borderColor: 'var(--line-2)',
                background: '#fff',
                fontSize: '12px',
                fontWeight: 600,
              }}
            >
              <span style={{ color: 'var(--accent-2)' }}>✦</span> 沉淀为业务知识
            </button>
          )}
        </div>
      </div>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(210px, 1fr))',
          gap: '14px 20px',
          marginTop: '12px',
          fontSize: '12.5px',
        }}
      >
        <div>
          <div
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: '10px',
              fontWeight: 600,
              letterSpacing: '0.15em',
              textTransform: 'uppercase',
              color: 'var(--ink-4)',
              marginBottom: '4px',
            }}
          >
            度量计算公式 · Formula
          </div>
          <div
            style={{
              fontFamily: 'var(--font-mono)',
              color: 'var(--ink)',
              fontSize: '12px',
              background: 'var(--paper-3)',
              padding: '4px 8px',
              borderRadius: 'var(--r-s)',
              border: '1px solid var(--border-subtle)',
              wordBreak: 'break-all',
              lineHeight: 1.5,
            }}
          >
            {caliber.formula}
          </div>
        </div>

        <div>
          <div
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: '10px',
              fontWeight: 600,
              letterSpacing: '0.15em',
              textTransform: 'uppercase',
              color: 'var(--ink-4)',
              marginBottom: '4px',
            }}
          >
            聚合维度层级 · Dimensions
          </div>
          <div style={{ color: 'var(--ink-2)', fontWeight: 500, paddingTop: '3px' }}>
            {caliber.dimensions.length > 0 ? caliber.dimensions.join(' × ') : '全表单一汇聚'}
          </div>
        </div>

        <div>
          <div
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: '10px',
              fontWeight: 600,
              letterSpacing: '0.15em',
              textTransform: 'uppercase',
              color: 'var(--ink-4)',
              marginBottom: '4px',
            }}
          >
            统计时间跨度 · Timeframe
          </div>
          <div style={{ color: 'var(--ink-2)', fontWeight: 500, paddingTop: '3px' }}>
            {caliber.timeWindow}
          </div>
        </div>

        <div>
          <div
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: '10px',
              fontWeight: 600,
              letterSpacing: '0.15em',
              textTransform: 'uppercase',
              color: 'var(--ink-4)',
              marginBottom: '4px',
            }}
          >
            严谨性审计保证 · Integrity
          </div>
          <div style={{ color: 'var(--ok)', fontSize: '11.5px', lineHeight: 1.6, paddingTop: '2px' }}>
            {caliber.safetyGuarantee}
          </div>
        </div>
      </div>
    </div>
  );
};
