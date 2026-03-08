#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz DCS Export.lua protocol parsers with arbitrary strings.
    // Covers telemetry batch parsing, export line parsing, and structured blocks.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = flight_dcs_export::protocol::parse_telemetry_batch(s);
        let _ = flight_dcs_export::protocol::parse_export_line(s);
        let _ = flight_dcs_export::protocol::parse_device_arg_block(s);
        let _ = flight_dcs_export::protocol::parse_instrument_block(s);
    }
});
