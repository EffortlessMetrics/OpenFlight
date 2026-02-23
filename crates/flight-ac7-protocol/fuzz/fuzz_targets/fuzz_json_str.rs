#![no_main]

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team
//
// Fuzz target: parse arbitrary UTF-8 strings as AC7 telemetry JSON.

use flight_ac7_protocol::Ac7TelemetryPacket;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = Ac7TelemetryPacket::from_json_str(data);
});
