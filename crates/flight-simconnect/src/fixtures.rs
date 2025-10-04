// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Session fixtures for testing and validation
//!
//! Provides recording and playback capabilities for SimConnect sessions,
//! enabling integration tests with recorded session data and validation
//! of adapter behavior without requiring a live MSFS connection.

use flight_bus::snapshot::BusSnapshot;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Session fixture containing recorded SimConnect data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFixture {
    /// Fixture metadata
    pub metadata: FixtureMetadata,
    /// Recorded events in chronological order
    pub events: Vec<FixtureEvent>,
    /// Aircraft information
    pub aircraft_info: Option<crate::aircraft::AircraftInfo>,
    /// Expected bus snapshots for validation
    pub expected_snapshots: Vec<TimestampedSnapshot>,
}

/// Fixture metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureMetadata {
    /// Fixture name/description
    pub name: String,
    /// Recording timestamp
    pub recorded_at: SystemTime,
    /// MSFS version
    pub msfs_version: Option<String>,
    /// Aircraft used in recording
    pub aircraft: Option<String>,
    /// Recording duration
    pub duration: Duration,
    /// Number of events recorded
    pub event_count: usize,
    /// Tags for categorization
    pub tags: Vec<String>,
}

/// Recorded SimConnect event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureEvent {
    /// Timestamp relative to recording start
    pub timestamp: Duration,
    /// Event type and data
    pub event: RecordedEvent,
}

/// Types of recorded events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordedEvent {
    /// Connection established
    Connected {
        app_name: String,
        app_version: (u32, u32, u32, u32),
        simconnect_version: (u32, u32, u32, u32),
    },
    /// Data received from SimConnect
    DataReceived {
        request_id: u32,
        object_id: u32,
        define_id: u32,
        data: Vec<u8>,
    },
    /// Event received from SimConnect
    EventReceived {
        group_id: u32,
        event_id: u32,
        data: u32,
    },
    /// Exception occurred
    Exception {
        exception: u32,
        send_id: u32,
        index: u32,
    },
    /// Aircraft loaded/changed
    AircraftChanged {
        aircraft_info: crate::aircraft::AircraftInfo,
    },
    /// Connection lost
    Disconnected,
}

/// Timestamped bus snapshot for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedSnapshot {
    /// Timestamp relative to recording start
    pub timestamp: Duration,
    /// Bus snapshot
    pub snapshot: BusSnapshot,
    /// Tolerance for validation
    pub tolerance: ValidationTolerance,
}

/// Validation tolerance for fixture comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationTolerance {
    /// Speed tolerance (knots)
    pub speed: f32,
    /// Angle tolerance (degrees)
    pub angle: f32,
    /// Percentage tolerance
    pub percentage: f32,
    /// G-force tolerance
    pub g_force: f32,
    /// Position tolerance (degrees)
    pub position: f64,
}

impl Default for ValidationTolerance {
    fn default() -> Self {
        Self {
            speed: 1.0,      // 1 knot
            angle: 0.5,      // 0.5 degrees
            percentage: 1.0, // 1%
            g_force: 0.1,    // 0.1 G
            position: 0.001, // ~100m at equator
        }
    }
}

/// Fixture error types
#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Fixture not found: {0}")]
    NotFound(String),
    #[error("Invalid fixture format: {0}")]
    InvalidFormat(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    #[error("Playback error: {0}")]
    PlaybackError(String),
}

/// Fixture recorder for capturing SimConnect sessions
pub struct FixtureRecorder {
    /// Recording metadata
    metadata: FixtureMetadata,
    /// Recorded events
    events: Vec<FixtureEvent>,
    /// Recording start time
    start_time: SystemTime,
    /// Current aircraft info
    current_aircraft: Option<crate::aircraft::AircraftInfo>,
    /// Expected snapshots
    expected_snapshots: Vec<TimestampedSnapshot>,
}

impl FixtureRecorder {
    /// Create a new fixture recorder
    pub fn new(name: String, tags: Vec<String>) -> Self {
        let start_time = SystemTime::now();
        
        Self {
            metadata: FixtureMetadata {
                name,
                recorded_at: start_time,
                msfs_version: None,
                aircraft: None,
                duration: Duration::ZERO,
                event_count: 0,
                tags,
            },
            events: Vec::new(),
            start_time,
            current_aircraft: None,
            expected_snapshots: Vec::new(),
        }
    }

    /// Record a SimConnect event
    pub fn record_event(&mut self, event: RecordedEvent) {
        let timestamp = self.start_time.elapsed().unwrap_or(Duration::ZERO);
        
        // Update metadata based on event
        match &event {
            RecordedEvent::Connected { app_name, .. } => {
                if app_name.contains("Flight Simulator") {
                    // Extract MSFS version if possible
                    self.metadata.msfs_version = Some(app_name.clone());
                }
            }
            RecordedEvent::AircraftChanged { aircraft_info } => {
                self.current_aircraft = Some(aircraft_info.clone());
                self.metadata.aircraft = Some(aircraft_info.title.clone());
            }
            _ => {}
        }

        self.events.push(FixtureEvent { timestamp, event });
        self.metadata.event_count = self.events.len();
        self.metadata.duration = timestamp;
    }

    /// Record an expected bus snapshot for validation
    pub fn record_expected_snapshot(&mut self, snapshot: BusSnapshot, tolerance: Option<ValidationTolerance>) {
        let timestamp = self.start_time.elapsed().unwrap_or(Duration::ZERO);
        
        self.expected_snapshots.push(TimestampedSnapshot {
            timestamp,
            snapshot,
            tolerance: tolerance.unwrap_or_default(),
        });
    }

    /// Finalize recording and create fixture
    pub fn finalize(mut self) -> SessionFixture {
        self.metadata.duration = self.start_time.elapsed().unwrap_or(Duration::ZERO);
        self.metadata.event_count = self.events.len();

        SessionFixture {
            metadata: self.metadata,
            events: self.events,
            aircraft_info: self.current_aircraft,
            expected_snapshots: self.expected_snapshots,
        }
    }

    /// Save fixture to file
    pub fn save_to_file<P: AsRef<Path>>(self, path: P) -> Result<(), FixtureError> {
        let fixture = self.finalize();
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &fixture)?;
        Ok(())
    }
}

/// Fixture player for replaying recorded sessions
pub struct FixturePlayer {
    /// Loaded fixture
    fixture: SessionFixture,
    /// Current playback position
    position: usize,
    /// Playback start time
    start_time: SystemTime,
    /// Playback speed multiplier
    speed: f64,
    /// Event callbacks
    event_callbacks: HashMap<String, Box<dyn Fn(&RecordedEvent) + Send + Sync>>,
}

impl FixturePlayer {
    /// Load fixture from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, FixtureError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let fixture: SessionFixture = serde_json::from_reader(reader)?;
        
        Ok(Self {
            fixture,
            position: 0,
            start_time: SystemTime::now(),
            speed: 1.0,
            event_callbacks: HashMap::new(),
        })
    }

    /// Create player from fixture
    pub fn new(fixture: SessionFixture) -> Self {
        Self {
            fixture,
            position: 0,
            start_time: SystemTime::now(),
            speed: 1.0,
            event_callbacks: HashMap::new(),
        }
    }

    /// Set playback speed (1.0 = normal, 2.0 = 2x speed, etc.)
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.max(0.1).min(10.0); // Clamp to reasonable range
    }

    /// Add event callback
    pub fn add_event_callback<F>(&mut self, name: String, callback: F)
    where
        F: Fn(&RecordedEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.insert(name, Box::new(callback));
    }

    /// Start playback
    pub fn start(&mut self) {
        self.start_time = SystemTime::now();
        self.position = 0;
        info!("Started fixture playback: {}", self.fixture.metadata.name);
    }

    /// Update playback (call regularly to process events)
    pub fn update(&mut self) -> Result<bool, FixtureError> {
        if self.position >= self.fixture.events.len() {
            return Ok(false); // Playback complete
        }

        let elapsed = self.start_time.elapsed().unwrap_or(Duration::ZERO);
        let scaled_elapsed = Duration::from_secs_f64(elapsed.as_secs_f64() * self.speed);

        // Process all events that should have occurred by now
        while self.position < self.fixture.events.len() {
            let event = &self.fixture.events[self.position];
            
            if event.timestamp <= scaled_elapsed {
                // Fire event callbacks
                for callback in self.event_callbacks.values() {
                    callback(&event.event);
                }
                
                debug!("Played fixture event at {:?}: {:?}", event.timestamp, event.event);
                self.position += 1;
            } else {
                break;
            }
        }

        Ok(self.position < self.fixture.events.len())
    }

    /// Get fixture metadata
    pub fn metadata(&self) -> &FixtureMetadata {
        &self.fixture.metadata
    }

    /// Get current playback progress (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.fixture.events.is_empty() {
            1.0
        } else {
            self.position as f64 / self.fixture.events.len() as f64
        }
    }

    /// Validate bus snapshots against expected values
    pub fn validate_snapshots(&self, actual_snapshots: &[TimestampedSnapshot]) -> Result<(), FixtureError> {
        if self.fixture.expected_snapshots.is_empty() {
            warn!("No expected snapshots in fixture for validation");
            return Ok(());
        }

        let mut validation_errors = Vec::new();

        for expected in &self.fixture.expected_snapshots {
            // Find closest actual snapshot by timestamp
            let closest_actual = actual_snapshots
                .iter()
                .min_by_key(|actual| {
                    let diff = if actual.timestamp > expected.timestamp {
                        actual.timestamp - expected.timestamp
                    } else {
                        expected.timestamp - actual.timestamp
                    };
                    diff.as_millis()
                });

            if let Some(actual) = closest_actual {
                if let Err(e) = validate_snapshot_match(&expected.snapshot, &actual.snapshot, &expected.tolerance) {
                    validation_errors.push(format!("At {:?}: {}", expected.timestamp, e));
                }
            } else {
                validation_errors.push(format!("No actual snapshot found near {:?}", expected.timestamp));
            }
        }

        if !validation_errors.is_empty() {
            return Err(FixtureError::ValidationFailed(validation_errors.join("; ")));
        }

        info!("Snapshot validation passed for {} expected snapshots", self.fixture.expected_snapshots.len());
        Ok(())
    }
}

/// Validate that two snapshots match within tolerance
fn validate_snapshot_match(
    expected: &BusSnapshot,
    actual: &BusSnapshot,
    tolerance: &ValidationTolerance,
) -> Result<(), String> {
    // Validate kinematics
    let exp_kin = &expected.kinematics;
    let act_kin = &actual.kinematics;

    if (exp_kin.ias.to_knots() - act_kin.ias.to_knots()).abs() > tolerance.speed {
        return Err(format!(
            "IAS mismatch: expected {}, actual {}, tolerance {}",
            exp_kin.ias.to_knots(),
            act_kin.ias.to_knots(),
            tolerance.speed
        ));
    }

    if (exp_kin.heading.to_degrees() - act_kin.heading.to_degrees()).abs() > tolerance.angle {
        return Err(format!(
            "Heading mismatch: expected {}, actual {}, tolerance {}",
            exp_kin.heading.to_degrees(),
            act_kin.heading.to_degrees(),
            tolerance.angle
        ));
    }

    if (exp_kin.g_force.value() - act_kin.g_force.value()).abs() > tolerance.g_force {
        return Err(format!(
            "G-force mismatch: expected {}, actual {}, tolerance {}",
            exp_kin.g_force.value(),
            act_kin.g_force.value(),
            tolerance.g_force
        ));
    }

    // Validate configuration
    if expected.config.gear != actual.config.gear {
        return Err(format!(
            "Gear state mismatch: expected {:?}, actual {:?}",
            expected.config.gear,
            actual.config.gear
        ));
    }

    if (expected.config.flaps.value() - actual.config.flaps.value()).abs() > tolerance.percentage {
        return Err(format!(
            "Flaps mismatch: expected {}%, actual {}%, tolerance {}%",
            expected.config.flaps.value(),
            actual.config.flaps.value(),
            tolerance.percentage
        ));
    }

    // Additional validations can be added here...

    Ok(())
}

/// Fixture library for managing collections of fixtures
pub struct FixtureLibrary {
    /// Library root directory
    root_dir: PathBuf,
    /// Loaded fixtures (name -> fixture)
    fixtures: HashMap<String, SessionFixture>,
}

impl FixtureLibrary {
    /// Create a new fixture library
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
            fixtures: HashMap::new(),
        }
    }

    /// Load all fixtures from the library directory
    pub fn load_all(&mut self) -> Result<usize, FixtureError> {
        if !self.root_dir.exists() {
            std::fs::create_dir_all(&self.root_dir)?;
            return Ok(0);
        }

        let mut loaded_count = 0;

        for entry in std::fs::read_dir(&self.root_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_fixture(&path) {
                    Ok(fixture) => {
                        let name = fixture.metadata.name.clone();
                        self.fixtures.insert(name, fixture);
                        loaded_count += 1;
                    }
                    Err(e) => {
                        warn!("Failed to load fixture {:?}: {}", path, e);
                    }
                }
            }
        }

        info!("Loaded {} fixtures from library", loaded_count);
        Ok(loaded_count)
    }

    /// Load a specific fixture
    pub fn load_fixture<P: AsRef<Path>>(&self, path: P) -> Result<SessionFixture, FixtureError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let fixture: SessionFixture = serde_json::from_reader(reader)?;
        Ok(fixture)
    }

    /// Get fixture by name
    pub fn get_fixture(&self, name: &str) -> Option<&SessionFixture> {
        self.fixtures.get(name)
    }

    /// List available fixtures
    pub fn list_fixtures(&self) -> Vec<&str> {
        self.fixtures.keys().map(|s| s.as_str()).collect()
    }

    /// Find fixtures by tags
    pub fn find_by_tags(&self, tags: &[String]) -> Vec<&SessionFixture> {
        self.fixtures
            .values()
            .filter(|fixture| {
                tags.iter().any(|tag| fixture.metadata.tags.contains(tag))
            })
            .collect()
    }

    /// Find fixtures by aircraft
    pub fn find_by_aircraft(&self, aircraft: &str) -> Vec<&SessionFixture> {
        self.fixtures
            .values()
            .filter(|fixture| {
                fixture.metadata.aircraft.as_ref()
                    .map(|a| a.contains(aircraft))
                    .unwrap_or(false)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::types::{AircraftId, SimId};
    use tempfile::TempDir;

    #[test]
    fn test_fixture_recorder() {
        let mut recorder = FixtureRecorder::new(
            "Test Recording".to_string(),
            vec!["test".to_string(), "c172".to_string()],
        );

        // Record some events
        recorder.record_event(RecordedEvent::Connected {
            app_name: "Microsoft Flight Simulator".to_string(),
            app_version: (1, 36, 0, 0),
            simconnect_version: (0, 4, 0, 0),
        });

        recorder.record_event(RecordedEvent::DataReceived {
            request_id: 1,
            object_id: 0,
            define_id: 1,
            data: vec![1, 2, 3, 4],
        });

        let fixture = recorder.finalize();
        assert_eq!(fixture.metadata.name, "Test Recording");
        assert_eq!(fixture.events.len(), 2);
        assert!(fixture.metadata.tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_fixture_save_load() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let fixture_path = temp_dir.path().join("test_fixture.json");

        // Create and save fixture
        let mut recorder = FixtureRecorder::new("Test".to_string(), vec!["test".to_string()]);
        recorder.record_event(RecordedEvent::Connected {
            app_name: "Test App".to_string(),
            app_version: (1, 0, 0, 0),
            simconnect_version: (0, 4, 0, 0),
        });

        recorder.save_to_file(&fixture_path)?;

        // Load fixture
        let player = FixturePlayer::load_from_file(&fixture_path)?;
        assert_eq!(player.metadata().name, "Test");
        assert_eq!(player.fixture.events.len(), 1);

        Ok(())
    }

    #[test]
    fn test_fixture_player() {
        let mut recorder = FixtureRecorder::new("Test".to_string(), vec![]);
        recorder.record_event(RecordedEvent::Connected {
            app_name: "Test".to_string(),
            app_version: (1, 0, 0, 0),
            simconnect_version: (0, 4, 0, 0),
        });

        let fixture = recorder.finalize();
        let mut player = FixturePlayer::new(fixture);

        // Add callback
        let mut callback_called = false;
        player.add_event_callback("test".to_string(), move |_event| {
            // Callback would be called during playback
        });

        player.start();
        assert_eq!(player.progress(), 0.0);

        // Simulate immediate playback (events at timestamp 0)
        let _ = player.update();
        // Progress should advance after processing events
    }

    #[test]
    fn test_validation_tolerance() {
        let tolerance = ValidationTolerance::default();
        assert_eq!(tolerance.speed, 1.0);
        assert_eq!(tolerance.angle, 0.5);
        assert_eq!(tolerance.percentage, 1.0);
        assert_eq!(tolerance.g_force, 0.1);
        assert_eq!(tolerance.position, 0.001);
    }

    #[test]
    fn test_fixture_library() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let mut library = FixtureLibrary::new(temp_dir.path());

        // Should start empty
        assert_eq!(library.load_all()?, 0);
        assert_eq!(library.list_fixtures().len(), 0);

        // Create a test fixture file
        let mut recorder = FixtureRecorder::new("Library Test".to_string(), vec!["library".to_string()]);
        recorder.record_event(RecordedEvent::Connected {
            app_name: "Test".to_string(),
            app_version: (1, 0, 0, 0),
            simconnect_version: (0, 4, 0, 0),
        });

        let fixture_path = temp_dir.path().join("library_test.json");
        recorder.save_to_file(&fixture_path)?;

        // Load fixtures
        assert_eq!(library.load_all()?, 1);
        assert_eq!(library.list_fixtures().len(), 1);
        assert!(library.get_fixture("Library Test").is_some());

        // Test tag search
        let tagged_fixtures = library.find_by_tags(&vec!["library".to_string()]);
        assert_eq!(tagged_fixtures.len(), 1);

        Ok(())
    }
}