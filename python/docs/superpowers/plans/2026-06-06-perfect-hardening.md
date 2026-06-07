# Continuum SDK 完美加固实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复所有审查发现的问题，实现零妥协的完美代码质量

**Architecture:** 四个阶段：P0安全修复 → P1质量修复 → P2测试补充 → P3文档同步

**Tech Stack:** Python 3.11+, pytest, pyright, black, ruff

---

## 核心原则（必须遵循）

1. **动态配置优先**：从环境变量、providers配置动态读取，绝不硬编码默认值
2. **备用映射逐个尝试**：失败时从 BUILTIN_PROVIDERS 按优先级依次尝试
3. **友好日志提示**：每次进入备用方案时，使用 `logger.info()` 告知用户
4. **禁止硬编码最终默认**：极端情况抛出 `RuntimeError` + 配置指引

---

## 文件结构

### 新增文件
```
continuum_sdk/
├── errors.py                    # 统一错误类型层次
├── env.py                       # 安全环境变量访问
└── protocols.py                 # Rust/Python 统一接口协议

tests/
├── test_errors.py               # 错误类型测试
├── test_checkpoint.py           # checkpoint 模块测试
├── test_history.py              # history 模块测试
├── test_fallback.py             # fallback 逻辑测试
└── test_env_security.py         # 环境变量安全测试
```

### 修改文件
```
continuum_sdk/
├── tools/bash.py                # 安全加固
├── tools/builtin.py              # 类型注解修复
├── tools/web.py                 # API Key 安全
├── python_impl.py               # SSRF 防护
├── agent/runtime.py             # 多处修复
├── agent/session.py             # 添加 recover 方法
├── config/loader.py             # 日志规范修复
├── llm/client.py                # API Key 安全存储
└── __init__.py                  # 导出更新

docs/
├── API_REFERENCE.md             # 同步实际 API
└── BEST_PRACTICES.md            # 修复错误导入
```

---

## Phase 0: 安全修复（阻塞发布）

### Task 0.1: 创建统一错误类型模块

**Files:**
- Create: `continuum_sdk/errors.py`
- Test: `tests/test_errors.py`

- [ ] **Step 1: 编写错误类型测试**

```python
# tests/test_errors.py
"""Unified error type tests"""

import pytest
from continuum_sdk.errors import (
    ContinuumError,
    ConfigError,
    ToolExecutionError,
    LLMError,
    AuthenticationError,
    RateLimitError,
    SecurityError,
    ValidationError,
)


class TestErrorHierarchy:
    """Test error type hierarchy"""

    def test_base_error_is_exception(self):
        assert issubclass(ContinuumError, Exception)

    def test_config_error_inherits_base(self):
        assert issubclass(ConfigError, ContinuumError)

    def test_tool_error_inherits_base(self):
        assert issubclass(ToolExecutionError, ContinuumError)

    def test_llm_error_inherits_base(self):
        assert issubclass(LLMError, ContinuumError)

    def test_auth_error_inherits_llm(self):
        assert issubclass(AuthenticationError, LLMError)

    def test_rate_limit_inherits_llm(self):
        assert issubclass(RateLimitError, LLMError)


class TestErrorMessages:
    """Test error message format"""

    def test_continuum_error_message(self):
        error = ContinuumError("Something went wrong")
        assert str(error) == "Something went wrong"

    def test_config_error_with_key(self):
        error = ConfigError("Invalid config", config_key="api_key")
        assert error.config_key == "api_key"

    def test_tool_error_with_tool_name(self):
        error = ToolExecutionError("Tool failed", tool_name="bash")
        assert error.tool_name == "bash"

    def test_llm_error_with_provider(self):
        error = LLMError("API error", provider="anthropic")
        assert error.provider == "anthropic"

    def test_auth_error_includes_suggestions(self):
        error = AuthenticationError("Invalid API key", provider="openai")
        assert error.suggestions is not None
        assert len(error.suggestions) > 0
```

- [ ] **Step 2: 运行测试验证失败**

```bash
pytest tests/test_errors.py -v
```
Expected: FAIL - 模块不存在

- [ ] **Step 3: 实现统一错误类型**

```python
# continuum_sdk/errors.py
"""
Continuum SDK Unified Error Types

Supports:
- Clear error hierarchy
- Structured error information  
- Error serialization/deserialization
- Friendly error messages and fix suggestions
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any

logger = logging.getLogger(__name__)


@dataclass
class ContinuumError(Exception):
    """Base class for all Continuum SDK errors."""

    message: str
    code: str | None = None
    timestamp: datetime = field(default_factory=datetime.now)
    context: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self):
        super().__init__(self.message)

    def __str__(self) -> str:
        return self.message

    def to_dict(self) -> dict[str, Any]:
        return {
            "type": self.__class__.__name__,
            "message": self.message,
            "code": self.code,
            "timestamp": self.timestamp.isoformat(),
            "context": self.context,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ContinuumError":
        error_type = data.get("type", "ContinuumError")
        error_classes = {
            "ContinuumError": cls,
            "ConfigError": ConfigError,
            "ToolExecutionError": ToolExecutionError,
            "LLMError": LLMError,
            "AuthenticationError": AuthenticationError,
            "RateLimitError": RateLimitError,
            "SecurityError": SecurityError,
            "ValidationError": ValidationError,
        }
        error_cls = error_classes.get(error_type, cls)
        if "timestamp" in data:
            data["timestamp"] = datetime.fromisoformat(data["timestamp"])
        return error_cls(**{k: v for k, v in data.items() if k != "type"})


@dataclass
class ConfigError(ContinuumError):
    """Configuration related error."""
    config_key: str | None = None
    suggestions: list[str] = field(default_factory=list)

    def __post_init__(self):
        if self.config_key and self.config_key not in self.message:
            self.message = f"{self.message} (key: {self.config_key})"
        super().__post_init__()


@dataclass
class ToolExecutionError(ContinuumError):
    """Tool execution error."""
    tool_name: str | None = None
    arguments: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self):
        if self.tool_name and self.tool_name not in self.message:
            self.message = f"[{self.tool_name}] {self.message}"
        super().__post_init__()


@dataclass
class LLMError(ContinuumError):
    """LLM API error."""
    provider: str | None = None
    model: str | None = None
    retry_after: int | None = None

    def __post_init__(self):
        if self.provider and self.provider not in self.message:
            self.message = f"[{self.provider}] {self.message}"
        super().__post_init__()


@dataclass
class AuthenticationError(LLMError):
    """Authentication error (API Key invalid or expired)."""
    suggestions: list[str] = field(default_factory=lambda: [
        "Verify API key is correctly set",
        "Confirm API key has not expired",
        "Verify API key has sufficient permissions",
        "Check environment variable name is correct",
    ])

    def __post_init__(self):
        self.code = self.code or "AUTH_FAILED"
        super().__post_init__()


@dataclass
class RateLimitError(LLMError):
    """Rate limit error."""
    limit: int | None = None
    remaining: int | None = None
    reset_at: datetime | None = None

    def __post_init__(self):
        self.code = self.code or "RATE_LIMITED"
        super().__post_init__()


@dataclass
class SecurityError(ContinuumError):
    """Security related error."""
    violation_type: str | None = None
    resource: str | None = None

    def __post_init__(self):
        self.code = self.code or "SECURITY_VIOLATION"
        super().__post_init__()


@dataclass
class ValidationError(ContinuumError):
    """Validation error."""
    field: str | None = None
    value: Any = None
    constraints: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self):
        self.code = self.code or "VALIDATION_FAILED"
        super().__post_init__()


__all__ = [
    "ContinuumError",
    "ConfigError",
    "ToolExecutionError",
    "LLMError",
    "AuthenticationError",
    "RateLimitError",
    "SecurityError",
    "ValidationError",
]
```

- [ ] **Step 4: 运行测试验证通过**

```bash
pytest tests/test_errors.py -v
```

- [ ] **Step 5: 更新 __init__.py 导出**

- [ ] **Step 6: 提交**

---

### Task 0.2: 修复 SSRF 漏洞

**Files:**
- Modify: `continuum_sdk/python_impl.py`
- Test: `tests/test_ssrf_protection.py`

**核心要点:**
- 私有 IP 验证：10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
- 阻止 localhost, 127.0.0.1, 0.0.0.0
- DNS 解析后验证
- 重定向目标验证

---

### Task 0.3: 加固 Shell 命令执行

**Files:**
- Modify: `continuum_sdk/tools/bash.py`
- Test: `tests/test_shell_security.py`

**核心要点:**
- 默认 shell=False 模式
- 扩展注入检测：$(), ``, <(), ${}, %0a, %00, ; | && ||
- 环境变量白名单过滤

---

## Phase 1: 质量修复

### Task 1.1: 修复 runtime.py 错误消息重复
### Task 1.2: 添加 AgentConfig.max_iterations 属性
### Task 1.3: 修复 builtin.py 类型注解
### Task 1.4: 创建统一环境变量访问模块
### Task 1.5: 添加 Session.recover 方法

---

## Phase 2: 测试补充

### Task 2.1: 补充 checkpoint 模块测试
### Task 2.2: 补充 history 模块测试
### Task 2.3: 补充 fallback 模块测试

---

## Phase 3: 文档同步

### Task 3.1: 更新 API_REFERENCE.md
### Task 3.2: 修复 BEST_PRACTICES.md 错误导入

---

## Phase 4: 全量接口测试覆盖验证

**Goal:** 确保每个公开接口的每个用例都有测试覆盖

**核心原则:**
1. 每个公开 API 必须有测试
2. 每个接口的正常路径和边界情况都要覆盖
3. 错误处理路径必须测试
4. 测试覆盖率报告作为验收标准

### Task 4.1: 接口清单梳理

- [ ] **Step 1: 提取所有公开接口**
  - 从 `__init__.py` 导出列表提取
  - 从 `continuum_sdk.errors` 提取错误类型
  - 从 `continuum_sdk.config` 提取配置类
  - 从 `continuum_sdk.agent` 提取 Agent、Session 类
  - 从 `continuum_sdk.tools` 提取工具接口

- [ ] **Step 2: 生成接口清单文档**
  - 模块名、类名、方法名、参数、返回值
  - 保存到 `docs/superpowers/api-coverage-checklist.md`

### Task 4.2: 逐接口测试覆盖

- [ ] **Step 1: errors 模块测试覆盖** (已完成: `tests/test_errors.py`)
- [ ] **Step 2: config 模块测试覆盖**
  - `test_config_loader.py`
  - `test_config_providers.py`
- [ ] **Step 3: agent 模块测试覆盖**
  - `test_agent_runtime.py`
  - `test_agent_session.py`
- [ ] **Step 4: tools 模块测试覆盖**
  - `test_tools_bash.py`
  - `test_tools_builtin.py`
- [ ] **Step 5: security 模块测试覆盖**
  - `test_security_path_validator.py`
  - `test_ssrf_protection.py`
  - `test_shell_security.py`

### Task 4.3: 覆盖率报告

```bash
# 生成覆盖率报告
pytest --cov=continuum_sdk --cov-report=html --cov-report=term

# 验收标准: 覆盖率 >= 80%
```

---

## 完成标准

### 最终验证

```bash
# 运行完整测试套件
pytest --tb=short -v

# 类型检查
pyright continuum_sdk/

# 代码风格检查
ruff check continuum_sdk/

# 安全扫描
bandit -r continuum_sdk/
```

Expected: 全部通过

---

## 执行完成状态 ✅

**完成日期**: 2026-06-07

### 测试覆盖验证

| 模块 | 测试文件 | 测试数量 | 状态 |
|------|----------|----------|------|
| errors | test_errors.py | 21 | ✅ 通过 |
| env | test_env.py | 17 | ✅ 通过 |
| config | test_config_loader_coverage.py | 84 | ✅ 通过 |
| agent | test_agent.py | 108 | ✅ 通过 |
| session | test_session.py | 95 | ✅ 通过 |
| tools | test_tools.py | 123 | ✅ 通过 |
| security | test_security.py | 56 | ✅ 通过 |
| ssrf | test_ssrf_protection.py | 52 | ✅ 通过 |
| shell | test_shell_security.py | 41 | ✅ 通过 |

**全量测试结果**: 3423 passed, 13 skipped

### 测试质量改进

- [x] 移除 27 个测试文件的 `sys.path.insert()` 调用
- [x] 转换 `os.environ['VAR'] = value` 为 `monkeypatch.setenv()` 
- [x] 移除 try/finally 环境变量恢复代码
- [x] 统一由 conftest.py 管理 Python 路径

### 关键实现总结

1. **统一错误类型** (`continuum_sdk/errors.py`)
   - 7 种错误类型：ContinuumError, ConfigError, ToolExecutionError, LLMError, AuthenticationError, RateLimitError, SecurityError, ValidationError
   - 支持序列化/反序列化，友好错误消息

2. **安全环境变量访问** (`continuum_sdk/env.py`)
   - 白名单机制，类型转换
   - 防止敏感信息泄露

3. **SSRF 防护** (`continuum_sdk/python_impl.py`)
   - 私有 IP 验证：10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
   - DNS 重绑定防护，重定向验证

4. **Shell 安全加固** (`continuum_sdk/tools/bash.py`)
   - `shell=False` 默认模式
   - 注入检测正则，环境变量白名单

5. **动态配置优先级**
   - CONTINUUM_MODEL > providers config > fallback chain
   - 友好日志提示进入备用方案

### 提交记录

- `feat: add unified error types module` - errors.py + tests
- `feat: add safe environment variable access module` - env.py + tests
- `fix(security): implement SSRF protection` - python_impl.py
- `fix(security): harden shell command execution` - bash.py
- `fix(config): complete env var whitelist` - loader.py
- `fix(i18n): use English for provider display_name` - providers.py
- `fix(tests): improve test isolation` - 27 test files
