#!/usr/bin/env bash
# JSON-only build smoke: no legacy-rpx commands, no .rpx input.
set -eu
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
TARGET="$ROOT/target/json-only"
BIN="$TARGET/debug/rpx"
T="$(mktemp -d "${TMPDIR:-/tmp}/rpx-json-only.XXXXXX")"
cleanup() { /bin/rm -rf "$T"; }
trap cleanup EXIT

CARGO_TARGET_DIR="$TARGET" cargo build --no-default-features -p rpx-cli
# The no-default graph must not contain rpx-core or enable rpx-model/legacy-rpx.
if cargo tree -p rpx-cli --no-default-features 2>/dev/null | grep -q 'rpx-core'; then
  printf 'JSON-only graph still contains rpx-core\n' >&2
  exit 1
fi
if cargo tree -p rpx-cli --no-default-features -e features 2>/dev/null | grep -q 'rpx-model feature "legacy-rpx"'; then
  printf 'JSON-only graph still enables rpx-model/legacy-rpx\n' >&2
  exit 1
fi

# The no-default binary must not expose the three legacy entry points.
if "$BIN" --help | grep -Eq 'format|import-rpx|compile'; then
  printf 'JSON-only CLI still exposes legacy commands\n' >&2
  exit 1
fi

cat > "$T/template.json" <<'JSON'
{
  "format": "rpx-report",
  "format_version": 1,
  "schema": "rpx-report/v1",
  "grid": {"rows": 1, "cols": 2},
  "cells": [
    {"row": 1, "col": 1, "expr": "to(1,3)"},
    {"row": 1, "col": 2, "expr": "A1*10"}
  ]
}
JSON

"$BIN" template validate "$T/template.json"
"$BIN" run "$T/template.json" --out-dir "$T/run" --export csv
"$BIN" result inspect "$T/run/artifacts/result.json" --limit 2

"$BIN" library init "$T/lib" --presets
REV="$("$BIN" report show complex-master --library "$T/lib" | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["revision"])')"
"$BIN" report run complex-master --library "$T/lib" --if-match "$REV" --export csv
"$BIN" report list --library "$T/lib" --limit 2
printf 'JSON-ONLY PASS: validate/run/result/library/report without legacy .rpx\n'
