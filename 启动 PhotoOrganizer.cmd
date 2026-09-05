@echo off
setlocal EnableExtensions

cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\manual-build-start.ps1" %*
set "PHOTO_ORGANIZER_EXIT_CODE=%ERRORLEVEL%"
if not "%PHOTO_ORGANIZER_EXIT_CODE%"=="0" (
  echo.
  echo PhotoOrganizer manual build/start failed. Exit code: %PHOTO_ORGANIZER_EXIT_CODE%
  pause
)
exit /b %PHOTO_ORGANIZER_EXIT_CODE%
