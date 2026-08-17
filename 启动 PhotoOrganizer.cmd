@echo off
setlocal

cd /d "%~dp0"
call "%~dp0scripts\start-desktop.cmd"
set "PHOTO_ORGANIZER_EXIT_CODE=%ERRORLEVEL%"
if not "%PHOTO_ORGANIZER_EXIT_CODE%"=="0" (
  echo.
  echo PhotoOrganizer start failed. Exit code: %PHOTO_ORGANIZER_EXIT_CODE%
  pause
)
exit /b %PHOTO_ORGANIZER_EXIT_CODE%
