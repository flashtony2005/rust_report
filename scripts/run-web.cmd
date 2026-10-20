@echo off
REM 报表设计器前端（Vite dev server）：监听 5288，/api 代理到 127.0.0.1:8090。
REM 需要后端（run-server.cmd）也在跑，否则 /api 会 502。
REM 关闭本窗口即停止服务。
cd /d "%~dp0..\web"
if not exist "node_modules\vite\bin\vite.js" (
  echo [ERROR] 未找到 node_modules\vite\bin\vite.js
  echo         请先在本目录执行： npm install
  pause
  exit /b 1
)
set "NODE=C:\Users\Administrator\.workbuddy\binaries\node\versions\22.22.2-6\node.exe"
if not exist "%NODE%" set "NODE=node"
echo 设计器   http://127.0.0.1:5288
echo 画廊     http://127.0.0.1:5288/gallery.html
echo 后端     http://127.0.0.1:8090
"%NODE%" node_modules\vite\bin\vite.js --port 5288 --strictPort --host 127.0.0.1
pause
