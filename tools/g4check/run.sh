#!/usr/bin/env bash
# Build g4check once, then check each GDML file given. Needs a Geant4 with GDML
# on the environment (source its geant4.sh first). G4CHECK_EVENTS sets the
# number of geantinos (default 2000). Exit status 1 if any file fails.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
if [ ! -x "$here/build/g4check" ]; then
    cmake -S "$here" -B "$here/build" -DCMAKE_BUILD_TYPE=Release > /dev/null
    cmake --build "$here/build" -j > /dev/null
fi
status=0
for f in "$@"; do
    echo "== $f"
    "$here/build/g4check" "$f" "${G4CHECK_EVENTS:-2000}" || status=1
done
exit $status
