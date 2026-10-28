import React, { useState } from 'react';
import { store } from '../../services/store';
import { useToast } from '../Toast';

interface Props {
  onClose: () => void;
}

export const LlmSettingsModal: React.FC<Props> = ({ onClose }) => {
  const { toast } = useToast();
  const current = store.getLlmConfig();
  const [provider, setProvider] = useState<'deepseek' | 'openai'>(current.provider);
  const [baseUrl, setBaseUrl] = useState(current.base_url);
  const [model, setModel] = useState(current.model);
  const [apiKey, setApiKey] = useState('');

  const handleSave = () => {
    store.setLlmConfig({
      provider,
      base_url: baseUrl.trim(),
      model: model.trim(),
      api_key: apiKey.trim() || undefined,
    });
    toast('理解层配置已保存');
    onClose();
  };

  return (
    <div className="modal-mask">
      <div className="modal" style={{ maxWidth: '540px' }}>
        <h2>理解层（大模型）</h2>
        <div className="sub muted" style={{ fontSize: '12px', lineHeight: '1.7' }}>
          自然语言 → <code>rpx-query/v1</code> 草案 → 编译器校验。
          <b>模型说的话不作数</b>：校验不过（未注册的指标/未决歧义）就拒绝编译。
          key 只落本机，不向外广播。
        </div>
        <div className="stack" style={{ marginTop: '14px' }}>
          <div>
            <label className="field">服务</label>
            <select
              value={provider}
              onChange={(e) => setProvider(e.target.value as 'deepseek' | 'openai')}
            >
              <option value="deepseek">deepseek (DeepSeek API)</option>
              <option value="openai">openai (OpenAI Compatible)</option>
            </select>
          </div>
          <div>
            <label className="field">Base URL</label>
            <input
              type="text"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://api.deepseek.com"
            />
          </div>
          <div>
            <label className="field">模型</label>
            <input
              type="text"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="deepseek-v4-flash"
            />
          </div>
          <div>
            <label className="field">API Key</label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={current.has_key ? '已存，留空不变' : '粘贴你的 API Key'}
            />
          </div>
        </div>
        <div className="row" style={{ justifyContent: 'flex-end', marginTop: '16px' }}>
          <button type="button" className="btn" onClick={onClose}>
            取消
          </button>
          <button type="button" className="btn primary" onClick={handleSave}>
            保存
          </button>
        </div>
      </div>
    </div>
  );
};
