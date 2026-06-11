//! 配置加载性能基准测试
//!
//! 测试 ConfigManager 的加载性能。

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sh_layer1::ConfigManager;

fn bench_config_creation(c: &mut Criterion) {
    c.bench_function("config_new", |b| b.iter(|| black_box(ConfigManager::new())));
}

fn bench_config_from_env(c: &mut Criterion) {
    c.bench_function("config_from_env", |b| {
        b.iter(|| black_box(ConfigManager::from_env()))
    });
}

fn bench_config_default_path(c: &mut Criterion) {
    c.bench_function("config_default_path", |b| {
        b.iter(|| black_box(ConfigManager::default_config_path()))
    });
}

fn bench_config_add_provider(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_provider");

    for size in [1, 10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut config = ConfigManager::new();
                for i in 0..*size {
                    config.add_provider(
                        &format!("provider_{}", i),
                        sh_layer1::ProviderConfig {
                            api_key: format!("key_{}", i),
                            base_url: format!("https://api{}.example.com", i),
                            model: format!("model-{}", i),
                            api_format: "openai".to_string(),
                            default_max_tokens: 4096,
                            default_temperature: 0.7,
                        },
                    );
                }
                black_box(config)
            })
        });
    }
    group.finish();
}

fn bench_config_list_providers(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_providers");

    for size in [1, 10, 50, 100].iter() {
        let mut config = ConfigManager::new();
        for i in 0..*size {
            config.add_provider(
                &format!("provider_{}", i),
                sh_layer1::ProviderConfig {
                    api_key: format!("key_{}", i),
                    base_url: format!("https://api{}.example.com", i),
                    model: format!("model-{}", i),
                    api_format: "openai".to_string(),
                    default_max_tokens: 4096,
                    default_temperature: 0.7,
                },
            );
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| black_box(config.list_providers()))
        });
    }
    group.finish();
}

fn bench_config_switch_provider(c: &mut Criterion) {
    // 在 bench 函数外部初始化配置
    let providers: Vec<(String, sh_layer1::ProviderConfig)> = (0..100)
        .map(|i| {
            (
                format!("provider_{}", i),
                sh_layer1::ProviderConfig {
                    api_key: format!("key_{}", i),
                    base_url: format!("https://api{}.example.com", i),
                    model: format!("model-{}", i),
                    api_format: "openai".to_string(),
                    default_max_tokens: 4096,
                    default_temperature: 0.7,
                },
            )
        })
        .collect();

    c.bench_function("config_switch_provider", |b| {
        b.iter(|| {
            let mut config = ConfigManager::new();
            for (name, prov_config) in &providers {
                config.add_provider(name, prov_config.clone());
            }
            // 执行切换操作
            for i in 0..10 {
                let provider_name = format!("provider_{}", i);
                let _ = config.use_provider(&provider_name);
            }
            black_box(config)
        })
    });
}

criterion_group!(
    benches,
    bench_config_creation,
    bench_config_from_env,
    bench_config_default_path,
    bench_config_add_provider,
    bench_config_list_providers,
    bench_config_switch_provider,
);

criterion_main!(benches);
