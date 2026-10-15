#!/usr/bin/env bash
cd /f/project/report/rpx || exit 1
export INCLUDE='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\include;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\ucrt;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\um;C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0\shared'
export LIB='C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.43.34808\lib\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x64;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64'
exec /c/Users/Administrator/.cargo/bin/cargo.exe clippy --workspace --all-targets 2>&1 |
  grep -v "lock file" |
  grep -E "^warning: [a-z]|^error|-->" | head -14
