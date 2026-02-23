#!/usr/bin/env bash
# GDML Studio — build backend, run tests, start backend + frontend, open browser
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

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
