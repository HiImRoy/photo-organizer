@echo off
setlocal EnableExtensions

pushd "%~dp0" >nul
if errorlevel 1 (
  echo Could not enter the PhotoOrganizer project directory: "%~dp0"
  exit /b 1
)

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\manual-build-start.ps1" %*
set "PHOTO_ORGANIZER_EXIT_CODE=%ERRORLEVEL%"
popd
if not "%PHOTO_ORGANIZER_EXIT_CODE%"=="0" (
  echo.
  echo PhotoOrganizer manual build/start failed. Exit code: %PHOTO_ORGANIZER_EXIT_CODE%
  if /i not "%~1"=="-CheckOnly" pause
)
exit /b %PHOTO_ORGANIZER_EXIT_CODE%
