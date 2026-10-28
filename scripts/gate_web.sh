#!/usr/bin/env bash
# G9：前端可验证性地基（M7-W W0-3，见 docs/spec-m7-web.md §4）。
#
# 判据两条，都必须过：
#   ① `npm run build` = `tsc --noEmit && vite build`（类型检查 + 构建）
#   ② `npm test`      = `vitest run`（纯逻辑单测）
#
# ⚠ **构建产物写进临时目录，不是 web/dist**。vite 默认 emptyOutDir，会 rmSync
#   掉整个 outDir —— 本项目 ~80 个 chunk。WorkBuddy 沙箱给 node 的 fs.rmSync
#   套了 safe-delete 包装，**一次删 >50 个文件**就抛
#   `SAFE_DELETE_BULK_CONFIRM_REQUIRED`，把 build 直接打红（2026-10-16 实测）。
#   而 dist/ 是 .gitignore 的构建产物，「把它清空」根本不是本 gate 的判据。
#   ⇒ 写到 mktemp 出来的空目录：待删条目数为 0，guard 直接放行，
#     沙箱内外行为一致，也不用在仓库里堆陈旧的 hash 文件名。
#   ⚠ **不要去改 node 的 fs.rmSync / safe-delete 包装来"绕过"**：那是别人工具
#     的安全机制，改它等于把仓库的门禁建在沙箱实现细节上，升级即失效且是
#     静默失效。
#
# ⚠ **缺 node_modules 必须红，不许静默跳过** —— 静默跳过的 gate 等于没有 gate
#   （本仓「固定报 N 条噪声的检查等于没有检查」的同一条纪律）。
# ⚠ 也**不自动 npm install**：装依赖要联网、是显式动作，藏在 gate 里会把
#   「依赖没装」这个真实问题掩盖成绿。
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB="$ROOT/web"

# ① npm 必须在 PATH 上（不写死路径：本仓要能在任何机器上跑）。
if ! command -v npm >/dev/null 2>&1; then
  printf 'G9 FAIL: npm 不在 PATH 上。装好 Node/npm 再跑。\n' >&2
  exit 1
fi

# ② 依赖必须装齐。
if [ ! -d "$WEB/node_modules" ]; then
  printf 'G9 FAIL: web/node_modules 不存在 —— 先跑 `cd web && npm install`\n' >&2
  printf '         （本 gate 不自动安装，见文件头说明）\n' >&2
  exit 1
fi
if [ ! -d "$WEB/node_modules/vitest" ]; then
  printf 'G9 FAIL: vitest 未安装 —— 先跑 `cd web && npm install -D vitest`\n' >&2
  exit 1
fi

cd "$WEB" || exit 1

# 临时产物目录：见文件头「⚠ 构建产物写进临时目录」。
# 清理用 /bin/rm 而不是裸 rm —— 本机 PATH 上的 rm 是安全删除包装（搬进
# ~/.Trash 而不是真删），每次 gate 白塞 ~7MB 垃圾进废纸篓。/bin/rm 是真删
# （判据：删完 `ls -A ~/.Trash | grep rpx-g9` 应为 0）。
TMP_OUT="$(mktemp -d "${TMPDIR:-/tmp}/rpx-g9-build.XXXXXX")" || exit 1
cleanup() { /bin/rm -rf "$TMP_OUT" 2>/dev/null || true; }
trap cleanup EXIT

# ③ 类型检查 + 构建。`--outDir` 见文件头。
#    输出压掉 vite 的 chunk 体积表（每行都含 " gzip: "，纯噪声），
#    再取末 30 行：失败信息在末尾 ⇒ tail 不损失失败可见性。
printf -- '--- npm run build (outDir=%s) ---\n' "$TMP_OUT"
npm run build -- --outDir "$TMP_OUT" 2>&1 | grep -v ' gzip: ' | tail -30
build_rc=${PIPESTATUS[0]}
if [ "$build_rc" -ne 0 ]; then
  printf 'G9 FAIL: npm run build exit=%s\n' "$build_rc" >&2
  exit 1
fi

# ④ 纯逻辑单测（输出短，全打）。
printf -- '--- npm test ---\n'
npm test
test_rc=$?
if [ "$test_rc" -ne 0 ]; then
  printf 'G9 FAIL: npm test exit=%s\n' "$test_rc" >&2
  exit 1
fi

printf 'G9: 前端 build + vitest 全过。\n'
exit 0
