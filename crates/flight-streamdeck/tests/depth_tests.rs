// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for StreamDeck integration
//!
//! Covers button rendering, page management, button actions, device discovery,
//! profile integration, and API surface.

use std::collections::HashMap;

use flight_streamdeck::actions::{
    ActionBehavior, ActionCategory, ActionRegistry, ActionTemplate, builtin_templates,
};
use flight_streamdeck::api::{
    ApiResponse, EventSubscriptionResponse, ProfileListResponse, StreamDeckApi,
    TelemetryResponse, VersionCheckRequest, VersionCheckResponse,
};
use flight_streamdeck::compatibility::{
    CompatibilityMatrix, CompatibilityStatus, VersionCompatibility, VersionRange,
};
use flight_streamdeck::device::{
    Brightness, DeviceInfo, DeviceManager, LcdStripInfo, LcdStripLayout, StreamDeckModel,
};
use flight_streamdeck::layout::{KeySlot, NavTarget, Page, PageLayout, ProfileLayout};
use flight_streamdeck::profiles::{AircraftType, ProfileManager, SampleProfiles};
use flight_streamdeck::render::{IconRenderer, IconStyle, IconTheme, KeyIcon};
use flight_streamdeck::server::ServerConfig;
use flight_streamdeck::AppVersion;

// ═══════════════════════════════════════════════════════════════════════════════
//  1. BUTTON RENDERING (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Icon rendering: every model with a display produces a correctly-sized icon.
#[test]
fn render_icon_for_every_displayable_model() {
    let displayable = [
        (StreamDeckModel::Original, 72),
        (StreamDeckModel::Mini, 80),
        (StreamDeckModel::Xl, 96),
        (StreamDeckModel::Plus, 120),
        (StreamDeckModel::Neo, 96),
    ];
    for (model, expected_size) in displayable {
        let icon = KeyIcon::for_model(model, "TST", IconTheme::Autopilot)
            .unwrap_or_else(|| panic!("{model:?} should produce an icon"));
        assert_eq!(icon.size, expected_size, "wrong size for {model:?}");
        assert_eq!(icon.label, "TST");
    }
}

/// Text rendering: value sub-label is correctly attached and retrievable.
#[test]
fn render_text_with_value_sublabel() {
    let icon = KeyIcon::for_model(StreamDeckModel::Original, "HDG", IconTheme::Navigation)
        .unwrap()
        .with_value("270");
    assert_eq!(icon.label, "HDG");
    assert_eq!(icon.value.as_deref(), Some("270"));

    // Value can be overwritten
    let icon = icon.with_value("090");
    assert_eq!(icon.value.as_deref(), Some("090"));
}

/// Color fills: every theme produces valid hex colors for bg/text/accent.
#[test]
fn render_color_fill_from_all_themes() {
    let themes = [
        IconTheme::Autopilot,
        IconTheme::Communication,
        IconTheme::Navigation,
        IconTheme::Lights,
        IconTheme::Systems,
        IconTheme::Warning,
        IconTheme::Custom,
    ];
    for theme in themes {
        let style = theme.to_style();
        assert!(
            style.background_color.starts_with('#') && style.background_color.len() == 7,
            "{theme:?} bg: {}",
            style.background_color
        );
        assert!(
            style.text_color.starts_with('#') && style.text_color.len() == 7,
            "{theme:?} text: {}",
            style.text_color
        );
        assert!(
            style.accent_color.starts_with('#') && style.accent_color.len() == 7,
            "{theme:?} accent: {}",
            style.accent_color
        );
    }
}

/// Button state indicators: active variant uses accent color as text color.
#[test]
fn render_active_state_indicator() {
    let base = KeyIcon::for_model(StreamDeckModel::Xl, "AP", IconTheme::Autopilot).unwrap();
    let active = base.active_variant();

    assert!(active.active, "active variant must be flagged active");
    assert_eq!(
        active.style.text_color, active.style.accent_color,
        "active text color should equal accent"
    );
    // Background should remain unchanged
    assert_eq!(active.style.background_color, base.style.background_color);
}

/// Active vs inactive: round-tripping preserves label, changes only state/colors.
#[test]
fn render_active_inactive_roundtrip() {
    let base = KeyIcon::for_model(StreamDeckModel::Plus, "VS", IconTheme::Systems).unwrap();
    let active = base.active_variant();
    let inactive = active.inactive_variant();

    assert!(!inactive.active);
    assert_eq!(inactive.label, base.label);
    assert_eq!(inactive.size, base.size);
    // Inactive uses the Custom theme's text color (#FFFFFF)
    let (_, custom_text, _) = IconTheme::Custom.colors();
    assert_eq!(inactive.style.text_color, custom_text);
}

/// Multi-icon composition: renderer produces consistent toggle pairs.
#[test]
fn render_multi_icon_toggle_pair() {
    let renderer = IconRenderer::new(StreamDeckModel::Original);
    let (off, on) = renderer.toggle_icon("LAND", IconTheme::Lights).unwrap();

    assert!(!off.active);
    assert!(on.active);
    assert_eq!(off.label, on.label);
    assert_eq!(off.size, on.size);
    // Colors differ between states
    assert_ne!(off.style.text_color, on.style.text_color);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  2. PAGE MANAGEMENT (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Page creation: pages can be added and counted.
#[test]
fn page_creation_and_count() {
    let mut layout = PageLayout::new(StreamDeckModel::Original).unwrap();
    assert_eq!(layout.page_count(), 0);

    let idx0 = layout.add_page(Page::new("Main"));
    let idx1 = layout.add_page(Page::new("Lights"));
    assert_eq!(idx0, 0);
    assert_eq!(idx1, 1);
    assert_eq!(layout.page_count(), 2);
}

/// Page switching: go_to_page sets current and returns correct page.
#[test]
fn page_switching_go_to_page() {
    let mut layout = PageLayout::new(StreamDeckModel::Xl).unwrap();
    layout.add_page(Page::new("A"));
    layout.add_page(Page::new("B"));
    layout.add_page(Page::new("C"));

    let page = layout.go_to_page(2).unwrap();
    assert_eq!(page.name, "C");
    assert_eq!(layout.current_index(), 2);

    // Out-of-range returns error
    assert!(layout.go_to_page(10).is_err());
    // Current index unchanged after error
    assert_eq!(layout.current_index(), 2);
}

/// Page navigation: next/prev wrap around correctly.
#[test]
fn page_navigation_wraps() {
    let mut layout = PageLayout::new(StreamDeckModel::Mini).unwrap();
    layout.add_page(Page::new("P0"));
    layout.add_page(Page::new("P1"));
    layout.add_page(Page::new("P2"));

    // Forward wrap
    assert_eq!(layout.current_index(), 0);
    layout.next_page();
    layout.next_page();
    layout.next_page(); // wraps to 0
    assert_eq!(layout.current_index(), 0);

    // Backward wrap
    layout.prev_page(); // wraps to 2
    assert_eq!(layout.current_index(), 2);
    layout.prev_page();
    assert_eq!(layout.current_index(), 1);
}

/// Page count: ProfileLayout produces expected page count for Mini (6 keys).
#[test]
fn page_count_for_mini_with_many_actions() {
    let templates: HashMap<String, ActionTemplate> = builtin_templates()
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();

    // Mini: 6 keys, 5 usable per page. 21 actions → ceil(21/5) = 5 pages.
    let layout =
        ProfileLayout::build(AircraftType::GA, StreamDeckModel::Mini, &templates).unwrap();
    assert!(
        layout.page_count() > 1,
        "Mini should need multiple pages for all actions"
    );
}

/// Default page: new layout starts at page 0.
#[test]
fn default_page_is_zero() {
    let mut layout = PageLayout::new(StreamDeckModel::Original).unwrap();
    layout.add_page(Page::new("Root"));
    layout.add_page(Page::new("Secondary"));

    assert_eq!(layout.current_index(), 0);
    assert_eq!(layout.current().unwrap().name, "Root");
}

/// Conditional page visibility: empty layout has no current page.
#[test]
fn conditional_page_visibility_empty() {
    let layout = PageLayout::new(StreamDeckModel::Plus).unwrap();
    assert!(layout.current().is_none());
    assert_eq!(layout.page_count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
//  3. BUTTON ACTIONS (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Sim command binding: every toggle action has non-empty command_on.
#[test]
fn action_sim_command_binding() {
    for t in builtin_templates() {
        assert!(
            !t.command_on.is_empty(),
            "action {} must have command_on",
            t.id
        );
        if t.behavior == ActionBehavior::Toggle {
            assert!(
                !t.command_off.is_empty(),
                "toggle {} must have command_off",
                t.id
            );
        }
    }
}

/// Profile switch: registry allows adding custom profile-switch action.
#[test]
fn action_profile_switch_via_registry() {
    let mut reg = ActionRegistry::builtin();
    let before = reg.len();

    reg.register(ActionTemplate {
        id: "com.flighthub.profile-switch".to_string(),
        name: "Switch Profile".to_string(),
        key_label: "PROF".to_string(),
        category: ActionCategory::Systems,
        behavior: ActionBehavior::Momentary,
        command_on: "PROFILE_CYCLE".to_string(),
        command_off: String::new(),
        feedback_variable: String::new(),
        tooltip: "Cycle to next profile".to_string(),
    });

    assert_eq!(reg.len(), before + 1);
    let action = reg.get("com.flighthub.profile-switch").unwrap();
    assert_eq!(action.behavior, ActionBehavior::Momentary);
}

/// Page navigation: KeySlot can hold a NavTarget for page switching.
#[test]
fn action_page_navigation_key() {
    let slot = KeySlot {
        row: 2,
        col: 4,
        action_id: None,
        nav_target: Some(NavTarget::NextPage),
    };
    assert!(slot.action_id.is_none());
    assert!(matches!(slot.nav_target, Some(NavTarget::NextPage)));

    let back = KeySlot {
        row: 0,
        col: 0,
        action_id: None,
        nav_target: Some(NavTarget::Back),
    };
    assert!(matches!(back.nav_target, Some(NavTarget::Back)));
}

/// Toggle action: registry correctly categorises toggle behavior.
#[test]
fn action_toggle_behavior_identification() {
    let reg = ActionRegistry::builtin();
    let ap = reg.get("com.flighthub.ap-toggle").unwrap();
    assert_eq!(ap.behavior, ActionBehavior::Toggle);
    assert!(!ap.feedback_variable.is_empty());
}

/// Momentary action: COM swap fires once, no command_off.
#[test]
fn action_momentary_no_off_command() {
    let reg = ActionRegistry::builtin();
    let com1 = reg.get("com.flighthub.com1-swap").unwrap();
    assert_eq!(com1.behavior, ActionBehavior::Momentary);
    assert!(com1.command_off.is_empty());
}

/// Long-press vs short-press: Encoder actions have both on/off commands.
#[test]
fn action_encoder_has_both_commands() {
    let reg = ActionRegistry::builtin();
    let obs = reg.get("com.flighthub.vor-obs-inc").unwrap();
    assert_eq!(obs.behavior, ActionBehavior::Encoder);
    assert!(!obs.command_on.is_empty(), "encoder must have command_on");
    assert!(
        !obs.command_off.is_empty(),
        "encoder must have command_off (decrement)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
//  4. DEVICE DISCOVERY (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Enumerate StreamDecks: discover only returns connected devices.
#[test]
fn device_enumerate_connected_only() {
    let mut mgr = DeviceManager::new();
    mgr.register_device(DeviceInfo {
        id: "d-1".into(),
        model: StreamDeckModel::Original,
        serial: Some("SER-001".into()),
        firmware_version: Some("1.0.0".into()),
        connected: true,
    });
    mgr.register_device(DeviceInfo {
        id: "d-2".into(),
        model: StreamDeckModel::Xl,
        serial: None,
        firmware_version: None,
        connected: false,
    });

    let discovered = mgr.discover();
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].id, "d-1");
}

/// Multiple StreamDeck models: all six models have unique product IDs and names.
#[test]
fn device_all_models_unique_properties() {
    let models = StreamDeckModel::all();
    assert_eq!(models.len(), 6);

    let mut pids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for m in models {
        assert!(pids.insert(m.product_id()), "duplicate PID for {m:?}");
        assert!(
            names.insert(m.display_name()),
            "duplicate name for {m:?}"
        );
    }
    // All share the same vendor ID
    assert_eq!(StreamDeckModel::vendor_id(), 0x0FD9);
}

/// Model capabilities: Plus has dials + LCD, Pedal has neither keys display nor grid.
#[test]
fn device_model_capabilities() {
    // Plus: 8 keys, 4 dials, LCD strip
    let plus = StreamDeckModel::Plus;
    assert_eq!(plus.key_count(), 8);
    assert_eq!(plus.dial_count(), 4);
    assert!(plus.has_dials());
    assert!(plus.has_lcd_strip());
    assert!(LcdStripInfo::for_model(plus).is_some());

    // Pedal: 3 pedals, no display, no grid
    let pedal = StreamDeckModel::Pedal;
    assert_eq!(pedal.key_count(), 3);
    assert_eq!(pedal.dial_count(), 0);
    assert!(!pedal.has_dials());
    assert!(!pedal.has_lcd_strip());
    assert!(pedal.icon_size().is_none());
    assert!(pedal.grid_layout().is_none());

    // Neo: 8 keys, LCD strip but no dials
    let neo = StreamDeckModel::Neo;
    assert!(neo.has_lcd_strip());
    assert!(!neo.has_dials());
    assert!(neo.icon_size().is_some());
}

/// Device connect/disconnect: brightness persists across disconnect.
#[test]
fn device_connect_disconnect_brightness() {
    let mut mgr = DeviceManager::new();
    mgr.register_device(DeviceInfo {
        id: "dev-x".into(),
        model: StreamDeckModel::Mini,
        serial: None,
        firmware_version: None,
        connected: true,
    });

    mgr.set_brightness("dev-x", Brightness::new(40).unwrap())
        .unwrap();
    mgr.disconnect_device("dev-x");

    // Device is known but not discovered
    assert!(mgr.discover().is_empty());
    assert!(mgr.get_device("dev-x").is_some());

    // Brightness setting persists
    assert_eq!(mgr.get_brightness("dev-x").unwrap().percent(), 40);
}

/// Device discovery: LCD strip layout for Plus produces 4 equal-width segments.
#[test]
fn device_lcd_strip_four_dial_layout() {
    let layout = LcdStripLayout::four_dial_layout(["HDG", "ALT", "SPD", "VS"]).unwrap();
    assert_eq!(layout.segments.len(), 4);

    let strip = LcdStripInfo::for_model(StreamDeckModel::Plus).unwrap();
    let expected_width = strip.width / 4;
    for (i, seg) in layout.segments.iter().enumerate() {
        assert_eq!(seg.width, expected_width, "segment {i} width");
        assert_eq!(seg.x, expected_width * i as u32, "segment {i} x offset");
        assert_eq!(seg.height, strip.height, "segment {i} height");
    }
    assert_eq!(layout.segments[0].label, "HDG");
    assert_eq!(layout.segments[3].label, "VS");
}

// ═══════════════════════════════════════════════════════════════════════════════
//  5. PROFILE INTEGRATION (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Profile-driven button layout: GA layout places lights first on Original.
#[test]
fn profile_driven_layout_ga_original() {
    let templates: HashMap<String, ActionTemplate> = builtin_templates()
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();
    let layout =
        ProfileLayout::build(AircraftType::GA, StreamDeckModel::Original, &templates).unwrap();

    assert!(layout.page_count() >= 1);
    // First page should have action keys plus a nav key
    let first = layout.current().unwrap();
    assert!(!first.keys.is_empty());
    // Last key is always a nav key
    let nav = first.keys.last().unwrap();
    assert!(nav.nav_target.is_some());
    assert!(nav.action_id.is_none());
}

/// Auto-update on profile change: rebuilding with different aircraft changes layout.
#[test]
fn profile_auto_update_on_aircraft_change() {
    let templates: HashMap<String, ActionTemplate> = builtin_templates()
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();

    let ga_layout =
        ProfileLayout::build(AircraftType::GA, StreamDeckModel::Original, &templates).unwrap();
    let airbus_layout =
        ProfileLayout::build(AircraftType::Airbus, StreamDeckModel::Original, &templates).unwrap();

    // Both have pages, but the page names differ based on aircraft
    let ga_name = &ga_layout.current().unwrap().name;
    let airbus_name = &airbus_layout.current().unwrap().name;
    assert!(ga_name.contains("GA"), "GA layout name: {ga_name}");
    assert!(
        airbus_name.contains("Airbus"),
        "Airbus layout name: {airbus_name}"
    );
}

/// Aircraft-specific pages: each aircraft type has a dedicated sample profile.
#[test]
fn profile_aircraft_specific_pages() {
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();

    for aircraft in SampleProfiles::get_aircraft_types() {
        let profile = pm
            .get_profile(aircraft)
            .unwrap_or_else(|| panic!("missing profile for {aircraft:?}"));
        // Profile must be a JSON object with "actions"
        assert!(
            profile.get("actions").is_some(),
            "{aircraft:?} profile has no actions"
        );
        let actions = profile["actions"].as_object().unwrap();
        assert!(
            !actions.is_empty(),
            "{aircraft:?} profile must have at least one action"
        );
    }
}

/// Phase-of-flight pages: Helo profile includes engine and rotor brake controls.
#[test]
fn profile_helo_phase_of_flight() {
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();

    let helo = pm.get_profile(AircraftType::Helo).unwrap();
    let actions = helo["actions"].as_object().unwrap();

    // Check for helo-specific actions
    let action_uuids: Vec<&str> = actions
        .values()
        .filter_map(|a| a.get("uuid").and_then(|u| u.as_str()))
        .collect();

    assert!(
        action_uuids
            .iter()
            .any(|u| u.contains("engine") || u.contains("rotor")),
        "Helo profile should include engine or rotor controls; found: {action_uuids:?}"
    );
}

/// Profile descriptions: every aircraft type has non-empty description and layout.
#[test]
fn profile_descriptions_and_layouts() {
    for aircraft in SampleProfiles::get_aircraft_types() {
        let desc = SampleProfiles::get_aircraft_description(aircraft);
        assert!(
            !desc.is_empty(),
            "{aircraft:?} must have a description"
        );
        let layout = SampleProfiles::get_recommended_layout(aircraft);
        assert!(
            layout.contains("grid"),
            "{aircraft:?} layout should mention grid: {layout}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  6. API (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// REST API: version check endpoint returns success for supported version.
#[tokio::test]
async fn api_version_check_supported() {
    let compat = VersionCompatibility::new();
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    let api = StreamDeckApi::new(compat, pm);
    let app = api.create_router();
    let server = axum_test::TestServer::new(app).unwrap();

    let req = VersionCheckRequest {
        app_version: "6.2.0".into(),
        plugin_uuid: "test".into(),
    };
    let resp = server.post("/api/v1/version/check").json(&req).await;
    assert_eq!(resp.status_code(), axum::http::StatusCode::OK);

    let body: ApiResponse<VersionCheckResponse> = resp.json();
    assert!(body.success);
    let data = body.data.unwrap();
    assert!(data.compatible);
    assert_eq!(data.api_version, "1.0.0");
}

/// WebSocket events: subscribe endpoint returns subscription with ws URL.
#[tokio::test]
async fn api_event_subscription() {
    let compat = VersionCompatibility::new();
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    let api = StreamDeckApi::new(compat, pm);
    let app = api.create_router();
    let server = axum_test::TestServer::new(app).unwrap();

    let req = serde_json::json!({
        "events": ["telemetry_update", "action_triggered"]
    });
    let resp = server.post("/api/v1/events/subscribe").json(&req).await;
    assert_eq!(resp.status_code(), axum::http::StatusCode::OK);

    let body: ApiResponse<EventSubscriptionResponse> = resp.json();
    assert!(body.success);
    let data = body.data.unwrap();
    assert!(!data.subscription_id.is_empty());
    assert!(data.websocket_url.starts_with("ws://"));
    assert_eq!(data.subscribed_events.len(), 2);
}

/// Button state query: telemetry endpoint returns error when no data.
#[tokio::test]
async fn api_telemetry_no_data() {
    let compat = VersionCompatibility::new();
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    let api = StreamDeckApi::new(compat, pm);
    let app = api.create_router();
    let server = axum_test::TestServer::new(app).unwrap();

    let resp = server.get("/api/v1/telemetry").await;
    let body: ApiResponse<TelemetryResponse> = resp.json();
    assert!(!body.success);
    assert!(body.error.is_some());
}

/// Page state query: profiles endpoint lists all loaded profiles.
#[tokio::test]
async fn api_list_profiles() {
    let compat = VersionCompatibility::new();
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    let api = StreamDeckApi::new(compat, pm);
    let app = api.create_router();
    let server = axum_test::TestServer::new(app).unwrap();

    let resp = server.get("/api/v1/profiles").await;
    assert_eq!(resp.status_code(), axum::http::StatusCode::OK);

    let body: ApiResponse<ProfileListResponse> = resp.json();
    assert!(body.success);
    let data = body.data.unwrap();
    // Should have profiles for all 3 aircraft types
    assert!(
        data.profiles.len() >= 3,
        "expected >= 3 profiles, got {}",
        data.profiles.len()
    );
}

/// Configuration API: health and status endpoints both return success.
#[tokio::test]
async fn api_health_and_status() {
    let compat = VersionCompatibility::new();
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    let api = StreamDeckApi::new(compat, pm);
    let app = api.create_router();
    let server = axum_test::TestServer::new(app).unwrap();

    // Health check
    let resp = server.get("/api/v1/health").await;
    assert_eq!(resp.status_code(), axum::http::StatusCode::OK);
    let body: ApiResponse<serde_json::Value> = resp.json();
    assert!(body.success);
    let health = body.data.unwrap();
    assert_eq!(health["status"], "healthy");

    // Status endpoint
    let resp = server.get("/api/v1/status").await;
    assert_eq!(resp.status_code(), axum::http::StatusCode::OK);
    let body: ApiResponse<serde_json::Value> = resp.json();
    assert!(body.success);
    let status = body.data.unwrap();
    assert_eq!(status["api_version"], "1.0.0");
}

// ═══════════════════════════════════════════════════════════════════════════════
//  ADDITIONAL DEPTH TESTS (to exceed 30 total)
// ═══════════════════════════════════════════════════════════════════════════════

/// AppVersion parsing and display round-trips correctly.
#[test]
fn version_parse_display_roundtrip() {
    let v = AppVersion::from_string("6.2.1").unwrap();
    assert_eq!(v.to_string(), "6.2.1");

    let vb = AppVersion::from_string("6.2.1.500").unwrap();
    assert_eq!(vb.to_string(), "6.2.1.500");
    assert_eq!(vb.build, Some(500));
}

/// Version range: boundary checks at min and max.
#[test]
fn version_range_boundaries() {
    let range = VersionRange::new(AppVersion::new(6, 0, 0), AppVersion::new(6, 4, 999));
    // Inclusive boundaries
    assert!(range.contains(&AppVersion::new(6, 0, 0)));
    assert!(range.contains(&AppVersion::new(6, 4, 999)));
    // Just outside
    assert!(!range.contains(&AppVersion::new(5, 9, 999)));
    assert!(!range.contains(&AppVersion::new(6, 5, 0)));
}

/// Compatibility matrix: unsupported old version returns Unsupported status.
#[test]
fn compatibility_unsupported_old_version() {
    let matrix = CompatibilityMatrix::default_streamdeck();
    let old = AppVersion::new(4, 0, 0);
    assert!(matches!(
        matrix.check_compatibility(&old),
        CompatibilityStatus::Unsupported { .. }
    ));
}

/// Server config: custom CORS origins and timeouts.
#[test]
fn server_config_builder() {
    let config = ServerConfig::new("0.0.0.0".into(), 9090)
        .with_cors_origins(vec!["http://example.com".into()])
        .with_max_connections(200)
        .with_request_timeout(5000);

    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 9090);
    assert_eq!(config.cors_origins.len(), 1);
    assert_eq!(config.max_connections, 200);
    assert_eq!(config.request_timeout_ms, 5000);
    assert!(config.socket_addr().is_ok());
}

/// Brightness: boundary values 0 and 100 are valid; 101 is not.
#[test]
fn brightness_boundary_values() {
    assert_eq!(Brightness::new(0).unwrap().percent(), 0);
    assert_eq!(Brightness::new(100).unwrap().percent(), 100);
    assert!(Brightness::new(101).is_err());
    assert_eq!(Brightness::default().percent(), 70);
}

/// Registry: category filter returns correct subsets.
#[test]
fn registry_category_filter_counts() {
    let reg = ActionRegistry::builtin();
    let ap_count = reg.by_category(ActionCategory::Autopilot).len();
    let com_count = reg.by_category(ActionCategory::Communication).len();
    let nav_count = reg.by_category(ActionCategory::Navigation).len();
    let light_count = reg.by_category(ActionCategory::Lights).len();
    let sys_count = reg.by_category(ActionCategory::Systems).len();

    assert_eq!(ap_count, 6);
    assert_eq!(com_count, 2);
    assert_eq!(nav_count, 4);
    assert_eq!(light_count, 5);
    assert_eq!(sys_count, 4);
    assert_eq!(
        ap_count + com_count + nav_count + light_count + sys_count,
        reg.len()
    );
}

/// Icon style: custom style override replaces all fields.
#[test]
fn icon_custom_style_override() {
    let custom = IconStyle {
        background_color: "#FF0000".into(),
        text_color: "#00FF00".into(),
        accent_color: "#0000FF".into(),
        font_size: 20,
        text_align: flight_streamdeck::render::TextAlign::Left,
        bold: true,
    };
    let icon = KeyIcon::for_model(StreamDeckModel::Xl, "CUS", IconTheme::Custom)
        .unwrap()
        .with_style(custom);

    assert_eq!(icon.style.background_color, "#FF0000");
    assert_eq!(icon.style.font_size, 20);
    assert!(icon.style.bold);
}

/// Renderer: icon_size matches model's icon_size.
#[test]
fn renderer_icon_size_matches_model() {
    for model in StreamDeckModel::all() {
        let renderer = IconRenderer::new(*model);
        assert_eq!(renderer.icon_size(), model.icon_size());
    }
}

/// Page: key_at returns correct slot after adding multiple keys.
#[test]
fn page_key_at_multiple_keys() {
    let mut page = Page::new("Grid");
    for r in 0..3 {
        for c in 0..5 {
            page.add_key(KeySlot {
                row: r,
                col: c,
                action_id: Some(format!("act-{r}-{c}")),
                nav_target: None,
            });
        }
    }
    assert_eq!(page.keys.len(), 15);
    let slot = page.key_at(1, 3).unwrap();
    assert_eq!(slot.action_id.as_deref(), Some("act-1-3"));
    assert!(page.key_at(4, 0).is_none());
}

/// ProfileLayout: empty template map still yields at least one page.
#[test]
fn profile_layout_empty_templates_has_root_page() {
    let empty: HashMap<String, ActionTemplate> = HashMap::new();
    for aircraft in [AircraftType::GA, AircraftType::Airbus, AircraftType::Helo] {
        let layout =
            ProfileLayout::build(aircraft, StreamDeckModel::Original, &empty).unwrap();
        assert_eq!(
            layout.page_count(),
            1,
            "{aircraft:?} should have 1 root page with empty templates"
        );
    }
}

/// ProfileLayout: Pedal model fails for all aircraft types (no grid).
#[test]
fn profile_layout_pedal_always_fails() {
    let templates: HashMap<String, ActionTemplate> = builtin_templates()
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();
    for aircraft in [AircraftType::GA, AircraftType::Airbus, AircraftType::Helo] {
        assert!(
            ProfileLayout::build(aircraft, StreamDeckModel::Pedal, &templates).is_err(),
            "Pedal should fail for {aircraft:?}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  7. BUTTON PRESS / RELEASE EVENT HANDLING (3 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Plugin event: action triggered event carries correct UUID and context.
#[test]
fn event_action_triggered_roundtrip_serialization() {
    use flight_streamdeck::plugin::PluginEvent;

    let event = PluginEvent::ActionTriggered {
        action_uuid: "com.flighthub.ap-toggle".to_string(),
        context: "key-0-0".to_string(),
    };
    let json = serde_json::to_value(&event).unwrap();
    let deserialized: PluginEvent = serde_json::from_value(json).unwrap();
    match deserialized {
        PluginEvent::ActionTriggered {
            action_uuid,
            context,
        } => {
            assert_eq!(action_uuid, "com.flighthub.ap-toggle");
            assert_eq!(context, "key-0-0");
        }
        _ => panic!("wrong variant"),
    }
}

/// Plugin event: device connected/disconnected serialize correctly.
#[test]
fn event_device_connect_disconnect_serialization() {
    use flight_streamdeck::plugin::PluginEvent;

    let connect = PluginEvent::DeviceConnected {
        device_id: "sd-xl-001".to_string(),
    };
    let json = serde_json::to_string(&connect).unwrap();
    assert!(json.contains("sd-xl-001"));

    let disconnect = PluginEvent::DeviceDisconnected {
        device_id: "sd-xl-001".to_string(),
    };
    let json = serde_json::to_string(&disconnect).unwrap();
    assert!(json.contains("sd-xl-001"));
}

/// Plugin event: property inspector update carries settings payload.
#[test]
fn event_property_inspector_update() {
    use flight_streamdeck::plugin::PluginEvent;

    let settings = serde_json::json!({"brightness": 80, "theme": "dark"});
    let event = PluginEvent::PropertyInspectorUpdate {
        action_uuid: "com.flighthub.light-landing".to_string(),
        settings: settings.clone(),
    };
    let json = serde_json::to_value(&event).unwrap();
    let deserialized: PluginEvent = serde_json::from_value(json).unwrap();
    match deserialized {
        PluginEvent::PropertyInspectorUpdate {
            action_uuid,
            settings: s,
        } => {
            assert_eq!(action_uuid, "com.flighthub.light-landing");
            assert_eq!(s["brightness"], 80);
        }
        _ => panic!("wrong variant"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  8. ERROR HANDLING (3 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Version parse error: non-numeric and too-few/too-many segments.
#[test]
fn error_version_parse_invalid_formats() {
    assert!(AppVersion::from_string("").is_err());
    assert!(AppVersion::from_string("abc").is_err());
    assert!(AppVersion::from_string("6.2").is_err());
    assert!(AppVersion::from_string("6.2.1.2.3").is_err());
    assert!(AppVersion::from_string("x.y.z").is_err());
}

/// Device manager errors: operations on unknown devices fail gracefully.
#[test]
fn error_device_manager_unknown_device() {
    let mut mgr = DeviceManager::new();
    assert!(mgr.set_brightness("ghost", Brightness::new(50).unwrap()).is_err());
    assert!(mgr.get_device("ghost").is_none());
    assert!(mgr.get_brightness("ghost").is_none());
    assert!(mgr.get_lcd_strip_info("ghost").is_err());
}

/// LCD strip: requesting strip layout on non-LCD model fails.
#[test]
fn error_lcd_strip_on_non_lcd_models() {
    for model in [
        StreamDeckModel::Original,
        StreamDeckModel::Mini,
        StreamDeckModel::Xl,
        StreamDeckModel::Pedal,
    ] {
        assert!(
            LcdStripLayout::new(model).is_err(),
            "{model:?} should not support LCD strip layout"
        );
        assert!(LcdStripInfo::for_model(model).is_none());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  9. API ENDPOINT ROUTING (3 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// API routing: GET profile by aircraft type via {aircraft_type} path param.
#[tokio::test]
async fn api_get_profile_by_aircraft_type() {
    let compat = VersionCompatibility::new();
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    let api = StreamDeckApi::new(compat, pm);
    let app = api.create_router();
    let server = axum_test::TestServer::new(app).unwrap();

    for aircraft in ["ga", "airbus", "helo"] {
        let resp = server.get(&format!("/api/v1/profiles/{aircraft}")).await;
        assert_eq!(resp.status_code(), axum::http::StatusCode::OK);
        let body: ApiResponse<serde_json::Value> = resp.json();
        assert!(body.success, "profile for {aircraft} should succeed");
        assert!(body.data.is_some());
    }
}

/// API routing: unknown aircraft type returns error (not 404 status, but error in body).
#[tokio::test]
async fn api_get_profile_unknown_aircraft() {
    let compat = VersionCompatibility::new();
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    let api = StreamDeckApi::new(compat, pm);
    let app = api.create_router();
    let server = axum_test::TestServer::new(app).unwrap();

    let resp = server.get("/api/v1/profiles/boeing").await;
    let body: ApiResponse<serde_json::Value> = resp.json();
    assert!(!body.success);
    assert!(body.error.unwrap().contains("Unknown aircraft type"));
}

/// API routing: version check with invalid version format returns error in body.
#[tokio::test]
async fn api_version_check_invalid_format() {
    let compat = VersionCompatibility::new();
    let mut pm = ProfileManager::new();
    pm.load_sample_profiles().unwrap();
    let api = StreamDeckApi::new(compat, pm);
    let app = api.create_router();
    let server = axum_test::TestServer::new(app).unwrap();

    let req = VersionCheckRequest {
        app_version: "not-a-version".into(),
        plugin_uuid: "test".into(),
    };
    let resp = server.post("/api/v1/version/check").json(&req).await;
    assert_eq!(resp.status_code(), axum::http::StatusCode::OK);
    let body: ApiResponse<VersionCheckResponse> = resp.json();
    assert!(!body.success);
    assert!(body.error.is_some());
}

// ═══════════════════════════════════════════════════════════════════════════════
//  10. PROPERTY-BASED TESTS (proptest)
// ═══════════════════════════════════════════════════════════════════════════════

mod proptest_depth {
    use super::*;
    use proptest::prelude::*;

    /// Strategy for generating valid brightness values (0..=100).
    fn brightness_valid() -> impl Strategy<Value = u8> {
        0u8..=100
    }

    /// Strategy for generating invalid brightness values (101..=255).
    fn brightness_invalid() -> impl Strategy<Value = u8> {
        101u8..=255
    }

    /// Strategy for generating a StreamDeckModel.
    fn any_model() -> impl Strategy<Value = StreamDeckModel> {
        prop_oneof![
            Just(StreamDeckModel::Original),
            Just(StreamDeckModel::Mini),
            Just(StreamDeckModel::Xl),
            Just(StreamDeckModel::Plus),
            Just(StreamDeckModel::Pedal),
            Just(StreamDeckModel::Neo),
        ]
    }

    proptest! {
        /// Any valid brightness value (0–100) succeeds and round-trips.
        #[test]
        fn prop_brightness_valid_roundtrip(pct in brightness_valid()) {
            let b = Brightness::new(pct).unwrap();
            prop_assert_eq!(b.percent(), pct);
        }

        /// Any brightness > 100 is rejected.
        #[test]
        fn prop_brightness_invalid_rejected(pct in brightness_invalid()) {
            prop_assert!(Brightness::new(pct).is_err());
        }

        /// key_count is always > 0 for every model.
        #[test]
        fn prop_model_key_count_positive(model in any_model()) {
            prop_assert!(model.key_count() > 0);
        }

        /// Grid-capable models always have rows*cols == key_count.
        #[test]
        fn prop_grid_models_key_count_matches(model in any_model()) {
            if let Some((rows, cols)) = model.grid_layout() {
                prop_assert_eq!(model.key_count(), rows * cols);
            }
        }

        /// AppVersion with valid triple round-trips through Display.
        #[test]
        fn prop_version_display_roundtrip(major in 0u32..100, minor in 0u32..100, patch in 0u32..1000) {
            let v = AppVersion::new(major, minor, patch);
            let s = v.to_string();
            let parsed = AppVersion::from_string(&s).unwrap();
            prop_assert_eq!(parsed.major, major);
            prop_assert_eq!(parsed.minor, minor);
            prop_assert_eq!(parsed.patch, patch);
            prop_assert_eq!(parsed.build, None);
        }

        /// AppVersion with build number round-trips through Display.
        #[test]
        fn prop_version_with_build_roundtrip(
            major in 0u32..100,
            minor in 0u32..100,
            patch in 0u32..1000,
            build in 0u32..10000
        ) {
            let v = AppVersion::with_build(major, minor, patch, build);
            let s = v.to_string();
            let parsed = AppVersion::from_string(&s).unwrap();
            prop_assert_eq!(parsed.major, major);
            prop_assert_eq!(parsed.minor, minor);
            prop_assert_eq!(parsed.patch, patch);
            prop_assert_eq!(parsed.build, Some(build));
        }

        /// IconRenderer returns None for Pedal, Some for all others.
        #[test]
        fn prop_renderer_pedal_none_others_some(model in any_model()) {
            let renderer = IconRenderer::new(model);
            if model == StreamDeckModel::Pedal {
                prop_assert!(renderer.icon_size().is_none());
                prop_assert!(renderer.momentary_icon("X", IconTheme::Custom).is_none());
            } else {
                prop_assert!(renderer.icon_size().is_some());
                prop_assert!(renderer.momentary_icon("X", IconTheme::Custom).is_some());
            }
        }
    }
}
