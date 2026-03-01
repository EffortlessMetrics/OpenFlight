#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# test_install_paths.sh — Verify expected files exist after a Flight Hub
# Linux installation.
#
# Usage:
#   sudo ./test_install_paths.sh            # check real system paths
#   ./test_install_paths.sh /tmp/fakeroot   # check a staging prefix

set -euo pipefail

PREFIX="${1:-}"
PASS=0
FAIL=0

check_file() {
    local path="$PREFIX$1"
    if [ -f "$path" ]; then
        echo "  [OK]   $path"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] $path — not found"
        FAIL=$((FAIL + 1))
    fi
}

check_dir() {
    local path="$PREFIX$1"
    if [ -d "$path" ]; then
        echo "  [OK]   $path/"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] $path/ — not found"
        FAIL=$((FAIL + 1))
    fi
}

check_executable() {
    local path="$PREFIX$1"
    if [ -x "$path" ]; then
        echo "  [OK]   $path (executable)"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] $path — not executable or not found"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== Flight Hub Install Path Verification ==="
echo "Prefix: ${PREFIX:-(system root)}"
echo ""

echo "-- Binaries --"
check_executable /usr/bin/flightd
check_executable /usr/bin/flightctl

echo ""
echo "-- Systemd Unit --"
check_file /usr/lib/systemd/user/flightd.service

echo ""
echo "-- Shared Data --"
check_dir  /usr/share/flight-hub
check_file /usr/share/flight-hub/99-flight-hub.rules

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
