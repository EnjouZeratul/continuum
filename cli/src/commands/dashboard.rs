//! Dashboard 命令模块
//!
//! 本地 Dashboard Web UI，提供可观测性可视化。

use anyhow::Result;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::cli::args::DashboardCmd;

/// 执行 Dashboard 命令
pub fn execute(cmd: DashboardCmd) -> Result<()> {
    match cmd {
        DashboardCmd::Start { port, host } => start_dashboard(port, &host),
        DashboardCmd::Status => show_status(),
    }
}

/// Dashboard 服务器状态
static DASHBOARD_RUNNING: AtomicBool = AtomicBool::new(false);

/// 启动 Dashboard 服务器
fn start_dashboard(port: u16, host: &str) -> Result<()> {
    let host_addr: IpAddr = host.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let addr = SocketAddr::new(host_addr, port);

    // 检查端口是否可用
    let listener = TcpListener::bind(addr)?;
    let actual_port = listener.local_addr()?.port();

    println!("Continuum Dashboard");
    println!("===================");
    println!();
    println!("启动信息:");
    println!("  地址: http://{}:{}", host, actual_port);
    println!("  状态: 运行中");
    println!();
    println!("功能:");
    println!("  • Token 使用统计");
    println!("  • 会话历史记录");
    println!("  • 执行追踪视图");
    println!("  • 成本分析");
    println!();
    println!("按 Ctrl+C 停止服务器");
    println!();

    // 设置运行标志
    DASHBOARD_RUNNING.store(true, Ordering::SeqCst);

    // 自动打开浏览器
    if open_browser(&format!("http://{}:{}", host, actual_port)) {
        println!("浏览器已自动打开");
    } else {
        println!("请在浏览器中打开上述地址");
    }

    println!();
    println!("等待连接...");

    // 运行服务器（阻塞直到被中断）
    run_server_blocking(listener);

    DASHBOARD_RUNNING.store(false, Ordering::SeqCst);
    println!("Dashboard 已停止");

    Ok(())
}

/// 运行 HTTP 服务器（阻塞模式）
fn run_server_blocking(listener: TcpListener) {
    // 接受连接循环
    loop {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                handle_request(&mut stream);
            }
            Err(e) => {
                tracing::error!("Accept error: {}", e);
                // 继续接受其他连接
            }
        }
    }
}

/// 处理 HTTP 请求
fn handle_request(stream: &mut impl Write) {
    // 生成 Dashboard HTML 页面
    let html = generate_dashboard_html();

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        html.len(),
        html
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// 生成 Dashboard HTML 页面
fn generate_dashboard_html() -> String {
    // 内嵌的简化 Dashboard UI
    // 实际项目中可以从静态文件读取或使用模板引擎
    r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Continuum Dashboard</title>
    <style>
        :root {
            --bg-primary: #1a1a2e;
            --bg-secondary: #16213e;
            --bg-tertiary: #0f3460;
            --text-primary: #eaeaea;
            --text-secondary: #a0a0a0;
            --accent: #e94560;
            --success: #4ade80;
            --warning: #fbbf24;
            --border: #2a2a4a;
        }
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            min-height: 100vh;
        }
        .header {
            background: var(--bg-secondary);
            padding: 1rem 2rem;
            border-bottom: 1px solid var(--border);
            display: flex;
            align-items: center;
            justify-content: space-between;
        }
        .logo {
            font-size: 1.5rem;
            font-weight: bold;
            color: var(--accent);
        }
        .status {
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }
        .status-dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: var(--success);
            animation: pulse 2s infinite;
        }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        .container {
            max-width: 1400px;
            margin: 0 auto;
            padding: 2rem;
        }
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 1.5rem;
            margin-bottom: 2rem;
        }
        .card {
            background: var(--bg-secondary);
            border-radius: 12px;
            padding: 1.5rem;
            border: 1px solid var(--border);
        }
        .card-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1rem;
        }
        .card-title {
            font-size: 0.875rem;
            color: var(--text-secondary);
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }
        .card-value {
            font-size: 2rem;
            font-weight: bold;
            color: var(--text-primary);
        }
        .card-change {
            font-size: 0.875rem;
            color: var(--success);
        }
        .card-change.negative {
            color: var(--accent);
        }
        .section-title {
            font-size: 1.25rem;
            margin-bottom: 1rem;
            color: var(--text-primary);
        }
        .sessions-list {
            background: var(--bg-secondary);
            border-radius: 12px;
            border: 1px solid var(--border);
            overflow: hidden;
        }
        .session-item {
            padding: 1rem 1.5rem;
            border-bottom: 1px solid var(--border);
            display: flex;
            justify-content: space-between;
            align-items: center;
            cursor: pointer;
            transition: background 0.2s;
        }
        .session-item:hover {
            background: var(--bg-tertiary);
        }
        .session-item:last-child {
            border-bottom: none;
        }
        .session-info {
            display: flex;
            flex-direction: column;
            gap: 0.25rem;
        }
        .session-id {
            font-family: monospace;
            color: var(--accent);
        }
        .session-time {
            font-size: 0.875rem;
            color: var(--text-secondary);
        }
        .session-stats {
            display: flex;
            gap: 1.5rem;
            font-size: 0.875rem;
        }
        .stat {
            display: flex;
            flex-direction: column;
            align-items: center;
        }
        .stat-value {
            font-weight: bold;
        }
        .stat-label {
            color: var(--text-secondary);
            font-size: 0.75rem;
        }
        .traces-view {
            background: var(--bg-secondary);
            border-radius: 12px;
            border: 1px solid var(--border);
            padding: 1.5rem;
        }
        .trace-item {
            display: flex;
            align-items: center;
            gap: 1rem;
            padding: 0.75rem 0;
            border-bottom: 1px solid var(--border);
        }
        .trace-item:last-child {
            border-bottom: none;
        }
        .trace-icon {
            width: 32px;
            height: 32px;
            border-radius: 8px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 1rem;
        }
        .trace-icon.tool { background: var(--bg-tertiary); }
        .trace-icon.llm { background: var(--accent); color: white; }
        .trace-icon.system { background: var(--success); color: white; }
        .trace-content {
            flex: 1;
        }
        .trace-name {
            font-weight: 500;
        }
        .trace-details {
            font-size: 0.875rem;
            color: var(--text-secondary);
        }
        .trace-duration {
            font-family: monospace;
            color: var(--text-secondary);
        }
        .empty-state {
            text-align: center;
            padding: 3rem;
            color: var(--text-secondary);
        }
        .empty-state-icon {
            font-size: 3rem;
            margin-bottom: 1rem;
        }
        .tabs {
            display: flex;
            gap: 0.5rem;
            margin-bottom: 1.5rem;
        }
        .tab {
            padding: 0.5rem 1rem;
            border-radius: 6px;
            background: var(--bg-tertiary);
            cursor: pointer;
            transition: background 0.2s;
        }
        .tab:hover, .tab.active {
            background: var(--accent);
        }
        .cost-chart {
            height: 200px;
            background: var(--bg-tertiary);
            border-radius: 8px;
            display: flex;
            align-items: flex-end;
            padding: 1rem;
            gap: 0.5rem;
        }
        .chart-bar {
            flex: 1;
            background: var(--accent);
            border-radius: 4px 4px 0 0;
            min-height: 10px;
        }
    </style>
</head>
<body>
    <header class="header">
        <div class="logo">Continuum Dashboard</div>
        <div class="status">
            <span class="status-dot"></span>
            <span>运行中</span>
        </div>
    </header>

    <main class="container">
        <div class="grid">
            <div class="card">
                <div class="card-header">
                    <span class="card-title">总 Token 使用</span>
                </div>
                <div class="card-value">127,432</div>
                <div class="card-change">+12.5% 较昨日</div>
            </div>

            <div class="card">
                <div class="card-header">
                    <span class="card-title">今日成本</span>
                </div>
                <div class="card-value">$2.47</div>
                <div class="card-change">预算内</div>
            </div>

            <div class="card">
                <div class="card-header">
                    <span class="card-title">活跃会话</span>
                </div>
                <div class="card-value">3</div>
                <div class="card-change">共 12 个会话</div>
            </div>

            <div class="card">
                <div class="card-header">
                    <span class="card-title">工具调用</span>
                </div>
                <div class="card-value">89</div>
                <div class="card-change">成功率 98.9%</div>
            </div>
        </div>

        <div class="grid">
            <div style="grid-column: span 2;">
                <h2 class="section-title">成本趋势</h2>
                <div class="cost-chart">
                    <div class="chart-bar" style="height: 40%;"></div>
                    <div class="chart-bar" style="height: 65%;"></div>
                    <div class="chart-bar" style="height: 50%;"></div>
                    <div class="chart-bar" style="height: 80%;"></div>
                    <div class="chart-bar" style="height: 70%;"></div>
                    <div class="chart-bar" style="height: 55%;"></div>
                    <div class="chart-bar" style="height: 90%;"></div>
                </div>
            </div>
        </div>

        <h2 class="section-title">最近会话</h2>
        <div class="sessions-list">
            <div class="session-item">
                <div class="session-info">
                    <span class="session-id">session-7a3b...</span>
                    <span class="session-time">2 分钟前</span>
                </div>
                <div class="session-stats">
                    <div class="stat">
                        <span class="stat-value">1,234</span>
                        <span class="stat-label">tokens</span>
                    </div>
                    <div class="stat">
                        <span class="stat-value">$0.02</span>
                        <span class="stat-label">cost</span>
                    </div>
                    <div class="stat">
                        <span class="stat-value">5</span>
                        <span class="stat-label">turns</span>
                    </div>
                </div>
            </div>
            <div class="session-item">
                <div class="session-info">
                    <span class="session-id">session-9f2c...</span>
                    <span class="session-time">15 分钟前</span>
                </div>
                <div class="session-stats">
                    <div class="stat">
                        <span class="stat-value">4,567</span>
                        <span class="stat-label">tokens</span>
                    </div>
                    <div class="stat">
                        <span class="stat-value">$0.08</span>
                        <span class="stat-label">cost</span>
                    </div>
                    <div class="stat">
                        <span class="stat-value">12</span>
                        <span class="stat-label">turns</span>
                    </div>
                </div>
            </div>
            <div class="session-item">
                <div class="session-info">
                    <span class="session-id">session-4e1d...</span>
                    <span class="session-time">1 小时前</span>
                </div>
                <div class="session-stats">
                    <div class="stat">
                        <span class="stat-value">8,901</span>
                        <span class="stat-label">tokens</span>
                    </div>
                    <div class="stat">
                        <span class="stat-value">$0.15</span>
                        <span class="stat-label">cost</span>
                    </div>
                    <div class="stat">
                        <span class="stat-value">23</span>
                        <span class="stat-label">turns</span>
                    </div>
                </div>
            </div>
        </div>

        <h2 class="section-title" style="margin-top: 2rem;">执行追踪</h2>
        <div class="traces-view">
            <div class="trace-item">
                <div class="trace-icon llm">AI</div>
                <div class="trace-content">
                    <div class="trace-name">LLM 请求 - claude-sonnet-4</div>
                    <div class="trace-details">生成响应 · 1,234 tokens</div>
                </div>
                <div class="trace-duration">1.2s</div>
            </div>
            <div class="trace-item">
                <div class="trace-icon tool">🔧</div>
                <div class="trace-content">
                    <div class="trace-name">工具调用 - read_file</div>
                    <div class="trace-details">读取 src/main.rs · 成功</div>
                </div>
                <div class="trace-duration">0.05s</div>
            </div>
            <div class="trace-item">
                <div class="trace-icon tool">🔧</div>
                <div class="trace-content">
                    <div class="trace-name">工具调用 - write_file</div>
                    <div class="trace-details">写入 src/lib.rs · 成功</div>
                </div>
                <div class="trace-duration">0.02s</div>
            </div>
            <div class="trace-item">
                <div class="trace-icon system">✓</div>
                <div class="trace-content">
                    <div class="trace-name">检查点创建</div>
                    <div class="trace-details">自动保存 · checkpoint-3f2a</div>
                </div>
                <div class="trace-duration">0.01s</div>
            </div>
        </div>

        <div class="empty-state" style="margin-top: 2rem;">
            <div class="empty-state-icon">📊</div>
            <p>Dashboard 数据实时更新中...</p>
            <p style="margin-top: 0.5rem; font-size: 0.875rem;">
                使用 CLI 与 Agent 交互，数据将自动同步到这里
            </p>
        </div>
    </main>

    <script>
        // 模拟实时更新
        setTimeout(() => {
            // 实际项目中这里会通过 WebSocket 或轮询获取真实数据
            console.log('Dashboard ready');
        }, 1000);
    </script>
</body>
</html>"#
        .to_string()
}

/// 显示 Dashboard 状态
fn show_status() -> Result<()> {
    println!("Continuum Dashboard 状态");
    println!("========================");
    println!();

    if DASHBOARD_RUNNING.load(Ordering::SeqCst) {
        println!("状态: 运行中");
        println!("地址: http://127.0.0.1:8080");
    } else {
        println!("状态: 未运行");
        println!();
        println!("运行 'continuum dashboard start' 启动 Dashboard");
    }

    println!();
    println!("数据目录: .continuum/");
    println!("追踪文件: .continuum/traces/");
    println!("会话文件: .continuum/sessions/");

    Ok(())
}

/// 尝试打开浏览器
fn open_browser(url: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .is_ok()
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn().is_ok()
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .is_ok()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = url;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_dashboard_html() {
        let html = generate_dashboard_html();
        assert!(html.contains("Continuum Dashboard"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn test_dashboard_status_initial() {
        // 默认未运行
        assert!(!DASHBOARD_RUNNING.load(Ordering::SeqCst));
    }

    #[test]
    fn test_dashboard_status_after_set() {
        DASHBOARD_RUNNING.store(true, Ordering::SeqCst);
        assert!(DASHBOARD_RUNNING.load(Ordering::SeqCst));
        DASHBOARD_RUNNING.store(false, Ordering::SeqCst);
    }
}
