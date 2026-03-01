#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# test_service_status.sh — Verify the Flight Hub systemd user service can
# be enabled, started, and stopped.
#
# This test must be run as a regular user (not root) because flightd is a
# systemd **user** service.
#
# Usage:
#   ./test_service_status.sh

set -euo pipefail

SERVICE="flightd.service"
PASS=0
FAIL=0

assert_ok() {
    local desc="$1"; shift
    if "$@" >/dev/null 2>&1; then
        echo "  [OK]   $desc"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] $desc"
        FAIL=$((FAIL + 1))
    fi
}

assert_status() {
    local desc="$1"
    local expected="$2"
    local actual
    actual=$(systemctl --user is-active "$SERVICE" 2>/dev/null || true)
    if [ "$actual" = "$expected" ]; then
        echo "  [OK]   $desc (status=$actual)"
        PASS=$((PASS + 1))
    else
        echo "  [FAIL] $desc (expected=$expected, got=$actual)"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== Flight Hub Service Status Test ==="
echo ""

# Ensure the unit file is visible to systemd
echo "-- Reload daemon --"
assert_ok "systemctl --user daemon-reload" systemctl --user daemon-reload

echo ""
echo "-- Enable service --"
assert_ok "enable $SERVICE" systemctl --user enable "$SERVICE"

echo ""
echo "-- Start service --"
assert_ok "start $SERVICE" systemctl --user start "$SERVICE"
sleep 1
assert_status "service is active after start" "active"

echo ""
echo "-- Stop service --"
assert_ok "stop $SERVICE" systemctl --user stop "$SERVICE"
sleep 1
assert_status "service is inactive after stop" "inactive"

echo ""
echo "-- Disable service --"
assert_ok "disable $SERVICE" systemctl --user disable "$SERVICE"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
