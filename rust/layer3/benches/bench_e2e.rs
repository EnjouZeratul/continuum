//! End-to-end agent loop benchmark — aligns with AutoAgents 2026 methodology.
//!
//! Measures the full agent loop (session → LLM step → tool selection →
//! tool execution → response) using the built-in simulate_llm_step fallback
//! (no real LLM, no network I/O). This isolates the framework overhead.
//!
//! Each iteration creates a fresh AgentRuntime (serverless-style: one runtime
//! per request) to avoid session accumulation across criterion iterations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sh_layer2::{
    AgentConfig, AgentRuntime, AgentRuntimeTrait, Tool as Layer2Tool, ToolRegistry,
    ToolRegistryTrait, ToolResult,
};
use sh_layer3::register_builtin_tools;
use sh_layer3::{DefaultToolExecutor, ToolExecutor, ToolRequest};
use std::sync::Arc;
use std::time::Instant;

/// Deterministic mock tool — returns immediately, no I/O.
struct FastMockTool;

#[async_trait::async_trait]
impl Layer2Tool for FastMockTool {
    fn name(&self) -> &str {
        "mock_fast"
    }
    fn description(&self) -> &str {
        "Deterministic mock tool for benchmarking (no I/O)"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {"task": {"type": "string"}}})
    }
    async fn execute(&self, _args: &str) -> sh_layer2::Layer2Result<ToolResult> {
        Ok(ToolResult {
            tool_call_id: String::new(),
            name: "mock_fast".to_string(),
            content: "ok".to_string(),
            is_error: false,
        })
    }
}

/// I/O-touching tool — simulates 1KB read.
struct IoMockTool;

#[async_trait::async_trait]
impl Layer2Tool for IoMockTool {
    fn name(&self) -> &str {
        "mock_io"
    }
    fn description(&self) -> &str {
        "I/O mock tool for benchmarking"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    async fn execute(&self, _args: &str) -> sh_layer2::Layer2Result<ToolResult> {
        let data = vec![0u8; 1024];
        Ok(ToolResult {
            tool_call_id: String::new(),
            name: "mock_io".to_string(),
            content: String::from_utf8_lossy(&data).to_string(),
            is_error: false,
        })
    }
}

/// Create a fresh agent runtime (serverless-style: one per request)
fn make_runtime(tool: Box<dyn Layer2Tool>) -> AgentRuntime {
    let registry = Arc::new(ToolRegistry::new());
    ToolRegistryTrait::register(&*registry, tool).expect("register");
    AgentRuntime::new(
        Arc::new(sh_layer2::ConcurrentSessionManager::new(1_000_000)),
        registry,
    )
}

/// E2E: fresh runtime + full agent loop per iteration
fn bench_agent_loop(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("e2e_agent_loop");

    for &max_iter in &[1i32, 5, 10] {
        let config = AgentConfig {
            max_iterations: max_iter,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::new("full_loop", max_iter),
            &max_iter,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let runtime = make_runtime(Box::new(FastMockTool));
                        let start = Instant::now();
                        let result = runtime
                            .run(black_box("benchmark task"), config.clone())
                            .await
                            .expect("agent run");
                        (start.elapsed(), result.iterations)
                    })
                });
            },
        );
    }

    // With I/O tool (1KB per call)
    {
        let config = AgentConfig {
            max_iterations: 5,
            ..Default::default()
        };

        group.bench_function("loop_with_io_1kb", |b| {
            b.iter(|| {
                rt.block_on(async {
                    let runtime = make_runtime(Box::new(IoMockTool));
                    let start = Instant::now();
                    let _ = runtime
                        .run(black_box("read task"), config.clone())
                        .await
                        .expect("agent run");
                    start.elapsed()
                })
            });
        });
    }

    group.finish();
}

/// E2E with 14 builtin tools registered (realistic dispatch)
fn bench_agent_loop_builtin(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = AgentConfig {
        max_iterations: 3,
        ..Default::default()
    };

    let mut group = c.benchmark_group("e2e_agent_loop");
    group.bench_function("loop_14_builtin_tools", |b| {
        b.iter(|| {
            rt.block_on(async {
                let registry = Arc::new(ToolRegistry::new());
                register_builtin_tools(&registry).expect("register");
                let runtime = AgentRuntime::new(
                    Arc::new(sh_layer2::ConcurrentSessionManager::new(1_000_000)),
                    registry,
                );
                let start = Instant::now();
                let result = runtime
                    .run(black_box("builtin task"), config.clone())
                    .await
                    .expect("agent run");
                (start.elapsed(), result.iterations)
            })
        })
    });
    group.finish();
}

/// Tool dispatch chain (layer3 DefaultToolExecutor, 50+ tools registered)
fn bench_tool_dispatch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let executor = DefaultToolExecutor::new();

    let mut group = c.benchmark_group("tool_dispatch");

    group.bench_function("uuid_generate", |b| {
        b.iter(|| {
            rt.block_on(async {
                executor
                    .execute(ToolRequest {
                        call_id: "bench".into(),
                        name: "uuid_generate".into(),
                        arguments: serde_json::json!({}),
                    })
                    .await
                    .expect("uuid")
            })
        });
    });

    group.bench_function("count_lines", |b| {
        b.iter(|| {
            rt.block_on(async {
                executor
                    .execute(ToolRequest {
                        call_id: "bench".into(),
                        name: "count_lines".into(),
                        arguments: serde_json::json!({"text": "line1\nline2\nline3"}),
                    })
                    .await
                    .expect("count")
            })
        });
    });

    group.finish();
}

/// Cold start: registry initialization
fn bench_cold_start(c: &mut Criterion) {
    c.bench_function("cold_start_registry_50_tools", |b| {
        b.iter(|| {
            let start = Instant::now();
            let executor = DefaultToolExecutor::new();
            let _count = executor.list_tools().len();
            black_box((start.elapsed(), _count));
        });
    });

    c.bench_function("cold_start_agent_runtime", |b| {
        b.iter(|| {
            let start = Instant::now();
            let registry = Arc::new(ToolRegistry::new());
            let _ = register_builtin_tools(&registry);
            let _runtime = AgentRuntime::new(
                Arc::new(sh_layer2::ConcurrentSessionManager::new(1_000_000)),
                registry,
            );
            black_box(start.elapsed());
        });
    });
}

criterion_group!(
    benches,
    bench_agent_loop,
    bench_agent_loop_builtin,
    bench_tool_dispatch,
    bench_cold_start,
);
criterion_main!(benches);
