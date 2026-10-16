import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// M4 前端 dev server 配置（端口约定见 docs/spec-m4.md §3：web 5288 / server 8090）。
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5288,
    strictPort: true,
    // M3-05/M4-06：`POST /api/render`（multipart 上传 .rpx + CSV）转发到 rpx-server。
    // 起服务：`cargo run -p rpx-server -- serve`（默认 127.0.0.1:8090）。
    // ⚠ rewrite 剥 `/api` 前缀：vite/http-proxy 默认**原样转发全路径**，
    // 而服务端路由是 `/render`（`docs/spec-m3.md` §3.2）——不剥就是 404
    // （2026-10-14 实测：MITM 抓到 `/api/health` 原样进引擎）。
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
      },
    },
  },
})
