// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive snapshot tests for `flight-simconnect` structured outputs.
//!
//! Covers SimVar registry, event mapping catalog, and aircraft database.
//! Run `cargo insta review` to accept changes.

#![cfg(windows)]

use flight_simconnect::aircraft_db::{AircraftType, MsfsAircraftDb};
use flight_simconnect::event_mapping::{
    SIM_EVENT_CATALOG, SimEventCategory, SimEventMapper, catalog_by_category,
};
use flight_simconnect::var_registry::{SimVarCategory, SimVarRegistry};

// ── SimVar registry serialization (all registered vars) ──────────────────────

#[test]
fn snapshot_simvar_registry_all_names_sorted() {
    let reg = SimVarRegistry::new();
    let mut names: Vec<&str> = reg.all().iter().map(|v| v.name).collect();
    names.sort();
    insta::assert_debug_snapshot!("simvar_registry_all_names", names);
}

#[test]
fn snapshot_simvar_registry_by_category_flight_controls() {
    let reg = SimVarRegistry::new();
    let mut vars: Vec<(&str, &str, bool)> = reg
        .by_category(SimVarCategory::FlightControls)
        .iter()
        .map(|v| (v.name, v.unit, v.writable))
        .collect();
    vars.sort_by_key(|v| v.0);
    insta::assert_debug_snapshot!("simvar_flight_controls", vars);
}

#[test]
fn snapshot_simvar_registry_by_category_engine() {
    let reg = SimVarRegistry::new();
    let mut vars: Vec<(&str, &str, bool)> = reg
        .by_category(SimVarCategory::Engine)
        .iter()
        .map(|v| (v.name, v.unit, v.writable))
        .collect();
    vars.sort_by_key(|v| v.0);
    insta::assert_debug_snapshot!("simvar_engine", vars);
}

#[test]
fn snapshot_simvar_registry_by_category_navigation() {
    let reg = SimVarRegistry::new();
    let mut vars: Vec<(&str, &str, bool)> = reg
        .by_category(SimVarCategory::Navigation)
        .iter()
        .map(|v| (v.name, v.unit, v.writable))
        .collect();
    vars.sort_by_key(|v| v.0);
    insta::assert_debug_snapshot!("simvar_navigation", vars);
}

#[test]
fn snapshot_simvar_registry_writable_vars() {
    let reg = SimVarRegistry::new();
    let mut names: Vec<&str> = reg.writable_vars().iter().map(|v| v.name).collect();
    names.sort();
    insta::assert_debug_snapshot!("simvar_writable_vars", names);
}

#[test]
fn snapshot_simvar_registry_category_counts() {
    let reg = SimVarRegistry::new();
    let categories = [
        SimVarCategory::FlightControls,
        SimVarCategory::Engine,
        SimVarCategory::Navigation,
        SimVarCategory::Electrical,
        SimVarCategory::Fuel,
        SimVarCategory::Landing,
        SimVarCategory::Environment,
        SimVarCategory::Instruments,
        SimVarCategory::Autopilot,
        SimVarCategory::Communication,
    ];
    let mut counts: Vec<(String, usize)> = categories
        .iter()
        .map(|cat| (format!("{:?}", cat), reg.by_category(*cat).len()))
        .collect();
    counts.sort_by_key(|c| c.0.clone());
    insta::assert_debug_snapshot!("simvar_category_counts", counts);
}

// ── Event mapping serialization ──────────────────────────────────────────────

#[test]
fn snapshot_event_catalog_all_names_sorted() {
    let mut names: Vec<&str> = SIM_EVENT_CATALOG.iter().map(|e| e.name).collect();
    names.sort();
    insta::assert_debug_snapshot!("event_catalog_all_names", names);
}

#[test]
fn snapshot_event_catalog_by_category_flight_controls() {
    let events = catalog_by_category(SimEventCategory::FlightControls);
    let mut items: Vec<(&str, bool)> = events.iter().map(|e| (e.name, e.toggle)).collect();
    items.sort_by_key(|i| i.0);
    insta::assert_debug_snapshot!("event_catalog_flight_controls", items);
}

#[test]
fn snapshot_event_catalog_by_category_autopilot() {
    let events = catalog_by_category(SimEventCategory::Autopilot);
    let mut items: Vec<(&str, bool)> = events.iter().map(|e| (e.name, e.toggle)).collect();
    items.sort_by_key(|i| i.0);
    insta::assert_debug_snapshot!("event_catalog_autopilot", items);
}

#[test]
fn snapshot_event_catalog_by_category_electrical() {
    let events = catalog_by_category(SimEventCategory::Electrical);
    let mut items: Vec<(&str, bool)> = events.iter().map(|e| (e.name, e.toggle)).collect();
    items.sort_by_key(|i| i.0);
    insta::assert_debug_snapshot!("event_catalog_electrical", items);
}

#[test]
fn snapshot_event_catalog_category_counts() {
    let categories = [
        SimEventCategory::FlightControls,
        SimEventCategory::Engine,
        SimEventCategory::Autopilot,
        SimEventCategory::Electrical,
        SimEventCategory::Radios,
        SimEventCategory::Views,
        SimEventCategory::Misc,
    ];
    let mut counts: Vec<(String, usize)> = categories
        .iter()
        .map(|cat| (format!("{:?}", cat), catalog_by_category(*cat).len()))
        .collect();
    counts.sort_by_key(|c| c.0.clone());
    insta::assert_debug_snapshot!("event_catalog_category_counts", counts);
}

#[test]
fn snapshot_event_mapper_export() {
    let mut mapper = SimEventMapper::new();
    mapper.map_button("btn_1", "GEAR_TOGGLE");
    mapper.map_button("btn_1", "TOGGLE_NAV_LIGHTS");
    mapper.map_button("btn_2", "AP_MASTER");
    mapper.map_button("btn_3", "FLAPS_INCR");
    mapper.map_button("hat_up", "ELEV_TRIM_UP");
    mapper.map_button("hat_down", "ELEV_TRIM_DN");

    let exported = mapper.export_mapping();
    insta::assert_debug_snapshot!("event_mapper_export", exported);
}

// ── Aircraft database snapshots ──────────────────────────────────────────────

#[test]
fn snapshot_aircraft_type_variants() {
    let types = [
        AircraftType::SingleProp,
        AircraftType::TwinProp,
        AircraftType::Turboprop,
        AircraftType::SingleJet,
        AircraftType::TwinJet,
        AircraftType::Helicopter,
        AircraftType::Glider,
    ];
    insta::assert_debug_snapshot!("aircraft_type_variants", types);
}

#[test]
fn snapshot_aircraft_db_all_icao_codes() {
    let db = MsfsAircraftDb::new();
    let mut codes = db.all_icao_codes();
    codes.sort();
    insta::assert_debug_snapshot!("aircraft_db_all_icao_codes", codes);
}

#[test]
fn snapshot_aircraft_db_c172_info() {
    let db = MsfsAircraftDb::new();
    let info = db.get("C172").expect("C172 must be in database");
    insta::assert_debug_snapshot!("aircraft_db_c172", info);
}

#[test]
fn snapshot_aircraft_db_a320_info() {
    let db = MsfsAircraftDb::new();
    let info = db.get("A320").expect("A320 must be in database");
    insta::assert_debug_snapshot!("aircraft_db_a320", info);
}
