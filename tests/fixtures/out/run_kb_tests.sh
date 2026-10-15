#!/usr/bin/env bash
# 带 cd 与环境的三 crate 测试（避免调用方漏 cd 的重复踩坑）。
cd /f/project/report/rpx || exit 1
export INCLUDE='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\include;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\um;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\shared'
export LIB='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\lib\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64'
export RPX_KB_URL='postgres://system:infosys668@192.168.1.77:54323/test'
export RPX_KB_USER=system
export RPX_KB_PASS=infosys668
/c/Users/Administrator/.cargo/bin/cargo.exe fmt --all
/c/Users/Administrator/.cargo/bin/cargo.exe fmt --all --check || { echo "G1_FMT_FAIL"; exit 1; }
/c/Users/Administrator/.cargo/bin/cargo.exe test -p rpx-core -p rpx-engine -p rpx-server 2>&1 |
  grep -v "lock file" |
  grep -E "test result|FAILED|panicked|sql_fixture" | head -14
