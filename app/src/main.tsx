import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import './index.css';

// 没有 Toast 容器：错误一律**就地显示**（页头下方的红字卡片），
// 不做 5 秒消失的提示 —— 会消失的提示等于没提示（见 app/README.md 的取舍记录）。
ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
