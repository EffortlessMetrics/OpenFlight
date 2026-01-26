#![no_main]

use libfuzzer_sys::fuzz_target;
use prost::Message;
use flight_ipc::proto::NegotiateFeaturesRequest;

fuzz_target!(|data: &[u8]| {
    // Fuzz protobuf parsing for NegotiateFeaturesRequest
    if let Ok(request) = NegotiateFeaturesRequest::decode(data) {
        // Just verify we can access fields without panic
        let _ = request.client_version;
        let _ = request.supported_features;
    }
});
