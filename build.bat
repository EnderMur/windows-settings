@echo off
setlocal
cd /d "%~dp0"

echo === Building windows-settings (release) ===
cargo build --release
if errorlevel 1 (
    echo.
    echo Build FAILED.
    pause
    exit /b 1
)

echo.
echo === Build OK ===
echo Executable: %~dp0target\release\windows-settings.exe
pause
endlocal
