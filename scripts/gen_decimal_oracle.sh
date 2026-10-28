#!/usr/bin/env bash
# 生成 `tests/fixtures/_bdo/out_jre8.txt` 与 `out_jdk21.txt`
# （crates/rpx-engine/tests/decimal_oracle.rs 的唯一期望值来源）。
#
# 铁律
#   1. oracle 只能由真实 JVM 跑出来 —— 禁止手写、禁止"从一个 JVM 复制成两份"。
#      `decimal_oracle` 的 `the_two_jvm_oracles_agree_byte_for_byte` 会断言两份
#      版本头行不同；同源复制会让这条断言先红，那正是它该干的事。
#   2. 必须先自检再落盘 —— 本机没 JVM 时**必须大声失败**，不许产出空文件/截断文件
#      （产出垃圾 oracle 比没有 oracle 更糟：测试会拿错的东西当基准）。
#
# 用法
#   scripts/gen_decimal_oracle.sh \
#       --javac /path/to/javac \
#       --jre8  /path/to/product-jre/bin/java \
#       --jdk21 /path/to/jdk21/bin/java
#
# 产物落地（脚本不会自动 git add，避免把没验证过的东西带到索引里）
#   git add tests/fixtures/_bdo/out_jre8.txt tests/fixtures/_bdo/out_jdk21.txt
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SRC="tests/fixtures/ProbeBigDecimalToString.java"   # 仓库内已跟踪的探针源
OUTDIR="tests/fixtures/_bdo"
FQCN="rpx.fixtures.ProbeBigDecimalToString"

JAVAC=""
JRE8_JAVA=""
JDK21_JAVA=""

die() { printf 'gen_decimal_oracle: %s\n' "$*" >&2; exit 2; }

# 两份 oracle 一起清掉。绝不允许「只删坏的那份」—— 那会让
# 「旧 out_jdk21 + 新坏 out_jre8」的新旧混搭状态留在盘上，
# decimal_oracle 的跨 JVM 比对会拿旧文件当真。
purge() { rm -f "$OUTDIR/out_jre8.txt" "$OUTDIR/out_jdk21.txt"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --javac) [ $# -ge 2 ] || die "--javac 缺参数"; JAVAC="$2"; shift 2 ;;
    --jre8)  [ $# -ge 2 ] || die "--jre8 缺参数";  JRE8_JAVA="$2"; shift 2 ;;
    --jdk21) [ $# -ge 2 ] || die "--jdk21 缺参数"; JDK21_JAVA="$2"; shift 2 ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    *) die "未知参数：$1（用 --help 看用法）" ;;
  esac
done

[ -n "$JAVAC" ]     || die "缺 --javac（任一版本的 javac，探针是纯 JDK 代码）"
[ -n "$JRE8_JAVA" ] || die "缺 --jre8（产品自带 JRE 1.8 的 java）"
[ -n "$JDK21_JAVA" ] || die "缺 --jdk21（JDK 21 的 java）"

for exe in "$JAVAC" "$JRE8_JAVA" "$JDK21_JAVA"; do
  [ -x "$exe" ] || command -v "$exe" >/dev/null || die "可执行不存在或不可执行：$exe"
done
[ -f "$SRC" ] || die "探针源不在：$SRC"

BUILD="$(mktemp -d)"
trap 'rm -rf "$BUILD"' EXIT

mkdir -p "$OUTDIR"
# 先清掉上一轮的产物：失败时不至於让「旧文件」冒充新 oracle
purge

printf '>>> 编译 %s\n' "$SRC"
# -source/-target 8：让 javac（可能是 21）产出 JRE 1.8 也能加载的字节码。
"$JAVAC" -d "$BUILD" -source 8 -target 8 "$SRC" \
  || { purge; die "编译失败（javac=${JAVAC}）"; }

run_one() {  # $1=java 可执行  $2=输出文件名
  local java="$1" out="$2"
  # 重定向会先把文件截成 0 字节，所以失败也必须 purge，否则留下一个空 oracle
  "$java" -cp "$BUILD" "$FQCN" > "$OUTDIR/$out" \
    || { purge; die "运行失败：$java"; }
}

mkdir -p "$OUTDIR"
run_one "$JRE8_JAVA"  out_jre8.txt
run_one "$JDK21_JAVA" out_jdk21.txt

# ---- 自检：任一不过就删掉产物并失败，绝不留半成品 -----------------------
# ---- 自检：任一不过就两份一起删掉并失败，绝不留半成品 -------------------
check_one() {  # $1=文件名
  local f="$OUTDIR/$1"
  [ -s "$f" ] || { purge; die "$f 是空文件 —— 产出无效"; }
  head -1 "$f" | grep -q '^java.version=' \
    || { purge; die "$f 首行不是 java.version= 头行：$(head -1 "$f")"; }
  local n
  n="$(grep -vc '^java.version=' "$f" || true)"
  [ "$n" -ge 25 ] || { purge; die "$f 只有 $n 条用例（应 >= 25）—— 文件被截断？"; }
  local bad
  bad="$(grep -v '^java.version=' "$f" | grep -vc '^[^|]*|[^|]*|[^|]*|[^|]*|[^|]*$' || true)"
  [ "$bad" -eq 0 ] || { purge; die "$f 有 $bad 行不是 5 段 '|' 格式"; }
  return 0
}

check_one out_jre8.txt
check_one out_jdk21.txt

h8="$(head -1 "$OUTDIR/out_jre8.txt")"
h21="$(head -1 "$OUTDIR/out_jdk21.txt")"
if [ "$h8" = "$h21" ]; then
  purge
  die "两份 oracle 版本头行相同（${h8}）—— 同源复制会让 \
the_two_jvm_oracles_agree_byte_for_byte 红掉；请用两个不同的 JVM 重跑"
fi

printf 'OK：%s\n' "$OUTDIR/out_jre8.txt"
printf 'OK：%s\n' "$OUTDIR/out_jdk21.txt"
printf '下一步：git add %s/out_jre8.txt %s/out_jdk21.txt\n' "$OUTDIR" "$OUTDIR"
