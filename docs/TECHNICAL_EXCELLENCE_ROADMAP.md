# Technical Excellence Roadmap

**目标**：把 Continuum 在技术工程维度做到业界顶级——可衡量、可证明、不依赖市场认可。
**原则**：每个任务必须有**机器可验证的完成标准**（DoD）。"我觉得做完了"不算完成。
**适用前提**：单人 + 纯技术投入。不追求 star/用户，追求代码与工程本身的卓越。

---

## 0. 当前置信度（基线，2026-06-19）

| 维度 | 当前状态 | 数据来源 |
|------|---------|---------|
| 单元测试 | 1465 通过 | `cargo test --workspace` |
| 编译警告 | 0 | `cargo clippy --workspace --all-targets -- -D warnings` |
| 格式 | clean | `cargo fmt --all --check` |
| **测试覆盖率** | **未知（无度量）** | ⚠️ 缺 |
| **Benchmark** | 3 个 bench 文件，**从未运行** | ⚠️ 缺 |
| **Fuzz 测试** | 0 | ⚠️ 缺 |
| **属性测试** | 0 | ⚠️ 缺 |
| 公开 API doc 覆盖 | 部分 | 待度量 |
| 错误模型 | anyhow 杂混 | 待结构化 |
| CI | 基础 | 待完善 |

**结论**：功能多，但"可证明的工程质量"基础设施几乎为零。**第一阶段先把度量建立起来，否则后面所有进步都是盲飞。**

---

## 1. 度量基础设施（P0，必须先做）

没有度量就没有进步。这一节是所有后续任务的前提。

### Task 1.1: 测试覆盖率度量
- **DoD**: `cargo tarpaulin --workspace --out Html` 产出覆盖率报告；CI 上每次 PR 跑覆盖率；README 显示徽章
- **目标**: 建立基线数字 → 每月不下降
- **路径**: 加 `cargo-tarpaulin` 到 dev 工具链；写 `scripts/coverage.sh`

### Task 1.2: 代码复杂度度量
- **DoD**: `cargo install cargo-cyclonedx` 或 `badness`/`complexity` 跑出复杂度报告；圈复杂度 > 20 的函数列入整改清单
- **目标**: 圈复杂度中位数 < 10，无 > 30 的函数

### Task 1.3: Benchmark 基础设施
- **DoD**: `cargo bench --workspace` 能跑且产出可读报告；至少 3 个 benchmark 覆盖核心路径（tool dispatch / session 序列化 / context 管理）
- **目标**: 建立"本地开销"基线数字，供后续优化对比
- **路径**: 修复现有 3 个未运行的 bench，加 criterion baseline 持久化

### Task 1.4: 依赖审计
- **DoD**: `cargo install cargo-deny && cargo deny check` 通过；无已知 CVE；无重复依赖
- **目标**: 零 CVE，依赖树干净

### Task 1.5: 死代码检测
- **DoD**: `cargo install cargo-udeps && cargo udeps --workspace` 无未使用依赖
- **目标**: 零死代码

---

## 2. 测试深化（P0）

### Task 2.1: 单元覆盖率 → 80%
- **DoD**: tarpaulin 报告 line coverage ≥ 80%（当前未知，先建立再追）
- **目标**: 80% → 90%（12 个月）
- **重点模块优先**: `builtin_tools/*`（用户直接接触）、`tool_executor`、`memory_system`

### Task 2.2: 属性测试（proptest）
- **DoD**: `proptest` 覆盖所有"不变量"函数：`safe_truncate`、`path_safety::check_path_danger`、`secret_scrub`、`read_state::verify`、`validate_regex_safety`
- **DoD**: 每个属性至少 1000 个随机输入
- **示例不变量**: "safe_truncate_bytes 永远返回合法 UTF-8"、"check_path_danger 对 `/` 子树永远返回 critical"

### Task 2.3: Fuzz 测试（cargo-fuzz）⭐ 单点可领先
- **DoD**: `cargo fuzz` 覆盖所有**解析入口**：URL 解析（network_safety）、路径解析（file_ops）、JSON args 解析（adapter）、regex pattern（grep/regex_match）、HTML 解析（web_fetch）
- **DoD**: 每个 fuzz target 跑 ≥ 24 小时无 crash
- **为何这是单点领先**: 多数 agent 项目（含 OpenCode/Aider）没有系统性 fuzz。做了就是单点业界最好之一。

### Task 2.4: 集成测试矩阵
- **DoD**: 跨平台测试矩阵（Linux/macOS/Windows）在 CI 跑
- **DoD**: 每个 provider 至少一个 mock 集成测试（wiremock，已依赖）

### Task 2.5: 性能回归测试
- **DoD**: CI 上 benchmark 对比 main 分支，回归 > 5% 阻断合并
- **路径**: criterion 的 `--save-baseline` + CI 比较

---

## 3. 性能基准与对标（P1）

### Task 3.1: 对齐 AutoAgents benchmark 方法论
- **DoD**: 复现 AutoAgents 的 ReAct 单步 benchmark 场景（[方法论](https://dev.to/saivishwak/benchmarking-ai-agent-frameworks-in-2026-autoagents-rust-vs-langchain-langgraph-llamaindex-338f)）
- **DoD**: Continuum 在该 benchmark 上产出 P50/P95/P99 延迟、吞吐、内存、冷启动数字
- **目标**: 内存 < 1.1 GB（对标 Rig/AutoAgents），冷启动 < 10 ms

### Task 3.2: 本地开销细分 benchmark
- **DoD**: 拆分 benchmark 到子路径：tool dispatch、args 解析、stale-read hash、session 序列化、context 管理
- **目标**: 每个 LLM 调用的本地开销 < 1 ms（可忽略于网络延迟）

### Task 3.3: 内存占用优化
- **DoD**: 峰值 RSS < 1 GB（对标 Rig 的 1.019 GB）
- **路径**: `cargo install cargo-bhol` 或 `dhat` 找内存热点；优化 read_state store、session buffer

### Task 3.4: 冷启动优化
- **DoD**: 进程冷启动到首字节输出 < 10 ms
- **路径**: profiling 启动路径；lazy 初始化；削减启动期分配

---

## 4. 架构与代码质量（P1）

### Task 4.1: 错误模型结构化
- **DoD**: Layer 3 公开 API 的错误从裸 `anyhow` 升级为 `thiserror` 枚举；每个错误变体有 doc
- **DoD**: 内部实现层保留 anyhow；边界层（公开 API）用结构化错误
- **目标**: 用户可 `match` 错误类型，而非解析字符串

### Task 4.2: 模块边界审计
- **DoD**: 生成依赖图（`cargo depgraph`）；验证无循环依赖；Layer 间依赖方向严格（高 → 低）
- **DoD**: 写一份 `docs/ARCHITECTURE.md` 记录每个 crate 的职责与边界

### Task 4.3: 公开 API 审计与稳定化
- **DoD**: 标记每个 `pub` 为 `#[doc(hidden)]`（内部）/ 公开稳定 / 实验性（`#[deprecated]` 或 versioned）
- **DoD**: 写 `docs/API_STABILITY.md` 说明 SemVer 承诺

### Task 4.4: miri 检查（unsafe 审计）
- **DoD**: `cargo +nightly miri test` 通过（所有 unsafe 块验证）
- **路径**: 找出所有 `unsafe`，逐个验证或消除（如 `std::env::set_var` 的 unsafe）

### Task 4.5: clippy pedantic + nursery
- **DoD**: `cargo clippy -- -W clippy::pedantic -W clippy::nursery` 通过（允许少量 `#[allow]` 但需注释理由）
- **目标**: 超越默认 clippy，达到 pedantic 级

---

## 5. 文档（P1）

### Task 5.1: 公开 API doc 100%
- **DoD**: `cargo doc --workspace` 无 warning；每个 `pub` fn/struct/enum 有 doc comment；≥ 50% 有 `# Example`
- **度量**: `cargo install cargo-doc-coverage` 跑覆盖率

### Task 5.2: 架构决策记录（ADR）
- **DoD**: `docs/adr/` 目录，每个重大决策一份（为何 5 层、为何 additive trait 方法而非 blanket impl、为何 SHA-256 而非 mtime）
- **模板**: 每个 ADR 有 Context / Decision / Consequences

### Task 5.3: 渐进式 example
- **DoD**: `examples/` 目录从 50 行最简 agent → 逐步加功能，每步可跑
- **目标**: 学习者能从 example 看懂整个架构

### Task 5.4: 竞品技术对比文档（可核实）
- **DoD**: `docs/COMPARISON.md` 每一行都附证据链接；不写未经核实的主张（吸取我之前 5 轮错误的教训）
- **原则**: 每个对比单元格 footnote 到源码/changelog/benchmark

---

## 6. 安全加固深化（P1）⭐ Continuum 已领先的点，继续拉大

### Task 6.1: 安全不变量形式化
- **DoD**: 列出所有安全不变量（如"write_file 不会静默覆盖"、"edit_file 不会无前置 read"、"SSRF 不会访问 metadata IP"）
- **DoD**: 每个不变量有对应的 fuzz/属性测试证明

### Task 6.2: 安全测试套件
- **DoD**: `tests/security/` 目录，针对 OWASP CWE 编号测试（CWE-22 路径遍历、CWE-918 SSRF、CWE-400 资源耗尽、CWE-532 日志泄密）

### Task 6.3: 第三方安全审计清单
- **DoD**: 写一份自查清单，可供外部审计复用
- **目标**: 让 Continuum 能通过企业安全团队评估

### Task 6.4: 漏洞响应流程
- **DoD**: `SECURITY.md` 描述漏洞披露流程、SLA、CVE 申请流程

---

## 7. CI/Release 自动化（P2）

### Task 7.1: CI 完整化
- **DoD**: CI 跑 test + clippy(-D warnings) + fmt --check + tarpaulin + cargo-deny + cargo-udeps + miri(nightly job)
- **DoD**: 覆盖率/benchmark 回归阻断合并

### Task 7.2: Release 自动化
- **DoD**: `cargo release` 配置；自动 publish 到 crates.io；CHANGELOG 自动生成
- **DoD**: 发布 checklist 文档

### Task 7.3: 二进制发布
- **DoD**: GitHub Releases 自动产出 Linux/macOS/Windows 二进制（cross-compile 或 CI matrix）

---

## 8. 性能优化深入（P2，benchmark 驱动）

### Task 8.1: 分配器选择
- **DoD**: 测评 jemalloc/mimalloc vs 系统 allocator；选更优者配置到 release

### Task 8.2: 零拷贝路径审计
- **DoD**: 标注所有"大块数据"路径（HTTP body、文件内容、LLM stream）；消除不必要的 clone/分配

### Task 8.3: SIMD 加速
- **DoD**: 评估 regex（已 SIMD）、JSON 解析（simd-json）、SHA-256 的 SIMD 路径

---

## 里程碑

### M1（1-2 个月）：度量建立
- 完成第 1 节全部（Task 1.1–1.5）
- **产出**: 覆盖率基线、benchmark 基线、依赖审计报告
- **意义**: 从此所有进步可衡量

### M2（2-4 个月）：测试深化
- 完成第 2 节（Task 2.1–2.5）
- **产出**: 覆盖率 ≥ 80%、fuzz 跑 24h 无 crash、proptest 覆盖不变量
- **意义**: 工程质量进入业界前 10%

### M3（4-6 个月）：对标与架构
- 完成第 3、4 节（Task 3.x、4.x）
- **产出**: AutoAgents 方法论对标 benchmark、结构化错误、miri 通过
- **意义**: 性能/架构可证明

### M4（6-12 个月）：领先单点
- 完成第 5、6 节
- **产出**: 安全不变量形式化 + fuzz 全覆盖；竞品对比文档（可核实）
- **意义**: 在"工具层安全防护严格度"这个单点做到业界最好

### M5（12+ 个月）：全面卓越
- 第 7、8 节持续
- **产出**: CI 全自动化、性能优化到 benchmark 领先

---

## 反自欺机制

1. **每个任务的 DoD 必须机器可验证**——拒绝"差不多做完了"
2. **每月跑一次全量度量**（覆盖率/benchmark/复杂度），数字写进 `docs/METRICS.md`
3. **benchmark 必须在 CI 上持续跑**——防止悄悄回归
4. **竞品对比每一行附证据**——禁止未经核实的"领先"主张（吸取本会话 5 轮证伪的教训）
5. **fuzz/proptest 持续运行**——不是跑一次就完

---

## 如何使用本文档

- 每完成一个 Task，在本文档对应行打勾 `- [x]` 并附 commit hash
- 每月 review：哪些 Task 推进了、哪些卡住、是否需要调整优先级
- DoD 不可妥协——达不到就保持 `- [ ]`，不自欺

**这份文档本身就是诚实的标尺。**
