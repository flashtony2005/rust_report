#!/usr/bin/env bash
# rpx-server clippy（强制重编译，看全部 warning；避免调用方漏 cd）。
cd /f/project/report/rpx || exit 1
export INCLUDE='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\include;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\um;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\shared'
export LIB='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\lib\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64'
touch crates/rpx-server/src/*.rs crates/rpx-server/tests/*.rs
/c/Users/Administrator/.cargo/bin/cargo.exe clippy -p rpx-server --all-targets 2>&1 |
  grep -v "lock file" |
  grep -B1 -A6 "^warning: [a-z]" | head -24
