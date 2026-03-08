// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for `flight-device-common` — stable IDs, health, manager traits, metrics.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use flight_device_common::device::DeviceId;
use flight_device_common::health::DeviceHealth;
use flight_device_common::manager::{DeviceManager, IdentifiedDevice};
use flight_device_common::metrics::DeviceMetrics;
use flight_metrics::common::DEVICE_METRICS_SHARED;
use flight_metrics::{Metric, MetricsRegistry};

use proptest::prelude::*;

// ─── DeviceId ─────────────────────────────────────────────────────────────

#[test]
fn device_id_new_stores_all_fields() {
    let id = DeviceId::new(0x044f, 0xb679, Some("SN-42".into()), "/dev/hidraw0");
    assert_eq!(id.vendor_id, 0x044f);
    assert_eq!(id.product_id, 0xb679);
    assert_eq!(id.serial_number.as_deref(), Some("SN-42"));
    assert_eq!(id.device_path, "/dev/hidraw0");
}

#[test]
fn device_id_new_without_serial() {
    let id = DeviceId::new(0x1234, 0x5678, None, "path");
    assert!(id.serial_number.is_none());
}

#[test]
fn device_id_vid_pid_lowercase_hex() {
    let id = DeviceId::new(0x06A3, 0x0762, None, "p");
    assert_eq!(id.vid_pid(), "06a3:0762");
}

#[test]
fn device_id_vid_pid_zero_padded() {
    let id = DeviceId::new(0x0001, 0x0002, None, "p");
    assert_eq!(id.vid_pid(), "0001:0002");
}

#[test]
fn device_id_vid_pid_max_values() {
    let id = DeviceId::new(0xFFFF, 0xFFFF, None, "p");
    assert_eq!(id.vid_pid(), "ffff:ffff");
}

#[test]
fn device_id_vid_pid_zero() {
    let id = DeviceId::new(0, 0, None, "p");
    assert_eq!(id.vid_pid(), "0000:0000");
}

#[test]
fn virtual_device_sets_zero_vid_pid() {
    let id = DeviceId::virtual_device("vdev-1");
    assert_eq!(id.vendor_id, 0);
    assert_eq!(id.product_id, 0);
}

#[test]
fn virtual_device_path_format() {
    let id = DeviceId::virtual_device("my-vdev");
    assert_eq!(id.device_path, "virtual://my-vdev");
}

#[test]
fn virtual_device_serial_matches_input() {
    let id = DeviceId::virtual_device("SER-99");
    assert_eq!(id.serial_number.as_deref(), Some("SER-99"));
}

#[test]
fn virtual_device_empty_serial() {
    let id = DeviceId::virtual_device("");
    assert_eq!(id.device_path, "virtual://");
    assert_eq!(id.serial_number.as_deref(), Some(""));
}

#[test]
fn display_with_serial_contains_all_parts() {
    let id = DeviceId::new(0x231d, 0x0136, Some("ABC".into()), "/dev/hidraw0");
    let s = id.to_string();
    assert!(s.contains("231d:0136"));
    assert!(s.contains("ABC"));
    assert!(s.contains("/dev/hidraw0"));
    assert!(s.contains('#'), "serial should be preceded by '#'");
}

#[test]
fn display_without_serial_no_hash() {
    let id = DeviceId::new(0x044f, 0xb679, None, "\\\\?\\HID#path");
    let s = id.to_string();
    // The path itself contains '#', so just check vid:pid format
    assert!(s.contains("044f:b679"));
    assert!(s.contains("\\\\?\\HID#path"));
}

#[test]
fn device_id_clone_is_equal() {
    let id = DeviceId::new(0xABCD, 0x1234, Some("s1".into()), "p");
    let cloned = id.clone();
    assert_eq!(id, cloned);
}

#[test]
fn device_id_equality_requires_all_fields() {
    let a = DeviceId::new(0x1, 0x2, Some("s".into()), "p");
    let b = DeviceId::new(0x1, 0x2, Some("s".into()), "p");
    assert_eq!(a, b);

    let c = DeviceId::new(0x1, 0x2, Some("s".into()), "q");
    assert_ne!(a, c, "different path should differ");

    let d = DeviceId::new(0x1, 0x2, None, "p");
    assert_ne!(a, d, "different serial should differ");

    let e = DeviceId::new(0x1, 0x3, Some("s".into()), "p");
    assert_ne!(a, e, "different product_id should differ");

    let f = DeviceId::new(0x2, 0x2, Some("s".into()), "p");
    assert_ne!(a, f, "different vendor_id should differ");
}

#[test]
fn device_id_hash_set_dedup() {
    let mut set = HashSet::new();
    let id = DeviceId::new(0x1, 0x2, None, "p");
    set.insert(id.clone());
    set.insert(id.clone());
    assert_eq!(set.len(), 1);
}

#[test]
fn device_id_hash_map_lookup() {
    let mut map = HashMap::new();
    let id = DeviceId::new(0xAA, 0xBB, Some("s".into()), "path");
    map.insert(id.clone(), "value");
    assert_eq!(map.get(&id), Some(&"value"));
}

#[test]
fn device_id_ordering_by_vendor_id() {
    let a = DeviceId::new(0x0001, 0x0001, None, "a");
    let b = DeviceId::new(0x0002, 0x0001, None, "a");
    assert!(a < b);
}

#[test]
fn device_id_ordering_by_product_id() {
    let a = DeviceId::new(0x0001, 0x0001, None, "a");
    let b = DeviceId::new(0x0001, 0x0002, None, "a");
    assert!(a < b);
}

#[test]
fn device_id_ordering_by_serial() {
    let a = DeviceId::new(0x1, 0x1, None, "a");
    let b = DeviceId::new(0x1, 0x1, Some("s".into()), "a");
    assert!(a < b, "None sorts before Some");
}

#[test]
fn device_id_ordering_by_path() {
    let a = DeviceId::new(0x1, 0x1, None, "aaa");
    let b = DeviceId::new(0x1, 0x1, None, "bbb");
    assert!(a < b);
}

#[test]
fn device_id_collision_avoidance_different_paths() {
    let a = DeviceId::new(0x06A3, 0x0762, None, "/dev/hidraw0");
    let b = DeviceId::new(0x06A3, 0x0762, None, "/dev/hidraw1");
    assert_ne!(a, b, "same vid:pid but different paths should not collide");
    let mut set = HashSet::new();
    set.insert(a);
    set.insert(b);
    assert_eq!(set.len(), 2);
}

#[test]
fn device_id_collision_avoidance_different_serials() {
    let a = DeviceId::new(0x06A3, 0x0762, Some("SN1".into()), "/dev/hidraw0");
    let b = DeviceId::new(0x06A3, 0x0762, Some("SN2".into()), "/dev/hidraw0");
    assert_ne!(a, b, "same vid:pid+path but different serials should not collide");
}

#[test]
fn device_id_debug_format() {
    let id = DeviceId::new(0xABCD, 0x1234, Some("ser".into()), "path");
    let dbg = format!("{id:?}");
    assert!(dbg.contains("DeviceId"));
    assert!(dbg.contains("43981")); // 0xABCD decimal
}

// ─── DeviceHealth ─────────────────────────────────────────────────────────

#[test]
fn healthy_is_operational() {
    assert!(DeviceHealth::Healthy.is_operational());
}

#[test]
fn degraded_is_operational() {
    let h = DeviceHealth::Degraded {
        reason: "slow".into(),
    };
    assert!(h.is_operational());
}

#[test]
fn quarantined_is_not_operational() {
    let h = DeviceHealth::Quarantined {
        since: Instant::now(),
        reason: "errors".into(),
    };
    assert!(!h.is_operational());
}

#[test]
fn failed_is_not_operational() {
    let h = DeviceHealth::Failed {
        error: "disconnected".into(),
    };
    assert!(!h.is_operational());
}

#[test]
fn healthy_reason_is_none() {
    assert!(DeviceHealth::Healthy.reason().is_none());
}

#[test]
fn degraded_reason() {
    let h = DeviceHealth::Degraded {
        reason: "high jitter".into(),
    };
    assert_eq!(h.reason(), Some("high jitter"));
}

#[test]
fn quarantined_reason() {
    let h = DeviceHealth::Quarantined {
        since: Instant::now(),
        reason: "too many faults".into(),
    };
    assert_eq!(h.reason(), Some("too many faults"));
}

#[test]
fn failed_reason() {
    let h = DeviceHealth::Failed {
        error: "USB gone".into(),
    };
    assert_eq!(h.reason(), Some("USB gone"));
}

#[test]
fn health_clone_equality() {
    let h = DeviceHealth::Degraded {
        reason: "latency".into(),
    };
    assert_eq!(h.clone(), h);
}

#[test]
fn health_variants_not_equal() {
    let a = DeviceHealth::Healthy;
    let b = DeviceHealth::Failed {
        error: "e".into(),
    };
    assert_ne!(a, b);
}

#[test]
fn health_degraded_different_reasons_not_equal() {
    let a = DeviceHealth::Degraded {
        reason: "latency".into(),
    };
    let b = DeviceHealth::Degraded {
        reason: "jitter".into(),
    };
    assert_ne!(a, b);
}

#[test]
fn health_debug_format() {
    let h = DeviceHealth::Quarantined {
        since: Instant::now(),
        reason: "fault".into(),
    };
    let dbg = format!("{h:?}");
    assert!(dbg.contains("Quarantined"));
    assert!(dbg.contains("fault"));
}

// ─── DeviceManager (mock implementation) ──────────────────────────────────

#[derive(Debug, Clone)]
struct MockDevice {
    id: DeviceId,
}

impl IdentifiedDevice for MockDevice {
    fn device_id(&self) -> DeviceId {
        self.id.clone()
    }
}

#[derive(Debug)]
struct MockManager {
    devices: HashMap<DeviceId, (MockDevice, DeviceHealth)>,
}

impl MockManager {
    fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }
}

#[derive(Debug)]
struct MockError(#[allow(dead_code)] String);

impl DeviceManager for MockManager {
    type Device = MockDevice;
    type Error = MockError;

    fn enumerate_devices(&mut self) -> Result<Vec<Self::Device>, Self::Error> {
        Ok(self.devices.values().map(|(d, _)| d.clone()).collect())
    }

    fn register_device(&mut self, device: Self::Device) -> Result<(), Self::Error> {
        let id = device.device_id();
        if self.devices.contains_key(&id) {
            return Err(MockError(format!("already registered: {id}")));
        }
        self.devices.insert(id, (device, DeviceHealth::Healthy));
        Ok(())
    }

    fn unregister_device(&mut self, id: &DeviceId) -> Result<(), Self::Error> {
        self.devices
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| MockError(format!("not found: {id}")))
    }

    fn get_device_health(&self, id: &DeviceId) -> Option<DeviceHealth> {
        self.devices.get(id).map(|(_, h)| h.clone())
    }
}

fn make_device(vid: u16, pid: u16, path: &str) -> MockDevice {
    MockDevice {
        id: DeviceId::new(vid, pid, None, path),
    }
}

#[test]
fn manager_register_and_enumerate() {
    let mut mgr = MockManager::new();
    let d = make_device(0x1, 0x2, "p1");
    mgr.register_device(d).unwrap();
    let devices = mgr.enumerate_devices().unwrap();
    assert_eq!(devices.len(), 1);
}

#[test]
fn manager_register_duplicate_fails() {
    let mut mgr = MockManager::new();
    let d1 = make_device(0x1, 0x2, "p");
    let d2 = make_device(0x1, 0x2, "p");
    mgr.register_device(d1).unwrap();
    assert!(mgr.register_device(d2).is_err());
}

#[test]
fn manager_unregister_success() {
    let mut mgr = MockManager::new();
    let d = make_device(0x1, 0x2, "p");
    let id = d.id.clone();
    mgr.register_device(d).unwrap();
    mgr.unregister_device(&id).unwrap();
    assert!(mgr.enumerate_devices().unwrap().is_empty());
}

#[test]
fn manager_unregister_nonexistent_fails() {
    let mut mgr = MockManager::new();
    let id = DeviceId::new(0xFF, 0xFF, None, "noexist");
    assert!(mgr.unregister_device(&id).is_err());
}

#[test]
fn manager_health_after_register() {
    let mut mgr = MockManager::new();
    let d = make_device(0x1, 0x2, "p");
    let id = d.id.clone();
    mgr.register_device(d).unwrap();
    assert_eq!(mgr.get_device_health(&id), Some(DeviceHealth::Healthy));
}

#[test]
fn manager_health_unknown_device_is_none() {
    let mgr = MockManager::new();
    let id = DeviceId::new(0x99, 0x99, None, "x");
    assert!(mgr.get_device_health(&id).is_none());
}

#[test]
fn manager_multiple_devices() {
    let mut mgr = MockManager::new();
    for i in 0..5 {
        mgr.register_device(make_device(i, i, &format!("p{i}")))
            .unwrap();
    }
    assert_eq!(mgr.enumerate_devices().unwrap().len(), 5);
}

#[test]
fn identified_device_trait() {
    let d = make_device(0xAA, 0xBB, "path");
    let id = d.device_id();
    assert_eq!(id.vendor_id, 0xAA);
    assert_eq!(id.product_id, 0xBB);
}

// ─── DeviceMetrics ────────────────────────────────────────────────────────

#[test]
fn metrics_default_all_zeros() {
    let m = DeviceMetrics::default();
    assert_eq!(m.operations_total, 0);
    assert_eq!(m.operations_failed, 0);
    assert_eq!(m.bytes_transferred, 0);
    assert!(m.last_operation_time.is_none());
    assert!(m.last_operation_latency_ms.is_none());
}

#[test]
fn metrics_record_success() {
    let mut m = DeviceMetrics::default();
    m.record_operation(100, 0.5, true);
    assert_eq!(m.operations_total, 1);
    assert_eq!(m.operations_failed, 0);
    assert_eq!(m.bytes_transferred, 100);
    assert!(m.last_operation_time.is_some());
    assert_eq!(m.last_operation_latency_ms, Some(0.5));
}

#[test]
fn metrics_record_failure() {
    let mut m = DeviceMetrics::default();
    m.record_operation(64, 2.0, false);
    assert_eq!(m.operations_total, 1);
    assert_eq!(m.operations_failed, 1);
}

#[test]
fn metrics_accumulate_bytes() {
    let mut m = DeviceMetrics::default();
    m.record_operation(10, 1.0, true);
    m.record_operation(20, 1.0, true);
    m.record_operation(30, 1.0, true);
    assert_eq!(m.bytes_transferred, 60);
}

#[test]
fn metrics_nan_latency_ignored() {
    let mut m = DeviceMetrics::default();
    m.record_operation(0, f64::NAN, true);
    assert!(m.last_operation_latency_ms.is_none());
}

#[test]
fn metrics_negative_latency_ignored() {
    let mut m = DeviceMetrics::default();
    m.record_operation(0, -1.0, true);
    assert!(m.last_operation_latency_ms.is_none());
}

#[test]
fn metrics_infinity_latency_ignored() {
    let mut m = DeviceMetrics::default();
    m.record_operation(0, f64::INFINITY, true);
    assert!(m.last_operation_latency_ms.is_none());
}

#[test]
fn metrics_neg_infinity_latency_ignored() {
    let mut m = DeviceMetrics::default();
    m.record_operation(0, f64::NEG_INFINITY, true);
    assert!(m.last_operation_latency_ms.is_none());
}

#[test]
fn metrics_zero_latency_stored() {
    let mut m = DeviceMetrics::default();
    m.record_operation(0, 0.0, true);
    assert_eq!(m.last_operation_latency_ms, Some(0.0));
}

#[test]
fn metrics_valid_latency_overwrites_previous() {
    let mut m = DeviceMetrics::default();
    m.record_operation(0, 1.0, true);
    m.record_operation(0, 2.0, true);
    assert_eq!(m.last_operation_latency_ms, Some(2.0));
}

#[test]
fn metrics_invalid_latency_preserves_previous() {
    let mut m = DeviceMetrics::default();
    m.record_operation(0, 1.5, true);
    m.record_operation(0, f64::NAN, true);
    assert_eq!(m.last_operation_latency_ms, Some(1.5));
}

#[test]
fn metrics_error_rate_no_ops() {
    let m = DeviceMetrics::default();
    assert_eq!(m.error_rate_percent(), 0.0);
}

#[test]
fn metrics_error_rate_all_success() {
    let mut m = DeviceMetrics::default();
    for _ in 0..10 {
        m.record_operation(0, 1.0, true);
    }
    assert_eq!(m.error_rate_percent(), 0.0);
}

#[test]
fn metrics_error_rate_all_failures() {
    let mut m = DeviceMetrics::default();
    for _ in 0..10 {
        m.record_operation(0, 1.0, false);
    }
    assert!((m.error_rate_percent() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn metrics_error_rate_mixed() {
    let mut m = DeviceMetrics::default();
    m.record_operation(0, 1.0, true);
    m.record_operation(0, 1.0, false);
    assert!((m.error_rate_percent() - 50.0).abs() < f64::EPSILON);
}

#[test]
fn metrics_with_registry_increments_counter() {
    let mut m = DeviceMetrics::default();
    let reg = MetricsRegistry::new();
    m.record_operation_with_registry(&reg, DEVICE_METRICS_SHARED, 128, 0.5, true);

    let snap = reg.snapshot();
    let count = snap.iter().any(|metric| {
        matches!(metric, Metric::Counter { name, value }
            if *name == DEVICE_METRICS_SHARED.operations_total && *value == 1)
    });
    assert!(count, "expected operations_total counter == 1");
}

#[test]
fn metrics_with_registry_increments_error_counter_on_failure() {
    let mut m = DeviceMetrics::default();
    let reg = MetricsRegistry::new();
    m.record_operation_with_registry(&reg, DEVICE_METRICS_SHARED, 0, 1.0, false);

    let snap = reg.snapshot();
    let has_error = snap.iter().any(|metric| {
        matches!(metric, Metric::Counter { name, value }
            if *name == DEVICE_METRICS_SHARED.errors_total && *value == 1)
    });
    assert!(has_error, "expected errors_total counter == 1");
}

#[test]
fn metrics_with_registry_no_error_counter_on_success() {
    let mut m = DeviceMetrics::default();
    let reg = MetricsRegistry::new();
    m.record_operation_with_registry(&reg, DEVICE_METRICS_SHARED, 0, 1.0, true);

    let snap = reg.snapshot();
    let has_error = snap.iter().any(|metric| {
        matches!(metric, Metric::Counter { name, value }
            if *name == DEVICE_METRICS_SHARED.errors_total && *value > 0)
    });
    assert!(!has_error, "errors_total should be 0 on success");
}

#[test]
fn metrics_with_registry_records_histogram() {
    let mut m = DeviceMetrics::default();
    let reg = MetricsRegistry::new();
    m.record_operation_with_registry(&reg, DEVICE_METRICS_SHARED, 0, 2.71, true);

    let snap = reg.snapshot();
    let has_hist = snap.iter().any(|metric| {
        matches!(metric, Metric::Histogram { name, .. }
            if *name == DEVICE_METRICS_SHARED.operation_latency_ms)
    });
    assert!(has_hist, "expected histogram observation");
}

#[test]
fn metrics_clone_independence() {
    let mut m = DeviceMetrics::default();
    m.record_operation(100, 1.0, true);
    let mut cloned = m.clone();
    cloned.record_operation(50, 2.0, false);
    assert_eq!(m.operations_total, 1, "original should be unaffected");
    assert_eq!(cloned.operations_total, 2);
}

#[test]
fn metrics_last_operation_time_updates() {
    let mut m = DeviceMetrics::default();
    let before = Instant::now();
    m.record_operation(0, 1.0, true);
    let t1 = m.last_operation_time.unwrap();
    assert!(t1 >= before);
}

// ─── Property-based tests ─────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_vid_pid_format_is_valid(vid in 0u16..=u16::MAX, pid in 0u16..=u16::MAX) {
        let id = DeviceId::new(vid, pid, None, "p");
        let vp = id.vid_pid();
        prop_assert_eq!(vp.len(), 9, "vid:pid should be exactly 9 chars: {}", vp);
        prop_assert_eq!(&vp[4..5], ":");
        u16::from_str_radix(&vp[..4], 16).unwrap();
        u16::from_str_radix(&vp[5..], 16).unwrap();
    }

    #[test]
    fn prop_vid_pid_roundtrip(vid in 0u16..=u16::MAX, pid in 0u16..=u16::MAX) {
        let id = DeviceId::new(vid, pid, None, "p");
        let vp = id.vid_pid();
        let parsed_vid = u16::from_str_radix(&vp[..4], 16).unwrap();
        let parsed_pid = u16::from_str_radix(&vp[5..], 16).unwrap();
        prop_assert_eq!(parsed_vid, vid);
        prop_assert_eq!(parsed_pid, pid);
    }

    #[test]
    fn prop_device_id_eq_reflexive(
        vid in 0u16..=u16::MAX,
        pid in 0u16..=u16::MAX,
        path in "[a-z0-9/]{1,30}",
    ) {
        let id = DeviceId::new(vid, pid, None, &path);
        prop_assert_eq!(&id, &id.clone());
    }

    #[test]
    fn prop_device_id_hash_consistency(
        vid in 0u16..=u16::MAX,
        pid in 0u16..=u16::MAX,
        path in "[a-z]{1,10}",
    ) {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let id = DeviceId::new(vid, pid, None, &path);
        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        id.hash(&mut h1);
        id.clone().hash(&mut h2);
        prop_assert_eq!(h1.finish(), h2.finish(), "equal objects must have equal hashes");
    }

    #[test]
    fn prop_virtual_device_always_zero_vid_pid(serial in "[a-zA-Z0-9_-]{0,20}") {
        let id = DeviceId::virtual_device(&serial);
        prop_assert_eq!(id.vendor_id, 0);
        prop_assert_eq!(id.product_id, 0);
        prop_assert_eq!(id.serial_number.as_deref(), Some(serial.as_str()));
        prop_assert!(id.device_path.starts_with("virtual://"));
    }

    #[test]
    fn prop_display_contains_vid_pid(vid in 0u16..=u16::MAX, pid in 0u16..=u16::MAX) {
        let id = DeviceId::new(vid, pid, None, "path");
        let s = id.to_string();
        prop_assert!(s.contains(&id.vid_pid()), "Display should contain vid:pid");
    }

    #[test]
    fn prop_display_contains_path(path in "[a-z/]{1,20}") {
        let id = DeviceId::new(0, 0, None, &path);
        let s = id.to_string();
        prop_assert!(s.contains(&path), "Display should contain device_path");
    }

    #[test]
    fn prop_ordering_antisymmetric(
        vid_a in 0u16..=u16::MAX,
        pid_a in 0u16..=u16::MAX,
        vid_b in 0u16..=u16::MAX,
        pid_b in 0u16..=u16::MAX,
    ) {
        let a = DeviceId::new(vid_a, pid_a, None, "p");
        let b = DeviceId::new(vid_b, pid_b, None, "p");
        if a < b {
            prop_assert!(b >= a, "ordering must be antisymmetric");
        }
    }

    #[test]
    fn prop_collision_avoidance_unique_paths(
        vid in 0u16..=u16::MAX,
        pid in 0u16..=u16::MAX,
        path_a in "[a-z]{1,10}",
        path_b in "[a-z]{1,10}",
    ) {
        let a = DeviceId::new(vid, pid, None, &path_a);
        let b = DeviceId::new(vid, pid, None, &path_b);
        if path_a != path_b {
            prop_assert_ne!(a, b, "same vid:pid with different paths must differ");
        }
    }

    #[test]
    fn prop_error_rate_bounded(
        success_count in 0u64..100,
        fail_count in 0u64..100,
    ) {
        let mut m = DeviceMetrics::default();
        for _ in 0..success_count {
            m.record_operation(0, 1.0, true);
        }
        for _ in 0..fail_count {
            m.record_operation(0, 1.0, false);
        }
        let rate = m.error_rate_percent();
        prop_assert!(rate >= 0.0, "error rate must be >= 0");
        prop_assert!(rate <= 100.0, "error rate must be <= 100");
    }

    #[test]
    fn prop_bytes_accumulate(ops in proptest::collection::vec((0u64..1024, 0.0f64..10.0), 1..50)) {
        let mut m = DeviceMetrics::default();
        let expected: u64 = ops.iter().map(|(b, _)| b).sum();
        for (bytes, latency) in &ops {
            m.record_operation(*bytes, *latency, true);
        }
        prop_assert_eq!(m.bytes_transferred, expected);
        prop_assert_eq!(m.operations_total, ops.len() as u64);
    }

    #[test]
    fn prop_health_operational_iff_healthy_or_degraded(reason in "[a-z ]{1,20}") {
        let healthy = DeviceHealth::Healthy;
        let degraded = DeviceHealth::Degraded { reason: reason.clone() };
        let quarantined = DeviceHealth::Quarantined { since: Instant::now(), reason: reason.clone() };
        let failed = DeviceHealth::Failed { error: reason };
        prop_assert!(healthy.is_operational());
        prop_assert!(degraded.is_operational());
        prop_assert!(!quarantined.is_operational());
        prop_assert!(!failed.is_operational());
    }
}
