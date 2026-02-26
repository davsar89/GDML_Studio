@echo off
REM GDML Studio — build backend, run tests, start backend + frontend, open browser
setlocal

set SCRIPT_DIR=%~dp0
cd /d "%SCRIPT_DIR%"

REM --- Check prerequisites ---
set "MISSING_RUST=0"
set "MISSING_NODE=0"

where cargo >nul 2>&1
if errorlevel 1 set "MISSING_RUST=1"

where node >nul 2>&1
if errorlevel 1 set "MISSING_NODE=1"

where npm >nul 2>&1
if errorlevel 1 set "MISSING_NODE=1"

if "%MISSING_RUST%"=="1" if "%MISSING_NODE%"=="1" goto :missing_both
if "%MISSING_RUST%"=="1" goto :missing_rust
if "%MISSING_NODE%"=="1" goto :missing_node
goto :prereqs_ok

:missing_both
echo ERROR: Missing required tools.
echo.
echo   Rust (cargo) is not installed.
echo     Install: https://rustup.rs/ (download and run rustup-init.exe)
echo.
echo   Node.js / npm is not installed.
echo     Install: https://nodejs.org/ (download LTS installer)
echo.
pause
exit /b 1

:missing_rust
echo ERROR: Missing required tools.
echo.
echo   Rust (cargo) is not installed.
echo     Install: https://rustup.rs/ (download and run rustup-init.exe)
echo.
pause
exit /b 1

:missing_node
echo ERROR: Missing required tools.
echo.
echo   Node.js / npm is not installed.
echo     Install: https://nodejs.org/ (download LTS installer)
echo.
pause
exit /b 1

:prereqs_ok
echo Prerequisites OK.
echo.

echo === Building backend (release) ===
cd backend
cargo build --release
if errorlevel 1 (
    echo Backend build failed.
    pause
    exit /b 1
)

echo.
echo === Running backend tests ===
taskkill /F /IM gdml-studio-backend.exe /T >nul 2>&1
cargo test
if errorlevel 1 (
    echo Backend tests failed.
    pause
    exit /b 1
)

echo.
echo === Type-checking frontend ===
cd /d "%SCRIPT_DIR%\frontend"
call npm install --silent
call npx tsc --noEmit
if errorlevel 1 (
    echo Frontend type-check failed.
    pause
    exit /b 1
)

echo.
echo === Starting backend ===
cd /d "%SCRIPT_DIR%\backend"
start "GDML-Backend" cmd /c "cargo run --release"

REM Wait for backend to be ready
echo Waiting for backend on port 4001...
set /a attempts=0
:wait_backend
set /a attempts+=1
if %attempts% gtr 30 (
    echo Warning: Backend may not be ready yet.
    goto start_frontend
)
powershell -Command "try { $null = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 4001); exit 0 } catch { exit 1 }" >nul 2>&1
if errorlevel 1 (
    timeout /t 1 /nobreak >nul
    goto wait_backend
)
echo Backend is ready.

:start_frontend
echo.
echo === Starting frontend ===
cd /d "%SCRIPT_DIR%\frontend"
start "GDML-Frontend" cmd /c "npm run dev"

REM Wait for Vite, then open browser
timeout /t 3 /nobreak >nul
start http://localhost:5173

echo.
echo === GDML Studio is running ===
echo   Backend:  http://127.0.0.1:4001
echo   Frontend: http://localhost:5173
echo.
echo Close this window or press Ctrl+C to stop.
pause
