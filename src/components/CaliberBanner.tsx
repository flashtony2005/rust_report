import React, { useState } from 'react';
import { CaliberInfo } from '../types';
import { store } from '../services/store';
import { useToast } from './Toast';

interface Props {
  dsnId: string;
  caliber: CaliberInfo;
  originalQuery: string;
  onSaved?: () => void;
}

export const CaliberBanner: React.FC<Props> = ({ dsnId, caliber, originalQuery, onSaved }) => {
  const { toast } = useToast();
  const [isSaved, setIsSaved] = useState(false);

  const handleSaveToBizModel = () => {
    try {
      store.saveCaliberToBizModel(dsnId, caliber, originalQuery);
      setIsSaved(true);
      toast(`已将「${caliber.metricName}」统计口径沉淀至 ${dsnId} 业务知识模型！`);
      if (onSaved) onSaved();
    } catch (err: any) {
      toast(err.message || '沉淀失败', true);
    }
  };

  return (
    <div
      style={{
        background: 'linear-gradient(135deg, #f0fdf4 0%, #ecfdf5 100%)',
        border: '1.5px solid #059669',
        borderRadius: '10px',
        padding: '12px 16px',
        marginBottom: '16px',
        boxShadow: '0 2px 8px rgba(5, 150, 105, 0.08)',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '8px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span
            style={{
              background: '#d1fae5',
              color: '#065f46',
              padding: '2px 8px',
              borderRadius: '6px',
              fontSize: '11.5px',
              fontWeight: 700,
              display: 'inline-flex',
              alignItems: 'center',
              gap: '4px',
            }}
          >
            <span>✓</span> 业务口径声明 (Caliber Declaration)
          </span>
          <span style={{ fontSize: '13px', fontWeight: 700, color: '#064e3b' }}>
            {caliber.metricName}
          </span>
        </div>

        <div>
          {isSaved ? (
            <span
              style={{
                background: '#059669',
                color: '#ffffff',
                padding: '4px 10px',
                borderRadius: '6px',
                fontSize: '12px',
                fontWeight: 600,
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
              }}
            >
              ★ 已沉淀至业务模型
            </span>
          ) : (
            <button
              type="button"
              className="btn small"
              style={{
                background: '#ffffff',
                borderColor: '#059669',
                color: '#047857',
                fontWeight: 600,
              }}
              onClick={handleSaveToBizModel}
            >
              ★ 沉淀为业务知识 (Save to BizModel)
            </button>
          )}
        </div>
      </div>

      {/* Details Grid */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
          gap: '12px',
          marginTop: '10px',
          paddingTop: '10px',
          borderTop: '1px dashed #a7f3d0',
          fontSize: '12.5px',
        }}
      >
        <div>
          <span style={{ color: '#047857', fontWeight: 600 }}>📐 度量计算公式：</span>
          <div style={{ fontFamily: 'monospace', color: '#064e3b', marginTop: '2px', wordBreak: 'break-all' }}>
            {caliber.formula}
          </div>
        </div>

        <div>
          <span style={{ color: '#047857', fontWeight: 600 }}>📍 聚合维度层级：</span>
          <div style={{ color: '#064e3b', marginTop: '2px' }}>
            {caliber.dimensions.length > 0 ? caliber.dimensions.join(' × ') : '全表单一汇聚'}
          </div>
        </div>

        <div>
          <span style={{ color: '#047857', fontWeight: 600 }}>⏳ 统计时间跨度：</span>
          <div style={{ color: '#064e3b', marginTop: '2px' }}>{caliber.timeWindow}</div>
        </div>

        <div>
          <span style={{ color: '#047857', fontWeight: 600 }}>🛡️ 严谨保证：</span>
          <div style={{ color: '#047857', marginTop: '2px', fontSize: '11.5px' }}>
            {caliber.safetyGuarantee}
          </div>
        </div>
      </div>
    </div>
  );
};
