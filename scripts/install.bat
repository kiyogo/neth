@echo off
setlocal
echo ==========================================================
echo          Neth Server Windows Launcher / Installer
echo             KIYOVN - https://kiyomc.com
echo ==========================================================
echo.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
if %ERRORLEVEL% neq 0 (
    echo.
    echo Installation encountered an error.
    pause
    exit /b %ERRORLEVEL%
)
endlocal
