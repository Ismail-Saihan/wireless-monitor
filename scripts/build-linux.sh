#!/usr/bin/env bash
#
# Automated Linux package build script for Wireless Virtual Monitor.
# Builds standard Debian (.deb) and AppImage turnkey distribution packages.
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

echo "=========================================================="
echo " Building Wireless Virtual Monitor Linux Packages (.deb/AppImage) "
echo "=========================================================="

# 1. Environment check
echo "[1/5] Checking build environment & prerequisites..."
command -v cargo >/dev/null 2>&1 || {
    echo "Error: Rust toolchain ('cargo') is not installed. Visit https://rustup.rs/" >&2
    exit 1
}

command -v node >/dev/null 2>&1 || {
    echo "Error: Node.js is not installed. Please install Node.js v18+." >&2
    exit 1
}

command -v pnpm >/dev/null 2>&1 || {
    echo "Warning: pnpm not found in PATH, attempting npx pnpm..." >&2
}

echo "  -> Cargo: $(cargo --version)"
echo "  -> Rustc: $(rustc --version)"
echo "  -> Node:  $(node --version)"

# 2. Check icon suite
echo ""
echo "[2/5] Validating application icon suite..."
if [ ! -f "src-tauri/icons/icon.png" ]; then
    echo "  -> Synthesizing application icons..."
    python3 scripts/generate_icons.py
fi
echo "  -> Application icons ready."

# 3. Build frontend
echo ""
echo "[3/5] Building frontend production bundle (Vite + Tailwind)..."
pnpm run build
echo "  -> Frontend build successful (dist/ ready)."

# 4. Target flags
TARGET_FLAG="${1:-all}"
BUILD_ARGS=("tauri" "build")

if [ "${TARGET_FLAG}" = "deb" ]; then
    BUILD_ARGS+=("--bundles" "deb")
elif [ "${TARGET_FLAG}" = "appimage" ]; then
    BUILD_ARGS+=("--bundles" "appimage")
else
    BUILD_ARGS+=("--bundles" "deb,appimage")
fi

# 5. Build Tauri packages
echo ""
echo "[5/5] Compiling Rust backend and generating Linux distribution packages..."
pnpm "${BUILD_ARGS[@]}"

echo ""
echo "=========================================================="
echo " Linux Packages Built Successfully!                       "
echo "=========================================================="
echo "Generated package artifacts:"
find src-tauri/target/release/bundle -type f \( -name "*.deb" -o -name "*.AppImage" \) -exec ls -lh {} +
