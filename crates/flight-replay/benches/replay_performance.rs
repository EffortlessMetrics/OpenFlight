//! Performance benchmarks for replay harness

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use flight_replay::{ReplayHarness, ReplayConfig, ReplayMode, ToleranceConfig};
use flight_core::blackbox::{BlackboxWriter, BlackboxConfig};
use flight_axis::{AxisFrame, EngineConfig as AxisEngineConfig};
use flight_ffb::{FfbConfig, FfbMode};

async fn create_benchmark_blackbox(frame_count: usize) -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let config = BlackboxConfig {
        output_dir: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let mut writer = BlackboxWriter::new(config);
    let filepath = writer.start_recording(
        "benchmark_sim".to_string(),
        "benchmark_aircraft".to_string(),
        "1.0.0".to_string(),
    ).await.unwrap();

    // Write benchmark data
    for i in 0..frame_count {
        let timestamp = i as u64 * 4_000_000; // 4ms intervals
        let input_value = 0.5 * (i as f32 / 1000.0).sin();
        let frame = AxisFrame::new(input_value, timestamp);
        let axis_data = bincode::serialize(&frame).unwrap();
        writer.record_axis_frame(timestamp, &axis_data).unwrap();
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    writer.stop_recording().await.unwrap();

    (temp_dir, filepath)
}

fn bench_replay_modes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // Create test data with different sizes
    let frame_counts = vec![100, 1000, 5000];
    
    for &frame_count in &frame_counts {
        let (_temp_dir, filepath) = rt.block_on(create_benchmark_blackbox(frame_count));
        
        let mut group = c.benchmark_group("replay_modes");
        group.measurement_time(Duration::from_secs(10));
        
        // Benchmark FastForward mode
        group.bench_with_input(
            BenchmarkId::new("fast_forward", frame_count),
            &frame_count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = ReplayConfig {
                            mode: ReplayMode::FastForward,
                            max_duration: Duration::from_secs(30),
                            validate_outputs: false,
                            collect_metrics: true,
                            ..Default::default()
                        };
                        
                        let mut harness = ReplayHarness::new(config).unwrap();
                        
                        // Add test device
                        let axis_config = AxisEngineConfig::default();
                        harness.add_axis_device("bench_device".to_string(), axis_config).unwrap();
                        
                        let result = harness.replay_file(&filepath).await.unwrap();
                        std::hint::black_box(result);
                    })
                });
            },
        );
        
        // Benchmark with validation enabled
        group.bench_with_input(
            BenchmarkId::new("fast_forward_with_validation", frame_count),
            &frame_count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = ReplayConfig {
                            mode: ReplayMode::FastForward,
                            max_duration: Duration::from_secs(30),
                            validate_outputs: true,
                            tolerance: ToleranceConfig::strict(),
                            collect_metrics: true,
                            ..Default::default()
                        };
                        
                        let mut harness = ReplayHarness::new(config).unwrap();
                        
                        // Add test device
                        let axis_config = AxisEngineConfig::default();
                        harness.add_axis_device("bench_device".to_string(), axis_config).unwrap();
                        
                        let result = harness.replay_file(&filepath).await.unwrap();
                        std::hint::black_box(result);
                    })
                });
            },
        );
        
        group.finish();
    }
}

fn bench_tolerance_configurations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (_temp_dir, filepath) = rt.block_on(create_benchmark_blackbox(1000));
    
    let mut group = c.benchmark_group("tolerance_configs");
    
    // Benchmark strict tolerance
    group.bench_function("strict_tolerance", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = ReplayConfig {
                    mode: ReplayMode::FastForward,
                    max_duration: Duration::from_secs(30),
                    validate_outputs: true,
                    tolerance: ToleranceConfig::strict(),
                    collect_metrics: true,
                    ..Default::default()
                };
                
                let mut harness = ReplayHarness::new(config).unwrap();
                let axis_config = AxisEngineConfig::default();
                harness.add_axis_device("bench_device".to_string(), axis_config).unwrap();
                
                let result = harness.replay_file(&filepath).await.unwrap();
                std::hint::black_box(result);
            })
        });
    });
    
    // Benchmark relaxed tolerance
    group.bench_function("relaxed_tolerance", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = ReplayConfig {
                    mode: ReplayMode::FastForward,
                    max_duration: Duration::from_secs(30),
                    validate_outputs: true,
                    tolerance: ToleranceConfig::relaxed(),
                    collect_metrics: true,
                    ..Default::default()
                };
                
                let mut harness = ReplayHarness::new(config).unwrap();
                let axis_config = AxisEngineConfig::default();
                harness.add_axis_device("bench_device".to_string(), axis_config).unwrap();
                
                let result = harness.replay_file(&filepath).await.unwrap();
                std::hint::black_box(result);
            })
        });
    });
    
    group.finish();
}

fn bench_engine_configurations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (_temp_dir, filepath) = rt.block_on(create_benchmark_blackbox(1000));
    
    let mut group = c.benchmark_group("engine_configs");
    
    // Benchmark axis-only configuration
    group.bench_function("axis_only", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = ReplayConfig {
                    mode: ReplayMode::FastForward,
                    max_duration: Duration::from_secs(30),
                    validate_outputs: false,
                    collect_metrics: true,
                    ..Default::default()
                };
                
                let mut harness = ReplayHarness::new(config).unwrap();
                let axis_config = AxisEngineConfig::default();
                harness.add_axis_device("bench_device".to_string(), axis_config).unwrap();
                
                let result = harness.replay_file(&filepath).await.unwrap();
                std::hint::black_box(result);
            })
        });
    });
    
    // Benchmark axis + FFB configuration
    group.bench_function("axis_and_ffb", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = ReplayConfig {
                    mode: ReplayMode::FastForward,
                    max_duration: Duration::from_secs(30),
                    validate_outputs: false,
                    collect_metrics: true,
                    ..Default::default()
                };
                
                let mut harness = ReplayHarness::new(config).unwrap();
                
                let axis_config = AxisEngineConfig::default();
                harness.add_axis_device("bench_device".to_string(), axis_config).unwrap();
                
                let ffb_config = FfbConfig {
                    max_torque_nm: 15.0,
                    fault_timeout_ms: 50,
                    interlock_required: false,
                    mode: FfbMode::TelemetrySynth,
                    device_path: None,
                };
                harness.add_ffb_device("bench_ffb".to_string(), ffb_config).unwrap();
                
                let result = harness.replay_file(&filepath).await.unwrap();
                std::hint::black_box(result);
            })
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_replay_modes,
    bench_tolerance_configurations,
    bench_engine_configurations
);
criterion_main!(benches);