#!/usr/bin/env bash
# Tier-0 机器闸：G1–G9。任一 FAIL ⇒ 不许开 PR（见 docs/code-review.md §3）。
#
# 设计约束：
#   * 不 `set -e` —— 必须跑完全部 gate，让作者一次看全红在哪。
#   * 每个 gate 的原始输出都打印，不许只打 PASS/FAIL（"只报通过"等于没报）。
#   * 从任何目录调用都能工作。
#
# G1–G8 = Rust 侧；**G9 = 前端侧**（M7-W W0-3，docs/spec-m7-web.md §4）：
# `web/` 的 `npm run build` + `vitest run`。它**缺 node_modules 就红**，不静默跳过。
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 1

FAILED=0
declare -a RESULTS=()

hr() { printf '%s\n' "------------------------------------------------------------"; }

# run_gate <id> <name> <cmd...>
run_gate() {
  local id="$1"; shift
  local name="$1"; shift
  hr
  printf '>>> %s %s\n' "$id" "$name"
  hr
  "$@"
  local rc=$?
  if [ $rc -eq 0 ]; then
    RESULTS+=("PASS  $id $name")
  else
    RESULTS+=("FAIL  $id $name   (exit=$rc)")
    FAILED=1
  fi
  printf '\n'
}

# ---- G1 格式 -------------------------------------------------------------
run_gate G1 "cargo fmt --all --check" cargo fmt --all --check

# ---- G2 lint -------------------------------------------------------------
run_gate G2 "clippy -D warnings (all targets)" \
  cargo clippy --workspace --all-targets -- -D warnings

# ---- G3 测试 -------------------------------------------------------------
run_gate G3 "cargo test --no-fail-fast" \
  cargo test --workspace --no-fail-fast

# ---- G4 每 .rs ≤ 300 行 --------------------------------------------------
# ---- G4 每个 .rs <= 500 行 -------------------------------------------------
# 阈值沿革：300（2026-10-13 立稿）→ 500（同日调整）。
# 调高的真实原因：本仓长期靠**手写高密度排版**才守住 300 行，而 2026-10-13 全仓
# `cargo fmt --all` 之后 rustfmt 的竖向展开（max_width=100）一次性把 11 个文件
# 撑过 300。用户选择"调阈值"而非"拆 10 个文件"。代价与约束写在
# docs/code-review.md §3.1 —— 这不是"从此可以写到 500"的许可。
MAX=500
# 逼近阈值时提前喊一声，避免无声 creep 到顶格才被发现。
WARN=$((MAX - 50))

over="$(find crates -name '*.rs' -exec wc -l {} + | awk -v m="$MAX" '$1>m && $2!="total" {print "OVER("$1"): "$2}')"
near="$(find crates -name '*.rs' -exec wc -l {} + | awk -v w="$WARN" -v m="$MAX" '$1>w && $1<=m && $2!="total" {print "NEAR("$1"): "$2}')"
hr
printf '>>> G4 每个 .rs <= %d 行\n' "$MAX"
hr
if [ -z "$over" ]; then
  printf '全部 .rs 均在 %d 行以内。\n' "$MAX"
  RESULTS+=("PASS  G4 每 .rs <= $MAX 行")
else
  printf '%s\n' "$over"
  RESULTS+=("FAIL  G4 每 .rs <= $MAX 行")
  FAILED=1
fi
if [ -n "$near" ]; then
  printf '⚠ 已越过 %d 行、逼近上限（该考虑拆了）：\n%s\n' "$WARN" "$near"
fi
printf '\n'

# ---- G5 依赖策略（分层白名单；正文见 docs/code-review.md §3.2）-------------
# 零依赖层：这些 crate 的 [dependencies] 只许 path 依赖，一条非 path 即 FAIL。
# 其余 crate：非 path 依赖必须在 scripts/dep_allowlist.txt 里登记，否则 FAIL。
# 白名单里的**死条目**（登记了但代码里没有）同样 FAIL。
ALLOWLIST="scripts/dep_allowlist.txt"
dep_out="$(awk -v allowfile="$ALLOWLIST" '
  BEGIN {
    zero["rpx-core"] = 1
    zero["rpx-engine"] = 1
    while ((getline l < allowfile) > 0) {
      sub(/#.*/, "", l); gsub(/^[ \t]+|[ \t]+$/, "", l)
      if (l == "") continue
      if (split(l, p, /[ \t]+/) >= 2) allow[p[1] " " p[2]] = 1
    }
    close(allowfile)
  }
  FNR == 1 {
    crate = FILENAME
    sub(/^crates\//, "", crate); sub(/\/Cargo.toml$/, "", crate)
    insec = 0
  }
  /^\[dependencies\]/ { insec = 1; next }
  /^\[/              { insec = 0 }
  insec {
    line = $0
    sub(/#.*/, "", line); gsub(/^[ \t]+|[ \t]+$/, "", line)
    if (line == "") next
    if (!match(line, /^[A-Za-z0-9_-]+/)) next
    dep = substr(line, 1, RLENGTH)
    if (line ~ /path[ \t]*=/) next            # path 依赖一律放行
    key = crate " " dep
    if (crate in zero) {
      printf "  零依赖层违规  %s: %s —— rpx-core/rpx-engine 只许 path 依赖\n", crate, dep
      bad = 1; next
    }
    if (!(key in allow)) {
      printf "  未登记依赖    %s: %s —— 请先在 %s 登记并在 docs/code-review.md §3.2 写明理由\n", crate, dep, allowfile
      bad = 1; next
    }
    used[key] = 1
  }
  END {
    for (k in allow) if (!(k in used)) {
      printf "  白名单死条目  %s —— 该依赖已不存在，请从 %s 删除\n", k, allowfile
      bad = 1
    }
    exit bad
  }
' crates/*/Cargo.toml)"
hr
printf '>>> G5 依赖策略（分层白名单：零依赖层 + 受审白名单）\n'
hr
if [ -z "$dep_out" ]; then
  printf '零依赖层（rpx-core / rpx-engine）无第三方依赖；%s 内无未登记项。\n' "$ALLOWLIST"
  RESULTS+=("PASS  G5 依赖策略")
else
  printf '%s\n' "$dep_out"
  RESULTS+=("FAIL  G5 依赖策略（零依赖层违规 / 未登记依赖 / 白名单死条目）")
  FAILED=1
fi
printf '\n'

# ---- G6 「$var + 全角标点」陷阱（bash 会把标点吃进变量名，见 docs/code-review.md §5.12）-----
hr
printf '>>> G6 shell 脚本无「$var + 全角标点」陷阱\n'
hr
if python3 scripts/scan_sh_trap.py scripts; then
  RESULTS+=("PASS  G6 无 bash 变量名延长陷阱")
else
  RESULTS+=("FAIL  G6 存在 bash 变量名延长陷阱")
  FAILED=1
fi
printf '\n'

# ---- G7 文档引用完整性（代码/文档指向 `*.md §N.M` 的引用必须解析成功）---------
# 触发点：2026-10-13 一天内查出 8 处代码注释指向 m2-engine-design.md §5.4（从未写出）、
# merge.rs → java-reference-map.md §6.6（不存在）、names.rs → code-review.md §5.9（不存在）。
# 规则与理由见 docs/code-review.md §3；判据实现见 scripts/check_doc_refs.py（已做故障注入）。
hr
printf '>>> G7 文档引用完整性（*.md §N.M 必须存在）\n'
hr
if python3 scripts/check_doc_refs.py; then
  RESULTS+=("PASS  G7 文档引用完整性")
else
  RESULTS+=("FAIL  G7 存在指向不存在章节/文档的引用")
  FAILED=1
fi
printf '\n'

# ---- G8 crate 分层矩阵（M3-07：依赖方向 = 架构，越界即 FAIL）------------------
# 允许矩阵（docs/spec-m3.md §3.1）：数据/引擎向下，供数方（rpx-ds）不依赖消费方
# （rpx-engine）——M3-03 定案。新 crate 进仓必须在这里登记它的行，默认拒绝。
# 实现：扫描各 crate [dependencies] 的 path 依赖，对照矩阵；越界 / 未知 crate 均红。
hr
printf '>>> G8 crate 分层矩阵（path 依赖方向）\n'
hr
layer_out="$(awk '
  BEGIN {
    # allowed["crate dep"] = 1
    allowed["rpx-engine rpx-core"] = 1
    allowed["rpx-ds rpx-core"] = 1
    allowed["rpx-cli rpx-core"] = 1
    allowed["rpx-cli rpx-engine"] = 1
    allowed["rpx-cli rpx-export"] = 1
    allowed["rpx-server rpx-core"] = 1
    allowed["rpx-server rpx-engine"] = 1
    allowed["rpx-server rpx-ds"] = 1
    allowed["rpx-server rpx-export"] = 1
    # M6-01：导出面（受审层）吃已算完的 CalcGrid ⇒ 只依赖 engine，向下不溜。
    allowed["rpx-export rpx-engine"] = 1
    known["rpx-core"] = 1; known["rpx-engine"] = 1; known["rpx-ds"] = 1
    known["rpx-cli"] = 1; known["rpx-server"] = 1; known["rpx-export"] = 1
  }
  FNR == 1 {
    crate = FILENAME
    sub(/^crates\//, "", crate); sub(/\/Cargo.toml$/, "", crate)
    insec = 0
  }
  /^\[dependencies\]/ { insec = 1; next }
  /^\[/               { insec = 0 }
  insec {
    line = $0
    sub(/#.*/, "", line); gsub(/^[ \t]+|[ \t]+$/, "", line)
    if (line == "" || line !~ /path[ \t]*=/) next
    if (!match(line, /^[A-Za-z0-9_-]+/)) next
    dep = substr(line, 1, RLENGTH)
    if (dep == crate) next
    if (!(crate in known)) {
      printf "  未登记 crate   %s —— 新 crate 必须先在 G8 矩阵登记允许的依赖行\n", crate
      bad = 1; next
    }
    if (!((crate " " dep) in allowed)) {
      printf "  分层违规      %s -> %s —— 越出允许矩阵（docs/spec-m3.md §3.1）\n", crate, dep
      bad = 1
    }
  }
  END { exit bad }
' crates/*/Cargo.toml)"
if [ -z "$layer_out" ]; then
  printf '全部 path 依赖在分层矩阵内（core <- {engine, ds} <- {cli, server, export}）。\n'
  RESULTS+=("PASS  G8 crate 分层矩阵")
else
  printf '%s\n' "$layer_out"
  RESULTS+=("FAIL  G8 crate 分层矩阵（越界依赖 / 未登记 crate）")
  FAILED=1
fi
printf '\n'

# ---- G9 前端可验证性地基（M7-W W0-3）------------------------------------
# 逻辑在 scripts/gate_web.sh：npm run build + vitest run，缺 node_modules 即红。
run_gate G9 "前端 build + vitest（M7-W W0-3）" bash scripts/gate_web.sh

# ---- 汇总 ----------------------------------------------------------------
hr
printf '汇总\n'
hr
for r in "${RESULTS[@]}"; do
  printf '  %s\n' "$r"
done
hr
if [ $FAILED -eq 0 ]; then
  printf 'GATE: 全绿，可以开 PR。\n'
  exit 0
fi
printf 'GATE: 有红项，不许开 PR。修完重跑 scripts/gate.sh。\n'
exit 1
