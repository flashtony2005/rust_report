import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// M4 前端 dev server 配置（端口约定见 docs/spec-m4.md §3：web 5288 / server 8090）。
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5288,
    strictPort: true,
    // M3-05（axum）就绪后启用：把 /api 转发到 rpx-server
    // proxy: { '/api': 'http://127.0.0.1:8090' },
  },
})
