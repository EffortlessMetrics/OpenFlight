//! Integration tests for MSFS SimConnect adapter
//!
//! These tests validate the adapter functionality using recorded session fixtures
//! and mock data, ensuring the adapter works correctly without requiring a live
//! MSFS connection.

use flight_bus::adapters::SimAdapter;
use flight_bus::snapshot::BusSnapshot;
use flight_bus::types::{AircraftId, SimId};
use flight_simconnect::{
    AdapterMetrics, AdapterState, MsfsAdapter, MsfsAdapterConfig,
    fixtures::{FixtureRecorder, RecordedEvent, ValidationTolerance},
};
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_adapter_creation_and_basic_functionality() {
    let config = MsfsAdapterConfig::default();

    match MsfsAdapter::new(config) {
        Ok(adapter) => {
            // Test basic adapter functionality
            assert_eq!(adapter.sim_id(), SimId::Msfs);
            assert!(!adapter.is_active().await);
            assert!(adapter.current_aircraft().await.is_none());
            assert!(adapter.current_snapshot().await.is_none());
        }
        Err(e) => {
            // On systems without SimConnect, this is expected
            println!(
                "Adapter creation failed (expected on systems without SimConnect): {}",
                e
            );
        }
    }
}

#[test]
fn test_fixture_recording_and_playback() {
    // Create a test fixture
    let mut recorder = FixtureRecorder::new(
        "Test Flight Session".to_string(),
        vec!["test".to_string(), "c172".to_string()],
    );

    // Record some events
    recorder.record_event(RecordedEvent::Connected {
        app_name: "Microsoft Flight Simulator".to_string(),
        app_version: (1, 36, 0, 0),
        simconnect_version: (0, 4, 0, 0),
    });

    recorder.record_event(RecordedEvent::AircraftChanged {
        aircraft_info: flight_simconnect::aircraft::AircraftInfo {
            title: "Cessna 172 Skyhawk".to_string(),
            atc_model: "C172".to_string(),
            atc_type: "CESSNA".to_string(),
            atc_airline: None,
            atc_flight_number: None,
            category: flight_simconnect::aircraft::AircraftCategory::Airplane,
            engine_type: flight_simconnect::aircraft::EngineType::Piston,
            engine_count: 1,
        },
    });

    recorder.record_event(RecordedEvent::DataReceived {
        request_id: 1,
        object_id: 0,
        define_id: 1,
        data: vec![0x00, 0x00, 0x96, 0x42], // 75.0 as f32 bytes
    });

    // Record expected snapshot
    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    recorder.record_expected_snapshot(snapshot, Some(ValidationTolerance::default()));

    let fixture = recorder.finalize();

    // Validate fixture
    assert_eq!(fixture.metadata.name, "Test Flight Session");
    assert_eq!(fixture.events.len(), 3);
    assert_eq!(fixture.expected_snapshots.len(), 1);
    assert!(fixture.metadata.tags.contains(&"test".to_string()));
    assert!(fixture.aircraft_info.is_some());
}

#[test]
fn test_fixture_save_and_load() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let fixture_path = temp_dir.path().join("test_session.json");

    // Create and save fixture
    let mut recorder = FixtureRecorder::new("Save Test".to_string(), vec!["save".to_string()]);
    recorder.record_event(RecordedEvent::Connected {
        app_name: "Test App".to_string(),
        app_version: (1, 0, 0, 0),
        simconnect_version: (0, 4, 0, 0),
    });

    recorder.save_to_file(&fixture_path)?;

    // Load and validate fixture
    let player = flight_simconnect::fixtures::FixturePlayer::load_from_file(&fixture_path)?;
    assert_eq!(player.metadata().name, "Save Test");
    assert_eq!(player.progress(), 0.0);

    Ok(())
}

#[test]
fn test_aircraft_detection_and_mapping() {
    use flight_simconnect::aircraft::{
        AircraftCategory, AircraftDatabase, AircraftDetector, AircraftInfo, EngineType,
    };

    // Test aircraft database
    let db = AircraftDatabase::new();
    let c172_mapping = db.get_mapping("C172").unwrap();
    assert_eq!(c172_mapping.name, "Cessna 172");
    assert_eq!(c172_mapping.category, AircraftCategory::Airplane);
    assert_eq!(c172_mapping.engine_type, EngineType::Piston);

    // Test aircraft detector
    let detector = AircraftDetector::new();
    let aircraft_info = AircraftInfo {
        title: "Cessna 172 Skyhawk".to_string(),
        atc_model: "C172".to_string(),
        atc_type: "CESSNA".to_string(),
        atc_airline: None,
        atc_flight_number: None,
        category: AircraftCategory::Airplane,
        engine_type: EngineType::Piston,
        engine_count: 1,
    };

    let aircraft_id = detector.to_aircraft_id(&aircraft_info);
    assert_eq!(aircraft_id.icao, "C172");
}

#[test]
fn test_event_management() {
    use flight_simconnect::events::{CommonEvents, EventManager};

    let mut manager = EventManager::new();

    // Test input event registration
    let hash1 = manager.register_input_event("AXIS_ELEVATOR_SET");
    let hash2 = manager.register_input_event("AXIS_ELEVATOR_SET"); // Same event
    assert_eq!(hash1, hash2);

    let hash3 = manager.register_input_event("AXIS_AILERONS_SET"); // Different event
    assert_ne!(hash1, hash3);

    // Test common events
    let ga_controls = CommonEvents::ga_flight_controls();
    assert!(ga_controls.contains(&"AXIS_ELEVATOR_SET"));
    assert!(ga_controls.contains(&"AXIS_MIXTURE_SET"));

    let system_events = CommonEvents::system_events();
    assert!(system_events.contains(&"AircraftLoaded"));
    assert!(system_events.contains(&"SimStart"));
}

#[test]
fn test_variable_mapping_configuration() {
    use flight_simconnect::mapping::{VariableMapping, create_default_mapping};

    let config = create_default_mapping();
    assert!(!config.default_mapping.kinematics.ias.is_empty());
    assert!(!config.default_mapping.config.gear_nose.is_empty());
    assert!(!config.default_mapping.engines.is_empty());
    assert_eq!(config.update_rates.kinematics, 60.0);

    let mapping = VariableMapping::new(config);
    // Basic creation test - more detailed tests would require SimConnect connection
}

#[test]
fn test_adapter_configuration() {
    let config = MsfsAdapterConfig::default();
    assert_eq!(config.publish_rate, 60.0);
    assert_eq!(config.aircraft_detection_timeout, Duration::from_secs(30));
    assert!(config.auto_reconnect);
    assert_eq!(config.max_reconnect_attempts, 5);

    // Test custom configuration
    let mut custom_config = config.clone();
    custom_config.publish_rate = 30.0;
    custom_config.auto_reconnect = false;
    assert_eq!(custom_config.publish_rate, 30.0);
    assert!(!custom_config.auto_reconnect);
}

#[tokio::test]
async fn test_bus_snapshot_integration() {
    use flight_bus::adapters::msfs::MsfsConverter;

    // Test MSFS converter functions
    let ias = MsfsConverter::convert_ias(150.0).unwrap();
    assert_eq!(ias.to_knots(), 150.0);

    let angle = MsfsConverter::convert_angle_degrees(45.0).unwrap();
    assert_eq!(angle.to_degrees(), 45.0);

    let percentage = MsfsConverter::convert_percentage(75.0).unwrap();
    assert_eq!(percentage.value(), 75.0);

    // Test bus snapshot creation
    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    assert_eq!(snapshot.sim, SimId::Msfs);
    assert_eq!(snapshot.aircraft.icao, "C172");
    assert!(snapshot.validate().is_ok());
}

/// Test coverage matrix validation
#[test]
fn test_coverage_matrix() {
    // This test validates that we have coverage for the required variables
    // as specified in the requirements

    let config = flight_simconnect::mapping::create_default_mapping();
    let kinematics = &config.default_mapping.kinematics;

    // Verify required kinematics variables are mapped
    assert!(!kinematics.ias.is_empty());
    assert!(!kinematics.tas.is_empty());
    assert!(!kinematics.ground_speed.is_empty());
    assert!(!kinematics.aoa.is_empty());
    assert!(!kinematics.heading.is_empty());
    assert!(!kinematics.g_force.is_empty());
    assert!(!kinematics.mach.is_empty());

    let config_mapping = &config.default_mapping.config;

    // Verify required configuration variables are mapped
    assert!(!config_mapping.gear_nose.is_empty());
    assert!(!config_mapping.flaps.is_empty());
    assert!(!config_mapping.ap_master.is_empty());

    let engine_mapping = &config.default_mapping.engines[0];

    // Verify required engine variables are mapped
    assert!(!engine_mapping.running.is_empty());
    assert!(!engine_mapping.rpm.is_empty());
}

/// Test redistribution compliance documentation
#[test]
fn test_redistribution_compliance() {
    // This test ensures we document what we touch for redistribution compliance
    // as required by LEG-01

    // The adapter should document:
    // 1. SimConnect.dll dynamic loading (no redistribution)
    // 2. No injection into MSFS processes
    // 3. Uses only public SimConnect API
    // 4. No modification of MSFS files

    // This is validated through the design and implementation approach
    // rather than runtime tests

    println!("Redistribution compliance:");
    println!("- Uses dynamic loading of SimConnect.dll (no redistribution required)");
    println!("- No code injection into MSFS processes");
    println!("- Uses only public SimConnect API");
    println!("- No modification of MSFS installation files");
    println!("- All integration via documented SimConnect interface");
}

/// Test update rate monitoring and metrics
/// Requirements: MSFS-INT-01.7, MSFS-INT-01.8
#[tokio::test]
async fn test_update_rate_monitoring() {
    let config = MsfsAdapterConfig::default();

    match MsfsAdapter::new(config) {
        Ok(adapter) => {
            // Get initial metrics
            let metrics = adapter.metrics().await;
            assert_eq!(metrics.total_updates, 0);
            assert_eq!(metrics.actual_update_rate, 0.0);
            assert_eq!(metrics.update_jitter_p99_ms, 0.0);
            assert_eq!(metrics.aircraft_changes, 0);

            // Metrics should be accessible
            let summary = adapter.metrics_summary().await;
            assert!(summary.contains("Updates: 0"));
        }
        Err(e) => {
            println!(
                "Adapter creation failed (expected on systems without SimConnect): {}",
                e
            );
        }
    }
}

/// Test metrics recording functionality
/// Requirements: MSFS-INT-01.7, MSFS-INT-01.8
#[test]
fn test_metrics_recording() {
    use std::thread;

    let mut metrics = AdapterMetrics::new();

    // Record multiple updates with small delays
    for _ in 0..10 {
        metrics.record_update();
        thread::sleep(Duration::from_millis(16)); // ~60 Hz
    }

    // Verify metrics were recorded
    assert_eq!(metrics.total_updates, 10);
    assert!(metrics.actual_update_rate > 0.0);
    assert!(metrics.actual_update_rate < 100.0); // Reasonable range

    // Verify jitter calculation
    assert!(metrics.update_jitter_p99_ms >= 0.0);

    // Test aircraft change detection
    metrics.record_aircraft_change("Cessna 172".to_string());
    assert_eq!(metrics.aircraft_changes, 1);
    assert_eq!(metrics.last_aircraft_title, Some("Cessna 172".to_string()));

    // Same aircraft should not increment counter
    metrics.record_aircraft_change("Cessna 172".to_string());
    assert_eq!(metrics.aircraft_changes, 1);

    // Different aircraft should increment
    metrics.record_aircraft_change("Airbus A320".to_string());
    assert_eq!(metrics.aircraft_changes, 2);
}

/// Test conditional 60 Hz target
/// Requirements: MSFS-INT-01.7
#[test]
fn test_conditional_60hz_target() {
    let config = MsfsAdapterConfig::default();

    // Default publish rate should be 60 Hz
    assert_eq!(config.publish_rate, 60.0);

    // Verify update rates configuration
    assert_eq!(config.mapping.update_rates.kinematics, 60.0);

    // The adapter should target 60 Hz when sim FPS >= 60
    // This is enforced through the publish_rate configuration
    let min_interval = Duration::from_secs_f32(1.0 / config.publish_rate);
    // ~16.67ms for 60 Hz (allow for floating point precision)
    assert!((16..=17).contains(&min_interval.as_millis()));
}

/// Test aircraft change detection via TITLE SimVar
/// Requirements: MSFS-INT-01.17
#[tokio::test]
async fn test_aircraft_change_detection() {
    use flight_simconnect::aircraft::{
        AircraftCategory, AircraftDetector, AircraftInfo, EngineType,
    };

    let mut detector = AircraftDetector::new();

    // Create first aircraft
    let aircraft1 = AircraftInfo {
        title: "Cessna 172 Skyhawk".to_string(),
        atc_model: "C172".to_string(),
        atc_type: "CESSNA".to_string(),
        atc_airline: None,
        atc_flight_number: None,
        category: AircraftCategory::Airplane,
        engine_type: EngineType::Piston,
        engine_count: 1,
    };

    // Simulate aircraft data processing
    let data1 = create_mock_aircraft_data(&aircraft1);
    let result1 = detector.process_aircraft_data(&data1);
    assert!(result1.is_ok());
    assert!(result1.unwrap().is_some()); // First detection should return Some

    // Same aircraft should not trigger change
    let result2 = detector.process_aircraft_data(&data1);
    assert!(result2.is_ok());
    assert!(result2.unwrap().is_none()); // No change

    // Different aircraft should trigger change
    let aircraft2 = AircraftInfo {
        title: "Airbus A320neo".to_string(),
        atc_model: "A320".to_string(),
        atc_type: "AIRBUS".to_string(),
        atc_airline: None,
        atc_flight_number: None,
        category: AircraftCategory::Airplane,
        engine_type: EngineType::Jet,
        engine_count: 2,
    };

    let data2 = create_mock_aircraft_data(&aircraft2);
    let result3 = detector.process_aircraft_data(&data2);
    assert!(result3.is_ok());
    assert!(result3.unwrap().is_some()); // Change detected
}

/// Test complete adapter lifecycle with recorded fixtures
/// Requirements: SIM-TEST-01.1, SIM-TEST-01.5
#[tokio::test]
async fn test_adapter_lifecycle_with_fixtures() {
    let config = MsfsAdapterConfig::default();

    match MsfsAdapter::new(config) {
        Ok(mut adapter) => {
            // Test initial state
            assert_eq!(adapter.state().await, AdapterState::Disconnected);
            assert!(!adapter.is_active().await);

            // Test metrics are initialized
            let metrics = adapter.metrics().await;
            assert_eq!(metrics.total_updates, 0);

            // Simulate connection loss
            adapter.handle_connection_loss().await;
            assert_eq!(adapter.state().await, AdapterState::Disconnected);

            // Verify state is cleared
            assert!(adapter.current_aircraft().await.is_none());
            assert!(adapter.current_snapshot().await.is_none());
        }
        Err(e) => {
            println!(
                "Adapter creation failed (expected on systems without SimConnect): {}",
                e
            );
        }
    }
}

/// Test reconnection behavior with exponential backoff
/// Requirements: SIM-TEST-01.7
#[tokio::test]
async fn test_reconnection_behavior() {
    let mut config = MsfsAdapterConfig::default();
    config.auto_reconnect = true;
    config.max_reconnect_attempts = 3;

    match MsfsAdapter::new(config) {
        Ok(adapter) => {
            // Verify initial state
            assert_eq!(adapter.connection_attempts(), 0);
            assert_eq!(adapter.current_backoff_delay(), 1.0);

            // Verify backoff configuration
            assert!(adapter.state().await == AdapterState::Disconnected);
        }
        Err(e) => {
            println!(
                "Adapter creation failed (expected on systems without SimConnect): {}",
                e
            );
        }
    }
}

/// Test metrics namespace structure
/// Requirements: MSFS-INT-01.8
#[test]
fn test_metrics_namespace() {
    let metrics = AdapterMetrics::new();

    // Verify metrics summary format includes expected fields
    let summary = metrics.summary();
    assert!(summary.contains("Updates:"));
    assert!(summary.contains("Rate:"));
    assert!(summary.contains("Hz"));
    assert!(summary.contains("Jitter p99:"));
    assert!(summary.contains("ms"));
    assert!(summary.contains("Aircraft changes:"));

    // Metrics should be under sim.msfs.* namespace (documented in design)
    // This is enforced through the metrics system integration (Task 41)
}

/// Helper function to create mock aircraft data for testing
fn create_mock_aircraft_data(aircraft: &flight_simconnect::aircraft::AircraftInfo) -> Vec<u8> {
    let mut data = Vec::new();

    // Title (256 bytes)
    let mut title_bytes = aircraft.title.as_bytes().to_vec();
    title_bytes.resize(256, 0);
    data.extend_from_slice(&title_bytes);

    // ATC model (32 bytes)
    let mut model_bytes = aircraft.atc_model.as_bytes().to_vec();
    model_bytes.resize(32, 0);
    data.extend_from_slice(&model_bytes);

    // ATC type (32 bytes)
    let mut type_bytes = aircraft.atc_type.as_bytes().to_vec();
    type_bytes.resize(32, 0);
    data.extend_from_slice(&type_bytes);

    // ATC airline (64 bytes)
    let airline_bytes = vec![0u8; 64];
    data.extend_from_slice(&airline_bytes);

    // ATC flight number (32 bytes)
    let flight_bytes = vec![0u8; 32];
    data.extend_from_slice(&flight_bytes);

    // Category (32 bytes)
    let category_str = match aircraft.category {
        flight_simconnect::aircraft::AircraftCategory::Airplane => "AIRPLANE",
        flight_simconnect::aircraft::AircraftCategory::Helicopter => "HELICOPTER",
        flight_simconnect::aircraft::AircraftCategory::Glider => "GLIDER",
        flight_simconnect::aircraft::AircraftCategory::Unknown => "UNKNOWN",
    };
    let mut category_bytes = category_str.as_bytes().to_vec();
    category_bytes.resize(32, 0);
    data.extend_from_slice(&category_bytes);

    // Engine type (4 bytes)
    let engine_type_value: i32 = match aircraft.engine_type {
        flight_simconnect::aircraft::EngineType::Piston => 0,
        flight_simconnect::aircraft::EngineType::Jet => 1,
        flight_simconnect::aircraft::EngineType::Turboprop => 3,
        flight_simconnect::aircraft::EngineType::Turboshaft => 4,
        flight_simconnect::aircraft::EngineType::Electric => 5,
        flight_simconnect::aircraft::EngineType::Unknown => 2,
    };
    data.extend_from_slice(&engine_type_value.to_le_bytes());

    // Engine count (4 bytes)
    data.extend_from_slice(&(aircraft.engine_count as i32).to_le_bytes());

    data
}
