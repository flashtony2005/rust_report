import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// 端口固定 5289：5288 是现有的 Univer 设计器（web/），8090 是 Rust 服务端，
// 8778 是 Python studio（已冻结）。四个端口各占一件事，不要挤。
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5289,
    host: '127.0.0.1',
    strictPort: true,
    // 前端只认 `/api`：开发期由 vite 转发到 Rust，生产期由同一个域名反代到 rpx-server。
    // 这样代码里不出现 localhost:8090，换部署形态不用改源码。
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8090',
        changeOrigin: false,
        // Rust 侧路由是 `/reports`，没有 `/api` 前缀（http.rs:34）⇒ 转发时剥掉。
        // 漏了这句的典型症状：所有请求 404，且响应体是 `{"error":{"code":"NOT_FOUND"}}`。
        rewrite: (p) => p.replace(/^\/api/, ''),
      },
    },
  },
  preview: { port: 5289, host: '127.0.0.1', strictPort: true },
});
