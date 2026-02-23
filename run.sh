#!/usr/bin/env bash
# GDML Studio — build backend, run tests, start backend + frontend, open browser
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# --- Check prerequisites ---
MISSING=""

if ! command -v cargo > /dev/null 2>&1; then
    MISSING="rust"
fi

if ! command -v node > /dev/null 2>&1 || ! command -v npm > /dev/null 2>&1; then
    if [ -z "$MISSING" ]; then
        MISSING="node"
    else
        MISSING="both"
    fi
fi

if [ -n "$MISSING" ]; then
    echo "ERROR: Missing required tools."
    echo ""
    if [ "$MISSING" = "rust" ] || [ "$MISSING" = "both" ]; then
        echo "  Rust (cargo) is not installed."
        echo "    Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        echo "    Docs:    https://rustup.rs/"
        echo ""
    fi
    if [ "$MISSING" = "node" ] || [ "$MISSING" = "both" ]; then
        echo "  Node.js / npm is not installed."
        echo "    Install: https://nodejs.org/ (download LTS)"
        echo "    Or use nvm: curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash"
        echo ""
    fi
    exit 1
fi

echo "Prerequisites OK: cargo $(cargo --version | cut -d' ' -f2), node $(node --version), npm $(npm --version)"
echo ""

echo "=== Building backend (release) ==="
cd backend
cargo build --release

echo ""
echo "=== Running backend tests ==="
cargo test

echo ""
echo "=== Type-checking frontend ==="
cd "$SCRIPT_DIR/frontend"
npm install --silent
npx tsc --noEmit

echo ""
echo "=== Starting backend ==="
cd "$SCRIPT_DIR/backend"
cargo run --release &
BACKEND_PID=$!

# Wait for backend to be ready
echo "Waiting for backend on port 4001..."
for i in $(seq 1 30); do
    if curl -s http://127.0.0.1:4001/api/health > /dev/null 2>&1 || nc -z 127.0.0.1 4001 2>/dev/null; then
        echo "Backend is ready."
        break
    fi
    sleep 1
done

echo ""
echo "=== Starting frontend ==="
cd "$SCRIPT_DIR/frontend"
npm run dev &
FRONTEND_PID=$!

# Wait for Vite to be ready, then open browser
sleep 3
if command -v xdg-open > /dev/null; then
    xdg-open http://localhost:5173
elif command -v open > /dev/null; then
    open http://localhost:5173
fi

echo ""
echo "=== GDML Studio is running ==="
echo "  Backend:  http://127.0.0.1:4001"
echo "  Frontend: http://localhost:5173"
echo ""
echo "Press Ctrl+C to stop both servers."

# Trap Ctrl+C to kill both processes
cleanup() {
    echo ""
    echo "Shutting down..."
    kill $FRONTEND_PID 2>/dev/null
    kill $BACKEND_PID 2>/dev/null
    wait $FRONTEND_PID 2>/dev/null
    wait $BACKEND_PID 2>/dev/null
    echo "Done."
}
trap cleanup INT TERM

wait
