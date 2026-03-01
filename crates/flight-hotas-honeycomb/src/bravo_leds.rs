// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! LED output control for the Honeycomb Bravo Throttle Quadrant.
//!
//! # Protocol
//!
//! LED state is transmitted via a 5-byte HID **feature report**:
//!
//! ```text
//! [0x00, ap_byte, gear_byte, annunciator1_byte, annunciator2_byte]
//! ```
//!
//! - `data[0]` = report ID (`0x00`)
//! - `data[1]` = AP mode LEDs
//! - `data[2]` = landing gear + master warning/fire
//! - `data[3]` = annunciator row 1 (oil, fuel, anti-ice, etc.)
//! - `data[4]` = annunciator row 2 (aux pump, parking brake, etc.)
//!
//! In Rust / hidapi, send with:
//! ```ignore
//! device.send_feature_report(&serialize_led_report(&leds))?;
//! ```
//!
//! Protocol confirmed from BetterBravoLights (RoystonS, GitHub), which is a
//! production-tested MSFS companion tool for the Bravo.

/// Complete LED state for the Bravo Throttle Quadrant.
///
/// Set the appropriate boolean fields to `true` to illuminate an LED,
/// then call [`serialize_led_report`] to produce the feature report bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BravoLedState {
    // ── Autopilot mode LEDs (byte 1) ────────────────────────────────────
    /// HDG mode LED.
    pub hdg: bool,
    /// NAV mode LED.
    pub nav: bool,
    /// APR (approach) mode LED.
    pub apr: bool,
    /// REV (back course) mode LED.
    pub rev: bool,
    /// ALT (altitude hold) mode LED.
    pub alt: bool,
    /// VS (vertical speed) mode LED.
    pub vs: bool,
    /// IAS (indicated airspeed) mode LED.
    pub ias: bool,
    /// AUTOPILOT master LED.
    pub autopilot: bool,

    // ── Landing gear LEDs (byte 2, bits 0–5) ────────────────────────────
    /// Left main gear: green (down-and-locked).
    pub gear_l_green: bool,
    /// Left main gear: red (in transit / unsafe).
    pub gear_l_red: bool,
    /// Centre gear: green.
    pub gear_c_green: bool,
    /// Centre gear: red.
    pub gear_c_red: bool,
    /// Right main gear: green.
    pub gear_r_green: bool,
    /// Right main gear: red.
    pub gear_r_red: bool,

    // ── Warning LEDs (byte 2, bits 6–7) ─────────────────────────────────
    /// MASTER WARNING light.
    pub master_warning: bool,
    /// ENGINE FIRE light.
    pub engine_fire: bool,

    // ── Annunciator row 1 (byte 3) ───────────────────────────────────────
    /// Low oil pressure.
    pub low_oil_pressure: bool,
    /// Low fuel pressure.
    pub low_fuel_pressure: bool,
    /// Anti-ice system active.
    pub anti_ice: bool,
    /// Starter engaged.
    pub starter_engaged: bool,
    /// APU active.
    pub apu: bool,
    /// MASTER CAUTION light.
    pub master_caution: bool,
    /// Low vacuum.
    pub vacuum: bool,
    /// Low hydraulic pressure.
    pub low_hyd_pressure: bool,

    // ── Annunciator row 2 (byte 4) ───────────────────────────────────────
    /// Auxiliary fuel pump active.
    pub aux_fuel_pump: bool,
    /// Parking brake set.
    pub parking_brake: bool,
    /// Low volts / battery.
    pub low_volts: bool,
    /// Door open.
    pub door: bool,
}

impl BravoLedState {
    /// Returns a state with all LEDs off.
    pub fn all_off() -> Self {
        Self::default()
    }

    /// Returns a state with all LEDs on (useful for lamp test).
    pub fn all_on() -> Self {
        Self {
            hdg: true,
            nav: true,
            apr: true,
            rev: true,
            alt: true,
            vs: true,
            ias: true,
            autopilot: true,
            gear_l_green: true,
            gear_l_red: true,
            gear_c_green: true,
            gear_c_red: true,
            gear_r_green: true,
            gear_r_red: true,
            master_warning: true,
            engine_fire: true,
            low_oil_pressure: true,
            low_fuel_pressure: true,
            anti_ice: true,
            starter_engaged: true,
            apu: true,
            master_caution: true,
            vacuum: true,
            low_hyd_pressure: true,
            aux_fuel_pump: true,
            parking_brake: true,
            low_volts: true,
            door: true,
        }
    }

    /// Set all three landing gear LEDs to the same colour at once.
    ///
    /// `green = true` → green, `green = false` → red.
    pub fn set_all_gear(&mut self, green: bool) {
        self.gear_l_green = green;
        self.gear_c_green = green;
        self.gear_r_green = green;
        self.gear_l_red = !green;
        self.gear_c_red = !green;
        self.gear_r_red = !green;
    }
}

/// Serialise a [`BravoLedState`] into the 5-byte HID feature report.
///
/// The returned array is ready to pass directly to
/// `hidapi::HidDevice::send_feature_report`.
///
/// # Layout
///
/// | Byte | Content |
/// |------|---------|
/// | 0    | Report ID = 0x00 |
/// | 1    | AP mode LEDs |
/// | 2    | Gear + warning LEDs |
/// | 3    | Annunciator row 1 |
/// | 4    | Annunciator row 2 |
pub fn serialize_led_report(leds: &BravoLedState) -> [u8; 5] {
    let mut data = [0u8; 5];
    // data[0] = report ID 0x00 (already zero)

    // Byte 1 — AP mode
    data[1] |= leds.hdg as u8;
    data[1] |= (leds.nav as u8) << 1;
    data[1] |= (leds.apr as u8) << 2;
    data[1] |= (leds.rev as u8) << 3;
    data[1] |= (leds.alt as u8) << 4;
    data[1] |= (leds.vs as u8) << 5;
    data[1] |= (leds.ias as u8) << 6;
    data[1] |= (leds.autopilot as u8) << 7;

    // Byte 2 — Gear + warnings
    data[2] |= leds.gear_l_green as u8;
    data[2] |= (leds.gear_l_red as u8) << 1;
    data[2] |= (leds.gear_c_green as u8) << 2;
    data[2] |= (leds.gear_c_red as u8) << 3;
    data[2] |= (leds.gear_r_green as u8) << 4;
    data[2] |= (leds.gear_r_red as u8) << 5;
    data[2] |= (leds.master_warning as u8) << 6;
    data[2] |= (leds.engine_fire as u8) << 7;

    // Byte 3 — Annunciator row 1
    data[3] |= leds.low_oil_pressure as u8;
    data[3] |= (leds.low_fuel_pressure as u8) << 1;
    data[3] |= (leds.anti_ice as u8) << 2;
    data[3] |= (leds.starter_engaged as u8) << 3;
    data[3] |= (leds.apu as u8) << 4;
    data[3] |= (leds.master_caution as u8) << 5;
    data[3] |= (leds.vacuum as u8) << 6;
    data[3] |= (leds.low_hyd_pressure as u8) << 7;

    // Byte 4 — Annunciator row 2
    data[4] |= leds.aux_fuel_pump as u8;
    data[4] |= (leds.parking_brake as u8) << 1;
    data[4] |= (leds.low_volts as u8) << 2;
    data[4] |= (leds.door as u8) << 3;
    // bits 4–7: unused

    data
}

/// Deserialise a 5-byte HID feature report into a [`BravoLedState`].
///
/// This is the inverse of [`serialize_led_report`], useful for diagnostics
/// and round-trip testing. The report ID byte (`data[0]`) is ignored.
///
/// # Panics
///
/// Panics if `data.len() < 5`.
pub fn deserialize_led_report(data: &[u8; 5]) -> BravoLedState {
    BravoLedState {
        hdg: data[1] & (1 << 0) != 0,
        nav: data[1] & (1 << 1) != 0,
        apr: data[1] & (1 << 2) != 0,
        rev: data[1] & (1 << 3) != 0,
        alt: data[1] & (1 << 4) != 0,
        vs: data[1] & (1 << 5) != 0,
        ias: data[1] & (1 << 6) != 0,
        autopilot: data[1] & (1 << 7) != 0,

        gear_l_green: data[2] & (1 << 0) != 0,
        gear_l_red: data[2] & (1 << 1) != 0,
        gear_c_green: data[2] & (1 << 2) != 0,
        gear_c_red: data[2] & (1 << 3) != 0,
        gear_r_green: data[2] & (1 << 4) != 0,
        gear_r_red: data[2] & (1 << 5) != 0,
        master_warning: data[2] & (1 << 6) != 0,
        engine_fire: data[2] & (1 << 7) != 0,

        low_oil_pressure: data[3] & (1 << 0) != 0,
        low_fuel_pressure: data[3] & (1 << 1) != 0,
        anti_ice: data[3] & (1 << 2) != 0,
        starter_engaged: data[3] & (1 << 3) != 0,
        apu: data[3] & (1 << 4) != 0,
        master_caution: data[3] & (1 << 5) != 0,
        vacuum: data[3] & (1 << 6) != 0,
        low_hyd_pressure: data[3] & (1 << 7) != 0,

        aux_fuel_pump: data[4] & (1 << 0) != 0,
        parking_brake: data[4] & (1 << 1) != 0,
        low_volts: data[4] & (1 << 2) != 0,
        door: data[4] & (1 << 3) != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_off_produces_zero_data_bytes() {
        let report = serialize_led_report(&BravoLedState::all_off());
        assert_eq!(report[0], 0x00, "report ID must be 0");
        assert_eq!(report[1], 0, "AP byte should be 0 when all off");
        assert_eq!(report[2], 0, "gear byte should be 0 when all off");
        assert_eq!(report[3], 0, "annunciator1 byte should be 0");
        assert_eq!(report[4], 0, "annunciator2 byte should be 0");
    }

    #[test]
    fn test_all_on_produces_correct_report_id() {
        let report = serialize_led_report(&BravoLedState::all_on());
        assert_eq!(report[0], 0x00, "report ID must always be 0");
    }

    #[test]
    fn test_hdg_led_only() {
        let mut leds = BravoLedState::all_off();
        leds.hdg = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[1], 0b0000_0001, "HDG is bit 0 of byte 1");
        assert_eq!(report[2], 0);
        assert_eq!(report[3], 0);
        assert_eq!(report[4], 0);
    }

    #[test]
    fn test_autopilot_led_only() {
        let mut leds = BravoLedState::all_off();
        leds.autopilot = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[1], 0b1000_0000, "AUTOPILOT is bit 7 of byte 1");
    }

    #[test]
    fn test_all_ap_leds() {
        let mut leds = BravoLedState::all_off();
        leds.hdg = true;
        leds.nav = true;
        leds.apr = true;
        leds.rev = true;
        leds.alt = true;
        leds.vs = true;
        leds.ias = true;
        leds.autopilot = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[1], 0xFF, "all AP LEDs = 0xFF");
    }

    #[test]
    fn test_gear_l_green() {
        let mut leds = BravoLedState::all_off();
        leds.gear_l_green = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[2], 0b0000_0001);
    }

    #[test]
    fn test_gear_r_red() {
        let mut leds = BravoLedState::all_off();
        leds.gear_r_red = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[2], 0b0010_0000);
    }

    #[test]
    fn test_master_warning() {
        let mut leds = BravoLedState::all_off();
        leds.master_warning = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[2], 0b0100_0000);
    }

    #[test]
    fn test_engine_fire() {
        let mut leds = BravoLedState::all_off();
        leds.engine_fire = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[2], 0b1000_0000);
    }

    #[test]
    fn test_low_oil_pressure() {
        let mut leds = BravoLedState::all_off();
        leds.low_oil_pressure = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[3], 0b0000_0001);
    }

    #[test]
    fn test_low_hyd_pressure() {
        let mut leds = BravoLedState::all_off();
        leds.low_hyd_pressure = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[3], 0b1000_0000);
    }

    #[test]
    fn test_all_annunciator1() {
        let mut leds = BravoLedState::all_off();
        leds.low_oil_pressure = true;
        leds.low_fuel_pressure = true;
        leds.anti_ice = true;
        leds.starter_engaged = true;
        leds.apu = true;
        leds.master_caution = true;
        leds.vacuum = true;
        leds.low_hyd_pressure = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[3], 0xFF);
    }

    #[test]
    fn test_aux_fuel_pump() {
        let mut leds = BravoLedState::all_off();
        leds.aux_fuel_pump = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[4], 0b0000_0001);
    }

    #[test]
    fn test_door() {
        let mut leds = BravoLedState::all_off();
        leds.door = true;
        let report = serialize_led_report(&leds);
        assert_eq!(report[4], 0b0000_1000);
    }

    #[test]
    fn test_annunciator2_high_bits_unused() {
        let report = serialize_led_report(&BravoLedState::all_on());
        // Bits 4–7 of byte 4 are unused; they must be 0
        assert_eq!(report[4] & 0xF0, 0, "high nibble of byte 4 must be 0");
    }

    #[test]
    fn test_all_gear_helper_green() {
        let mut leds = BravoLedState::all_off();
        leds.set_all_gear(true);
        let report = serialize_led_report(&leds);
        // bits 0,2,4 = green for L/C/R; bits 1,3,5 = red for L/C/R (off)
        assert_eq!(report[2] & 0b0001_0101, 0b0001_0101, "L/C/R green bits");
        assert_eq!(report[2] & 0b0010_1010, 0, "L/C/R red bits must be 0");
    }

    #[test]
    fn test_all_gear_helper_red() {
        let mut leds = BravoLedState::all_off();
        leds.set_all_gear(false);
        let report = serialize_led_report(&leds);
        assert_eq!(report[2] & 0b0010_1010, 0b0010_1010, "L/C/R red bits");
        assert_eq!(report[2] & 0b0001_0101, 0, "L/C/R green bits must be 0");
    }

    #[test]
    fn test_roundtrip_all_on() {
        // Verify every named field in all_on() is represented in the report
        let report = serialize_led_report(&BravoLedState::all_on());
        assert_eq!(report[0], 0x00);
        assert_eq!(report[1], 0xFF); // all 8 AP mode LEDs
        assert_eq!(report[2], 0xFF); // all gear + warning LEDs
        assert_eq!(report[3], 0xFF); // all annunciator 1 LEDs
        // annunciator 2 only has 4 named LEDs (bits 0–3), high nibble unused
        assert_eq!(report[4] & 0x0F, 0x0F);
    }

    #[test]
    fn test_deserialize_all_off() {
        let report = [0x00, 0, 0, 0, 0];
        let leds = deserialize_led_report(&report);
        assert_eq!(leds, BravoLedState::all_off());
    }

    #[test]
    fn test_deserialize_all_on_roundtrip() {
        let original = BravoLedState::all_on();
        let report = serialize_led_report(&original);
        let deserialized = deserialize_led_report(&report);
        assert_eq!(deserialized, original);
    }

    #[test]
    fn test_deserialize_single_led_hdg() {
        let report = [0x00, 0b0000_0001, 0, 0, 0];
        let leds = deserialize_led_report(&report);
        assert!(leds.hdg);
        assert!(!leds.nav);
        assert!(!leds.autopilot);
    }

    #[test]
    fn test_deserialize_gear_and_warning() {
        let report = [0x00, 0, 0b1100_0101, 0, 0];
        let leds = deserialize_led_report(&report);
        assert!(leds.gear_l_green);
        assert!(!leds.gear_l_red);
        assert!(leds.gear_c_green);
        assert!(!leds.gear_c_red);
        assert!(!leds.gear_r_green);
        assert!(!leds.gear_r_red);
        assert!(leds.master_warning);
        assert!(leds.engine_fire);
    }

    #[test]
    fn test_roundtrip_arbitrary_state() {
        let mut leds = BravoLedState::all_off();
        leds.nav = true;
        leds.alt = true;
        leds.gear_c_green = true;
        leds.parking_brake = true;
        leds.master_caution = true;
        let report = serialize_led_report(&leds);
        let deserialized = deserialize_led_report(&report);
        assert_eq!(deserialized, leds);
    }
}
