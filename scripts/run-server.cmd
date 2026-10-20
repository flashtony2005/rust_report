@echo off
REM 报表引擎后端服务（rpx-server）：提供 /reports 报表库、/render 渲染、/auth/status。
REM 监听 8090；前端 Vite（5288）通过 /api 代理转发到这里。
REM 关闭本窗口即停止服务。
cd /d "%~dp0.."
if not exist "target\debug\rpx-server.exe" (
  echo [ERROR] 未找到 target\debug\rpx-server.exe
  echo         请先在本目录执行： cargo build -p rpx-server
  pause
  exit /b 1
)
if not exist "examples\library\.library.json" (
  echo [ERROR] 未找到报表库 examples\library\.library.json
  pause
  exit /b 1
)
echo rpx-server  http://127.0.0.1:8090   报表库: %CD%\examples\library
target\debug\rpx-server.exe serve --library "%CD%\examples\library" --addr 127.0.0.1:8090
pause
