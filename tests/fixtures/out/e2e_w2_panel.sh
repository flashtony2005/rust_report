#!/usr/bin/env bash
# W2-2 浏览器端到端编排（M7-W）：起 rpx-server + vite dev → 跑 playwright 驱动 → 收摊。
#
# 用法：bash tests/fixtures/out/e2e_w2_panel.sh
#   前置：cargo build -p rpx-server（本脚本直接跑 target/debug/rpx-server.exe）；
#        web/node_modules 已 npm install（vite + 驱动用的 playwright 都要）。
# 端口固定 8090/5288（vite proxy 写死 8090，见 web/vite.config.ts）；
# 本脚本用随机资产库 + trap 收摊，跑完清理。
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT" || exit 1
BIN="target/debug/rpx-server.exe"
[ -x "$BIN" ] || { echo "[w2e2e] 缺 $BIN —— 先 cargo build -p rpx-server"; exit 2; }
[ -d web/node_modules/vite ] || { echo "[w2e2e] 缺 web/node_modules —— 先 cd web && npm install"; exit 2; }

TMP="$(mktemp -d "${TMPDIR:-/tmp}/rpx-w2e2e.XXXXXX")" || exit 1
DB="target/e2e-w2-$$.db"
cleanup() {
  [ -n "${SRV_PID:-}" ] && kill "$SRV_PID" 2>/dev/null
  [ -n "${VITE_PID:-}" ] && kill "$VITE_PID" 2>/dev/null
  /bin/rm -f "$DB" 2>/dev/null
  /bin/rm -rf "$TMP" 2>/dev/null
}
trap cleanup EXIT

# 前置检查：8090/5288 不能被占（否则后面全是鬼影服务）。
if curl --noproxy '*' -s -o /dev/null --max-time 1 http://127.0.0.1:8090/health; then
  echo "[w2e2e] 8090 已被占用 —— 先停掉旧服务"; exit 2
fi
if curl --noproxy '*' -s -o /dev/null --max-time 1 http://127.0.0.1:5288/; then
  echo "[w2e2e] 5288 已被占用 —— 先停掉旧 vite"; exit 2
fi

echo "[w2e2e] 起 rpx-server（8090，资产库 ${DB}）"
"$BIN" serve --addr 127.0.0.1:8090 --db "$DB" >"$TMP/server.log" 2>&1 &
SRV_PID=$!
for _ in $(seq 1 30); do
  [ "$(curl --noproxy '*' -s --max-time 2 http://127.0.0.1:8090/health || true)" = "ok" ] && break
  sleep 0.5
done
if [ "$(curl --noproxy '*' -s --max-time 2 http://127.0.0.1:8090/health || true)" != "ok" ]; then
  echo "[w2e2e] 服务未就绪"; tail -5 "$TMP/server.log"; exit 1
fi

echo "[w2e2e] 起 vite dev（5288；--host 127.0.0.1 必须，否则绑 IPv6 ::1 —— 口径见 docs/spec-m4.md §6）"
node web/node_modules/vite/bin/vite.js "$ROOT/web" --port 5288 --strictPort --host 127.0.0.1 >"$TMP/vite.log" 2>&1 &
VITE_PID=$!
for _ in $(seq 1 40); do
  code="$(curl --noproxy '*' -s -o /dev/null -w '%{http_code}' --max-time 2 http://127.0.0.1:5288/ || true)"
  [ "$code" = "200" ] && break
  sleep 0.5
done
if [ "$code" != "200" ]; then
  echo "[w2e2e] vite 未就绪（code=${code}）"; tail -10 "$TMP/vite.log"; exit 1
fi

echo "[w2e2e] 跑浏览器驱动（${RPX_DRIVER:-tests/fixtures/out/e2e_w2_panel.mjs}）"
node "${RPX_DRIVER:-tests/fixtures/out/e2e_w2_panel.mjs}"
RC=$?

if [ "$RC" -ne 0 ]; then
  echo "[w2e2e] 驱动失败（rc=${RC}）—— vite 日志尾部："; tail -5 "$TMP/vite.log"
fi
exit "$RC"
