# Playwright MCP 集成技术方案

> 版本: v1.0
> 日期: 2026-05-28
> 状态: 评估完成

---

## 一、项目概述

### 1.1 Playwright MCP 简介

Playwright MCP 是 Microsoft 官方维护的浏览器自动化 MCP 服务器，为 AI Agent 提供完整的浏览器控制能力。

| 属性 | 值 |
|------|-----|
| 包名 | `@playwright/mcp` |
| 版本 | 0.0.75 |
| 维护者 | Microsoft |
| 协议版本 | MCP 2024-11-05 |
| 仓库 | https://github.com/microsoft/playwright-mcp |

### 1.2 核心能力

1. **浏览器控制**
   - 多浏览器: Chrome, Firefox, WebKit, Edge
   - Headless/Headed 模式
   - 设备模拟 (viewport, user-agent)

2. **页面交互**
   - 导航、点击、输入、截图
   - 等待元素、处理弹窗
   - 表单填写、文件上传

3. **高级功能**
   - Vision 能力 (`--caps vision`)
   - PDF 生成 (`--caps pdf`)
   - DevTools 集成 (`--caps devtools`)
   - Code Generation (TypeScript)

---

## 二、架构分析

### 2.1 Playwright MCP 架构

```
┌─────────────────────────────────────────────┐
│              Playwright MCP                  │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐    │
│  │ Browser │  │  Page    │  │ Snapshot │    │
│  │ Control │  │  Actions │  │  Vision  │    │
│  └────┬────┘  └────┬────┘  └────┬────┘    │
│       │            │            │           │
│  ┌────▼────────────▼────────────▼────┐     │
│  │         MCP Protocol Layer         │     │
│  │    (stdio / SSE / TCP transport)   │     │
│  └─────────────────────────────────────┘     │
└─────────────────────────────────────────────┘
                        │
                        ▼
              MCP Client (Continuum)
```

### 2.2 Continuum MCP Bridge 现状

Layer4 的 `mcp_bridge` 模块已实现：

| 模块 | 功能 | 状态 |
|------|------|------|
| `protocol.rs` | MCP 消息类型定义 | ✅ 完成 |
| `transport.rs` | Stdio/TCP/Memory 传输 | ✅ 完成 |
| `client.rs` | MCP 客户端管理器 | ✅ 完成 |
| `bridge.rs` | MCP Bridge 服务端 | ✅ 完成 |
| `handler.rs` | 请求处理器 | ✅ 完成 |

**协议版本**: MCP 2024-11-05 (与 Playwright MCP 一致)

### 2.3 兼容性评估

| 维度 | Playwright MCP | Continuum MCP Bridge | 兼容性 |
|------|-----------------|---------------------|--------|
| 协议版本 | 2024-11-05 | 2024-11-05 | ✅ 完全兼容 |
| 传输层 | stdio, SSE | stdio, TCP, Memory | ✅ 可适配 |
| 消息格式 | JSON-RPC 2.0 | JSON-RPC 2.0 | ✅ 完全兼容 |
| 工具调用 | `tools/call` | `tools/call` | ✅ 完全兼容 |

---

## 三、集成方案设计

### 3.1 配置层集成

在 Continuum YAML 配置中添加 Playwright MCP:

```yaml
# super.yaml
model:
  provider: openai
  name: gpt-4-turbo

tools:
  mcp:
    - name: playwright
      command: npx
      args: 
        - "@playwright/mcp@latest"
        - "--headless"
        - "--browser"
        - "chrome"
        - "--caps"
        - "vision,pdf"
      # 可选安全限制
      # env:
      #   ALLOWED_HOSTS: "example.com,trusted-site.com"
```

### 3.2 代码层集成

在 `rust/layer4/src/mcp_bridge/client.rs` 添加 Playwright 预设:

```rust
/// 预设的 MCP 服务器配置
pub fn preset_servers() -> Vec<McpServerConfig> {
    vec![
        // 文件系统 MCP
        McpServerConfig {
            name: "filesystem".to_string(),
            transport: McpTransportType::Stdio {
                command: "mcp-server-filesystem".to_string(),
                args: vec!["--root".to_string(), ".".to_string()],
            },
            auto_reconnect: true,
            reconnect_interval_ms: 5000,
        },
        // Playwright MCP (新增)
        McpServerConfig {
            name: "playwright".to_string(),
            transport: McpTransportType::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "@playwright/mcp@latest".to_string(),
                    "--headless".to_string(),
                    "--browser".to_string(),
                    "chrome".to_string(),
                ],
            },
            auto_reconnect: true,
            reconnect_interval_ms: 5000,
        },
    ]
}
```

### 3.3 工具映射

Playwright MCP 提供的工具 (预估):

| 工具名 | 功能 | 用途 |
|--------|------|------|
| `browser_navigate` | 导航到 URL | 页面访问 |
| `browser_click` | 点击元素 | 交互操作 |
| `browser_type` | 输入文本 | 表单填写 |
| `browser_screenshot` | 截图 | 视觉验证 |
| `browser_evaluate` | 执行 JS | 数据提取 |
| `browser_wait_for` | 等待元素 | 同步控制 |

### 3.4 Agent 使用示例

```rust
use sh_layer4::mcp_bridge::{McpClientManager, McpServerConfig};

async fn web_automation_example() -> Result<()> {
    let manager = McpClientManager::new();
    
    // 添加 Playwright MCP
    manager.add_server(McpServerConfig {
        name: "playwright".to_string(),
        transport: McpTransportType::Stdio {
            command: "npx".to_string(),
            args: vec!["@playwright/mcp@latest".to_string()],
        },
        auto_reconnect: true,
        reconnect_interval_ms: 5000,
    }).await?;
    
    // 连接
    manager.connect("playwright").await?;
    
    // 导航到页面
    let result = manager.call_tool("browser_navigate", serde_json::json!({
        "url": "https://example.com"
    })).await?;
    
    // 截图
    let screenshot = manager.call_tool("browser_screenshot", serde_json::json!({})).await?;
    
    Ok(())
}
```

---

## 四、依赖清单

### 4.1 运行时依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| Node.js | >= 18.0 | 运行 Playwright MCP |
| npm/npx | >= 9.0 | 包管理 |
| @playwright/mcp | 0.0.75 | MCP 服务器 |
| Playwright browsers | - | 浏览器引擎 |

### 4.2 可选依赖

| 依赖 | 用途 |
|------|------|
| `--caps vision` | AI 视觉能力 |
| `--caps pdf` | PDF 生成 |
| `--caps devtools` | DevTools 访问 |

---

## 五、风险分析

### 5.1 技术风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 资源消耗高 | 内存/CPU 占用大 | 配置 `--headless`，限制并发 |
| 启动慢 | 浏览器启动耗时 | 预热连接池 |
| 依赖 Node.js | 增加部署复杂度 | Docker 容器化 |
| 网络安全 | Agent 可访问任意站点 | `--allowed-hosts` 限制 |

### 5.2 安全建议

```bash
# 生产环境配置
npx @playwright/mcp@latest \
  --headless \
  --browser chrome \
  --allowed-hosts "trusted-domain.com" \
  --blocked-origins "malware-site.com"
```

---

## 六、实施计划

### 6.1 Phase 1: 基础集成 (P0)

- [ ] 添加 Playwright MCP 到预设配置
- [ ] 测试 stdio 传输连接
- [ ] 验证基础工具调用

### 6.2 Phase 2: 功能增强 (P1)

- [ ] 添加 YAML 配置支持
- [ ] 实现连接池管理
- [ ] 添加安全配置选项

### 6.3 Phase 3: 示例与文档 (P2)

- [ ] 编写使用示例
- [ ] 添加 API 文档
- [ ] 创建演示场景

---

## 七、结论

### 7.1 可行性评估

| 维度 | 评估 |
|------|------|
| 技术可行性 | ✅ 高 - MCP 协议完全兼容 |
| 集成复杂度 | ⭐⭐ 低 - 配置驱动即可 |
| 维护成本 | ⭐ 低 - Microsoft 官方维护 |
| 业务价值 | ⭐⭐⭐⭐⭐ 极高 - Web 自动化核心能力 |

### 7.2 建议

**强烈推荐集成**。Playwright MCP 提供:

1. **完整能力**: 完整的浏览器自动化工具链
2. **官方维护**: Microsoft 维护，版本更新频繁
3. **无缝集成**: MCP 协议与 Continuum 完全兼容
4. **低门槛**: npx 一键启动，无需额外配置

### 7.3 下一步行动

1. **立即**: 将 Playwright MCP 添加到预设配置
2. **本周**: 完成基础集成测试
3. **下周**: 编写用户文档和示例

---

**文档状态**: 评估完成，待实施
