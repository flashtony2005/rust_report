import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// M4 前端 dev server 配置（端口约定见 docs/spec-m4.md §3：web 5288 / server 8090）。
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5288,
    strictPort: true,
    // 代理共享 JSON 报表库与固定运行产物；服务端显式指定 --library DIR。
    // 浏览器使用 /api/reports，后端路由为 /reports。
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8090',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
    },
  },
  build: {
    rollupOptions: {
      // 多页入口：`index.html`（产品页）+ `probe.html`（W2 准入探针，见
      // `src/probe.tsx`）。⚠ 加上 probe 入口是为了让 G9 的 `tsc --noEmit &&
      // vite build` **也覆盖探针页** —— 否则探针页的类型错误不会被门禁拦住，
      // 而它恰恰是 W2 分期决策的依据。
      input: {
        index: 'index.html',
        probe: 'probe.html',
        gallery: 'gallery.html',
      },
    },
  },
})
