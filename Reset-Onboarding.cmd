@echo off
setlocal
echo DigView - reset setup status only
echo Quit DigView from the tray before continuing.
echo Your library, folders and preferences will be kept.
echo.
where python >nul 2>nul
if errorlevel 1 (
  where py >nul 2>nul
  if errorlevel 1 (
    echo Python 3 is required. This helper is for the local development workspace.
    pause
    exit /b 1
  )
  py -3 "%~dp0scripts\reset-onboarding.py"
) else (
  python "%~dp0scripts\reset-onboarding.py"
)
if errorlevel 1 (
  echo.
  echo Reset did not complete. Read the message above.
  pause
  exit /b 1
)
echo.
pause
