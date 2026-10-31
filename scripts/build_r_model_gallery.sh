#!/usr/bin/env bash
set -euo pipefail
RPX_SAMPLE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RPX_SAMPLE_ROOT"
python3 scripts/prepare_r_model.py
# 把 examples/presets 的 30 份目录包同步进 catalog（幂等；无运行记录会现跑一次取校验值）。
python3 scripts/sync_gallery_presets.py
cargo run --locked -p rpx-runtime --example report_gallery -- \
  examples/r-model web/public/report-gallery output/r-model/datasources.json
