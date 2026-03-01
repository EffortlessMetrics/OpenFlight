#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team
#
# Build script for the Flight Hub Debian package (.deb).
#
# Usage:
#   ./installer/debian/build.sh [VERSION] [OUTPUT_DIR]
#
# Arguments:
#   VERSION     Package version string (default: read from workspace Cargo.toml)
#   OUTPUT_DIR  Where to write the .deb file (default: ./installer/debian/output)
#
# Requirements:
#   - Rust toolchain (for cargo build)
#   - dpkg-deb (for package assembly)
#
# Examples:
#   ./installer/debian/build.sh
#   ./installer/debian/build.sh 1.2.3
#   ./installer/debian/build.sh 1.2.3 /tmp/packages
#   SKIP_BUILD=1 ./installer/debian/build.sh   # skip cargo build, use existing binaries

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# ── Arguments ──────────────────────────────────────────────────────────────────

VERSION="${1:-}"
OUTPUT_DIR="${2:-"$SCRIPT_DIR/output"}"
SKIP_BUILD="${SKIP_BUILD:-0}"
CONFIGURATION="${CONFIGURATION:-release}"

# ── Helper functions ───────────────────────────────────────────────────────────

info()    { echo -e "\033[36m=== $* ===\033[0m"; }
success() { echo -e "\033[32m[OK] $*\033[0m"; }
warn()    { echo -e "\033[33m[WARN] $*\033[0m"; }
error()   { echo -e "\033[31m[ERROR] $*\033[0m" >&2; exit 1; }

# ── Resolve version ────────────────────────────────────────────────────────────

if [[ -z "$VERSION" ]]; then
    CARGO_TOML="$REPO_ROOT/Cargo.toml"
    if [[ ! -f "$CARGO_TOML" ]]; then
        error "Cargo.toml not found at $CARGO_TOML"
    fi
    VERSION="$(grep -m1 '^version' "$CARGO_TOML" | sed 's/.*= *"\(.*\)"/\1/')"
    if [[ -z "$VERSION" ]]; then
        error "Could not extract version from Cargo.toml"
    fi
fi

PKG_NAME="flight-hub_${VERSION}_amd64"
BIN_DIR="$REPO_ROOT/target/$CONFIGURATION"

echo ""
echo "============================================================"
echo "   Flight Hub Debian Package Builder"
echo "============================================================"
echo "  Version:    $VERSION"
echo "  Config:     $CONFIGURATION"
echo "  Output dir: $OUTPUT_DIR"
echo "  Repo root:  $REPO_ROOT"
echo ""

# ── Step 1: Build Rust binaries ───────────────────────────────────────────────

if [[ "$SKIP_BUILD" == "1" ]]; then
    warn "Skipping Rust build (SKIP_BUILD=1)"
else
    info "Building Rust binaries ($CONFIGURATION)"
    pushd "$REPO_ROOT" >/dev/null
    CARGO_ARGS=("build" "-p" "flight-service" "-p" "flight-cli")
    if [[ "$CONFIGURATION" == "release" ]]; then
        CARGO_ARGS+=("--release")
    fi
    cargo "${CARGO_ARGS[@]}"
    success "Rust binaries built"
    popd >/dev/null
fi

# Verify binaries exist
for bin in flightd flightctl; do
    if [[ ! -f "$BIN_DIR/$bin" ]]; then
        error "Binary not found: $BIN_DIR/$bin — run without SKIP_BUILD=1 to build first"
    fi
done

# ── Step 2: Create package directory structure ────────────────────────────────

info "Creating package directory structure"

PKG_DIR="$SCRIPT_DIR/$PKG_NAME"
if [[ -d "$PKG_DIR" ]]; then
    rm -rf "$PKG_DIR"
fi

mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/flight-hub"
mkdir -p "$PKG_DIR/usr/lib/systemd/user"

success "Package directory created: $PKG_DIR"

# ── Step 3: Copy and configure DEBIAN control files ──────────────────────────

info "Copying DEBIAN control files"

cp "$SCRIPT_DIR/postinst" "$PKG_DIR/DEBIAN/"
cp "$SCRIPT_DIR/postrm"   "$PKG_DIR/DEBIAN/"
chmod 0755 "$PKG_DIR/DEBIAN/postinst" "$PKG_DIR/DEBIAN/postrm"

# Copy prerm if it exists (may live in installer/linux/debian/ or installer/debian/)
PRERM_CANDIDATES=(
    "$SCRIPT_DIR/../linux/debian/prerm"
    "$SCRIPT_DIR/prerm"
)
for candidate in "${PRERM_CANDIDATES[@]}"; do
    if [[ -f "$candidate" ]]; then
        cp "$candidate" "$PKG_DIR/DEBIAN/prerm"
        chmod 0755 "$PKG_DIR/DEBIAN/prerm"
        success "prerm script staged"
        break
    fi
done

# Substitute version placeholder
sed "s/{{VERSION}}/$VERSION/" "$SCRIPT_DIR/control" > "$PKG_DIR/DEBIAN/control"

success "Control files staged"

# ── Step 4: Stage application files ──────────────────────────────────────────

info "Staging application files"

# Binaries
install -m 0755 "$BIN_DIR/flightd"   "$PKG_DIR/usr/bin/flightd"
install -m 0755 "$BIN_DIR/flightctl" "$PKG_DIR/usr/bin/flightctl"
success "Binaries staged"

# udev rules
install -m 0644 "$SCRIPT_DIR/99-flight-hub.rules" "$PKG_DIR/usr/share/flight-hub/99-flight-hub.rules"
success "udev rules staged"

# Systemd user service
install -m 0644 "$SCRIPT_DIR/flightd.service" "$PKG_DIR/usr/lib/systemd/user/flightd.service"
success "Systemd user service staged"

# RT setup script (optional — may not exist yet)
RT_SCRIPT="$REPO_ROOT/scripts/setup-linux-rt.sh"
if [[ -f "$RT_SCRIPT" ]]; then
    install -m 0755 "$RT_SCRIPT" "$PKG_DIR/usr/share/flight-hub/setup-linux-rt.sh"
    success "RT setup script staged"
else
    warn "RT setup script not found at $RT_SCRIPT — skipping (postinst step 4 will still reference it)"
fi

# ── Step 5: Build the package ─────────────────────────────────────────────────

info "Building .deb package"

mkdir -p "$OUTPUT_DIR"
DEB_FILE="$OUTPUT_DIR/${PKG_NAME}.deb"

dpkg-deb --build --root-owner-group "$PKG_DIR" "$DEB_FILE"
success ".deb package created: $DEB_FILE"

# ── Step 6: Generate checksum ─────────────────────────────────────────────────

sha256sum "$DEB_FILE" > "${DEB_FILE}.sha256"
success "Checksum written: ${DEB_FILE}.sha256"

# ── Step 7: Clean up staging directory ───────────────────────────────────────

rm -rf "$PKG_DIR"
success "Staging directory cleaned up"

# ── Summary ───────────────────────────────────────────────────────────────────

DEB_SIZE="$(du -sh "$DEB_FILE" | cut -f1)"
echo ""
echo "============================================================"
echo "   Build Complete!"
echo "============================================================"
echo ""
echo "  Package:  $DEB_FILE"
echo "  Size:     $DEB_SIZE"
echo "  Version:  $VERSION"
echo ""
echo "  To install:"
echo "    sudo dpkg -i $DEB_FILE"
echo ""
echo "  To install and satisfy dependencies:"
echo "    sudo apt install $DEB_FILE"
echo ""
