// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for kRPC protobuf message deserialization.
//!
//! Exercises the prost-generated decoders against arbitrary byte sequences to
//! ensure no panics or undefined behaviour.
//!
//! Run with: `cargo +nightly fuzz run fuzz_ksp_response`

#![no_main]

use libfuzzer_sys::fuzz_target;
use flight_ksp::protocol::{ConnectionResponse, Response, decode_double, decode_float, decode_bool};
use prost::Message;

fuzz_target!(|data: &[u8]| {
    // Protobuf decoders must never panic on arbitrary input
    let _ = ConnectionResponse::decode(data);
    let _ = Response::decode(data);

    // Scalar decoders must also be panic-free
    let _ = decode_double(data);
    let _ = decode_float(data);
    let _ = decode_bool(data);
});
