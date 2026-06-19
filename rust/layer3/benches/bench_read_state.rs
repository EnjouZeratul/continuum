//! Benchmark for ReadStateStore (stale-read prevention core path).
//!
//! Measures SHA-256 hashing + state recording overhead at varying file sizes.
//! This is the per-read cost added by stale-read prevention.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sh_layer3::builtin_tools::read_state::ReadStateStore;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

fn bench_record_read(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("bench.txt");

    let mut group = c.benchmark_group("read_state_record");
    for &size in &[1024usize, 64 * 1024, 1024 * 1024] {
        group.throughput(Throughput::Bytes(size as u64));
        let content = vec![b'x'; size];
        std::fs::write(&path, &content).unwrap();
        let canonical: PathBuf = std::fs::canonicalize(&path).unwrap();
        let store: Arc<ReadStateStore> = ReadStateStore::new().into_arc();

        group.bench_with_input(BenchmarkId::from_parameter(size), &canonical, |b, canon| {
            b.iter(|| rt.block_on(store.record_read(black_box(canon.clone()))));
        });
    }
    group.finish();
}

fn bench_verify(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("verify.txt");

    let mut group = c.benchmark_group("read_state_verify");
    for &size in &[1024usize, 64 * 1024, 1024 * 1024] {
        group.throughput(Throughput::Bytes(size as u64));
        let content = vec![b'x'; size];
        std::fs::write(&path, &content).unwrap();
        let canonical: PathBuf = std::fs::canonicalize(&path).unwrap();
        let store: Arc<ReadStateStore> = ReadStateStore::new().into_arc();
        // Pre-record so verify has something to compare
        rt.block_on(store.record_read(canonical.clone())).unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(size), &canonical, |b, canon| {
            b.iter(|| {
                let _ = rt.block_on(store.verify(black_box(canon), true));
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_record_read, bench_verify);
criterion_main!(benches);
