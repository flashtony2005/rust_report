#!/usr/bin/env bash
set -euo pipefail
RPX_SAMPLE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RPX_SAMPLE_ROOT"
python3 scripts/prepare_r_model.py
cargo run --locked -p rpx-runtime --example report_gallery -- \
  examples/r-model web/public/report-gallery output/r-model/datasources.json
