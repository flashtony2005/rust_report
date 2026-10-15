#!/usr/bin/env bash
# 带 cd 与环境的三 crate 测试（避免调用方漏 cd 的重复踩坑）。
# 全量日志落盘 target/kb_tests.log——过滤显示可能吞掉编译错误（两次实锤），
# 失败时必须能回看全文。
cd /f/project/report/rpx || exit 1
export INCLUDE='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\include;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\um;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\shared'
export LIB='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\lib\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64'
export RPX_KB_URL='postgres://system:infosys668@192.168.1.77:54323/test'
export RPX_KB_USER=system
export RPX_KB_PASS=infosys668

/c/Users/Administrator/.cargo/bin/cargo.exe fmt --all
/c/Users/Administrator/.cargo/bin/cargo.exe fmt --all --check || { echo "G1_FMT_FAIL"; exit 1; }

/c/Users/Administrator/.cargo/bin/cargo.exe test -p rpx-core -p rpx-engine -p rpx-server \
  > target/kb_tests.log 2>&1
RC=$?
grep -v "lock file" target/kb_tests.log |
  grep -E "test result|FAILED|panicked|sql_fixture|running" | head -24
if [ $RC -ne 0 ]; then
  echo "=== TEST FAILED (RC=$RC)——编译错误与失败详情："
  grep -v "lock file" target/kb_tests.log | grep -E "^error|^warning: unused|--> " | head -30
  exit 1
fi
echo "ALL_GREEN"
