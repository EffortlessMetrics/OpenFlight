// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for IPC message JSON deserialization.
//!
//! Exercises `IpcMessage::from_json` against arbitrary strings to ensure
//! no panics on malformed JSON input from IPC clients.
//!
//! Run with: `cargo +nightly fuzz run fuzz_ipc_message`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    // Must never panic — errors are expected and ignored.
    let _ = flight_ipc::messages::IpcMessage::from_json(input);
});
