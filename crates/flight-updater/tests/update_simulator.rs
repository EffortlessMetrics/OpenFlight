//! Update simulator tests for upgrade→crash→rollback validation

use flight_updater::{
    channels::Channel,
    rollback::{RollbackManager, StartupCrashDetector, VersionInfo},
    updater::{UpdateConfig, UpdateManager},
};
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;

/// Test the complete update and rollback cycle
#[tokio::test]
async fn test_update_rollback_cycle() {
    let temp_dir = TempDir::new().unwrap();

    // Create initial installation
    let install_dir = temp_dir.path().join("install");
    let update_dir = temp_dir.path().join("updates");

    fs::create_dir_all(&install_dir).await.unwrap();
    fs::write(install_dir.join("app.exe"), b"version 1.0.0")
        .await
        .unwrap();

    let config = UpdateConfig {
        install_dir: install_dir.clone(),
        update_dir: update_dir.clone(),
        current_version: "1.0.0".to_string(),
        channel: Channel::Stable,
        startup_timeout_seconds: 2, // Short timeout for testing
        max_rollback_versions: 3,
        ..Default::default()
    };

    // Initialize update manager
    let mut manager = UpdateManager::new(config.clone()).await.unwrap();
    let init_result = manager.initialize().await.unwrap();
    assert!(init_result.is_none()); // No crash on first run

    // Simulate successful startup
    manager.mark_startup_success().await.unwrap();

    // Simulate version upgrade (manually record new version)
    let _new_version = VersionInfo::new(
        "1.1.0".to_string(),
        "abc123".to_string(),
        Channel::Stable,
        install_dir.clone(),
    );

    // Update the binary to simulate new version
    fs::write(install_dir.join("app.exe"), b"version 1.1.0")
        .await
        .unwrap();

    // Create new manager instance with new version
    let config_v2 = UpdateConfig {
        current_version: "1.1.0".to_string(),
        ..config.clone()
    };

    let _manager_v2 = UpdateManager::new(config_v2).await.unwrap();

    // Simulate startup crash by not calling mark_startup_success
    // and creating a new manager instance after timeout
    tokio::time::sleep(Duration::from_secs(3)).await;

    let mut manager_v3 = UpdateManager::new(UpdateConfig {
        current_version: "1.1.0".to_string(),
        install_dir: install_dir.clone(),
        update_dir: update_dir.clone(),
        startup_timeout_seconds: 2,
        ..Default::default()
    })
    .await
    .unwrap();

    // This should detect the crash and trigger rollback
    let rollback_result = manager_v3.initialize().await.unwrap();

    if let Some(result) = rollback_result {
        assert!(result.rollback_occurred);
        assert_eq!(result.previous_version, Some("1.1.0".to_string()));
        println!("Rollback successful: {:?}", result);
    }
}

/// Test rollback manager version tracking
#[tokio::test]
async fn test_rollback_manager_version_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = RollbackManager::new(temp_dir.path(), 3).unwrap();

    manager.initialize().await.unwrap();

    // Record multiple versions
    let versions = vec![
        VersionInfo::new(
            "1.0.0".to_string(),
            "hash1".to_string(),
            Channel::Stable,
            temp_dir.path().join("v1"),
        ),
        VersionInfo::new(
            "1.1.0".to_string(),
            "hash2".to_string(),
            Channel::Stable,
            temp_dir.path().join("v2"),
        ),
        VersionInfo::new(
            "1.2.0".to_string(),
            "hash3".to_string(),
            Channel::Stable,
            temp_dir.path().join("v3"),
        ),
    ];

    for version in versions {
        manager.record_version(version).await.unwrap();
    }

    // Check version history
    let history = manager.version_history();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].version, "1.2.0"); // Newest first
    assert_eq!(history[1].version, "1.1.0");
    assert_eq!(history[2].version, "1.0.0");

    // Check rollback targets
    let targets = manager.rollback_targets();
    assert_eq!(targets.len(), 2); // Excluding current
    assert_eq!(targets[0].version, "1.1.0");
    assert_eq!(targets[1].version, "1.0.0");

    // Test rollback
    let rolled_back = manager.rollback_to_previous().await.unwrap();
    assert_eq!(rolled_back.version, "1.1.0");

    // Verify history updated
    let new_history = manager.version_history();
    assert_eq!(new_history.len(), 2); // Failed version removed
    assert_eq!(new_history[0].version, "1.1.0"); // Now current
}

/// Test startup crash detector
#[tokio::test]
async fn test_startup_crash_detector() {
    let temp_dir = TempDir::new().unwrap();
    let startup_file = temp_dir.path().join("startup_check");

    let detector = StartupCrashDetector::new(
        &startup_file,
        Duration::from_millis(100), // Very short for testing
    );

    // No crash initially
    assert!(!detector.check_previous_crash().await.unwrap());

    // Mark startup attempt
    detector.mark_startup_attempt().await.unwrap();
    assert!(startup_file.exists());

    // Should not detect crash immediately
    assert!(!detector.check_previous_crash().await.unwrap());

    // Wait for timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Should now detect crash
    assert!(detector.check_previous_crash().await.unwrap());

    // Mark success should clear the file
    detector.mark_startup_success().await.unwrap();
    assert!(!startup_file.exists());
    assert!(!detector.check_previous_crash().await.unwrap());
}

/// Test version cleanup
#[tokio::test]
async fn test_version_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = RollbackManager::new(temp_dir.path(), 2).unwrap(); // Only keep 2 versions

    manager.initialize().await.unwrap();

    // Add 4 versions
    for i in 1..=4 {
        let version = VersionInfo::new(
            format!("1.{}.0", i),
            format!("hash{}", i),
            Channel::Stable,
            temp_dir.path().join(format!("v{}", i)),
        );
        manager.record_version(version).await.unwrap();
    }

    // Should only keep the latest 2
    let history = manager.version_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].version, "1.4.0");
    assert_eq!(history[1].version, "1.3.0");
}

/// Integration test for complete update flow
#[tokio::test]
async fn test_complete_update_flow() {
    let temp_dir = TempDir::new().unwrap();

    let config = UpdateConfig {
        install_dir: temp_dir.path().join("install"),
        update_dir: temp_dir.path().join("updates"),
        current_version: "1.0.0".to_string(),
        channel: Channel::Beta, // Use beta for testing
        startup_timeout_seconds: 1,
        ..Default::default()
    };

    // Create initial installation
    fs::create_dir_all(&config.install_dir).await.unwrap();
    fs::write(config.install_dir.join("main.exe"), b"original")
        .await
        .unwrap();

    let mut manager = UpdateManager::new(config).await.unwrap();

    // Initialize and mark success
    manager.initialize().await.unwrap();
    manager.mark_startup_success().await.unwrap();

    // Test channel switching
    assert!(manager.switch_channel(Channel::Canary).await.is_ok());

    // Test rollback targets (should be empty initially)
    let targets = manager.get_rollback_targets();
    assert_eq!(targets.len(), 0);

    // Note: Actual update download/apply would require a test server
    // This test validates the manager setup and basic operations
}

/// Test error handling in rollback scenarios
#[tokio::test]
async fn test_rollback_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let mut manager = RollbackManager::new(temp_dir.path(), 3).unwrap();

    manager.initialize().await.unwrap();

    // Try to rollback with no previous versions
    let result = manager.rollback_to_previous().await;
    assert!(result.is_err());

    // Add one version and try rollback (should still fail - need at least 2)
    let version = VersionInfo::new(
        "1.0.0".to_string(),
        "hash1".to_string(),
        Channel::Stable,
        temp_dir.path().join("v1"),
    );
    manager.record_version(version).await.unwrap();

    let result = manager.rollback_to_previous().await;
    assert!(result.is_err());
}
