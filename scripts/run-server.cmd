@echo off
REM 报表引擎后端服务（rpx-server）：提供 /reports 报表库、/render 渲染、/auth/status。
REM 监听 8090；前端 Vite（5288）通过 /api 代理转发到这里。
REM 关闭本窗口即停止服务。
cd /d "%~dp0.."
REM 优先 debug 构建；没有就用 release（例如清过 target\debug 之后）。
set "SERVER=target\debug\rpx-server.exe"
if not exist "%SERVER%" set "SERVER=target\release\rpx-server.exe"
if not exist "%SERVER%" (
  echo [ERROR] 未找到 rpx-server.exe（target\debug 与 target\release 都没有）
  echo         请先在本目录执行： cargo build -p rpx-server   或   cargo build --release -p rpx-server
  pause
  exit /b 1
)
if not exist "examples\library\.library.json" (
  echo [ERROR] 未找到报表库 examples\library\.library.json
  pause
  exit /b 1
)
echo rpx-server  http://127.0.0.1:8090   报表库: %CD%\examples\library
"%SERVER%" serve --library "%CD%\examples\library" --addr 127.0.0.1:8090
pause
