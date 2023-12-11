#!/bin/sh
# Build the twml documentation to ./dist/
set -e

# Script must be executed in docs
SCRIPT_PATH=$(realpath "$0")
SCRIPT_DIR=$(dirname "$SCRIPT_PATH")

if [ "$PWD" != "$SCRIPT_DIR" ]; then
    echo "[fatal] Script must be executed in ./docs/"
    exit 1
fi

# Recompile to use fresh version
echo "[info] Compiling debug build of twml"
(
    cd ..
    cargo build
)

# Convert twml document into pdf
echo "[info] Building documentation..."
../target/debug/twml-pdf docs.twml dist/docs.pdf

echo "[status] Built documentation to ./dist/"
