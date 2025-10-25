//! Performance benchmarks for Flight Hub IPC

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use flight_ipc::{
    negotiation::{negotiate_features, Version},
    proto::{
        Device, DeviceCapabilities, DeviceHealth, DeviceStatus, DeviceType, HealthEvent,
        HealthEventType, ListDevicesResponse, NegotiateFeaturesRequest,
        PerformanceMetrics, TransportType,
    },
};
use prost::Message;
use std::time::SystemTime;

fn bench_version_parsing(c: &mut Criterion) {
    c.bench_function("version_parsing", |b| {
        b.iter(|| {
            let version = Version::parse(std::hint::black_box("1.2.3")).unwrap();
            std::hint::black_box(version);
        });
    });
}

fn bench_version_compatibility(c: &mut Criterion) {
    let v1 = Version::parse("1.0.0").unwrap();
    let v2 = Version::parse("1.1.0").unwrap();
    
    c.bench_function("version_compatibility", |b| {
        b.iter(|| {
            let result = std::hint::black_box(&v2).is_compatible_with(std::hint::black_box(&v1));
            std::hint::black_box(result);
        });
    });
}

fn bench_feature_negotiation(c: &mut Criterion) {
    let request = NegotiateFeaturesRequest {
        client_version: "1.0.0".to_string(),
        supported_features: vec![
            "device-management".to_string(),
            "health-monitoring".to_string(),
            "profile-management".to_string(),
        ],
        preferred_transport: TransportType::NamedPipes.into(),
    };
    
    let server_features = vec![
        "device-management".to_string(),
        "health-monitoring".to_string(),
        "profile-management".to_string(),
        "force-feedback".to_string(),
    ];
    
    c.bench_function("feature_negotiation", |b| {
        b.iter(|| {
            let response = negotiate_features(std::hint::black_box(&request), std::hint::black_box(&server_features)).unwrap();
            std::hint::black_box(response);
        });
    });
}

fn bench_device_serialization(c: &mut Criterion) {
    let device = create_test_device();
    
    let mut group = c.benchmark_group("device_serialization");
    
    // Benchmark protobuf encoding
    group.bench_function("protobuf_encode", |b| {
        b.iter(|| {
            let encoded = std::hint::black_box(&device).encode_to_vec();
            std::hint::black_box(encoded);
        });
    });
    
    // Benchmark protobuf decoding
    let encoded = device.encode_to_vec();
    group.bench_function("protobuf_decode", |b| {
        b.iter(|| {
            let decoded = Device::decode(std::hint::black_box(&encoded[..])).unwrap();
            std::hint::black_box(decoded);
        });
    });
    
    // Note: JSON benchmarks are not available because Device type (generated from protobuf)
    // does not implement Serialize/Deserialize. To add JSON benchmarks, you would need to:
    // 1. Enable serde feature in prost-build configuration
    // 2. Add #[derive(Serialize, Deserialize)] to generated types
    // For now, we only benchmark protobuf encoding/decoding which is the primary use case.
    
    group.finish();
}

fn bench_device_list_serialization(c: &mut Criterion) {
    let devices: Vec<Device> = (0..100).map(|i| {
        let mut device = create_test_device();
        device.id = format!("device-{}", i);
        device
    }).collect();
    
    let response = ListDevicesResponse {
        devices,
        total_count: 100,
    };
    
    let mut group = c.benchmark_group("device_list_serialization");
    group.throughput(Throughput::Elements(100));
    
    // Benchmark large device list encoding
    group.bench_function("protobuf_encode_100_devices", |b| {
        b.iter(|| {
            let encoded = std::hint::black_box(&response).encode_to_vec();
            std::hint::black_box(encoded);
        });
    });
    
    // Benchmark large device list decoding
    let encoded = response.encode_to_vec();
    group.bench_function("protobuf_decode_100_devices", |b| {
        b.iter(|| {
            let decoded = ListDevicesResponse::decode(std::hint::black_box(&encoded[..])).unwrap();
            std::hint::black_box(decoded);
        });
    });
    
    group.finish();
}

fn bench_health_event_creation(c: &mut Criterion) {
    c.bench_function("health_event_creation", |b| {
        b.iter(|| {
            let event = HealthEvent {
                timestamp: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                r#type: HealthEventType::Performance.into(),
                message: "Performance metrics update".to_string(),
                device_id: "test-device-1".to_string(),
                error_code: String::new(),
                metadata: [("source".to_string(), "axis-engine".to_string())]
                    .iter()
                    .cloned()
                    .collect(),
                performance: Some(PerformanceMetrics {
                    jitter_p99_ms: 0.3,
                    hid_latency_p99_us: 150.0,
                    missed_ticks: 0,
                    dropped_frames: 0,
                    cpu_usage_percent: 2.5,
                    memory_usage_bytes: 1024 * 1024 * 50, // 50MB
                }),
            };
            std::hint::black_box(event);
        });
    });
}

fn create_test_device() -> Device {
    Device {
        id: "test-device-1".to_string(),
        name: "Test Flight Stick".to_string(),
        r#type: DeviceType::Joystick.into(),
        status: DeviceStatus::Connected.into(),
        capabilities: Some(DeviceCapabilities {
            supports_force_feedback: false,
            supports_raw_torque: false,
            max_torque_nm: 0,
            min_period_us: 1000,
            has_health_stream: true,
            supported_protocols: vec!["hid".to_string()],
        }),
        health: Some(DeviceHealth {
            temperature_celsius: 25.5,
            current_amperes: 0.1,
            packet_loss_count: 0,
            last_seen_timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            active_faults: vec![],
        }),
        metadata: [
            ("vendor".to_string(), "Test Corp".to_string()),
            ("model".to_string(), "FS-1000".to_string()),
            ("serial".to_string(), "ABC123456".to_string()),
        ]
        .iter()
        .cloned()
        .collect(),
    }
}

criterion_group!(
    benches,
    bench_version_parsing,
    bench_version_compatibility,
    bench_feature_negotiation,
    bench_device_serialization,
    bench_device_list_serialization,
    bench_health_event_creation
);

criterion_main!(benches);