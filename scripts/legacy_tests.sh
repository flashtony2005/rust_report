#!/usr/bin/env bash
# Independent legacy .rpx test target (WP1). Default workspace tests stay JSON-only.
set -eu
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo test -p rpx-model --features legacy-rpx
cargo test -p rpx-format --features legacy-rpx
cargo test -p rpx-runtime --features legacy-rpx
cargo test -p rpx-cli --features legacy-rpx --test format_cli --test input_workflow
printf 'LEGACY TESTS PASS: model/format/runtime/cli\n'
