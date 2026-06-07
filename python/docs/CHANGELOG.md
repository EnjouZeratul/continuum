# Changelog

All notable changes to Continuum SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-05-31

### Added

#### Core Features
- **Agent API**: Unified `Agent` class with streaming support
- **Session Management**: Persistent conversation sessions with recovery
- **Multi-Provider LLM**: Support for 13 LLM providers
  - Anthropic (Claude)
  - OpenAI (GPT)
  - Google (Gemini)
  - Cohere
  - HuggingFace
  - Together AI
  - Groq
  - DeepSeek
  - Moonshot
  - GLM (智谱AI)
  - KIMI (月之暗面)

#### Agent Intelligence
- **Checkpoint System**: State snapshot and recovery
- **History Management**: Conversation history with configurable limits
- **Task Planner**: Automatic task decomposition and planning
- **Progress Tracking**: Step-by-step execution monitoring
- **Self-Correction**: Automatic error detection and retry mechanism

#### Configuration
- **Theme System**: 8 preset themes + custom theme support
  - Dark, Light, Nord, Dracula, Gruvbox, Catppuccin, Tokyo Night, One Dark
- **TOML Configuration**: File-based configuration loading
- **Provider Configuration**: Model lists for 13 providers

#### Security Module
- **PathValidator**: Path boundary validation with symlink escape detection
- **PermissionChecker**: 5 permission types (READ, WRITE, EXECUTE, DELETE, CREATE)
- **AuditLogger**: Operation audit logging with JSON/CSV export
- **ChangePreviewer**: Risk assessment for file changes

#### Memory System
- **Multi-Tier Memory**: 4 memory tiers (WORKING, SESSION, PROJECT, LONGTERM)
- **SQLite Storage**: FTS5 full-text search support
- **Decay Policies**: Automatic memory cleanup

#### RAG Module
- **Document Loaders**: Multi-format document loading
- **Text Splitters**: Fixed-size, paragraph, and recursive chunking
- **Vector Store**: In-memory and file-based vector storage

#### Render Module
- **MarkdownRenderer**: Terminal markdown rendering using rich library
- **Syntax Highlighting**: Code block highlighting support

### Changed
- Refactored from monolithic to modular architecture
- Improved streaming performance for large responses
- Enhanced error handling with specific exception types

### Fixed
- Dataclass copy issue using `replace()` function
- Path comparison on Windows platforms
- Model list population after package installation

### Security
- Added path traversal protection
- Added sensitive file pattern detection
- Added audit logging for all file operations

## [0.9.0] - 2025-05-15

### Added
- Initial public release
- Basic Agent functionality
- Anthropic and OpenAI support
- Tool registration system

### Changed
- Migrated from sync to async API
- Improved configuration management

## [0.8.0] - 2025-05-01

### Added
- Session persistence
- Memory system foundation
- Basic RAG capabilities

## [0.7.0] - 2025-04-15

### Added
- Streaming response support
- Error handling improvements
- Configuration from environment

## [0.6.0] - 2025-04-01

### Added
- Tool calling support
- Function registration API
- Basic CLI interface

## [0.5.0] - 2025-03-15

### Added
- Initial LLM client implementation
- Anthropic Claude support
- Basic message handling

## [0.1.0] - 2025-03-01

### Added
- Project initialization
- Basic structure setup
- Development environment configuration

---

## Release Notes

### Version 1.0.0 Highlights

This release marks the first stable version of Continuum SDK with production-ready features:

1. **Multi-Provider Support**: Seamless switching between 13 LLM providers
2. **Agent Intelligence**: Self-correction, planning, and progress tracking
3. **Security First**: Comprehensive security module for safe file operations
4. **Memory System**: Multi-tier memory with SQLite persistence
5. **Developer Experience**: Rich documentation and example code

### Migration Guide

See [MIGRATION_GUIDE.md](./MIGRATION_GUIDE.md) for upgrading from previous versions.

### Future Roadmap

- [ ] OpenTelemetry integration
- [ ] Distributed session support
- [ ] Plugin system
- [ ] Web UI dashboard
- [ ] Benchmark suite
