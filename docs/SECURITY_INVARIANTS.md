# Security Invariants

**Continuum 工具层安全保证的形式化记录。**
每个不变量是一个"永不发生"的保证，附 OWASP CWE 映射 + 测试证明 + 实现位置。

> 这些不变量是 Continuum 在**无 sandbox 环境**也能安全运行的依据——安全下沉到工具层，不依赖外层隔离。

---

## 不变量目录

### I-1: 文件读取不撑爆内存（ReadFileTool）
- **陈述**：`read_file` 永不在无大小预检的情况下分配整个文件 buffer
- **CWE**: [CWE-400](https://cwe.mitre.org/data/definitions/400.html) Uncontrolled Resource Consumption
- **实现**: `file_ops.rs` `metadata().len() > max_read_bytes` 预检
- **测试**: `test_read_file_rejects_oversize`（单元）+ proptest `stb_*`（属性）

### I-2: 二进制内容不注入 LLM 上下文（ReadFileTool / BashTool）
- **陈述**：检测到 NUL 字节的文件/输出，永远被拒绝（不返回乱码给 LLM）
- **CWE**: [CWE-116](https://cwe.mitre.org/data/definitions/116.html) Improper Encoding
- **实现**: `file_ops.rs` binary_sniff / `shell.rs` stdout NUL 检测
- **测试**: `test_read_file_rejects_binary` + `test_bash_*`（单元）

### I-3: 写入不静默覆盖（WriteFileTool）
- **陈述**：已存在文件 + 未显式 `overwrite=true` → 永远拒绝
- **CWE**: [CWE-284](https://cwe.mitre.org/data/definitions/284.html) Improper Access Control
- **实现**: `file_ops.rs` `try_exists` + overwrite 检查
- **测试**: `test_write_file_rejects_existing_without_overwrite`

### I-4: 编辑须精确唯一匹配（EditFileTool）
- **陈述**：`old_string` 匹配 0 次或 >1 次 → 永远拒绝（不静默全替换）
- **CWE**: [CWE-787](https://cwe.mitre.org/data/definitions/787.html) Out-of-bounds Write（语义层面：意外修改多处）
- **实现**: `file_ops.rs` `content.matches().count()` 检查
- **测试**: `test_edit_file_rejects_multiple_matches` + `test_edit_file_rejects_zero_matches`

### I-5: stale-read 防护（EditFileTool / WriteFileTool overwrite）
- **陈述**：编辑/覆盖已存在文件 → 必须先 read 且文件未改（SHA-256 一致），否则拒绝
- **CWE**: [CWE-367](https://cwe.mitre.org/data/definitions/367.html) Time-of-check Time-of-use（lost-update）
- **实现**: `read_state.rs` SHA-256 hash + verify；`file_ops.rs` stale 检查
- **测试**: `test_stale_read_*`（4 个）+ proptest

### I-6: 关键路径不删除/移动（DeleteFileTool / MoveFileTool / CopyFileTool）
- **陈述**：`/`, `/etc`, `~/.ssh`, `C:\Windows` 等关键路径 → 永远拒绝（除非 `force=true`）
- **CWE**: [CWE-22](https://cwe.mitre.org/data/definitions/22.html) Path Traversal + [CWE-732](https://cwe.mitre.org/data/definitions/732.html) Incorrect Permission Assignment
- **实现**: `path_safety.rs` `check_path_danger`
- **测试**: `test_delete_rejects_*` + proptest `cpd_etc_subtree_critical`

### I-7: symlink 不误删 target（DeleteFileTool）
- **陈述**：symlink → 永远拒绝删除（要求显式解析 target）
- **CWE**: [CWE-59](https://cwe.mitre.org/data/definitions/59.html) Link Following
- **实现**: `file_ops.rs` `symlink_metadata().is_symlink()` 检查
- **测试**: `test_delete_rejects_symlink`

### I-8: 危险命令不执行（BashTool）
- **陈述**：`rm -rf /`, fork bomb, `mkfs` 等 forbidden pattern → 永远拒绝
- **CWE**: [CWE-78](https://cwe.mitre.org/data/definitions/78.html) OS Command Injection（破坏性命令防护）
- **实现**: `shell.rs` `FORBIDDEN_PATTERNS` denylist
- **测试**: `test_bash_rejects_rm_rf_root` + `test_bash_rejects_fork_bomb`

### I-9: SSRF 防护（HttpRequestTool / WebFetchTool）
- **陈述**：loopback / private IP / link-local / cloud metadata / 非 http(s) scheme / forbidden port → 永远拒绝
- **CWE**: [CWE-918](https://cwe.mitre.org/data/definitions/918.html) Server-Side Request Forgery
- **实现**: `network_safety.rs` `DefaultUrlValidator`
- **测试**: `test_http_request_rejects_localhost` / `test_http_request_rejects_aws_metadata` / `test_http_request_rejects_file_scheme` / proptest

### I-10: 敏感 env 不泄露（GetEnvTool / ListEnvTool / ProcessListTool）
- **陈述**：已知 secret env var + secret 模式 → 永远 redact（不注入 LLM context）
- **CWE**: [CWE-532](https://cwe.mitre.org/data/definitions/532.html) Insertion of Sensitive Information into Log
- **实现**: `secret_scrub.rs` `SecretScrubber` + `SENSITIVE_ENV_NAMES`
- **测试**: proptest `ss_aws_key` / `ss_openai_key` / `ss_idempotent`

### I-11: 危险 env 不设置（SetEnvTool）
- **陈述**：`LD_PRELOAD` / `GIT_DIR` / `PATH` 等 16 个危险 env → 永远拒绝
- **CWE**: [CWE-15](https://cwe.mitre.org/data/definitions/15.html) External Control of System Setting
- **实现**: `secret_scrub.rs` `DANGEROUS_ENV_NAMES`
- **测试**: `system_tools` 单元测试

### I-12: UTF-8 安全截断（全局）
- **陈述**：所有字符串截断永远落在 UTF-8 char 边界（不 panic）
- **CWE**: [CWE-170](https://cwe.mitre.org/data/definitions/170.html) Improper Null Termination（字符边界）
- **实现**: `safe_truncate.rs` `safe_truncate_bytes` / `safe_truncate_chars`
- **测试**: proptest `stb_never_panics` / `stb_multibyte_boundary`（1024 cases）

### I-13: git path traversal 防护（GitAddTool）
- **陈述**：`git_add` 的 path 含 `..` 或绝对路径 → 永远拒绝
- **CWE**: [CWE-22](https://cwe.mitre.org/data/definitions/22.html) Path Traversal
- **实现**: `git_tools.rs` path 验证
- **测试**: `git_tools` 单元测试

### I-14: HTTP 敏感 header 脱敏（HttpRequestTool）
- **陈述**：`Authorization` / `Cookie` / `Set-Cookie` 等响应 header → 永远 redact
- **CWE**: [CWE-532](https://cwe.mitre.org/data/definitions/532.html) Insertion of Sensitive Information into Log
- **实现**: `network.rs` `SENSITIVE_RESPONSE_HEADERS`
- **测试**: `network` 单元测试

---

## 验证层级

每个不变量有**三层验证**：

1. **单元测试**——已知 case（`cargo test`）
2. **属性测试**——所有输入（proptest，1024 cases，`tests/property_tests.rs`）
3. **fuzz 基础设施**——coverage-guided（`rust/layer3/fuzz/`，待 Linux 执行）

---

## 对比业界

| 不变量类别 | Continuum | Claude Code | LangChain |
|-----------|-----------|-------------|-----------|
| SSRF 防护 | ✅ I-9 | ❌（无通用 HTTP 工具）| ❌ |
| Secret scrubbing | ✅ I-10 | ❌（无 env 工具）| ❌ |
| Critical-path 检查 | ✅ I-6 | sandbox 权限（不同层）| ❌ |
| stale-read 防护 | ✅ I-5 | ✅ | ❌ |
| 危险 env 拒绝 | ✅ I-11 | ❌ | ❌ |

**Continuum 的安全下沉到工具层 = 无 sandbox 也能安全运行**（服务端/嵌入式场景的核心价值）。
