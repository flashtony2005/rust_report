#!/usr/bin/env bash
# e2e 冒烟（M3-07）：真 socket 全链路「上传 → render → 断言快照键集」。
#
# 与集成测试（tests/render_http.rs，cargo test 内跑）的区别：本脚本跑的是
# **真实二进制** `target/debug/rpx-server` 的 `serve` 子命令（含 main.rs 的
# 参数解析 / AssetStore 建库路径），覆盖「编译产物可执行」这一层。
# 调用：bash scripts/e2e_smoke.sh（需先 cargo build -p rpx-server）。
# 任一断言失败 exit 1；服务/临时文件必被清理（trap 兜底）。
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 1
BIN="target/debug/rpx-server.exe"
FIX="tests/fixtures"
FAILED=0

note() { printf '%s\n' "[e2e] $*"; }
fail() { printf '%s\n' "[e2e] FAIL: $*"; FAILED=1; }

[ -x "$BIN" ] || { echo "[e2e] 二进制不存在：$BIN —— 先 cargo build -p rpx-server"; exit 2; }

# 随机高位端口 + 临时资产库。
# ⚠ 临时库放仓库内**相对路径**：rpx-server.exe 是原生 Windows 程序，
# 不认 MSYS 的 /tmp（mktemp 产物），$TEMP 的反斜杠也会砸碎 sqlite URL。
PORT=$(( (RANDOM % 2000) + 18000 ))
ADDR="127.0.0.1:$PORT"
TMPDB="target/smoke-$$.db"
CURL=(curl --noproxy '*' -s --max-time 15)

cleanup() {
  [ -n "${SRV_PID:-}" ] && kill "$SRV_PID" 2>/dev/null
  rm -f "$TMPDB" "${JF:-}" 2>/dev/null
}
trap cleanup EXIT

note "起服务 ${ADDR}（资产库 smoke.db）"
"$BIN" serve --addr "$ADDR" --db "$TMPDB" >/dev/null 2>&1 &
SRV_PID=$!

# 轮询 /health（最多 10s）。
for _ in $(seq 1 20); do
  H="$("${CURL[@]}" "http://$ADDR/health" 2>/dev/null || true)"
  [ "$H" = "ok" ] && break
  sleep 0.5
done
[ "$H" = "ok" ] || { fail "服务未就绪（/health=${H}）"; exit 1; }
note "health ok"

# ① 上传 .rpx（full.rpx 声明两个静态参数）→ 断言 id。
UP="$("${CURL[@]}" -F "file=@$FIX/full.rpx" "http://$ADDR/reports")"
echo "$UP" | grep -q '"name":"full"' || fail "上传响应异常：$UP"
RID="$(printf '%s' "$UP" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')"
[ -n "$RID" ] || { fail "未拿到 report id：$UP"; exit 1; }
note "上传 ok（id=${RID}）"

# ② meta：静态参数清单（is_static + 默认值）。
META="$("${CURL[@]}" "http://$ADDR/reports/$RID/meta")"
echo "$META" | grep -q '"name":"startDate"' && echo "$META" | grep -q '"is_static":true' \
  || fail "meta 异常：$META"
echo "$META" | grep -q '"default_val":"2024-01-01"' || fail "meta 缺默认值：$META"
note "meta ok（startDate/topN 静态参数）"

# ③ render：GBK 模板 + CSV → 快照键集断言（契约 docs/spec-m4.md §4）。
RD="$("${CURL[@]}" -F "report=@$FIX/engine_gbk.rpx" -F "dataset=@$FIX/engine_gbk.csv" \
  "http://$ADDR/render")"
for key in '"id":"rpx-engine_gbk"' '"sheetOrder"' '"mergeData"' '"cellData"' '"rowData"' '"columnData"' '"pagination"' '"page_w_pt"'; do
  echo "$RD" | grep -q "$key" || fail "快照缺键 $key"
done
echo "$RD" | grep -q '华北' || fail "快照缺中文数据（GBK 过网）"
note "render ok（快照键集 + 中文数据齐全）"

# ④ 负路径：坏魔数上传必须 400 且不落库。
JF="target/smoke-junk-$$.rpx"
printf 'not an rpx' > "$JF"
BAD="$("${CURL[@]}" -o /dev/null -w '%{http_code}' -F "file=@$JF;filename=junk.rpx" "http://$ADDR/reports")"
[ "$BAD" = "400" ] || fail "坏魔数应 400，实得 $BAD"
LIST="$("${CURL[@]}" "http://$ADDR/reports")"
echo "$LIST" | grep -q 'junk' && fail "坏魔数不得入库：$LIST"
note "负路径 ok（坏魔数 400 且不入库）"

cleanup
if [ $FAILED -eq 0 ]; then
  note "E2E SMOKE: 全绿（上传→meta→render 键集→负路径）"
  exit 0
fi
note "E2E SMOKE: 有红项"
exit 1
