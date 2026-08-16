//! Memory + throughput benchmark — aligns with AutoAgents 2026 methodology.
//!
//! Measures:
//! 1. Peak RSS after N agent loop iterations
//! 2. Throughput (requests/second) under concurrent load
//! 3. Hardware/environment disclosure (for reproducibility)
//!
//! This is a standalone binary (not criterion) because RSS measurement
//! doesn't fit criterion's statistical sampling model.

use sh_layer2::{
    AgentConfig, AgentRuntime, AgentRuntimeTrait, Tool as Layer2Tool, ToolRegistry,
    ToolRegistryTrait, ToolResult,
};
use std::sync::Arc;
use std::time::Instant;

/// Get current process RSS in bytes (sysinfo 0.30 API)
fn get_rss_bytes() -> u64 {
    use sysinfo::{Pid, System};
    let mut sys = System::new();
    sys.refresh_process(Pid::from_u32(std::process::id()));
    sys.process(Pid::from_u32(std::process::id()))
        .map(|p| p.memory()) // sysinfo 0.30 returns bytes
        .unwrap_or(0)
}

/// Print environment disclosure (SPEC CPU 2026 run rules aligned)
fn print_environment() {
    println!("=== Environment Disclosure ===");
    println!("  OS: {}", std::env::consts::OS);
    println!("  Arch: {}", std::env::consts::ARCH);

    // CPU info
    let sys = sysinfo::System::new();
    let cpus = sys.cpus();
    if let Some(cpu) = cpus.first() {
        println!("  CPU: {} ({} cores)", cpu.brand(), cpus.len());
        println!("  CPU Frequency: {} MHz", cpu.frequency());
    }

    // Memory info (separate instance for memory refresh)
    let mem_sys = sysinfo::System::new_all();
    println!(
        "  Total Memory: {} GB",
        mem_sys.total_memory() / 1024 / 1024 / 1024
    );

    // Rust version
    println!("  Rust: {}", rustc_version());

    // Build profile
    println!(
        "  Profile: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!("=============================");
}

fn rustc_version() -> String {
    // We can't call rustc at runtime; use compile-time info
    option_env!("CARGO_PKG_RUST_VERSION")
        .unwrap_or("unknown")
        .to_string()
}

/// Mock tool for memory benchmark — allocates ~1KB per call
struct MemoryMockTool;

#[async_trait::async_trait]
impl Layer2Tool for MemoryMockTool {
    fn name(&self) -> &str {
        "mock_mem"
    }
    fn description(&self) -> &str {
        "Memory benchmark tool (allocates ~1KB)"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    async fn execute(&self, _args: &str) -> sh_layer2::Layer2Result<ToolResult> {
        // Simulate a tool that allocates some memory (like reading a file)
        let data = vec![0u8; 1024];
        Ok(ToolResult {
            tool_call_id: String::new(),
            name: "mock_mem".to_string(),
            content: format!("processed {} bytes", data.len()),
            is_error: false,
        })
    }
}

/// Measure peak RSS after running N agent iterations
async fn bench_memory(iterations: usize) {
    println!("\n=== Memory Benchmark ({} iterations) ===", iterations);

    let rss_before = get_rss_bytes();
    println!(
        "  RSS before: {:.2} MB",
        rss_before as f64 / 1024.0 / 1024.0
    );

    let peak_rss = Arc::new(std::sync::atomic::AtomicU64::new(rss_before));
    let peak = peak_rss.clone();

    let registry = Arc::new(ToolRegistry::new());
    ToolRegistryTrait::register(&*registry, Box::new(MemoryMockTool)).expect("register");

    let config = AgentConfig {
        max_iterations: 5,
        ..Default::default()
    };

    let start = Instant::now();

    // Spawn a monitor thread to track peak RSS
    let monitor_handle = tokio::spawn(async move {
        loop {
            let current = get_rss_bytes();
            peak.fetch_max(current, std::sync::atomic::Ordering::Relaxed);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });

    for i in 0..iterations {
        let runtime = AgentRuntime::new(
            Arc::new(sh_layer2::ConcurrentSessionManager::new(iterations + 10)),
            registry.clone(),
        );
        let _ = runtime
            .run(&format!("memory task {}", i), config.clone())
            .await
            .expect("agent run");
    }

    monitor_handle.abort();
    let elapsed = start.elapsed();
    let peak_val = peak_rss.load(std::sync::atomic::Ordering::Relaxed);

    println!(
        "  RSS after:  {:.2} MB",
        get_rss_bytes() as f64 / 1024.0 / 1024.0
    );
    println!("  Peak RSS:   {:.2} MB", peak_val as f64 / 1024.0 / 1024.0);
    println!("  Duration:   {:.2}s", elapsed.as_secs_f64());
    println!(
        "  Per-iter:   {:.1} µs",
        elapsed.as_micros() as f64 / iterations as f64
    );
}

/// Measure throughput: concurrent agent loops for fixed duration
async fn bench_throughput(concurrency: usize, duration_secs: u64) {
    println!(
        "\n=== Throughput Benchmark ({} concurrent, {}s) ===",
        concurrency, duration_secs
    );

    let registry = Arc::new(ToolRegistry::new());
    ToolRegistryTrait::register(&*registry, Box::new(MemoryMockTool)).expect("register");

    let config = AgentConfig {
        max_iterations: 5,
        ..Default::default()
    };

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let start = Instant::now();

    let mut handles = Vec::new();
    for worker in 0..concurrency {
        let registry = registry.clone();
        let config = config.clone();
        let counter = counter.clone();
        let duration = std::time::Duration::from_secs(duration_secs);

        handles.push(tokio::spawn(async move {
            let runtime = AgentRuntime::new(
                Arc::new(sh_layer2::ConcurrentSessionManager::new(100_000)),
                registry,
            );
            let mut local_count = 0usize;
            while start.elapsed() < duration {
                let _ = runtime
                    .run(&format!("throughput task {}", worker), config.clone())
                    .await;
                local_count += 1;
            }
            counter.fetch_add(local_count, std::sync::atomic::Ordering::Relaxed);
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let elapsed = start.elapsed();
    let total = counter.load(std::sync::atomic::Ordering::Relaxed);
    let rps = total as f64 / elapsed.as_secs_f64();

    println!("  Total requests: {}", total);
    println!("  Duration:        {:.2}s", elapsed.as_secs_f64());
    println!("  Throughput:      {:.2} req/s", rps);
    println!("  Per-worker:      {:.2} req/s", rps / concurrency as f64);
    println!(
        "  Peak RSS:        {:.2} MB",
        get_rss_bytes() as f64 / 1024.0 / 1024.0
    );
}

#[tokio::main]
async fn main() {
    print_environment();

    // Memory: 100, 1000, 5000 iterations
    bench_memory(100).await;
    bench_memory(1000).await;
    bench_memory(5000).await;

    // Throughput: 1, 4, 8, 16 concurrent workers × 10 seconds
    bench_throughput(1, 10).await;
    bench_throughput(4, 10).await;
    bench_throughput(8, 10).await;
    bench_throughput(16, 10).await;

    println!("\n=== Benchmark Complete ===");
}
