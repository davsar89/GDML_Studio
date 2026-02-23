@echo off
REM GDML Studio — build backend, run tests, start backend + frontend, open browser
setlocal

set SCRIPT_DIR=%~dp0
cd /d "%SCRIPT_DIR%"

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
