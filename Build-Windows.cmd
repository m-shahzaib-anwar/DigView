@echo off
setlocal DisableDelayedExpansion
cd /d "%~dp0"
title DigView - Windows build and installer
where.exe node >nul 2>&1
if errorlevel 1 (
  echo Node.js was not found. Install Node.js 22.13+ or 24+ and reopen this launcher.
  echo Read BUILD-WINDOWS.md for setup and manual commands.
  pause
  exit /b 1
)
node "%~dp0scripts\build-windows.mjs"
set "DIGVIEW_BUILD_EXIT=%ERRORLEVEL%"
echo.
if not "%DIGVIEW_BUILD_EXIT%"=="0" echo BUILD FAILED. Read the error and log path above. No installer was launched.
pause
exit /b %DIGVIEW_BUILD_EXIT%
