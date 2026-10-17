# Web 登录与 Bearer 鉴权

## 服务端

默认本地开发无鉴权：

```sh
cargo run --locked -p rpx-server -- serve --library ./reports
```

启用 Bearer 鉴权：

```sh
cargo run --locked -p rpx-server -- serve \
  --library ./reports --auth-token "$RPX_TOKEN"
# 或
RPX_AUTH_TOKEN="$RPX_TOKEN" cargo run --locked -p rpx-server -- serve --library ./reports
```

受保护路由：`/reports*`。
公开路由：`/health`、`/auth/status`。

无 token 或 token 错误：

```json
{"error":{"code":"AUTH_REQUIRED","message":"需要 Authorization: Bearer <token>"}}
```

## Web

- 启动时请求 `/api/auth/status`。
- `required=true` 且本地无 token → 显示登录页。
- token 存 `sessionStorage`（键 `rpx.auth.token`）。
- 所有 API 自动带 `Authorization: Bearer <token>`。
- 收到 401 会清除 token 并回到登录页。

## E2E

`web/e2e/auth.e2e.ts` 用 Playwright 覆盖：

1. 需要鉴权：登录页 → 输入 token → 进入报表库
2. 无鉴权：直接进入报表库

本地：

```sh
cd web
npm run e2e
```

CI 独立 job `web-e2e` 安装 Chromium 后执行。服务端鉴权另有 Rust HTTP 测试：

```sh
cargo test -p rpx-server --test library_http bearer_auth
```
