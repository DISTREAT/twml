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

# Convert each twml document into pdf
for document in *.twml; do
    printf "[info] Building documentation for '%s'\n" "$document"
    ../target/debug/twml-pdf "$document" "dist/$(basename "$document" .twml).pdf"
done

echo "[status] Built documentation to ./dist/"
