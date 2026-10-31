#!/usr/bin/env bash
# 对 scripts/gen_decimal_oracle.sh 做故障注入自测（不依赖真 JVM）。
# 每个用例都必须让脚本失败，且不得在 tests/fixtures/_bdo 留下坏 oracle。
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
GEN="$ROOT/scripts/gen_decimal_oracle.sh"
BDO="tests/fixtures/_bdo"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

PASS=0; FAIL=0
note() { printf '%s\n' "$*"; }
ok()   { PASS=$((PASS+1)); printf '  ok   %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); printf '  FAIL %s\n' "$1"; }

# 构造一个假 JVM：$1=可执行路径 $2=版本头行 $3=数据行数 $4=字段分隔符合法性(ok|bad)
mkfake() {
  local path="$1" ver="$2" n="$3" mode="$4"
  cat > "$path" <<EOF
#!/bin/sh
echo "$ver"
i=0
while [ \$i -lt $n ]; do
  if [ "$mode" = "bad" ]; then
    echo "\$i|01|1|1E+3"
  else
    echo "\$i|01|1|1E+3|1000"
  fi
  i=\$((i+1))
done
EOF
  chmod +x "$path"
}

mkjavac() { printf '#!/bin/sh\nexit 0\n' > "$1"; chmod +x "$1"; }

# 跑一次生成，期望失败；并断言没有留下坏文件
expect_die() {
  local label="$1"; shift
  rm -f "$BDO/out_jre8.txt" "$BDO/out_jdk21.txt"
  local out rc
  out="$("$GEN" "$@" 2>&1)"; rc=$?
  if [ $rc -eq 0 ]; then
    bad "${label}：脚本竟然成功了（exit=0）"
    return
  fi
  if [ ! -s "$BDO/out_jre8.txt" ] && [ ! -s "$BDO/out_jdk21.txt" ]; then
    ok "${label}（exit=${rc}，未留坏文件）"
  else
    bad "${label}：失败了但留下了 oracle 文件 —— 半成品会被当成基准"
  fi
}

note "=== 用例 0：正常路径（两个不同版本头、各 26 条合法行）→ 必须成功 ==="
mkfake "$TMP/j8"  "java.version=1.8.0_151" 26 ok
mkfake "$TMP/j21" "java.version=21.0.2"    26 ok
mkjavac "$TMP/javac"
rm -f "$BDO/out_jre8.txt" "$BDO/out_jdk21.txt"
out="$("$GEN" --javac "$TMP/javac" --jre8 "$TMP/j8" --jdk21 "$TMP/j21" 2>&1)"; rc=$?
if [ $rc -eq 0 ] && [ -s "$BDO/out_jre8.txt" ] && [ -s "$BDO/out_jdk21.txt" ]; then
  ok "正常路径产出两份 oracle"
else
  bad "正常路径失败：exit=$rc / $out"
fi
rm -f "$BDO/out_jre8.txt" "$BDO/out_jdk21.txt"

note "=== 故障注入：以下每一项都必须让脚本红 ==="

# 1) 同一 JVM 复制两份（版本头相同）
mkfake "$TMP/a" "java.version=1.8.0_151" 26 ok
mkfake "$TMP/b" "java.version=1.8.0_151" 26 ok
expect_die "同源复制（版本头相同）" \
  --javac "$TMP/javac" --jre8 "$TMP/a" --jdk21 "$TMP/b"

# 2) 用例数不足（截断）
mkfake "$TMP/short" "java.version=1.8.0_151" 10 ok
expect_die "oracle 用例被截断（10 < 25）" \
  --javac "$TMP/javac" --jre8 "$TMP/short" --jdk21 "$TMP/j21"

# 3) 行格式不对（不是 5 段）
mkfake "$TMP/badfmt" "java.version=1.8.0_151" 26 bad
expect_die "行不是 5 段 '|' 格式" \
  --javac "$TMP/javac" --jre8 "$TMP/badfmt" --jdk21 "$TMP/j21"

# 4) 缺版本头行
cat > "$TMP/nohdr" <<'EOF'
#!/bin/sh
i=0; while [ $i -lt 26 ]; do echo "$i|01|1|1E+3|1000"; i=$((i+1)); done
EOF
chmod +x "$TMP/nohdr"
expect_die "首行不是 java.version= 头行" \
  --javac "$TMP/javac" --jre8 "$TMP/nohdr" --jdk21 "$TMP/j21"

# 5) 空输出
printf '#!/bin/sh\nexit 0\n' > "$TMP/empty"; chmod +x "$TMP/empty"
expect_die "oracle 是空文件" \
  --javac "$TMP/javac" --jre8 "$TMP/empty" --jdk21 "$TMP/j21"

# 6) javac 编译失败
printf '#!/bin/sh\necho "boom" >&2\nexit 1\n' > "$TMP/badjavac"; chmod +x "$TMP/badjavac"
expect_die "javac 编译失败" \
  --javac "$TMP/badjavac" --jre8 "$TMP/j8" --jdk21 "$TMP/j21"

# 7) java 运行失败
printf '#!/bin/sh\nexit 3\n' > "$TMP/badjava"; chmod +x "$TMP/badjava"
expect_die "java 运行失败" \
  --javac "$TMP/javac" --jre8 "$TMP/badjava" --jdk21 "$TMP/j21"

note ""
note "自测结果：ok=$PASS fail=$FAIL"
[ $FAIL -eq 0 ] || exit 1
