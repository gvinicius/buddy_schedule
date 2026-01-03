#!/bin/bash
set -e

echo "Building WASM frontend..."

# Install wasm-pack if not available
if ! command -v wasm-pack &> /dev/null; then
    echo "Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Build WASM
wasm-pack build --target web --out-dir ../web/pkg

echo "WASM build complete! Files are in web/pkg/"
