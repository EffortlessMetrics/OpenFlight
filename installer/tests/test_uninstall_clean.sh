#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# test_uninstall_clean.sh — Verify that uninstalling Flight Hub removes
# program files but preserves user configuration.
#
# Usage:
#   sudo ./test_uninstall_clean.sh
#
# Prerequisites:
#   - Flight Hub must have been installed and then removed (dpkg -r flight-hub)
#   - User config should have been created in ~/.config/flight-hub/

set -euo pipefail

PASS=0
FAIL=0

assert_absent() {
    local path="$1"
    if [ ! -e "$path" ]; then
        echo "  [OK]   removed: $path"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] still present: $path"
        FAIL=$((FAIL + 1))
    fi
}

assert_present() {
    local path="$1"
    if [ -e "$path" ]; then
        echo "  [OK]   preserved: $path"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] missing (should be preserved): $path"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== Flight Hub Uninstall Cleanliness Test ==="
echo ""

echo "-- Program files should be removed --"
assert_absent /usr/bin/flightd
assert_absent /usr/bin/flightctl
assert_absent /usr/lib/systemd/user/flightd.service
assert_absent /usr/share/flight-hub/99-flight-hub.rules

echo ""
echo "-- udev rules should be removed from /etc --"
assert_absent /etc/udev/rules.d/99-flight-hub.rules

echo ""
echo "-- User configuration should be preserved --"
USER_HOME="${HOME:-$(eval echo ~"$(whoami)")}"
if [ -d "$USER_HOME/.config/flight-hub" ]; then
    assert_present "$USER_HOME/.config/flight-hub"
else
    echo "  [SKIP] $USER_HOME/.config/flight-hub not found (may not have been created)"
fi

if [ -d "$USER_HOME/.local/share/flight-hub" ]; then
    assert_present "$USER_HOME/.local/share/flight-hub"
else
    echo "  [SKIP] $USER_HOME/.local/share/flight-hub not found (may not have been created)"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
