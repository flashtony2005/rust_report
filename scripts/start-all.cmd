@echo off
REM 报表引擎一键启动：开 2 个控制台窗口（后端 8090 / 前端 5288），关窗口即停服务。
start "rpx-server 8090" cmd /k "%~dp0run-server.cmd"
timeout /t 2 /nobreak >nul
start "rpx-web 5288" cmd /k "%~dp0run-web.cmd"
echo.
echo   设计器   http://127.0.0.1:5288
echo   画廊     http://127.0.0.1:5288/gallery.html
echo   后端     http://127.0.0.1:8090
echo.
echo 关闭两个控制台窗口即可停止全部服务。
pause
