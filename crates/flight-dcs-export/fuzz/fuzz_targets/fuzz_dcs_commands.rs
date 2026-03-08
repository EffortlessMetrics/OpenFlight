#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz DCS wire command parser with arbitrary strings.
    // Covers CMD:/BTN:/TGL: prefixed commands and multi-line payloads.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = flight_dcs_export::control_injection::parse_wire_command(s);
        let _ = flight_dcs_export::control_injection::parse_wire_payload(s);
    }
});
