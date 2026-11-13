#!/usr/bin/env bash
# Build gdml-cad-worker for Linux in an Ubuntu 20.04 container, so that it runs on
# glibc 2.31 and newer, and stage it with its OCCT libraries and licenses in
# dist/cad/ (packaged into the desktop app as resources/cad/).
# OCCT is downloaded, verified and built once into third_party/ (about 30 min).
set -euo pipefail
cd "$(dirname "$0")"

if [ "${1:-}" != --in-container ]; then
    # OCCT 8.0.1 needs GCC 10 (GCC 9 rejects its defaulted noexcept constructors).
    # Ubuntu 20.04's GCC 10 runs on the system libstdc++6, so nothing more ships.
    docker build -q -t gdml-cad-build:focal - <<'DOCKERFILE' > /dev/null
FROM ubuntu:20.04
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        gcc-10 g++-10 cmake ninja-build curl ca-certificates && rm -rf /var/lib/apt/lists/*
ENV CC=gcc-10 CXX=g++-10
DOCKERFILE
    exec docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/w" -w /w gdml-cad-build:focal ./build-linux.sh --in-container
fi

. ./occt-source.txt
src=third_party/OCCT-$version
prefix=third_party/occt-$version-linux
if [ ! -f "$prefix/.installed" ]; then
    archive=third_party/OCCT-$version.tar.gz
    [ -f "$archive" ] || curl -fsSL -o "$archive" "$url"
    echo "$sha256  $archive" | sha256sum -c --quiet
    [ -d "$src" ] || tar -xzf "$archive" -C third_party
    cmake -G Ninja -C occt-options.cmake -S "$src" -B third_party/occt-build -DINSTALL_DIR="$PWD/$prefix"
    cmake --build third_party/occt-build
    cmake --install third_party/occt-build
    touch "$prefix/.installed"
fi

cmake -G Ninja -S . -B build-linux -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$PWD/$prefix"
cmake --build build-linux
(cd build-linux && ctest --output-on-failure)

# dist/cad: the worker (RUNPATH $ORIGIN/lib), the OCCT libraries it loads, licenses.
rm -rf dist/cad
cmake --install build-linux --prefix "$PWD/dist/cad" > /dev/null
mkdir -p dist/cad/lib dist/cad/licenses
ldd build-linux/gdml-cad-worker | awk -v p="$PWD/$prefix/" 'index($3, p) == 1 { print $3 }' |
    while read -r lib; do cp -L "$lib" dist/cad/lib/; done
strip --strip-unneeded dist/cad/gdml-cad-worker dist/cad/lib/*
cp "$src/LICENSE_LGPL_21.txt" "$src/OCCT_LGPL_EXCEPTION.txt" dist/cad/licenses/
cat > dist/cad/licenses/README.txt <<NOTICE
gdml-cad-worker uses Open CASCADE Technology (OCCT) $version, unmodified, as
the shared libraries in lib/, under the GNU LGPL 2.1 (LICENSE_LGPL_21.txt)
with the Open CASCADE exception (OCCT_LGPL_EXCEPTION.txt).
Source: $url
SHA-256: $sha256
Built with cad-worker/occt-options.cmake from https://github.com/davsar89/GDML_Studio;
any compatible build of these libraries can replace them.

Eight TKGeomBase source files of this release carry a commercial notice that
contradicts the LGPL declaration of the rest (OCCT issue 1564, open:
https://github.com/Open-Cascade-SAS/OCCT/issues/1564).
NOTICE
echo "Staged $(du -sh dist/cad | cut -f1) in cad-worker/dist/cad"
