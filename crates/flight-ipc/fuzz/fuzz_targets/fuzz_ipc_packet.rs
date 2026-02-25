// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Fuzz target for IPC protobuf message decoding.
//!
//! Exercises all major request/response message types to ensure that
//! no malformed protobuf payload can cause a panic or memory unsafety.
//!
//! Run with: `cargo +nightly fuzz run fuzz_ipc_packet`

#![no_main]

use libfuzzer_sys::fuzz_target;
use prost::Message;
use flight_ipc::proto::{
    NegotiateFeaturesRequest, NegotiateFeaturesResponse,
    ListDevicesRequest, ListDevicesResponse,
    ApplyProfileRequest, ApplyProfileResponse,
    SetCapabilityModeRequest, SetCapabilityModeResponse,
    GetCapabilityModeRequest, GetCapabilityModeResponse,
    GetServiceInfoRequest, GetServiceInfoResponse,
    HealthSubscribeRequest,
    GetSupportBundleRequest, GetSupportBundleResponse,
    GetSecurityStatusRequest, GetSecurityStatusResponse,
    DetectCurveConflictsRequest, DetectCurveConflictsResponse,
    ResolveCurveConflictRequest, ResolveCurveConflictResponse,
    ConfigureTelemetryRequest, ConfigureTelemetryResponse,
};

fuzz_target!(|data: &[u8]| {
    // All decode() calls must never panic — Err is fine.
    let _ = NegotiateFeaturesRequest::decode(data);
    let _ = NegotiateFeaturesResponse::decode(data);
    let _ = ListDevicesRequest::decode(data);
    let _ = ListDevicesResponse::decode(data);
    let _ = ApplyProfileRequest::decode(data);
    let _ = ApplyProfileResponse::decode(data);
    let _ = SetCapabilityModeRequest::decode(data);
    let _ = SetCapabilityModeResponse::decode(data);
    let _ = GetCapabilityModeRequest::decode(data);
    let _ = GetCapabilityModeResponse::decode(data);
    let _ = GetServiceInfoRequest::decode(data);
    let _ = GetServiceInfoResponse::decode(data);
    let _ = HealthSubscribeRequest::decode(data);
    let _ = GetSupportBundleRequest::decode(data);
    let _ = GetSupportBundleResponse::decode(data);
    let _ = GetSecurityStatusRequest::decode(data);
    let _ = GetSecurityStatusResponse::decode(data);
    let _ = DetectCurveConflictsRequest::decode(data);
    let _ = DetectCurveConflictsResponse::decode(data);
    let _ = ResolveCurveConflictRequest::decode(data);
    let _ = ResolveCurveConflictResponse::decode(data);
    let _ = ConfigureTelemetryRequest::decode(data);
    let _ = ConfigureTelemetryResponse::decode(data);
});
