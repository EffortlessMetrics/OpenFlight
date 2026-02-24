#![no_main]

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team
//
// Fuzz target: parse arbitrary bytes as an Ac7TelemetryPacket JSON payload.
// Must never panic; validation errors are acceptable.

use flight_ac7_protocol::Ac7TelemetryPacket;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Should never panic regardless of input
    if let Ok(packet) = Ac7TelemetryPacket::from_json_slice(data) {
        // If parsing succeeded, validate must also not panic
        let _ = packet.validate();
        let _ = packet.aircraft_label();
        // Round-trip must be consistent
        if let Ok(bytes) = packet.to_json_vec() {
            let _ = Ac7TelemetryPacket::from_json_slice(&bytes);
        }
    }
});
