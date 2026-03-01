#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz all X-Plane UDP/protocol parsers with arbitrary bytes.
    // None of these should panic on malformed input.
    let _ = flight_xplane::udp_protocol::parse_data_packet(data);
    let _ = flight_xplane::udp_protocol::parse_rref_response(data);
    let _ = flight_xplane::plugin_protocol::decode(data);
});
