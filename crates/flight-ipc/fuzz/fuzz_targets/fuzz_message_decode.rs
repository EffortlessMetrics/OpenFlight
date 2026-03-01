// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for IPC message decoding from raw bytes.
//!
//! Treats the first byte as a message-type selector and attempts to decode the
//! remaining bytes as both a protobuf message (via prost) and a JSON IPC
//! message, ensuring no panics on arbitrary input.
//!
//! Run with: `cargo +nightly fuzz run fuzz_message_decode`

#![no_main]

use libfuzzer_sys::fuzz_target;
use prost::Message;

fuzz_target!(|data: &[u8]| {
    // Attempt JSON-based IPC message decode.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = flight_ipc::messages::IpcMessage::from_json(s);
    }

    // Use the first byte to select a protobuf message type, exercising
    // decode paths that the packet-level fuzzer may not reach via simple
    // whole-buffer decoding.
    if data.len() < 2 {
        return;
    }
    let selector = data[0];
    let payload = &data[1..];
    match selector % 6 {
        0 => {
            let _ = flight_ipc::proto::NegotiateFeaturesRequest::decode(payload);
            let _ = flight_ipc::proto::NegotiateFeaturesResponse::decode(payload);
        }
        1 => {
            let _ = flight_ipc::proto::ListDevicesRequest::decode(payload);
            let _ = flight_ipc::proto::ListDevicesResponse::decode(payload);
        }
        2 => {
            let _ = flight_ipc::proto::ApplyProfileRequest::decode(payload);
            let _ = flight_ipc::proto::ApplyProfileResponse::decode(payload);
        }
        3 => {
            let _ = flight_ipc::proto::GetServiceInfoRequest::decode(payload);
            let _ = flight_ipc::proto::GetServiceInfoResponse::decode(payload);
        }
        4 => {
            let _ = flight_ipc::proto::HealthSubscribeRequest::decode(payload);
            let _ = flight_ipc::proto::GetSupportBundleResponse::decode(payload);
        }
        _ => {
            let _ = flight_ipc::proto::ConfigureTelemetryRequest::decode(payload);
            let _ = flight_ipc::proto::ConfigureTelemetryResponse::decode(payload);
        }
    }
});
