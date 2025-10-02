//! Test service implementation to verify safe mode functionality

use crate::{
    safe_mode::{SafeModeManager, SafeModeConfig},
    power::PowerChecker,
    health::HealthStream,
    error_taxonomy::ErrorTaxonomy,
};

#[tokio::test]
async fn test_safe_mode_initialization() {
    let config = SafeModeConfig::default();
    let mut manager = SafeModeManager::new(config);
    
    let result = manager.initialize().await;
    assert!(result.is_ok(), "Safe mode initialization should succeed");
    
    let status = manager.get_status();
    assert!(status.active, "Safe mode should be active");
}

#[tokio::test]
async fn test_power_checker() {
    let status = PowerChecker::check_power_configuration().await;
    
    // Should have some checks
    assert!(!status.checks.is_empty(), "Power checks should not be empty");
    
    // Status should be valid
    assert!(matches!(
        status.overall_status,
        crate::power::PowerCheckStatus::Optimal 
        | crate::power::PowerCheckStatus::Degraded 
        | crate::power::PowerCheckStatus::Critical
    ));
}

#[tokio::test]
async fn test_health_stream() {
    let health = HealthStream::new();
    
    health.register_component("test_component").await;
    health.info("test_component", "Test message").await;
    
    let status = health.get_health_status().await;
    assert!(status.components.contains_key("test_component"));
}

#[tokio::test]
async fn test_error_taxonomy() {
    let taxonomy = ErrorTaxonomy::new();
    
    // Should have standard errors
    assert!(taxonomy.get_error("HID_OUT_STALL").is_some());
    assert!(taxonomy.get_error("AXIS_JITTER").is_some());
    assert!(taxonomy.get_error("FFB_FAULT").is_some());
    
    // Test error creation
    let mut context = std::collections::HashMap::new();
    context.insert("device_id".to_string(), "test_device".to_string());
    
    let error = taxonomy.create_error("HID_OUT_STALL", "test_component", context);
    assert!(error.is_some());
}