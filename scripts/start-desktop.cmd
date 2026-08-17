@echo off
setlocal

rem Keep the desktop development entry point usable from a fresh terminal.
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
  set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

where cargo.exe >nul 2>nul
if errorlevel 1 (
  echo Rust cargo was not found. Install the MSVC Rust toolchain or add %%USERPROFILE%%\.cargo\bin to PATH.
  exit /b 1
)

rem Use an isolated persistent data directory for manual development testing.
rem Set PHOTO_ORGANIZER_DATA_DIR before calling this script to override it.
if "%PHOTO_ORGANIZER_DATA_DIR%"=="" set "PHOTO_ORGANIZER_DATA_DIR=%TEMP%\PhotoOrganizer-dev-data"
echo PhotoOrganizer dev data: %PHOTO_ORGANIZER_DATA_DIR%

rem The manual window is created by the Rust setup hook so this per-session
rem directory is passed directly to WebView2 instead of being ignored by a
rem static Tauri dataDirectory.
if "%PHOTO_ORGANIZER_WEBVIEW_DATA_DIR%"=="" set "PHOTO_ORGANIZER_WEBVIEW_DATA_DIR=%TEMP%\PhotoOrganizer-webview2-%RANDOM%"
echo PhotoOrganizer WebView2 profile: %PHOTO_ORGANIZER_WEBVIEW_DATA_DIR%
echo Close this terminal to stop the desktop development session.

rem Some Windows WebView2 development environments cannot create the sandbox
rem token or GPU process and leave the Tauri window black or invisible. Keep
rem this manual-only test entry reliable with software/in-process GPU rendering
rem and no sandbox. This is not used by the packaged application; callers can
rem override the variable when they need to test the default WebView2 path.
if "%WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS%"=="" set "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu --disable-gpu-compositing --in-process-gpu --no-sandbox"
echo PhotoOrganizer WebView2 arguments: %WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS%

rem Manual acceptance mode builds the frontend and serves it from the Tauri asset
rem protocol, so startup does not depend on a local Vite TCP port.
call npm.cmd run tauri -- dev --config src-tauri\tauri.manual.conf.json --no-dev-server
exit /b %ERRORLEVEL%
