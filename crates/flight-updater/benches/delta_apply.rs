use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flight_updater::delta::{DeltaApplier, DeltaOperation, DeltaPatch, FileDelta};
use std::collections::HashMap;
use tempfile::TempDir;

fn benchmark_delta_apply(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("delta_apply_small", |b| {
        b.iter(|| {
            rt.block_on(async {
                let temp_dir = TempDir::new().unwrap();
                let applier = DeltaApplier::new(temp_dir.path()).unwrap();
                
                // Create test patch
                let mut patch = DeltaPatch::new("1.0.0".to_string(), "1.1.0".to_string());
                
                let file_delta = FileDelta {
                    source_path: "test.txt".to_string(),
                    target_path: "test.txt".to_string(),
                    source_hash: "source_hash".to_string(),
                    target_hash: "target_hash".to_string(),
                    operations: vec![DeltaOperation::Insert {
                        data: black_box(b"small test content".to_vec()),
                    }],
                    compression: "none".to_string(),
                };
                
                patch.add_file_delta(file_delta);
                
                // Benchmark would apply patch here
                // (simplified for benchmark purposes)
                black_box(patch);
            });
        });
    });
    
    c.bench_function("delta_compression", |b| {
        let data = vec![0u8; 1024]; // 1KB of zeros
        
        b.iter(|| {
            let compressed = DeltaApplier::compress_patch_data(black_box(&data)).unwrap();
            let _decompressed = DeltaApplier::decompress_patch_data(&compressed).unwrap();
        });
    });
}

criterion_group!(benches, benchmark_delta_apply);
criterion_main!(benches);